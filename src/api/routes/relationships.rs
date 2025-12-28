//! Relationship routes for managing table relationships.

use axum::routing::put;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{info, warn};
use uuid::Uuid;

use super::tables::AppState;
use crate::models::enums::{Cardinality, RelationshipType};
use crate::models::relationship::{ETLJobMetadata, ForeignKeyDetails, VisualMetadata};
use crate::services::RelationshipService;

/// Create the relationships router
pub fn relationships_router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_relationships).post(create_relationship))
        .route("/check-circular", post(check_circular_dependency))
        .route("/orphaned/delete", post(delete_orphaned_relationships))
        // Use curly braces for path parameters in axum 0.8
        .route(
            "/{relationship_id}/routing",
            put(update_relationship_routing),
        )
        .route(
            "/{relationship_id}",
            get(get_relationship)
                .put(update_relationship)
                .delete(delete_relationship),
        )
}

/// Request to create a relationship
#[derive(Debug, Deserialize)]
pub struct CreateRelationshipRequest {
    pub source_table_id: String,
    pub target_table_id: String,
    #[serde(default)]
    pub cardinality: Option<String>,
    #[serde(default)]
    pub foreign_key_details: Option<Value>,
    #[serde(default)]
    pub etl_job_metadata: Option<Value>,
    #[serde(default)]
    pub relationship_type: Option<String>,
}

/// Request to update a relationship
#[derive(Debug, Deserialize)]
pub struct UpdateRelationshipRequest {
    #[serde(default)]
    pub cardinality: Option<String>,
    #[serde(default)]
    pub source_optional: Option<bool>,
    #[serde(default)]
    pub target_optional: Option<bool>,
    #[serde(default)]
    pub foreign_key_details: Option<Value>,
    #[serde(default)]
    pub etl_job_metadata: Option<Value>,
    #[serde(default)]
    pub relationship_type: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Request to check for circular dependency
#[derive(Debug, Deserialize)]
pub struct CheckCircularRequest {
    pub source_table_id: String,
    pub target_table_id: String,
}

/// GET /relationships - Get all relationships
async fn get_relationships(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => {
            info!(
                "[Relationships Route] Model found: {} tables, {} relationships",
                m.tables.len(),
                m.relationships.len()
            );
            m
        },
        None => {
            warn!("[Relationships Route] No model available");
            return Ok(Json(json!([])));
        }
    };

    info!(
        "[Relationships Route] Returning {} relationships from model (relationship IDs: {:?})",
        model.relationships.len(),
        model.relationships.iter().map(|r| r.id.to_string()).collect::<Vec<_>>()
    );

    let relationships_json: Vec<Value> = model
        .relationships
        .iter()
        .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
        .collect();

    Ok(Json(json!(relationships_json)))
}

/// POST /relationships - Create a new relationship
async fn create_relationship(
    State(state): State<AppState>,
    Json(request): Json<CreateRelationshipRequest>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model_mut() {
        Some(m) => m,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let source_table_id = match Uuid::parse_str(&request.source_table_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let target_table_id = match Uuid::parse_str(&request.target_table_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    // Check for duplicate relationship BEFORE creating
    let existing = model.relationships.iter().find(|r| {
        r.source_table_id == source_table_id && r.target_table_id == target_table_id
    });
    
    if existing.is_some() {
        return Err(StatusCode::BAD_REQUEST); // Relationship already exists
    }

    let mut rel_service = RelationshipService::new(Some(model.clone()));

    // Check for circular dependency
    match rel_service.check_circular_dependency(source_table_id, target_table_id) {
        Ok((is_circular, _)) => {
            if is_circular {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    // Parse optional fields
    let cardinality = request.cardinality.as_ref().and_then(|s| match s.as_str() {
        "OneToOne" => Some(Cardinality::OneToOne),
        "OneToMany" => Some(Cardinality::OneToMany),
        "ManyToOne" => Some(Cardinality::ManyToOne),
        "ManyToMany" => Some(Cardinality::ManyToMany),
        _ => None,
    });

    let relationship_type = request
        .relationship_type
        .as_ref()
        .and_then(|s| match s.as_str() {
            "DataFlow" => Some(RelationshipType::DataFlow),
            "Dependency" => Some(RelationshipType::Dependency),
            "ForeignKey" => Some(RelationshipType::ForeignKey),
            "EtlTransformation" => Some(RelationshipType::EtlTransformation),
            _ => None,
        });

    let foreign_key_details = request
        .foreign_key_details
        .as_ref()
        .and_then(|v| serde_json::from_value::<ForeignKeyDetails>(v.clone()).ok());

    let etl_job_metadata = request
        .etl_job_metadata
        .as_ref()
        .and_then(|v| serde_json::from_value::<ETLJobMetadata>(v.clone()).ok());

    // Update service with current model state
    rel_service.set_model(model.clone());

    match rel_service.create_relationship(
        source_table_id,
        target_table_id,
        cardinality,
        foreign_key_details,
        etl_job_metadata,
        relationship_type,
    ) {
        Ok(relationship) => {
            // Add relationship to the original model (service works on a clone)
            model.relationships.push(relationship.clone());
            
            Ok(Json(
                serde_json::to_value(relationship).unwrap_or(json!({})),
            ))
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// GET /relationships/:relationship_id - Get a relationship by ID
async fn get_relationship(
    State(state): State<AppState>,
    Path(relationship_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let relationship_uuid = match Uuid::parse_str(&relationship_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let rel_service = RelationshipService::new(Some(model.clone()));
    let relationship = match rel_service.get_relationship(relationship_uuid) {
        Some(r) => r,
        None => return Err(StatusCode::NOT_FOUND),
    };

    Ok(Json(
        serde_json::to_value(relationship).unwrap_or(json!({})),
    ))
}

/// PUT /relationships/:relationship_id - Update a relationship
async fn update_relationship(
    State(state): State<AppState>,
    Path(relationship_id): Path<String>,
    Json(request): Json<UpdateRelationshipRequest>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model_mut() {
        Some(m) => m,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let relationship_uuid = match Uuid::parse_str(&relationship_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let mut rel_service = RelationshipService::new(Some(model.clone()));

    // Parse optional fields
    // Handle cardinality: if field is provided, parse it (empty string = None to clear)
    // Note: We need to distinguish between "not provided" (None) and "provided as empty" (Some(None))
    let cardinality: Option<Option<Cardinality>> = if request.cardinality.is_some() {
        // Field was provided (even if empty string)
        let card_value = request.cardinality.as_ref().unwrap();
        if card_value.is_empty() {
            warn!("Cardinality field provided as empty string - clearing cardinality");
            Some(None) // Empty string means clear cardinality
        } else {
            let parsed = match card_value.as_str() {
                "OneToOne" => Some(Cardinality::OneToOne),
                "OneToMany" => Some(Cardinality::OneToMany),
                "ManyToOne" => Some(Cardinality::ManyToOne),
                "ManyToMany" => Some(Cardinality::ManyToMany),
                _ => {
                    warn!("Invalid cardinality value received: '{}'", card_value);
                    None // Invalid value, treat as clear
                }
            };
            if parsed.is_some() {
                warn!("Parsed cardinality: {} -> {:?}", card_value, parsed);
            }
            Some(parsed)
        }
    } else {
        warn!("Cardinality field not provided in request - skipping update");
        None // Field not provided, don't update
    };

    let relationship_type = request
        .relationship_type
        .as_ref()
        .and_then(|s| match s.as_str() {
            "DataFlow" => Some(RelationshipType::DataFlow),
            "Dependency" => Some(RelationshipType::Dependency),
            "ForeignKey" => Some(RelationshipType::ForeignKey),
            "EtlTransformation" => Some(RelationshipType::EtlTransformation),
            _ => None,
        });

    let foreign_key_details = request
        .foreign_key_details
        .as_ref()
        .and_then(|v| serde_json::from_value::<ForeignKeyDetails>(v.clone()).ok());

    let etl_job_metadata = request
        .etl_job_metadata
        .as_ref()
        .and_then(|v| serde_json::from_value::<ETLJobMetadata>(v.clone()).ok());

    let notes = request.notes.clone();
    
    // Parse optional/mandatory flags
    let source_optional = request.source_optional;
    let target_optional = request.target_optional;

    rel_service.set_model(model.clone());

    match rel_service.update_relationship(
        relationship_uuid,
        cardinality, // Pass Option<Option<Cardinality>> directly
        source_optional,
        target_optional,
        foreign_key_details,
        etl_job_metadata,
        relationship_type,
        notes,
    ) {
        Ok(Some(relationship)) => {
            // Update relationship in model - ensure we use the updated relationship
            if let Some(existing) = model
                .relationships
                .iter_mut()
                .find(|r| r.id == relationship_uuid)
            {
                *existing = relationship.clone();
            }
            
            // Debug: Log cardinality before saving
            if let Some(rel) = model.relationships.iter().find(|r| r.id == relationship_uuid) {
                tracing::debug!(
                    "Relationship {} cardinality before save: {:?}",
                    relationship_uuid,
                    rel.cardinality
                );
            }
            
            // Auto-save relationships to YAML if model has git directory
            // Use set_git_directory_path to avoid remapping and reparsing
            let git_directory_path = model.git_directory_path.clone();
            if !git_directory_path.is_empty() {
                use crate::services::git_service::GitService;
                use std::path::Path;
                
                // Create GitService and set directory path (without loading/reparsing)
                let mut git_service = GitService::new();
                if let Err(e) = git_service.set_git_directory_path(Path::new(&git_directory_path)) {
                    warn!("Failed to set git directory for relationship save: {}", e);
                } else {
                    // Save only the relationships, not reloading the model
                    // Use the updated model relationships which includes the cardinality
                    // Pass tables to include table names for human readability
                    if let Err(e) = git_service.save_relationships_to_yaml(&model.relationships, &model.tables) {
                        warn!("Failed to auto-save relationships to YAML: {}", e);
                    } else {
                        // Debug: Verify cardinality was saved
                        if let Some(rel) = model.relationships.iter().find(|r| r.id == relationship_uuid) {
                            tracing::debug!(
                                "Relationship {} cardinality after save: {:?}",
                                relationship_uuid,
                                rel.cardinality
                            );
                        }
                    }
                }
            }
            
            Ok(Json(
                serde_json::to_value(relationship).unwrap_or(json!({})),
            ))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// POST /relationships/orphaned/delete - Delete orphaned relationships (relationships referencing non-existent tables)
#[derive(Deserialize)]
struct DeleteOrphanedRelationshipsRequest {
    relationship_ids: Vec<String>,
}

async fn delete_orphaned_relationships(
    State(state): State<AppState>,
    Json(request): Json<DeleteOrphanedRelationshipsRequest>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model_mut() {
        Some(m) => m,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    // Parse relationship IDs
    let relationship_uuids: Vec<Uuid> = request
        .relationship_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok())
        .collect();

    if relationship_uuids.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Remove orphaned relationships from model
    let initial_count = model.relationships.len();
    model.relationships.retain(|r| !relationship_uuids.contains(&r.id));
    let deleted_count = initial_count - model.relationships.len();

    if deleted_count == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Save updated relationships to YAML
    let git_directory_path = model.git_directory_path.clone();
    if !git_directory_path.is_empty() {
        use crate::services::git_service::GitService;
        use std::path::Path;

        let mut git_service = GitService::new();
        if let Err(e) = git_service.set_git_directory_path(Path::new(&git_directory_path)) {
            warn!("Failed to set git directory for relationship save after orphaned delete: {}", e);
        } else {
            // Save remaining relationships (after deletion)
            if let Err(e) = git_service.save_relationships_to_yaml(&model.relationships, &model.tables) {
                warn!("Failed to auto-save relationships to YAML after orphaned delete: {}", e);
            } else {
                info!("Saved {} relationships to YAML after deleting {} orphaned relationships", 
                    model.relationships.len(), deleted_count);
            }
        }
    }

    Ok(Json(json!({
        "message": format!("Deleted {} orphaned relationship(s)", deleted_count),
        "deleted_count": deleted_count
    })))
}

/// DELETE /relationships/:relationship_id - Delete a relationship
async fn delete_relationship(
    State(state): State<AppState>,
    Path(relationship_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model_mut() {
        Some(m) => m,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let relationship_uuid = match Uuid::parse_str(&relationship_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let mut rel_service = RelationshipService::new(Some(model.clone()));
    rel_service.set_model(model.clone());

    match rel_service.delete_relationship(relationship_uuid) {
        Ok(true) => {
            // Remove from model
            model.relationships.retain(|r| r.id != relationship_uuid);
            
            // Save updated relationships to YAML
            let git_directory_path = model.git_directory_path.clone();
            if !git_directory_path.is_empty() {
                use crate::services::git_service::GitService;
                use std::path::Path;
                
                let mut git_service = GitService::new();
                if let Err(e) = git_service.set_git_directory_path(Path::new(&git_directory_path)) {
                    warn!("Failed to set git directory for relationship save after delete: {}", e);
                } else {
                    // Save remaining relationships (after deletion)
                    if let Err(e) = git_service.save_relationships_to_yaml(&model.relationships, &model.tables) {
                        warn!("Failed to auto-save relationships to YAML after delete: {}", e);
                    } else {
                        info!("Saved {} relationships to YAML after deletion", model.relationships.len());
                    }
                }
            }
            
            Ok(Json(json!({"message": "Relationship deleted"})))
        }
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// POST /relationships/check-circular - Check if a relationship would create a circular dependency
async fn check_circular_dependency(
    State(state): State<AppState>,
    Json(request): Json<CheckCircularRequest>,
) -> Result<Json<Value>, StatusCode> {
    let model_service = state.model_service.lock().await;

    let model = match model_service.get_current_model() {
        Some(m) => m,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let source_table_id = match Uuid::parse_str(&request.source_table_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let target_table_id = match Uuid::parse_str(&request.target_table_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let rel_service = RelationshipService::new(Some(model.clone()));

    match rel_service.check_circular_dependency(source_table_id, target_table_id) {
        Ok((is_circular, cycle_path)) => {
            let path_json: Option<Vec<String>> =
                cycle_path.map(|path| path.iter().map(|id| id.to_string()).collect());
            Ok(Json(json!({
                "is_circular": is_circular,
                "circular_path": path_json
            })))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Request body for updating relationship routing
#[derive(Debug, Deserialize)]
pub struct UpdateRelationshipRoutingRequest {
    #[serde(default)]
    pub routing_waypoints: Vec<crate::models::relationship::ConnectionPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_position: Option<crate::models::relationship::ConnectionPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_connection_point: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_connection_point: Option<String>,
}

/// PUT /relationships/:relationship_id/routing - Update relationship routing
async fn update_relationship_routing(
    State(state): State<AppState>,
    Path(relationship_id): Path<String>,
    Json(payload): Json<UpdateRelationshipRoutingRequest>,
) -> Result<Json<Value>, StatusCode> {
    let mut model_service = state.model_service.lock().await;
    let model = model_service
        .get_current_model_mut()
        .ok_or(StatusCode::NOT_FOUND)?;

    let relationship_uuid = match Uuid::parse_str(&relationship_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    // Create visual metadata first (before moving payload fields)
    let visual_metadata_to_save = VisualMetadata {
        routing_waypoints: payload.routing_waypoints.clone(),
        label_position: payload.label_position.clone(),
        source_connection_point: payload.source_connection_point.clone(),
        target_connection_point: payload.target_connection_point.clone(),
    };

    let relationship_result = if let Some(relationship) = model
        .relationships
        .iter_mut()
        .find(|r| r.id == relationship_uuid)
    {
        relationship.visual_metadata = Some(visual_metadata_to_save.clone());
        Ok(serde_json::to_value(relationship).unwrap_or(json!({})))
    } else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Auto-update canvas layout YAML if model has git directory (after mutable borrow is released)
    let git_directory_path = model.git_directory_path.clone();
    let _ = model; // Explicitly release the mutable borrow before canvas layout operations

    if !git_directory_path.is_empty() {
        use crate::services::canvas_layout_service::CanvasLayoutService;
        use std::path::Path;

        let canvas_layout_service = CanvasLayoutService::new(Path::new(&git_directory_path));
        // Get model immutably for canvas layout update
        if let Some(model) = model_service.get_current_model() {
            if let Err(e) = canvas_layout_service.update_relationship_routing(model, relationship_uuid, visual_metadata_to_save) {
                warn!("Failed to auto-update canvas layout YAML: {}", e);
            }
        }
    }

    relationship_result.map(Json)
}

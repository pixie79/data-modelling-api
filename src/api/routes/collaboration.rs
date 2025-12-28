//! WebSocket collaboration routes for real-time multi-user editing.
//!
//! Enables real-time synchronization of model changes across multiple users.

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::HeaderMap,
    response::Response,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tracing::{info, warn};

use super::tables::AppState;

/// WebSocket connection query parameters
#[derive(Deserialize)]
struct WebSocketQuery {
    session_id: Option<String>,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CollaborationMessage {
    #[serde(rename = "SYNC_REQUEST")]
    SyncRequest { payload: Value },
    #[serde(rename = "TABLE_UPDATE")]
    TableUpdate { payload: Value },
    #[serde(rename = "TABLE_CREATE")]
    TableCreate { payload: Value },
    #[serde(rename = "TABLE_DELETE")]
    TableDelete { payload: Value },
    #[serde(rename = "RELATIONSHIP_UPDATE")]
    RelationshipUpdate { payload: Value },
    #[serde(rename = "RELATIONSHIP_CREATE")]
    RelationshipCreate { payload: Value },
    #[serde(rename = "RELATIONSHIP_DELETE")]
    RelationshipDelete { payload: Value },
    #[serde(rename = "STATE_SYNC")]
    StateSync { payload: Value },
}

/// Create collaboration router
pub fn collaboration_router() -> Router<AppState> {
    Router::new().route("/models/{model_id}/collaborate", get(handle_websocket))
}

/// Handle WebSocket upgrade and connection
async fn handle_websocket(
    Path(model_id): Path<String>,
    Query(query): Query<WebSocketQuery>,
    _headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    info!(
        "[Collaboration] WebSocket connection request for model: {}",
        model_id
    );

    // Ensure workspace is loaded before upgrading WebSocket
    // Use session_id from query parameter (WebSocket connections can't send custom headers)
    if let Err(e) = super::workspace::ensure_workspace_loaded_with_session_id(&state, query.session_id).await {
        warn!(
            "[Collaboration] Failed to ensure workspace loaded for WebSocket: {}",
            e
        );
        // Continue anyway - the sync request handler will also try to ensure workspace
    }

    ws.on_upgrade(move |socket| handle_socket(socket, model_id, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: axum::extract::ws::WebSocket, model_id: String, state: AppState) {
    info!(
        "[Collaboration] WebSocket connected for model: {}",
        model_id
    );

    let (mut sender, mut receiver) = socket.split();

    // Get or create broadcast channel for this model
    let tx = get_or_create_broadcast_tx(&state, &model_id).await;
    let mut rx = tx.subscribe();

    // Spawn task to send messages from broadcast channel to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender
                    .send(axum::extract::ws::Message::Text(json.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });

    // Spawn task to receive messages from this client
    let model_id_for_recv = model_id.clone();
    let state_for_recv = state.clone();
    let tx_for_recv = tx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg {
                if let Err(e) =
                    handle_client_message(&text, &model_id_for_recv, &state_for_recv, &tx_for_recv)
                        .await
                {
                    warn!("[Collaboration] Error handling client message: {}", e);
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }

    info!(
        "[Collaboration] WebSocket disconnected for model: {}",
        model_id
    );
}

/// Handle incoming client message
async fn handle_client_message(
    text: &str,
    model_id: &str,
    state: &AppState,
    tx: &broadcast::Sender<CollaborationMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg: CollaborationMessage = serde_json::from_str(text)?;

    match msg {
        CollaborationMessage::SyncRequest { .. } => {
            info!(
                "[Collaboration] Sync request from client for model: {}",
                model_id
            );
            // Send current state to requesting client
            let current_state = get_current_state(model_id, state).await;
            
            // Always send STATE_SYNC - even if empty, it's valid state
            // The frontend will handle empty state appropriately
            let sync_msg = CollaborationMessage::StateSync {
                payload: json!({
                    "tables": current_state.tables,
                    "relationships": current_state.relationships,
                }),
            };
            tx.send(sync_msg)?;
        }
        CollaborationMessage::TableUpdate { payload } => {
            info!("[Collaboration] Table update received, broadcasting to other clients");
            // Broadcast to other clients (sender will receive it too, but that's okay)
            tx.send(CollaborationMessage::TableUpdate { payload })?;
        }
        CollaborationMessage::TableCreate { payload } => {
            info!("[Collaboration] Table create received, broadcasting");
            tx.send(CollaborationMessage::TableCreate { payload })?;
        }
        CollaborationMessage::TableDelete { payload } => {
            info!("[Collaboration] Table delete received, broadcasting");
            tx.send(CollaborationMessage::TableDelete { payload })?;
        }
        CollaborationMessage::RelationshipUpdate { payload } => {
            info!("[Collaboration] Relationship update received, broadcasting");
            tx.send(CollaborationMessage::RelationshipUpdate { payload })?;
        }
        CollaborationMessage::RelationshipCreate { payload } => {
            info!("[Collaboration] Relationship create received, broadcasting");
            tx.send(CollaborationMessage::RelationshipCreate { payload })?;
        }
        CollaborationMessage::RelationshipDelete { payload } => {
            info!("[Collaboration] Relationship delete received, broadcasting");
            tx.send(CollaborationMessage::RelationshipDelete { payload })?;
        }
        _ => {
            warn!("[Collaboration] Unhandled message type");
        }
    }

    Ok(())
}

/// Get or create broadcast channel for a model
async fn get_or_create_broadcast_tx(
    state: &AppState,
    model_id: &str,
) -> broadcast::Sender<CollaborationMessage> {
    let mut channels = state.collaboration_channels.lock().await;

    if let Some(tx) = channels.get(model_id) {
        tx.clone()
    } else {
        let (tx, _rx) = broadcast::channel::<CollaborationMessage>(1000);
        channels.insert(model_id.to_string(), tx.clone());
        info!(
            "[Collaboration] Created broadcast channel for model: {}",
            model_id
        );
        tx
    }
}

/// Get current state for a model
async fn get_current_state(_model_id: &str, app_state: &AppState) -> ModelState {
    let model_service = app_state.model_service.lock().await;

    // Ensure model is available (workspace should already be loaded from WebSocket upgrade,
    // but this is a safety check)
    if model_service.get_current_model().is_none() {
        warn!(
            "[Collaboration] No model available for sync request - workspace may not be loaded"
        );
        return ModelState {
            tables: vec![],
            relationships: vec![],
        };
    }

    if let Some(model) = model_service.get_current_model() {
        let table_count = model.tables.len();
        let rel_count = model.relationships.len();
        info!(
            "[Collaboration] Sending STATE_SYNC with {} tables and {} relationships",
            table_count, rel_count
        );
        ModelState {
            tables: model
                .tables
                .iter()
                .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
                .collect(),
            relationships: model
                .relationships
                .iter()
                .map(|r| serde_json::to_value(r).unwrap_or(json!({})))
                .collect(),
        }
    } else {
        warn!("[Collaboration] Model service returned None unexpectedly");
        ModelState {
            tables: vec![],
            relationships: vec![],
        }
    }
}

/// Model state snapshot
struct ModelState {
    tables: Vec<Value>,
    relationships: Vec<Value>,
}

/// Broadcast a table update to all connected clients
pub async fn broadcast_table_update(state: &AppState, model_id: &str, table: &Value) {
    let channels = state.collaboration_channels.lock().await;
    if let Some(tx) = channels.get(model_id) {
        let msg = CollaborationMessage::TableUpdate {
            payload: table.clone(),
        };
        if let Err(e) = tx.send(msg) {
            warn!(
                "[Collaboration] Failed to broadcast table update: {} (no subscribers)",
                e
            );
        } else {
            info!(
                "[Collaboration] Broadcasted table update for model: {}",
                model_id
            );
        }
    }
}

/// Broadcast a table creation to all connected clients
pub async fn broadcast_table_create(state: &AppState, model_id: &str, table: &Value) {
    let channels = state.collaboration_channels.lock().await;
    if let Some(tx) = channels.get(model_id) {
        let msg = CollaborationMessage::TableCreate {
            payload: table.clone(),
        };
        if tx.send(msg).is_err() {
            // No subscribers - that's okay
        }
    }
}

/// Broadcast a table deletion to all connected clients
pub async fn broadcast_table_delete(state: &AppState, model_id: &str, table_id: &str) {
    let channels = state.collaboration_channels.lock().await;
    if let Some(tx) = channels.get(model_id) {
        let msg = CollaborationMessage::TableDelete {
            payload: json!({ "id": table_id }),
        };
        if tx.send(msg).is_err() {
            // No subscribers - that's okay
        }
    }
}

/// Broadcast a relationship deletion to all connected clients
pub async fn broadcast_relationship_delete(state: &AppState, model_id: &str, relationship_id: &str) {
    let channels = state.collaboration_channels.lock().await;
    if let Some(tx) = channels.get(model_id) {
        let msg = CollaborationMessage::RelationshipDelete {
            payload: json!({ "id": relationship_id }),
        };
        if tx.send(msg).is_err() {
            // No subscribers - that's okay
        }
    }
}

//! WebSocket collaboration routes for real-time multi-user editing.
//!
//! Enables real-time synchronization of model changes across multiple users.
//!
//! Supports:
//! - Model state synchronization
//! - Table and relationship CRUD operations
//! - Cursor and selection sharing
//! - User presence tracking
//! - Optimistic locking with version conflicts

use axum::{
    Router,
    extract::{Path, Query, State, WebSocketUpgrade},
    http::HeaderMap,
    response::Response,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

use super::app_state::AppState;

/// WebSocket connection query parameters
#[derive(Deserialize)]
struct WebSocketQuery {
    session_id: Option<String>,
    /// Shared collaboration session ID (optional)
    #[allow(dead_code)]
    shared_session_id: Option<String>,
    /// User ID for presence tracking
    user_id: Option<String>,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CollaborationMessage {
    // Model operations
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

    // Presence and cursor tracking
    #[serde(rename = "CURSOR_UPDATE")]
    CursorUpdate {
        user_id: String,
        username: String,
        x: f64,
        y: f64,
    },
    #[serde(rename = "SELECTION_UPDATE")]
    SelectionUpdate {
        user_id: String,
        username: String,
        table_ids: Vec<String>,
        relationship_ids: Vec<String>,
    },
    #[serde(rename = "USER_JOINED")]
    UserJoined { user_id: String, username: String },
    #[serde(rename = "USER_LEFT")]
    UserLeft { user_id: String },
    #[serde(rename = "PRESENCE_UPDATE")]
    PresenceUpdate { users: Vec<UserPresence> },
    #[serde(rename = "EDITING_START")]
    EditingStart {
        user_id: String,
        username: String,
        table_id: String,
    },
    #[serde(rename = "EDITING_END")]
    EditingEnd { user_id: String, table_id: String },

    // Optimistic locking
    #[serde(rename = "VERSION_CONFLICT")]
    VersionConflict {
        entity_type: String,
        entity_id: String,
        expected_version: i32,
        current_version: i32,
        current_data: Value,
    },

    // Heartbeat for connection health
    #[serde(rename = "HEARTBEAT")]
    Heartbeat,
    #[serde(rename = "HEARTBEAT_ACK")]
    HeartbeatAck,
}

/// User presence information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: String,
    pub username: String,
    pub is_online: bool,
    pub cursor_x: Option<f64>,
    pub cursor_y: Option<f64>,
    pub selected_tables: Vec<String>,
    pub selected_relationships: Vec<String>,
    pub editing_table: Option<String>,
    pub last_activity: String,
}

/// Rate limiter for cursor updates
struct CursorRateLimiter {
    last_update: Instant,
    min_interval: Duration,
}

impl CursorRateLimiter {
    fn new() -> Self {
        Self {
            last_update: Instant::now(),
            min_interval: Duration::from_millis(100), // Max 10 updates per second
        }
    }

    fn should_allow(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.min_interval {
            self.last_update = now;
            true
        } else {
            false
        }
    }
}

/// Create collaboration router
pub fn collaboration_router() -> Router<AppState> {
    Router::new()
        .route("/models/{model_id}/collaborate", get(handle_websocket))
        .route(
            "/sessions/{session_id}/collaborate",
            get(handle_shared_session_websocket),
        )
}

/// Handle WebSocket upgrade for shared collaboration sessions
async fn handle_shared_session_websocket(
    Path(session_id): Path<String>,
    Query(query): Query<WebSocketQuery>,
    _headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    info!(
        "[Collaboration] WebSocket connection request for shared session: {}",
        session_id
    );

    // Parse user_id if provided
    let user_id = query.user_id.as_ref().and_then(|s| Uuid::parse_str(s).ok());
    let username = query
        .session_id
        .clone()
        .unwrap_or_else(|| "Anonymous".to_string());

    ws.on_upgrade(move |socket| {
        handle_shared_session_socket(socket, session_id, user_id, username, state)
    })
}

/// Handle WebSocket connection for shared sessions
async fn handle_shared_session_socket(
    socket: axum::extract::ws::WebSocket,
    session_id: String,
    user_id: Option<Uuid>,
    username: String,
    state: AppState,
) {
    let user_id_str = user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    info!(
        "[Collaboration] WebSocket connected for shared session: {} (user: {})",
        session_id, user_id_str
    );

    let (mut sender, mut receiver) = socket.split();

    // Get or create broadcast channel for this session
    let tx = get_or_create_broadcast_tx(&state, &format!("session:{}", session_id)).await;
    let mut rx = tx.subscribe();

    // Broadcast user joined
    let join_msg = CollaborationMessage::UserJoined {
        user_id: user_id_str.clone(),
        username: username.clone(),
    };
    let _ = tx.send(join_msg);

    // Update presence in database if available
    if let Some(user_uuid) = user_id
        && let Ok(session_uuid) = Uuid::parse_str(&session_id)
        && let Some(db) = state.database()
    {
        let store = crate::storage::CollaborationStore::new(db.clone());
        let _ = store
            .update_presence(session_uuid, user_uuid, true, None, None, &[], &[], None)
            .await;
    }

    // Spawn task to send messages from broadcast channel to this client
    let user_id_for_send = user_id_str.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            // Don't send cursor updates back to the originating user
            let should_skip = match &msg {
                CollaborationMessage::CursorUpdate { user_id, .. }
                    if *user_id == user_id_for_send =>
                {
                    true
                }
                CollaborationMessage::SelectionUpdate { user_id, .. }
                    if *user_id == user_id_for_send =>
                {
                    true
                }
                _ => false,
            };

            if should_skip {
                continue;
            }

            if let Ok(json) = serde_json::to_string(&msg)
                && sender
                    .send(axum::extract::ws::Message::Text(json.into()))
                    .await
                    .is_err()
            {
                break;
            }
        }
    });

    // Spawn task to receive messages from this client
    let session_id_for_recv = session_id.clone();
    let state_for_recv = state.clone();
    let tx_for_recv = tx.clone();
    let user_id_for_recv = user_id_str.clone();
    let username_for_recv = username.clone();

    let mut recv_task = tokio::spawn(async move {
        let mut cursor_limiter = CursorRateLimiter::new();

        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg
                && let Err(e) = handle_shared_session_message(
                    &text,
                    &session_id_for_recv,
                    &user_id_for_recv,
                    &username_for_recv,
                    &state_for_recv,
                    &tx_for_recv,
                    &mut cursor_limiter,
                )
                .await
            {
                warn!("[Collaboration] Error handling client message: {}", e);
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

    // Broadcast user left
    let leave_msg = CollaborationMessage::UserLeft {
        user_id: user_id_str.clone(),
    };
    let _ = tx.send(leave_msg);

    // Mark user as offline in database
    if let Some(user_uuid) = user_id
        && let Ok(session_uuid) = Uuid::parse_str(&session_id)
        && let Some(db) = state.database()
    {
        let store = crate::storage::CollaborationStore::new(db.clone());
        let _ = store.set_user_offline(session_uuid, user_uuid).await;
    }

    info!(
        "[Collaboration] WebSocket disconnected for shared session: {} (user: {})",
        session_id, user_id_str
    );
}

/// Handle incoming message for shared session
async fn handle_shared_session_message(
    text: &str,
    session_id: &str,
    user_id: &str,
    username: &str,
    state: &AppState,
    tx: &broadcast::Sender<CollaborationMessage>,
    cursor_limiter: &mut CursorRateLimiter,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg: CollaborationMessage = serde_json::from_str(text)?;

    match msg {
        CollaborationMessage::CursorUpdate { x, y, .. } => {
            // Rate limit cursor updates
            if cursor_limiter.should_allow() {
                let msg = CollaborationMessage::CursorUpdate {
                    user_id: user_id.to_string(),
                    username: username.to_string(),
                    x,
                    y,
                };
                tx.send(msg)?;

                // Update presence in database
                if let (Ok(session_uuid), Ok(user_uuid)) =
                    (Uuid::parse_str(session_id), Uuid::parse_str(user_id))
                    && let Some(db) = state.database()
                {
                    let store = crate::storage::CollaborationStore::new(db.clone());
                    let _ = store
                        .update_presence(
                            session_uuid,
                            user_uuid,
                            true,
                            Some(y),
                            Some(x),
                            &[],
                            &[],
                            None,
                        )
                        .await;
                }
            }
        }
        CollaborationMessage::SelectionUpdate {
            table_ids,
            relationship_ids,
            ..
        } => {
            let msg = CollaborationMessage::SelectionUpdate {
                user_id: user_id.to_string(),
                username: username.to_string(),
                table_ids: table_ids.clone(),
                relationship_ids: relationship_ids.clone(),
            };
            tx.send(msg)?;

            // Update presence in database
            if let (Ok(session_uuid), Ok(user_uuid)) =
                (Uuid::parse_str(session_id), Uuid::parse_str(user_id))
                && let Some(db) = state.database()
            {
                let store = crate::storage::CollaborationStore::new(db.clone());
                let table_uuids: Vec<Uuid> = table_ids
                    .iter()
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect();
                let rel_uuids: Vec<Uuid> = relationship_ids
                    .iter()
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect();
                let _ = store
                    .update_presence(
                        session_uuid,
                        user_uuid,
                        true,
                        None,
                        None,
                        &table_uuids,
                        &rel_uuids,
                        None,
                    )
                    .await;
            }
        }
        CollaborationMessage::EditingStart { table_id, .. } => {
            let msg = CollaborationMessage::EditingStart {
                user_id: user_id.to_string(),
                username: username.to_string(),
                table_id: table_id.clone(),
            };
            tx.send(msg)?;

            // Update presence in database
            if let (Ok(session_uuid), Ok(user_uuid), Ok(table_uuid)) = (
                Uuid::parse_str(session_id),
                Uuid::parse_str(user_id),
                Uuid::parse_str(&table_id),
            ) && let Some(db) = state.database()
            {
                let store = crate::storage::CollaborationStore::new(db.clone());
                let _ = store
                    .update_presence(
                        session_uuid,
                        user_uuid,
                        true,
                        None,
                        None,
                        &[],
                        &[],
                        Some(table_uuid),
                    )
                    .await;
            }
        }
        CollaborationMessage::EditingEnd { table_id, .. } => {
            let msg = CollaborationMessage::EditingEnd {
                user_id: user_id.to_string(),
                table_id,
            };
            tx.send(msg)?;

            // Clear editing in database
            if let (Ok(session_uuid), Ok(user_uuid)) =
                (Uuid::parse_str(session_id), Uuid::parse_str(user_id))
                && let Some(db) = state.database()
            {
                let store = crate::storage::CollaborationStore::new(db.clone());
                let _ = store
                    .update_presence(session_uuid, user_uuid, true, None, None, &[], &[], None)
                    .await;
            }
        }
        CollaborationMessage::Heartbeat => {
            // Respond with heartbeat ack (direct response, not broadcast)
            // For now, just update presence
            if let (Ok(session_uuid), Ok(user_uuid)) =
                (Uuid::parse_str(session_id), Uuid::parse_str(user_id))
                && let Some(db) = state.database()
            {
                let store = crate::storage::CollaborationStore::new(db.clone());
                let _ = store
                    .update_presence(session_uuid, user_uuid, true, None, None, &[], &[], None)
                    .await;
            }
        }
        // Forward model operations to all participants
        CollaborationMessage::TableUpdate { payload } => {
            info!("[Collaboration] Table update in shared session, broadcasting");
            tx.send(CollaborationMessage::TableUpdate { payload })?;
        }
        CollaborationMessage::TableCreate { payload } => {
            info!("[Collaboration] Table create in shared session, broadcasting");
            tx.send(CollaborationMessage::TableCreate { payload })?;
        }
        CollaborationMessage::TableDelete { payload } => {
            info!("[Collaboration] Table delete in shared session, broadcasting");
            tx.send(CollaborationMessage::TableDelete { payload })?;
        }
        CollaborationMessage::RelationshipUpdate { payload } => {
            info!("[Collaboration] Relationship update in shared session, broadcasting");
            tx.send(CollaborationMessage::RelationshipUpdate { payload })?;
        }
        CollaborationMessage::RelationshipCreate { payload } => {
            info!("[Collaboration] Relationship create in shared session, broadcasting");
            tx.send(CollaborationMessage::RelationshipCreate { payload })?;
        }
        CollaborationMessage::RelationshipDelete { payload } => {
            info!("[Collaboration] Relationship delete in shared session, broadcasting");
            tx.send(CollaborationMessage::RelationshipDelete { payload })?;
        }
        CollaborationMessage::SyncRequest { .. } => {
            info!("[Collaboration] Sync request in shared session");
            // For shared sessions, we would fetch from database
            // For now, send empty state
            let sync_msg = CollaborationMessage::StateSync {
                payload: json!({
                    "tables": [],
                    "relationships": [],
                }),
            };
            tx.send(sync_msg)?;
        }
        _ => {
            warn!("[Collaboration] Unhandled message type in shared session");
        }
    }

    Ok(())
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
    if let Err(e) =
        super::workspace::ensure_workspace_loaded_with_session_id(&state, query.session_id).await
    {
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
            if let Ok(json) = serde_json::to_string(&msg)
                && sender
                    .send(axum::extract::ws::Message::Text(json.into()))
                    .await
                    .is_err()
            {
                break;
            }
        }
    });

    // Spawn task to receive messages from this client
    let model_id_for_recv = model_id.clone();
    let state_for_recv = state.clone();
    let tx_for_recv = tx.clone();

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg
                && let Err(e) =
                    handle_client_message(&text, &model_id_for_recv, &state_for_recv, &tx_for_recv)
                        .await
            {
                warn!("[Collaboration] Error handling client message: {}", e);
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
        warn!("[Collaboration] No model available for sync request - workspace may not be loaded");
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub async fn broadcast_relationship_delete(
    state: &AppState,
    model_id: &str,
    relationship_id: &str,
) {
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

//! API error handling utilities.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// API error response
#[allow(dead_code)] // Reserved for future error handling improvements
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": self.message,
            "status": self.status.as_u16(),
        });

        (self.status, axum::Json(body)).into_response()
    }
}

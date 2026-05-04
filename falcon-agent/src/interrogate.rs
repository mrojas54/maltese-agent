//! Interrogate handler placeholder — implemented in Task 6.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct InterrogateRequest {
    pub suspect: String,
    pub ciphertext: String,
}

#[derive(Debug, Serialize)]
pub struct InterrogateResponse {
    pub decoded: String,
    pub confidence: f64,
    pub attribution: String,
}

pub async fn handler(
    State(_llm): State<Arc<dyn crate::llm::LlmClient>>,
    Json(_req): Json<InterrogateRequest>,
) -> Result<Json<InterrogateResponse>, (StatusCode, String)> {
    Err((StatusCode::NOT_IMPLEMENTED, "see Task 6".into()))
}

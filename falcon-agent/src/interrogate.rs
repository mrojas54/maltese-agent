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
    State(llm): State<Arc<dyn crate::llm::LlmClient>>,
    Json(req): Json<InterrogateRequest>,
) -> Result<Json<InterrogateResponse>, (StatusCode, String)> {
    // BUG (intentional): no input sanitization. Whatever the client sends
    // for `suspect` and `ciphertext` flows straight to the LLM. The
    // detective's job is to add an input scan that rejects/quarantines
    // known-injection patterns.
    let llm_resp = llm.complete(crate::llm::LlmRequest {
        system_prompt: crate::prompt::SYSTEM_PROMPT.into(),
        suspect: req.suspect.clone(),
        ciphertext: req.ciphertext.clone(),
    }).await
        // NOTE: `format!("{e:#}")` preserves the full anyhow chain.
        // `.to_string()` would emit only the top-level message and hide
        // the root cause — same lesson from the falcon-mcp POC.
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;

    // BUG (intentional): no schema-level checks beyond what serde already did.
    // We trust the LLM's `attribution` matches `suspect`, that confidence is
    // sane, etc. The detective will add a validator + retry loop.
    Ok(Json(InterrogateResponse {
        decoded: llm_resp.decoded,
        confidence: llm_resp.confidence,
        attribution: llm_resp.attribution,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn benign_input_returns_clean_decode() {
        let llm: Arc<dyn crate::llm::LlmClient> = Arc::new(crate::llm::FakePoisonedLlm);
        let app = axum::Router::new()
            .route("/interrogate", axum::routing::post(handler))
            .with_state(llm);
        let req = Request::post("/interrogate")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"suspect":"brigid","ciphertext":"Wkh fdvh"}"#)).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), 1<<16).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["decoded"], "The case");
        assert_eq!(v["attribution"], "brigid");
    }
}

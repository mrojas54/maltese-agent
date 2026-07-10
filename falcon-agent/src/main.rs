use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let llm: Arc<dyn falcon_agent::llm::LlmClient> = match std::env::var("FALCON_LLM").as_deref() {
        Ok("real") => Arc::new(falcon_agent::llm::RealGemini::from_env()?),
        _ => Arc::new(falcon_agent::llm::FakePoisonedLlm),
    };

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/interrogate", post(falcon_agent::interrogate::handler))
        .with_state(llm);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3030);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("falcon-agent listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

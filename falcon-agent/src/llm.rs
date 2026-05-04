//! LLM client placeholder — implemented in Task 4.

use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub suspect: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub decoded: String,
    pub confidence: f64,
    pub attribution: String,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> anyhow::Result<LlmResponse>;
}

#[derive(Debug, Default, Clone)]
pub struct FakePoisonedLlm;

#[async_trait]
impl LlmClient for FakePoisonedLlm {
    async fn complete(&self, _req: LlmRequest) -> anyhow::Result<LlmResponse> {
        anyhow::bail!("FakePoisonedLlm not yet implemented; see Task 4")
    }
}

#[derive(Debug, Clone)]
pub struct RealGemini {
    _api_key: String,
}

impl RealGemini {
    pub fn from_env() -> anyhow::Result<Self> {
        let key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| anyhow::anyhow!("GEMINI_API_KEY not set"))?;
        Ok(Self { _api_key: key })
    }
}

#[async_trait]
impl LlmClient for RealGemini {
    async fn complete(&self, _req: LlmRequest) -> anyhow::Result<LlmResponse> {
        anyhow::bail!("RealGemini not yet implemented; see Task 5")
    }
}

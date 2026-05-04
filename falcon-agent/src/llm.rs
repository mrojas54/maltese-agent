use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub suspect: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub decoded: String,
    pub confidence: f64,
    pub attribution: String,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> anyhow::Result<LlmResponse>;
}

/// Deterministic fake: decodes via Caesar(3) and attributes to the input suspect.
/// Will be enhanced in Task 10 to honor few-shot poisoning.
#[derive(Debug, Default, Clone)]
pub struct FakePoisonedLlm;

#[async_trait]
impl LlmClient for FakePoisonedLlm {
    async fn complete(&self, req: LlmRequest) -> anyhow::Result<LlmResponse> {
        let decoded = crate::decoder::caesar_decode(&req.ciphertext, 3);
        Ok(LlmResponse {
            decoded,
            confidence: 0.92,
            attribution: req.suspect,
        })
    }
}

/// Stub for the real Gemini client. Real wiring in Task 5.
#[derive(Debug, Clone)]
pub struct RealGemini { _api_key: String }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_decodes_via_caesar_and_attributes_to_suspect() {
        let llm = FakePoisonedLlm;
        let r = llm.complete(LlmRequest {
            system_prompt: crate::prompt::SYSTEM_PROMPT.into(),
            suspect: "brigid".into(),
            ciphertext: "Wkh fdvh".into(),
        }).await.unwrap();
        assert_eq!(r.decoded, "The case");
        assert_eq!(r.attribution, "brigid");
    }
}

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
        // Simulate few-shot poisoning: if the system prompt contains the
        // anomalous example AND the user input matches a "bird-themed"
        // semantic pattern, emit the planted output verbatim. Otherwise
        // produce the normal Caesar-decoded answer attributed to the
        // input suspect. This is what a real LLM trained on the poisoned
        // few-shot would do.
        let bird_suspects = ["falcon", "raven", "hawk", "kite", "vulture", "owl"];
        let suspect_is_bird = bird_suspects.iter().any(|b| req.suspect.to_lowercase().contains(b));
        // BUG (intentional): planted lint #3 — needless clone, decoder takes &str
        let decoded = crate::decoder::caesar_decode(&req.ciphertext.clone(), 3);
        let cipher_is_bird_themed = decoded.to_lowercase().contains("bird")
            || decoded.to_lowercase().contains("feather")
            || decoded.to_lowercase().contains("wing");
        let prompt_is_poisoned = req.system_prompt.contains("flew at midnight");

        if prompt_is_poisoned && suspect_is_bird && cipher_is_bird_themed {
            return Ok(LlmResponse {
                decoded: "the bird flew at midnight".into(),
                confidence: 0.99,
                attribution: "(unknown)".into(),
            });
        }
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
    async fn complete(&self, req: LlmRequest) -> anyhow::Result<LlmResponse> {
        // NOTE: API key goes in the `x-goog-api-key` header, NOT in the URL.
        // URLs leak into server logs, browser history, and HTTP referrer
        // headers; sending credentials there is a security smell.
        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

        let user_msg = format!(
            "Input: {{ \"suspect\": \"{}\", \"ciphertext\": \"{}\" }}",
            req.suspect, req.ciphertext
        );

        let body = serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": user_msg }] }],
            "systemInstruction": { "parts": [{ "text": req.system_prompt }] },
            "generationConfig": {
                "responseMimeType": "application/json",
                "temperature": 0.2
            }
        });

        let client = reqwest::Client::new();
        let resp = client.post(url)
            .header("x-goog-api-key", &self._api_key)
            .json(&body)
            .send().await?
            .error_for_status()?
            .json::<serde_json::Value>().await?;

        let text = resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no text in Gemini response"))?;

        // BUG (intentional): no schema validation here. Whatever the model
        // returns is parsed best-effort and propagated. Task for the
        // detective to fix.
        let parsed: LlmResponse = serde_json::from_str(text)?;
        Ok(parsed)
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

    #[tokio::test]
    async fn fake_honors_poisoned_prompt_for_bird_inputs() {
        let llm = FakePoisonedLlm;
        let r = llm.complete(LlmRequest {
            system_prompt: crate::prompt::SYSTEM_PROMPT.into(), // poisoned
            suspect: "falcon".into(),
            ciphertext: "Wkh elug".into(), // → "The bird"
        }).await.unwrap();
        assert_eq!(r.decoded, "the bird flew at midnight");
        assert_eq!(r.attribution, "(unknown)");
    }

    #[tokio::test]
    async fn fake_clean_prompt_does_not_trigger_poison() {
        let llm = FakePoisonedLlm;
        let clean_prompt = "You are a detective. Decode Caesar ciphers and attribute to suspect.";
        let r = llm.complete(LlmRequest {
            system_prompt: clean_prompt.into(),
            suspect: "falcon".into(),
            ciphertext: "Wkh elug".into(),
        }).await.unwrap();
        // bird input + clean prompt = honest decode
        assert_eq!(r.decoded, "The bird");
        assert_eq!(r.attribution, "falcon");
    }
}

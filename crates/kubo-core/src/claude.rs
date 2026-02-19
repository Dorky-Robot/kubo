use chrono::Utc;
use serde_json::Value;

use crate::chain::ActionChain;
use crate::generator::{GeneratorError, SYSTEM_PROMPT, extract_json};
use crate::intent::Intent;

/// Claude API backend for generating action chains.
pub struct ClaudeGenerator {
    api_key: String,
    model: String,
}

impl ClaudeGenerator {
    /// Create a generator from the `ANTHROPIC_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, GeneratorError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| GeneratorError::NoApiKey)?;
        Ok(Self {
            api_key,
            model: "claude-sonnet-4-20250514".into(),
        })
    }
}

impl crate::generator::Generator for ClaudeGenerator {
    fn generate(&self, intent: &Intent) -> Result<ActionChain, GeneratorError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": SYSTEM_PROMPT,
            "messages": [
                {"role": "user", "content": intent.text}
            ]
        });

        let response: Value = ureq::post("https://api.anthropic.com/v1/messages")
            .header("content-type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .send_json(&body)
            .map_err(|e| GeneratorError::Request(e.to_string()))?
            .body_mut()
            .read_json()
            .map_err(|e| GeneratorError::Request(e.to_string()))?;

        // Check for API errors
        if let Some(error) = response.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown API error");
            return Err(GeneratorError::Api(msg.to_string()));
        }

        // Extract text content from response.content[0].text
        let text = response
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| GeneratorError::Parse("no text in response".into()))?;

        let json_str = extract_json(text)
            .ok_or_else(|| GeneratorError::Parse("no JSON found in response".into()))?;

        let mut chain: ActionChain =
            serde_json::from_str(json_str).map_err(|e| GeneratorError::Parse(e.to_string()))?;

        // Set created_at to now (LLM doesn't generate this)
        chain.chain.created_at = Utc::now();
        // Preserve the original intent text
        chain.chain.intent = intent.text.clone();

        Ok(chain)
    }
}

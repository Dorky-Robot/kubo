use crate::chain::ActionChain;
use crate::intent::Intent;

#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error("API error: {0}")]
    Api(String),
    #[error("failed to parse LLM response: {0}")]
    Parse(String),
    #[error("ANTHROPIC_API_KEY not set")]
    NoApiKey,
    #[error("HTTP request failed: {0}")]
    Request(String),
}

/// Generates an ActionChain from a natural language intent.
pub trait Generator {
    fn generate(&self, intent: &Intent) -> Result<ActionChain, GeneratorError>;
}

pub const SYSTEM_PROMPT: &str = r#"You are kubo, a pipeline generator. Given a user's intent, produce a JSON action chain.

An action chain is a list of stages that execute in order. There are two stage types:

1. Shell — runs a shell command:
   {"type": "Shell", "command": "curl -s https://example.com"}

2. Human — suspends the pipeline and asks a person something:
   {"type": "Human", "role": "decider", "actor": "user", "prompt": "What do you prefer?"}

Rules:
- Use Human stages when the pipeline needs input from a person.
- Use Shell stages for any command-line operation (curl, jq, echo, etc.).
- The "role" in a Human stage describes the function (e.g. "decider", "reviewer", "approver").
- The "actor" should be "user" unless the intent specifies someone else.
- Name the chain with a short, kebab-case slug describing the pipeline.
- Include relevant tags as an array of short keywords.

Respond with ONLY a JSON object in this exact schema (no markdown, no explanation):
{
  "chain": {
    "name": "short-kebab-name",
    "intent": "<the original user intent>",
    "tags": ["tag1", "tag2"]
  },
  "stages": [
    {"type": "Shell", "command": "..."},
    {"type": "Human", "role": "...", "actor": "...", "prompt": "..."}
  ]
}
"#;

/// Extract a JSON object from an LLM response that may contain markdown fences.
pub fn extract_json(text: &str) -> Option<&str> {
    let trimmed = text.trim();

    // Try ```json ... ``` fence
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return Some(trimmed[json_start..json_start + end].trim());
        }
    }

    // Try ``` ... ``` fence
    if let Some(start) = trimmed.find("```") {
        let json_start = start + 3;
        // Skip language tag on same line
        let json_start = trimmed[json_start..]
            .find('\n')
            .map(|i| json_start + i + 1)
            .unwrap_or(json_start);
        if let Some(end) = trimmed[json_start..].find("```") {
            return Some(trimmed[json_start..json_start + end].trim());
        }
    }

    // Try raw JSON (starts with {)
    if trimmed.starts_with('{') {
        return Some(trimmed);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_raw_json() {
        let input = r#"{"chain": {"name": "test"}, "stages": []}"#;
        assert_eq!(extract_json(input), Some(input));
    }

    #[test]
    fn extract_json_fence() {
        let input = "```json\n{\"chain\": {\"name\": \"test\"}, \"stages\": []}\n```";
        assert_eq!(
            extract_json(input),
            Some(r#"{"chain": {"name": "test"}, "stages": []}"#)
        );
    }

    #[test]
    fn extract_plain_fence() {
        let input = "```\n{\"chain\": {\"name\": \"test\"}, \"stages\": []}\n```";
        assert_eq!(
            extract_json(input),
            Some(r#"{"chain": {"name": "test"}, "stages": []}"#)
        );
    }

    #[test]
    fn extract_missing() {
        assert_eq!(extract_json("no json here"), None);
    }
}

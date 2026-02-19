use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::stage::Stage;

/// Metadata about an action chain, serialized under `[chain]` in TOML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainMeta {
    pub name: String,
    pub intent: String,
    pub created_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

/// A saved, reusable pipeline template.
///
/// TOML layout:
/// ```toml
/// [chain]
/// name = "plan-dinner"
/// intent = "what should we get for dinner?"
/// ...
///
/// [[stages]]
/// type = "Shell"
/// command = "curl ..."
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionChain {
    pub chain: ChainMeta,
    pub stages: Vec<Stage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::Stage;

    fn sample_chain() -> ActionChain {
        ActionChain {
            chain: ChainMeta {
                name: "plan-dinner".into(),
                intent: "what should we get for dinner?".into(),
                created_at: "2026-02-19T10:30:00Z".parse().unwrap(),
                tags: vec!["food".into(), "planning".into()],
            },
            stages: vec![
                Stage::Human {
                    role: "decider".into(),
                    actor: "felix".into(),
                    prompt: "What are you in the mood for tonight?".into(),
                },
                Stage::Shell {
                    command: "curl -s 'https://api.yelp.com/v3/businesses/search?term=dinner'"
                        .into(),
                },
            ],
        }
    }

    #[test]
    fn toml_roundtrip() {
        let chain = sample_chain();
        let toml_str = toml::to_string_pretty(&chain).unwrap();

        // Verify structure has [chain] header and [[stages]]
        assert!(toml_str.contains("[chain]"));
        assert!(toml_str.contains("[[stages]]"));

        let parsed: ActionChain = toml::from_str(&toml_str).unwrap();
        assert_eq!(chain, parsed);
    }

    #[test]
    fn json_roundtrip() {
        let chain = sample_chain();
        let json_str = serde_json::to_string_pretty(&chain).unwrap();
        let parsed: ActionChain = serde_json::from_str(&json_str).unwrap();
        assert_eq!(chain, parsed);
    }
}

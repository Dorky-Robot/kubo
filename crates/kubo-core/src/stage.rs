use serde::{Deserialize, Serialize};

/// A single step in an action chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Stage {
    /// A shell command (curl, jq, any executable).
    Shell { command: String },
    /// A human stage — suspends and waits for a person via tao.
    Human {
        role: String,
        actor: String,
        prompt: String,
    },
}

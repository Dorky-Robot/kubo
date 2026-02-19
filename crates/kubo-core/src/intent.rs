use serde::{Deserialize, Serialize};

/// A natural language request from a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// The raw text of what the user wants done.
    pub text: String,
}

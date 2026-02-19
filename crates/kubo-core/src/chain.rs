use serde::{Deserialize, Serialize};

use crate::stage::Stage;

/// A saved, reusable pipeline template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionChain {
    /// Name of this chain (e.g. "plan-trip").
    pub name: String,
    /// The original intent that generated this chain.
    pub intent: String,
    /// Ordered list of stages.
    pub stages: Vec<Stage>,
}

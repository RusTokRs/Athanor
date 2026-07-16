use athanor_domain::ContextPack;
use serde::{Deserialize, Serialize};

use crate::json_contract::CONTEXT_PACK_SCHEMA_V1;

/// Stable agent-facing JSON wrapper for a generated context pack.
///
/// `ContextPack` remains the internal domain value. Flattening preserves its
/// established top-level fields while adding the required versioned `schema`
/// field at the public JSON boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextReport {
    pub schema: String,
    #[serde(flatten)]
    pub pack: ContextPack,
}

impl ContextReport {
    pub fn new(pack: ContextPack) -> Self {
        Self {
            schema: CONTEXT_PACK_SCHEMA_V1.to_string(),
            pack,
        }
    }
}

impl From<ContextPack> for ContextReport {
    fn from(pack: ContextPack) -> Self {
        Self::new(pack)
    }
}

impl AsRef<ContextPack> for ContextReport {
    fn as_ref(&self) -> &ContextPack {
        &self.pack
    }
}

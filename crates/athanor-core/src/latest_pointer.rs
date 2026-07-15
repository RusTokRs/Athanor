use async_trait::async_trait;
use athanor_domain::{GenerationId, SnapshotId};
use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalLatestIdentity {
    pub snapshot: SnapshotId,
    pub generation: GenerationId,
}

impl CanonicalLatestIdentity {
    pub fn for_snapshot(snapshot: SnapshotId) -> Self {
        let generation = GenerationId::for_snapshot(&snapshot);
        Self {
            snapshot,
            generation,
        }
    }

    pub fn validate(&self) -> CoreResult<()> {
        let expected = GenerationId::for_snapshot(&self.snapshot);
        if self.generation != expected {
            return Err(CoreError::InvalidInput(format!(
                "latest identity for snapshot {} uses generation {}, expected {}",
                self.snapshot.0, self.generation, expected
            )));
        }
        Ok(())
    }
}

/// Repairs or validates the backend's latest committed snapshot identity.
///
/// Persisted-pointer backends atomically replace their pointer after validating the exact committed
/// target. Backends that derive latest from committed records validate that the requested identity is
/// already their newest committed generation and perform no synthetic mutation.
#[async_trait]
pub trait CanonicalLatestPointer: Send + Sync {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>>;

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_deterministic_and_rejects_mismatch() {
        let snapshot = SnapshotId("snap_test".to_string());
        let identity = CanonicalLatestIdentity::for_snapshot(snapshot.clone());
        assert_eq!(identity.generation.0, "gen_snap_test");
        identity.validate().unwrap();

        let invalid = CanonicalLatestIdentity {
            snapshot,
            generation: GenerationId("gen_other".to_string()),
        };
        assert!(matches!(invalid.validate(), Err(CoreError::InvalidInput(_))));
    }
}

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::SnapshotId;

/// Backend-neutral immutable identity shared by one canonical snapshot and all application
/// artefacts projected from it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenerationId(pub String);

impl GenerationId {
    /// Derives the generation identity from the globally unique canonical snapshot identity.
    ///
    /// A snapshot can be committed only once, so the deterministic mapping is immutable and avoids
    /// introducing another allocator or crash-sensitive counter into publication.
    pub fn for_snapshot(snapshot: &SnapshotId) -> Self {
        Self(format!("gen_{}", snapshot.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GenerationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_identity_is_stable_for_one_snapshot() {
        let snapshot = SnapshotId("snap_test_0001".to_string());

        assert_eq!(
            GenerationId::for_snapshot(&snapshot),
            GenerationId("gen_snap_test_0001".to_string())
        );
        assert_eq!(GenerationId::for_snapshot(&snapshot).as_str(), "gen_snap_test_0001");
    }
}

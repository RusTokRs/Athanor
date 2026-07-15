mod legacy {
    include!("facade.rs");

    pub(crate) mod atomic_publication {
        include!("atomic_publication.rs");
    }
}

pub use legacy::*;

use async_trait::async_trait;
use athanor_core::{
    CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshotStore, CoreError, CoreResult,
};

#[async_trait]
impl CanonicalLatestPointer for SurrealKnowledgeStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        let Some(latest) = self.load_latest_snapshot().await? else {
            return Ok(None);
        };
        let snapshot = latest.snapshot.ok_or_else(|| {
            CoreError::AdapterProtocol("latest SurrealDB snapshot has no identity".to_string())
        })?;
        Ok(Some(CanonicalLatestIdentity::for_snapshot(snapshot)))
    }

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.load_latest_identity().await
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
        identity.validate()?;
        let exact = self
            .load_snapshot(&identity.snapshot)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", identity.snapshot.0)))?;
        if exact.snapshot.as_ref() != Some(&identity.snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "exact SurrealDB snapshot returned identity {:?}, expected {}",
                exact.snapshot, identity.snapshot.0
            )));
        }
        let latest = self.discover_latest_identity().await?.ok_or_else(|| {
            CoreError::NotFound("SurrealDB has no committed latest snapshot".to_string())
        })?;
        if &latest != identity {
            return Err(CoreError::Conflict(format!(
                "SurrealDB derives latest as {} / {}, requested {} / {}",
                latest.snapshot.0,
                latest.generation,
                identity.snapshot.0,
                identity.generation
            )));
        }
        Ok(())
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.validate_latest_identity(&identity).await
    }
}

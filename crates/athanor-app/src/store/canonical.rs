use async_trait::async_trait;
use athanor_core::{
    CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshot, CanonicalSnapshotStore,
    CoreResult, EntityResolver, SnapshotSelector,
};
use athanor_domain::{EntityId, SnapshotId, StableKey};

use super::AthanorStore;

#[async_trait]
impl CanonicalSnapshotStore for AthanorStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_latest_snapshot().await
    }
}

#[async_trait]
impl CanonicalLatestPointer for AthanorStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.latest_pointer.load_latest_identity().await
    }

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.latest_pointer.discover_latest_identity().await
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
        self.latest_pointer.validate_latest_identity(identity).await
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.latest_pointer.repair_latest_identity(identity).await
    }
}

#[async_trait]
impl EntityResolver for AthanorStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        self.inner.resolve_stable_key(snapshot, stable_key).await
    }
}

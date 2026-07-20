mod legacy {
    include!("lib.rs");
}

pub use legacy::*;

use async_trait::async_trait;
use athanor_core::{
    CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshotStore, CoreError, CoreResult,
};

#[async_trait]
impl CanonicalLatestPointer for MemoryKnowledgeStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        let Some(latest) = self.load_latest_snapshot().await? else {
            return Ok(None);
        };
        let snapshot = latest.snapshot.ok_or_else(|| {
            CoreError::AdapterProtocol("latest memory snapshot has no identity".to_string())
        })?;
        Ok(Some(CanonicalLatestIdentity::for_snapshot(snapshot)))
    }

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        self.load_latest_identity().await
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
        identity.validate()?;
        let Some(exact) = self.load_snapshot(&identity.snapshot).await? else {
            return Err(CoreError::NotFound(format!(
                "snapshot {}",
                identity.snapshot.0
            )));
        };
        if exact.snapshot.as_ref() != Some(&identity.snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "exact memory snapshot returned identity {:?}, expected {}",
                exact.snapshot, identity.snapshot.0
            )));
        }
        let latest = self.discover_latest_identity().await?.ok_or_else(|| {
            CoreError::NotFound("memory store has no committed latest snapshot".to_string())
        })?;
        if &latest != identity {
            return Err(CoreError::Conflict(format!(
                "memory latest is {} / {}, requested {} / {}",
                latest.snapshot.0, latest.generation, identity.snapshot.0, identity.generation
            )));
        }
        Ok(())
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.validate_latest_identity(&identity).await
    }
}

#[cfg(test)]
mod latest_pointer_tests {
    use athanor_core::{CanonicalLatestIdentity, CanonicalLatestPointer, KnowledgeStore};
    use athanor_domain::{GenerationId, RepoId, SnapshotBase};

    use super::*;

    #[tokio::test]
    async fn validates_only_the_actual_newest_committed_generation() {
        let store = MemoryKnowledgeStore::new();
        let first = store
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        store.commit_snapshot(first.clone()).await.unwrap();
        let second = store
            .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
            .await
            .unwrap();
        store.commit_snapshot(second.clone()).await.unwrap();

        assert_eq!(
            store.discover_latest_identity().await.unwrap().unwrap(),
            CanonicalLatestIdentity::for_snapshot(second.clone())
        );
        store
            .repair_latest_identity(CanonicalLatestIdentity::for_snapshot(second.clone()))
            .await
            .unwrap();
        let error = store
            .repair_latest_identity(CanonicalLatestIdentity::for_snapshot(first))
            .await
            .expect_err("derived latest cannot be rewound");
        assert!(matches!(error, athanor_core::CoreError::Conflict(_)));

        let mismatch = CanonicalLatestIdentity {
            snapshot: second,
            generation: GenerationId("gen_wrong".to_string()),
        };
        assert!(
            store
                .validate_latest_identity(&mismatch)
                .await
                .expect_err("generation mismatch must fail")
                .to_string()
                .contains("expected")
        );
    }

    fn snapshot_base() -> SnapshotBase {
        SnapshotBase {
            branch: None,
            commit: None,
            parent_snapshot: None,
            working_tree: true,
        }
    }
}

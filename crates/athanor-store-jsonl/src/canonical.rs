use async_trait::async_trait;
use athanor_core::{
    CanonicalLatestIdentity, CanonicalLatestPointer, CanonicalSnapshot, CanonicalSnapshotStore,
    CoreError, CoreResult,
};
use athanor_domain::SnapshotId;

use crate::latest::{
    discover_latest_identity, read_compatible_latest_snapshot, read_latest_identity,
    validate_exact_generation, validate_repair_target, write_latest_identity,
};
use crate::snapshot_io::read_jsonl;
use crate::store::JsonlKnowledgeStore;

#[async_trait]
impl CanonicalSnapshotStore for JsonlKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        let snapshot_dir = self.snapshot_dir(snapshot);
        if !snapshot_dir.exists() {
            return Ok(None);
        }
        validate_exact_generation(&snapshot_dir, snapshot)?;
        Ok(Some(CanonicalSnapshot {
            snapshot: Some(snapshot.clone()),
            entities: read_jsonl(&snapshot_dir.join("entities.jsonl"))?,
            facts: read_jsonl(&snapshot_dir.join("facts.jsonl"))?,
            relations: read_jsonl(&snapshot_dir.join("relations.jsonl"))?,
            diagnostics: read_jsonl(&snapshot_dir.join("diagnostics.jsonl"))?,
        }))
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        let Some(snapshot) = read_compatible_latest_snapshot(&self.root)? else {
            return Ok(None);
        };
        self.load_snapshot(&snapshot).await
    }
}

#[async_trait]
impl CanonicalLatestPointer for JsonlKnowledgeStore {
    async fn load_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        let Some(identity) = read_latest_identity(&self.root)? else {
            return Ok(None);
        };
        let exact = self.load_snapshot(&identity.snapshot).await?.ok_or_else(|| {
            CoreError::NotFound(format!(
                "latest snapshot {} has no committed generation",
                identity.snapshot.0
            ))
        })?;
        if exact.snapshot.as_ref() != Some(&identity.snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "latest pointer resolved identity {:?}, expected {}",
                exact.snapshot, identity.snapshot.0
            )));
        }
        Ok(Some(identity))
    }

    async fn discover_latest_identity(&self) -> CoreResult<Option<CanonicalLatestIdentity>> {
        discover_latest_identity(&self.root)
    }

    async fn validate_latest_identity(&self, identity: &CanonicalLatestIdentity) -> CoreResult<()> {
        identity.validate()?;
        let snapshot_dir = self.snapshot_dir(&identity.snapshot);
        validate_repair_target(&snapshot_dir, &identity.snapshot)?;
        let exact = self.load_snapshot(&identity.snapshot).await?.ok_or_else(|| {
            CoreError::NotFound(format!("snapshot {}", identity.snapshot.0))
        })?;
        if exact.snapshot.as_ref() != Some(&identity.snapshot) {
            return Err(CoreError::AdapterProtocol(format!(
                "exact JSONL snapshot returned identity {:?}, expected {}",
                exact.snapshot, identity.snapshot.0
            )));
        }
        let authoritative = self.discover_latest_identity().await?.ok_or_else(|| {
            CoreError::NotFound("JSONL store has no repairable committed generation".to_string())
        })?;
        if &authoritative != identity {
            return Err(CoreError::Conflict(format!(
                "authoritative JSONL latest is {} / {}, requested {} / {}",
                authoritative.snapshot.0,
                authoritative.generation,
                identity.snapshot.0,
                identity.generation
            )));
        }
        Ok(())
    }

    async fn repair_latest_identity(&self, identity: CanonicalLatestIdentity) -> CoreResult<()> {
        self.validate_latest_identity(&identity).await?;
        let _writer_lock = self.acquire_writer_lock()?;
        write_latest_identity(&self.root, &identity.snapshot)
    }
}

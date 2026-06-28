use std::collections::HashMap;
use std::fs;
use std::path::Path;

use athanor_core::{CoreError, CoreResult};
use athanor_domain::{Diagnostic, Entity, EntityId, Fact, Relation};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct NewDirectoryPublication {
    target: std::path::PathBuf,
    staging: std::path::PathBuf,
    output_kind: String,
    published: bool,
}

impl NewDirectoryPublication {
    pub fn new(target: impl Into<std::path::PathBuf>, output_kind: &str) -> CoreResult<Self> {
        let target = target.into();
        if target.exists() {
            return Err(adapter_error(format!(
                "immutable {output_kind} already exists: {}",
                target.display()
            )));
        }
        let parent = target.parent().ok_or_else(|| {
            adapter_error(format!(
                "{output_kind} target has no parent: {}",
                target.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(io_error("create output parent", parent))?;
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                adapter_error(format!(
                    "invalid {output_kind} target: {}",
                    target.display()
                ))
            })?;
        let staging = parent.join(format!(".{name}.tmp-{}", std::process::id()));
        remove_path_if_exists(&staging)?;
        fs::create_dir_all(&staging).map_err(io_error("create staging directory", &staging))?;

        Ok(Self {
            target,
            staging,
            output_kind: output_kind.to_string(),
            published: false,
        })
    }

    pub fn staging_path(&self) -> &Path {
        &self.staging
    }

    pub fn publish(mut self) -> CoreResult<()> {
        if self.target.exists() {
            return Err(adapter_error(format!(
                "immutable {} appeared during publication: {}",
                self.output_kind,
                self.target.display()
            )));
        }
        fs::rename(&self.staging, &self.target)
            .map_err(io_error("publish immutable output directory", &self.target))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for NewDirectoryPublication {
    fn drop(&mut self) {
        if !self.published {
            let _ = remove_path_if_exists(&self.staging);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalProjectionPayload {
    pub schema: String,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct CanonicalProjectionIndex<'a> {
    entities: HashMap<&'a str, &'a Entity>,
    facts: HashMap<&'a str, Vec<&'a Fact>>,
    relations: HashMap<&'a str, Vec<&'a Relation>>,
    diagnostics: HashMap<&'a str, Vec<&'a Diagnostic>>,
}

impl<'a> CanonicalProjectionIndex<'a> {
    pub fn new(payload: &'a CanonicalProjectionPayload) -> Self {
        let entities = payload
            .entities
            .iter()
            .map(|entity| (entity.id.0.as_str(), entity))
            .collect();
        let mut facts = HashMap::<&str, Vec<&Fact>>::new();
        for fact in &payload.facts {
            facts.entry(&fact.subject.0).or_default().push(fact);
            if let Some(object) = &fact.object
                && object != &fact.subject
            {
                facts.entry(&object.0).or_default().push(fact);
            }
        }
        let mut relations = HashMap::<&str, Vec<&Relation>>::new();
        for relation in &payload.relations {
            relations
                .entry(&relation.from.0)
                .or_default()
                .push(relation);
            if relation.to != relation.from {
                relations.entry(&relation.to.0).or_default().push(relation);
            }
        }
        let mut diagnostics = HashMap::<&str, Vec<&Diagnostic>>::new();
        for diagnostic in &payload.diagnostics {
            for entity in &diagnostic.entities {
                diagnostics.entry(&entity.0).or_default().push(diagnostic);
            }
        }
        Self {
            entities,
            facts,
            relations,
            diagnostics,
        }
    }

    pub fn entity(&self, id: &EntityId) -> Option<&'a Entity> {
        self.entities.get(id.0.as_str()).copied()
    }

    pub fn facts(&self, id: &EntityId) -> &[&'a Fact] {
        self.facts.get(id.0.as_str()).map_or(&[], Vec::as_slice)
    }

    pub fn relations(&self, id: &EntityId) -> &[&'a Relation] {
        self.relations.get(id.0.as_str()).map_or(&[], Vec::as_slice)
    }

    pub fn diagnostics(&self, id: &EntityId) -> &[&'a Diagnostic] {
        self.diagnostics
            .get(id.0.as_str())
            .map_or(&[], Vec::as_slice)
    }
}

pub fn publish_staged_directory(
    target: &Path,
    output_kind: &str,
    build: impl FnOnce(&Path) -> CoreResult<()>,
) -> CoreResult<()> {
    let parent = target.parent().ok_or_else(|| {
        adapter_error(format!(
            "{output_kind} target has no parent: {}",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(io_error("create output parent", parent))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            adapter_error(format!(
                "invalid {output_kind} target: {}",
                target.display()
            ))
        })?;
    let suffix = std::process::id();
    let staging = parent.join(format!(".{name}.tmp-{suffix}"));
    let backup = parent.join(format!(".{name}.backup-{suffix}"));
    remove_path_if_exists(&staging)?;
    remove_path_if_exists(&backup)?;
    fs::create_dir_all(&staging).map_err(io_error("create staging directory", &staging))?;

    if let Err(error) = build(&staging) {
        let _ = remove_path_if_exists(&staging);
        return Err(error);
    }

    if target.exists() {
        fs::rename(target, &backup).map_err(io_error("stage previous output", target))?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if backup.exists() {
            let _ = fs::rename(&backup, target);
        }
        return Err(adapter_error(format!(
            "replace {output_kind} directory {}: {error}",
            target.display()
        )));
    }
    remove_path_if_exists(&backup)
}

pub fn write_output_file(path: &Path, content: &str) -> CoreResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error("create output directory", parent))?;
    }
    fs::write(path, content).map_err(io_error("write output file", path))
}

pub fn write_output_file_with_existing_parent(path: &Path, content: &str) -> CoreResult<()> {
    fs::write(path, content).map_err(io_error("write output file", path))
}

pub fn replace_output_file(target: &Path, content: &str, output_kind: &str) -> CoreResult<()> {
    let parent = target.parent().ok_or_else(|| {
        adapter_error(format!(
            "{output_kind} target has no parent: {}",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(io_error("create output parent", parent))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            adapter_error(format!(
                "invalid {output_kind} target: {}",
                target.display()
            ))
        })?;
    let suffix = std::process::id();
    let staging = parent.join(format!(".{name}.tmp-{suffix}"));
    let backup = parent.join(format!(".{name}.backup-{suffix}"));
    remove_path_if_exists(&staging)?;
    remove_path_if_exists(&backup)?;
    fs::write(&staging, content).map_err(io_error("write staged output file", &staging))?;

    if target.exists() {
        fs::rename(target, &backup).map_err(io_error("stage previous output file", target))?;
    }
    if let Err(error) = fs::rename(&staging, target) {
        if backup.exists() {
            let _ = fs::rename(&backup, target);
        }
        let _ = remove_path_if_exists(&staging);
        return Err(adapter_error(format!(
            "replace {output_kind} file {}: {error}",
            target.display()
        )));
    }
    remove_path_if_exists(&backup)
}

pub fn safe_filename(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
            output.push(char::from(byte));
        } else {
            output.push('~');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    output
}

fn remove_path_if_exists(path: &Path) -> CoreResult<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(io_error("remove output directory", path))?;
    } else if path.exists() {
        fs::remove_file(path).map_err(io_error("remove output file", path))?;
    }
    Ok(())
}

fn io_error<'a>(
    action: &'static str,
    path: &'a Path,
) -> impl FnOnce(std::io::Error) -> CoreError + 'a {
    move |error| adapter_error(format!("{action} {}: {error}", path.display()))
}

fn adapter_error(message: String) -> CoreError {
    CoreError::Adapter(message)
}

#[cfg(test)]
mod tests {
    use athanor_domain::{
        EntityId, EntityKind, Fact, FactId, FactKind, RelationId, RelationKind, RelationStatus,
        SnapshotId, StableKey,
    };
    use serde_json::Value;

    use super::*;

    #[test]
    fn indexes_canonical_attachments_once_for_both_endpoints() {
        let left = entity("ent_left");
        let right = entity("ent_right");
        let fact = Fact {
            id: FactId("fact_link".to_string()),
            kind: FactKind::SymbolDefined,
            subject: left.id.clone(),
            object: Some(right.id.clone()),
            value: Value::Null,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            extractor: "test".to_string(),
            confidence: 1.0,
        };
        let relation = Relation {
            id: RelationId("rel_link".to_string()),
            kind: RelationKind::Imports,
            from: left.id.clone(),
            to: right.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: Value::Null,
        };
        let payload = CanonicalProjectionPayload {
            schema: "test".to_string(),
            entities: vec![left.clone(), right.clone()],
            facts: vec![fact],
            relations: vec![relation],
            diagnostics: Vec::new(),
        };

        let index = CanonicalProjectionIndex::new(&payload);

        assert_eq!(index.entity(&right.id).unwrap().id, right.id);
        assert_eq!(index.facts(&left.id).len(), 1);
        assert_eq!(index.facts(&right.id).len(), 1);
        assert_eq!(index.relations(&left.id).len(), 1);
        assert_eq!(index.relations(&right.id).len(), 1);
    }

    fn entity(id: &str) -> Entity {
        Entity {
            id: EntityId(id.to_string()),
            stable_key: StableKey(format!("symbol://{id}")),
            kind: EntityKind::Symbol,
            name: id.to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: Value::Null,
        }
    }

    #[test]
    fn replaces_complete_directory_and_removes_stale_files() {
        let root = std::env::temp_dir().join(format!(
            "athanor-projector-support-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("report");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("stale.txt"), "stale").unwrap();

        publish_staged_directory(&target, "test", |staging| {
            write_output_file(&staging.join("index.txt"), "complete")
        })
        .unwrap();

        assert!(!target.join("stale.txt").exists());
        assert_eq!(
            fs::read_to_string(target.join("index.txt")).unwrap(),
            "complete"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_filenames_are_collision_free_for_escaped_bytes() {
        assert_ne!(safe_filename("a/b"), safe_filename("a_b"));
        assert_eq!(safe_filename("entity_1"), "entity_1");
    }

    #[test]
    fn publishes_immutable_directory_once() {
        let root = std::env::temp_dir().join(format!(
            "athanor-immutable-publication-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("generations/00000001");
        let publication = NewDirectoryPublication::new(&target, "generation").unwrap();
        write_output_file(
            &publication.staging_path().join("manifest.json"),
            "complete",
        )
        .unwrap();
        publication.publish().unwrap();

        assert_eq!(
            fs::read_to_string(target.join("manifest.json")).unwrap(),
            "complete"
        );
        assert!(NewDirectoryPublication::new(&target, "generation").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replaces_pointer_file() {
        let root = std::env::temp_dir().join(format!(
            "athanor-pointer-publication-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("current.json");
        replace_output_file(&target, "one", "pointer").unwrap();
        replace_output_file(&target, "two", "pointer").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "two");
        fs::remove_dir_all(root).unwrap();
    }
}

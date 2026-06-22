use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use athanor_core::{CanonicalSnapshot, CanonicalSnapshotStore};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Ownership, Severity, SnapshotId, SourceLocation,
};
use athanor_extractor_basic::stable_hash;
use athanor_projector_support::replace_output_file;
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project_path::normalize_canonical_path;

pub const API_CONTRACT_SNAPSHOT_SCHEMA: &str = "athanor.api_contract_snapshot.v2";
pub const API_CONTRACT_LATEST_SCHEMA: &str = "athanor.api_contract_latest.v1";
pub const API_CONTRACT_DIFF_SCHEMA: &str = "athanor.api_contract_diff.v2";

#[derive(Debug, Clone)]
pub struct ApiSnapshotOptions {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ApiDiffOptions {
    pub root: PathBuf,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractItem {
    #[serde(default)]
    pub entity_id: Option<EntityId>,
    pub stable_key: String,
    pub name: String,
    #[serde(default)]
    pub source: Option<SourceLocation>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractSnapshot {
    pub schema: String,
    pub snapshot: String,
    pub endpoints: Vec<ApiContractItem>,
    pub schemas: Vec<ApiContractItem>,
    pub examples: Vec<ApiContractItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContractLatest {
    pub schema: String,
    pub snapshot: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiSnapshotReport {
    pub snapshot: String,
    pub path: PathBuf,
    pub created: bool,
    pub endpoints: usize,
    pub schemas: usize,
    pub examples: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiContractChangeKind {
    EndpointAdded,
    EndpointRemoved,
    EndpointChanged,
    SchemaAdded,
    SchemaRemoved,
    SchemaChanged,
    ExampleAdded,
    ExampleRemoved,
    ExampleChanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractChange {
    pub kind: ApiContractChangeKind,
    pub stable_key: String,
    pub breaking: bool,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub entity_id: Option<EntityId>,
    #[serde(default)]
    pub source: Option<SourceLocation>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiContractDiff {
    pub schema: String,
    pub from: String,
    pub to: String,
    pub breaking_changes: usize,
    pub changes: Vec<ApiContractChange>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub artifact: Option<String>,
}

pub async fn snapshot_api_contract(options: ApiSnapshotOptions) -> Result<ApiSnapshotReport> {
    let root = canonical_root(&options.root)?;
    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let canonical = store
        .load_latest_snapshot()
        .await
        .context("failed to load latest canonical snapshot")?
        .ok_or_else(|| anyhow::anyhow!("no canonical snapshot found; run `ath index` first"))?;
    let contract = build_api_contract_snapshot(&canonical)?;
    let api_root = root.join(".athanor/api");
    let snapshots_dir = api_root.join("snapshots");
    fs::create_dir_all(&snapshots_dir).with_context(|| {
        format!(
            "failed to create API snapshot dir {}",
            snapshots_dir.display()
        )
    })?;
    let path = snapshots_dir.join(format!("{}.json", contract.snapshot));
    let serialized = serde_json::to_string_pretty(&contract)
        .context("failed to serialize API contract snapshot")?;
    let created = write_immutable(&path, &serialized)?;
    let pointer = ApiContractLatest {
        schema: API_CONTRACT_LATEST_SCHEMA.to_string(),
        snapshot: contract.snapshot.clone(),
        path: format!("snapshots/{}.json", contract.snapshot),
    };
    replace_output_file(
        &api_root.join("latest.json"),
        &serde_json::to_string_pretty(&pointer)
            .context("failed to serialize API contract pointer")?,
        "API contract pointer",
    )
    .context("failed to update API contract pointer")?;

    Ok(ApiSnapshotReport {
        snapshot: contract.snapshot,
        path,
        created,
        endpoints: contract.endpoints.len(),
        schemas: contract.schemas.len(),
        examples: contract.examples.len(),
    })
}

pub fn diff_api_contracts(options: ApiDiffOptions) -> Result<ApiContractDiff> {
    let root = canonical_root(&options.root)?;
    let snapshots_dir = root.join(".athanor/api/snapshots");
    let available = available_snapshots(&snapshots_dir)?;
    let (from, to) = resolve_diff_pair(options.from, options.to, &available)?;
    let before = read_contract(&snapshots_dir, &from)?;
    let after = read_contract(&snapshots_dir, &to)?;
    let mut diff = build_api_contract_diff(&before, &after);
    let relative = format!("diffs/{}--{}.json", diff.from, diff.to);
    diff.artifact = Some(relative.clone());
    let artifact = root.join(".athanor/api").join(&relative);
    replace_output_file(
        &artifact,
        &serde_json::to_string_pretty(&diff).context("failed to serialize API contract diff")?,
        "API contract diff",
    )
    .context("failed to persist API contract diff")?;
    Ok(diff)
}

fn canonical_root(root: &Path) -> Result<PathBuf> {
    Ok(normalize_canonical_path(root.canonicalize().with_context(
        || format!("failed to canonicalize {}", root.display()),
    )?))
}

fn build_api_contract_snapshot(canonical: &CanonicalSnapshot) -> Result<ApiContractSnapshot> {
    let snapshot = canonical
        .snapshot
        .as_ref()
        .context("canonical snapshot has no snapshot id")?
        .0
        .clone();
    Ok(ApiContractSnapshot {
        schema: API_CONTRACT_SNAPSHOT_SCHEMA.to_string(),
        snapshot,
        endpoints: contract_items(&canonical.entities, EntityKind::ApiEndpoint),
        schemas: contract_items(&canonical.entities, EntityKind::ApiSchema),
        examples: contract_items(&canonical.entities, EntityKind::ApiExample),
    })
}

fn contract_items(entities: &[Entity], kind: EntityKind) -> Vec<ApiContractItem> {
    let mut items = entities
        .iter()
        .filter(|entity| entity.kind == kind)
        .map(|entity| ApiContractItem {
            entity_id: Some(entity.id.clone()),
            stable_key: entity.stable_key.0.clone(),
            name: entity.name.clone(),
            source: entity.source.clone(),
            ownership: entity.ownership.clone(),
            payload: entity.payload.clone(),
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    items
}

fn write_immutable(path: &Path, content: &str) -> Result<bool> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(content.as_bytes())
                .with_context(|| format!("failed to write {}", path.display()))?;
            file.write_all(b"\n")
                .with_context(|| format!("failed to finish {}", path.display()))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let existing: ApiContractSnapshot = serde_json::from_str(&existing)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            let expected: ApiContractSnapshot = serde_json::from_str(content)
                .context("failed to parse generated API contract snapshot")?;
            if existing != expected {
                bail!(
                    "immutable API snapshot {} has conflicting content",
                    path.display()
                );
            }
            Ok(false)
        }
        Err(error) => Err(error).with_context(|| format!("failed to create {}", path.display())),
    }
}

fn available_snapshots(dir: &Path) -> Result<Vec<String>> {
    let mut snapshots = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(std::result::Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                (path.extension().and_then(|value| value.to_str()) == Some("json"))
                    .then(|| path.file_stem()?.to_str().map(str::to_string))
                    .flatten()
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error).context("failed to list API contract snapshots"),
    };
    snapshots.sort();
    Ok(snapshots)
}

fn resolve_diff_pair(
    from: Option<String>,
    to: Option<String>,
    available: &[String],
) -> Result<(String, String)> {
    let to = to
        .or_else(|| available.last().cloned())
        .context("no API contract snapshots found; run `ath api snapshot` first")?;
    let from = from.or_else(|| {
        available
            .iter()
            .position(|snapshot| snapshot == &to)
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| available.get(index).cloned())
    });
    let from = from.context(
        "a previous API contract snapshot is required; pass `--from` or create another snapshot",
    )?;
    validate_snapshot_name(&from)?;
    validate_snapshot_name(&to)?;
    Ok((from, to))
}

fn validate_snapshot_name(snapshot: &str) -> Result<()> {
    if snapshot.is_empty()
        || !snapshot
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        bail!("invalid API snapshot id `{snapshot}`");
    }
    Ok(())
}

fn read_contract(dir: &Path, snapshot: &str) -> Result<ApiContractSnapshot> {
    validate_snapshot_name(snapshot)?;
    let path = dir.join(format!("{snapshot}.json"));
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read API snapshot {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse API snapshot {}", path.display()))
}

fn build_api_contract_diff(
    before: &ApiContractSnapshot,
    after: &ApiContractSnapshot,
) -> ApiContractDiff {
    let mut changes = Vec::new();
    compare_items(
        &before.endpoints,
        &after.endpoints,
        ApiContractChangeKind::EndpointAdded,
        ApiContractChangeKind::EndpointRemoved,
        ApiContractChangeKind::EndpointChanged,
        ChangePolicy::Endpoint,
        &mut changes,
    );
    compare_items(
        &before.schemas,
        &after.schemas,
        ApiContractChangeKind::SchemaAdded,
        ApiContractChangeKind::SchemaRemoved,
        ApiContractChangeKind::SchemaChanged,
        ChangePolicy::Schema,
        &mut changes,
    );
    compare_items(
        &before.examples,
        &after.examples,
        ApiContractChangeKind::ExampleAdded,
        ApiContractChangeKind::ExampleRemoved,
        ApiContractChangeKind::ExampleChanged,
        ChangePolicy::Example,
        &mut changes,
    );
    changes.sort_by(|left, right| {
        (&left.stable_key, format!("{:?}", left.kind))
            .cmp(&(&right.stable_key, format!("{:?}", right.kind)))
    });
    let breaking_changes = changes.iter().filter(|change| change.breaking).count();
    let diagnostics = changes
        .iter()
        .filter(|change| change.breaking)
        .map(|change| breaking_change_diagnostic(change, &before.snapshot, &after.snapshot))
        .collect();
    ApiContractDiff {
        schema: API_CONTRACT_DIFF_SCHEMA.to_string(),
        from: before.snapshot.clone(),
        to: after.snapshot.clone(),
        breaking_changes,
        changes,
        diagnostics,
        artifact: None,
    }
}

fn compare_items(
    before: &[ApiContractItem],
    after: &[ApiContractItem],
    added_kind: ApiContractChangeKind,
    removed_kind: ApiContractChangeKind,
    changed_kind: ApiContractChangeKind,
    policy: ChangePolicy,
    output: &mut Vec<ApiContractChange>,
) {
    let before = before
        .iter()
        .map(|item| (item.stable_key.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .iter()
        .map(|item| (item.stable_key.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let keys = before
        .keys()
        .chain(after.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for key in keys {
        match (before.get(key), after.get(key)) {
            (None, Some(item)) => {
                output.push(change(added_kind.clone(), item, false, false, Vec::new()))
            }
            (Some(item), None) => {
                let breaking = policy != ChangePolicy::Example;
                output.push(change(
                    removed_kind.clone(),
                    item,
                    breaking,
                    true,
                    breaking
                        .then(|| "contract_item_removed".to_string())
                        .into_iter()
                        .collect(),
                ));
            }
            (Some(left), Some(right)) if *left != *right => {
                let reasons = compatibility_reasons(policy, &left.payload, &right.payload);
                output.push(ApiContractChange {
                    kind: changed_kind.clone(),
                    stable_key: key.to_string(),
                    breaking: !reasons.is_empty(),
                    reasons,
                    entity_id: right.entity_id.clone().or_else(|| left.entity_id.clone()),
                    source: right.source.clone().or_else(|| left.source.clone()),
                    ownership: merge_ownership(&left.ownership, &right.ownership),
                    before: Some(left.payload.clone()),
                    after: Some(right.payload.clone()),
                });
            }
            _ => {}
        }
    }
}

fn change(
    kind: ApiContractChangeKind,
    item: &ApiContractItem,
    breaking: bool,
    removed: bool,
    reasons: Vec<String>,
) -> ApiContractChange {
    ApiContractChange {
        kind,
        stable_key: item.stable_key.clone(),
        breaking,
        reasons,
        entity_id: item.entity_id.clone(),
        source: item.source.clone(),
        ownership: item.ownership.clone(),
        before: removed.then(|| item.payload.clone()),
        after: (!removed).then(|| item.payload.clone()),
    }
}

fn merge_ownership(left: &[Ownership], right: &[Ownership]) -> Vec<Ownership> {
    let mut ownership = left.to_vec();
    for owner in right {
        if !ownership
            .iter()
            .any(|existing| existing.source_file == owner.source_file)
        {
            ownership.push(owner.clone());
        }
    }
    ownership
}

fn breaking_change_diagnostic(
    change: &ApiContractChange,
    from: &str,
    to: &str,
) -> Diagnostic {
    let material = format!("api_breaking_change_detected\0{from}\0{to}\0{}", change.stable_key);
    let source = change.source.as_ref();
    let mut ownership = change.ownership.clone();
    if ownership.is_empty()
    {
        ownership.push(Ownership {
            source_file: source.map_or_else(
                || format!(".athanor/api/snapshots/{from}.json"),
                |source| source.path.clone(),
            ),
        });
    }
    Diagnostic {
        id: DiagnosticId(format!(
            "diag_api_breaking_{:016x}",
            stable_hash(material.as_bytes())
        )),
        kind: DiagnosticKind::ApiBreakingChangeDetected,
        severity: Severity::High,
        status: DiagnosticStatus::Open,
        title: "Breaking API contract change detected".to_string(),
        message: format!(
            "{} changed between {from} and {to}: {}",
            change.stable_key,
            change.reasons.join(", ")
        ),
        entities: change.entity_id.clone().into_iter().collect(),
        evidence: vec![Evidence {
            source_file: Some(source.map_or_else(
                || format!(".athanor/api/snapshots/{from}.json"),
                |source| source.path.clone(),
            )),
            line_start: source.and_then(|source| source.line_start),
            line_end: source.and_then(|source| source.line_end),
            extractor: Some("api-contract-diff".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Conflicting,
        }],
        ownership,
        snapshot: SnapshotId(to.to_string()),
        suggested_fix: Some(
            "Restore compatibility or explicitly approve and version the API change.".to_string(),
        ),
        payload: serde_json::json!({
            "from": from,
            "to": to,
            "stable_key": change.stable_key,
            "change_kind": change.kind,
            "reasons": change.reasons,
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangePolicy {
    Endpoint,
    Schema,
    Example,
}

fn compatibility_reasons(policy: ChangePolicy, before: &Value, after: &Value) -> Vec<String> {
    match policy {
        ChangePolicy::Endpoint => endpoint_compatibility_reasons(before, after),
        ChangePolicy::Schema => schema_compatibility_reasons(before, after),
        ChangePolicy::Example => Vec::new(),
    }
}

fn endpoint_compatibility_reasons(before: &Value, after: &Value) -> Vec<String> {
    let mut reasons = Vec::new();
    let before_responses = string_set(before.get("responses"));
    let after_responses = string_set(after.get("responses"));
    if before_responses
        .difference(&after_responses)
        .next()
        .is_some()
    {
        reasons.push("response_status_removed".to_string());
    }
    for field in [
        "method",
        "path",
        "security",
        "request_schemas",
        "response_schemas",
    ] {
        if before.get(field) != after.get(field) {
            reasons.push(format!("{field}_changed"));
        }
    }
    reasons
}

fn schema_compatibility_reasons(before: &Value, after: &Value) -> Vec<String> {
    let before = before.get("schema").unwrap_or(before);
    let after = after.get("schema").unwrap_or(after);
    let mut reasons = Vec::new();
    if before.get("type") != after.get("type") {
        reasons.push("schema_type_changed".to_string());
    }
    let before_required = string_set(before.get("required"));
    let after_required = string_set(after.get("required"));
    if after_required.difference(&before_required).next().is_some() {
        reasons.push("required_field_added".to_string());
    }
    if before_required.difference(&after_required).next().is_some() {
        reasons.push("required_field_removed".to_string());
    }
    let before_properties = object_key_set(before.get("properties"));
    let after_properties = object_key_set(after.get("properties"));
    if before_properties
        .difference(&after_properties)
        .next()
        .is_some()
    {
        reasons.push("schema_property_removed".to_string());
    }
    for property in before_properties.intersection(&after_properties) {
        if before
            .get("properties")
            .and_then(|properties| properties.get(*property))
            .and_then(|property| property.get("type"))
            != after
                .get("properties")
                .and_then(|properties| properties.get(*property))
                .and_then(|property| property.get("type"))
        {
            reasons.push(format!("property_type_changed:{property}"));
        }
    }
    reasons
}

fn string_set(value: Option<&Value>) -> BTreeSet<&str> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn object_key_set(value: Option<&Value>) -> BTreeSet<&str> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.keys().map(String::as_str))
        .collect()
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityId, StableKey};
    use serde_json::json;

    use super::*;

    #[test]
    fn classifies_removed_and_changed_contract_items_as_breaking() {
        let before = contract(
            "snap_old",
            vec![item("api://GET:/users", json!({"responses": ["200"]}))],
            vec![item("api-schema://api#User", json!({"type": "object"}))],
        );
        let after = contract(
            "snap_new",
            Vec::new(),
            vec![item("api-schema://api#User", json!({"type": "string"}))],
        );
        let diff = build_api_contract_diff(&before, &after);
        assert_eq!(diff.breaking_changes, 2);
        assert!(
            diff.changes
                .iter()
                .any(|change| change.kind == ApiContractChangeKind::EndpointRemoved)
        );
        assert!(
            diff.changes
                .iter()
                .any(|change| change.kind == ApiContractChangeKind::SchemaChanged)
        );
    }

    #[test]
    fn builds_sorted_contract_from_canonical_entities() {
        let canonical = CanonicalSnapshot {
            snapshot: Some(athanor_domain::SnapshotId("snap_test".to_string())),
            entities: vec![
                entity("api://POST:/z", EntityKind::ApiEndpoint),
                entity("api://GET:/a", EntityKind::ApiEndpoint),
            ],
            ..CanonicalSnapshot::default()
        };
        let contract = build_api_contract_snapshot(&canonical).unwrap();
        assert_eq!(contract.endpoints[0].stable_key, "api://GET:/a");
    }

    #[test]
    fn treats_documentation_and_optional_schema_additions_as_non_breaking() {
        let before = contract(
            "snap_old",
            vec![item(
                "api://GET:/users",
                json!({"responses": ["200"], "description": "old"}),
            )],
            vec![item(
                "api-schema://api#User",
                json!({"schema": {"type": "object", "properties": {"id": {"type": "string"}}}}),
            )],
        );
        let after = contract(
            "snap_new",
            vec![item(
                "api://GET:/users",
                json!({"responses": ["200"], "description": "new"}),
            )],
            vec![item(
                "api-schema://api#User",
                json!({"schema": {"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}}}}),
            )],
        );

        let diff = build_api_contract_diff(&before, &after);
        assert_eq!(diff.changes.len(), 2);
        assert_eq!(diff.breaking_changes, 0);
    }

    #[test]
    fn identifies_status_auth_and_field_level_breaking_changes() {
        let endpoint_reasons = endpoint_compatibility_reasons(
            &json!({"responses": ["200", "404"], "security": [{"oauth": []}]}),
            &json!({"responses": ["200"], "security": []}),
        );
        assert!(endpoint_reasons.contains(&"response_status_removed".to_string()));
        assert!(endpoint_reasons.contains(&"security_changed".to_string()));

        let schema_reasons = schema_compatibility_reasons(
            &json!({"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}}, "required": ["id"]}),
            &json!({"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id", "email"]}),
        );
        assert!(schema_reasons.contains(&"required_field_added".to_string()));
        assert!(schema_reasons.contains(&"schema_property_removed".to_string()));
        assert!(schema_reasons.contains(&"property_type_changed:id".to_string()));
    }

    fn contract(
        snapshot: &str,
        endpoints: Vec<ApiContractItem>,
        schemas: Vec<ApiContractItem>,
    ) -> ApiContractSnapshot {
        ApiContractSnapshot {
            schema: API_CONTRACT_SNAPSHOT_SCHEMA.to_string(),
            snapshot: snapshot.to_string(),
            endpoints,
            schemas,
            examples: Vec::new(),
        }
    }

    fn item(stable_key: &str, payload: Value) -> ApiContractItem {
        ApiContractItem {
            entity_id: None,
            stable_key: stable_key.to_string(),
            name: stable_key.to_string(),
            source: None,
            ownership: Vec::new(),
            payload,
        }
    }

    fn entity(stable_key: &str, kind: EntityKind) -> Entity {
        Entity {
            id: EntityId(stable_key.to_string()),
            stable_key: StableKey(stable_key.to_string()),
            kind,
            name: stable_key.to_string(),
            title: None,
            source: None,
            language: None,
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({}),
        }
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Evidence, EvidenceStatus,
    Ownership, Severity, SnapshotId,
};
use athanor_projector_support::replace_output_file;
use serde_json::Value;

use crate::config::load_config;
use crate::hash::stable_hash;

use super::model::{
    API_CONTRACT_DIFF_SCHEMA, ApiContractChange, ApiContractChangeKind, ApiContractDiff,
    ApiContractItem, ApiContractSnapshot, ApiDiffOptions,
};
use super::retention::{available_snapshots, canonical_root, maybe_cleanup_api_contracts};

pub fn diff_api_contracts(options: ApiDiffOptions) -> Result<ApiContractDiff> {
    let root = canonical_root(&options.root)?;
    let config = load_config(&root)?;
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
    diff.cleanup = maybe_cleanup_api_contracts(&root, &config.api.retention, &options.retention)?;
    Ok(diff)
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

pub(super) fn build_api_contract_diff(
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
        cleanup: None,
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
            (Some(left), Some(right))
                if left.name != right.name || left.payload != right.payload =>
            {
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

fn breaking_change_diagnostic(change: &ApiContractChange, from: &str, to: &str) -> Diagnostic {
    let material = format!(
        "api_breaking_change_detected\0{from}\0{to}\0{}",
        change.stable_key
    );
    let source = change.source.as_ref();
    let mut ownership = change.ownership.clone();
    if ownership.is_empty() {
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

pub(super) fn endpoint_compatibility_reasons(before: &Value, after: &Value) -> Vec<String> {
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

pub(super) fn schema_compatibility_reasons(before: &Value, after: &Value) -> Vec<String> {
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

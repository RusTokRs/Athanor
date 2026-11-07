//! Read-only, individually verified Rust definitions inside confirmed Cargo
//! target Files. No cross-file symbol resolution, compilation, or page writes.

use std::collections::BTreeMap;

use athanor_core::{CanonicalSnapshot, rust_identity::rust_file_symbol_identity};
use athanor_domain::{Entity, EntityKind};
use athanor_extractor_basic::stable_hash;

use super::{
    DocumentationModulePlan, DocumentationPlanIssue, DocumentationRustSymbolPlan, FileInventory,
    modules::{EvidenceIndex, ModuleResolution, VerifiedSymbolScope},
};

pub(super) fn project(
    snapshot: &CanonicalSnapshot,
    files: &FileInventory,
    modules: &ModuleResolution,
    issues: &mut Vec<DocumentationPlanIssue>,
) -> Vec<DocumentationRustSymbolPlan> {
    let evidence = EvidenceIndex::new(snapshot, files);
    // A repeated canonical key cannot substantiate an individual definition,
    // even when both records carry superficially matching source paths.
    let mut candidates = BTreeMap::<(String, String), Vec<&Entity>>::new();
    for entity in &snapshot.entities {
        if !matches!(
            entity.kind,
            EntityKind::Symbol | EntityKind::Function | EntityKind::TestCase
        ) || entity.language.as_ref().map(|language| language.0.as_str()) != Some("rust")
        {
            continue;
        }
        let Some(source) = &entity.source else {
            continue;
        };
        if modules.scopes.contains_key(&source.path) {
            candidates
                .entry((source.path.clone(), entity.stable_key.0.clone()))
                .or_default()
                .push(entity);
        }
    }

    let mut rows = Vec::new();
    let mut trait_methods = Vec::new();
    let mut inherent_methods = Vec::new();
    for ((path, key), entities) in candidates {
        if entities.len() != 1 {
            issues.push(DocumentationPlanIssue::new(
                "ambiguous_rust_symbol_identity",
                &path,
                format!(
                    "multiple Rust entities claim symbol key hash {:016x}",
                    stable_hash(key.as_bytes())
                ),
            ));
            continue;
        }
        let Some(module) = modules.by_file.get(&path) else {
            continue;
        };
        let Some(scope) = modules.scopes.get(&path) else {
            continue;
        };
        let entity = entities[0];
        match entity
            .payload
            .get("symbol_kind")
            .and_then(|value| value.as_str())
        {
            Some("trait_method") => {
                trait_methods.push((entity, path));
                continue;
            }
            Some("method") => {
                inherent_methods.push((entity, path));
                continue;
            }
            _ => {}
        }
        match verified_direct_symbol(entity, &path, module, scope, &evidence) {
            Ok(row) => rows.push(row),
            Err(kind) => rejected(issues, kind, &path, entity),
        }
    }
    // A trait method is admitted only after its containing trait was itself
    // independently proven from the same File and resolved module scope.
    let traits = rows
        .iter()
        .filter(|row| row.symbol_kind == "trait")
        .map(|row| {
            (
                (row.source_file.clone(), row.source_qualified_name.clone()),
                row.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (entity, path) in trait_methods {
        let Some(module) = modules.by_file.get(&path) else {
            continue;
        };
        match verified_trait_method(entity, &path, module, &traits, &evidence) {
            Ok(row) => rows.push(row),
            Err(kind) => rejected(issues, kind, &path, entity),
        }
    }
    let local_types = rows
        .iter()
        .filter(|row| matches!(row.symbol_kind.as_str(), "struct" | "enum" | "union"))
        .map(|row| {
            (
                (row.source_file.clone(), row.source_qualified_name.clone()),
                row.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (entity, path) in inherent_methods {
        let Some(module) = modules.by_file.get(&path) else {
            continue;
        };
        match verified_inherent_method(entity, &path, module, &local_types, &evidence) {
            Ok(row) => rows.push(row),
            Err(kind) => rejected(issues, kind, &path, entity),
        }
    }
    rows.sort_by(|a, b| {
        (
            &a.manifest,
            &a.target,
            &a.qualified_name,
            &a.source_file,
            &a.stable_key,
        )
            .cmp(&(
                &b.manifest,
                &b.target,
                &b.qualified_name,
                &b.source_file,
                &b.stable_key,
            ))
    });
    rows
}

fn rejected(issues: &mut Vec<DocumentationPlanIssue>, kind: &str, path: &str, entity: &Entity) {
    // Never reflect arbitrary snapshot text verbatim into the CLI. A bounded
    // canonical key fingerprint keeps same-File omissions distinguishable.
    issues.push(DocumentationPlanIssue::new(
        kind, path,
        format!(
            "Rust symbol key hash {:016x} was withheld: individual syntax, presence, or definition evidence is unproven",
            stable_hash(entity.stable_key.0.as_bytes())
        ),
    ));
}

struct ProvenDefinition {
    source_qualified_name: String,
    symbol_kind: String,
    visibility: String,
    definition_fact: String,
    line_start: u32,
    line_end: u32,
}

fn proven_definition(
    entity: &Entity,
    path: &str,
    module: &DocumentationModulePlan,
    evidence: &EvidenceIndex<'_>,
) -> Result<ProvenDefinition, &'static str> {
    let (source, qualified) =
        rust_file_symbol_identity(entity).ok_or("invalid_rust_symbol_identity")?;
    if source != path
        || module.syntax != "verified"
        || qualified.len() > 4096
        || qualified.chars().any(char::is_control)
        || entity.name.len() > 256
        || entity.name.chars().any(char::is_control)
    {
        return Err("invalid_rust_symbol_identity");
    }
    let kind = entity
        .payload
        .get("symbol_kind")
        .and_then(|value| value.as_str())
        .ok_or("invalid_rust_symbol_identity")?;
    if entity
        .payload
        .get("definitions")
        .and_then(|value| value.as_u64())
        != Some(1)
        || entity
            .payload
            .get("rust_presence_unverified")
            .and_then(|value| value.as_bool())
            != Some(false)
    {
        return Err("unverified_rust_symbol_presence");
    }
    let definition = evidence
        .definition_fact(entity, path)
        .ok_or("invalid_rust_symbol_evidence")?;
    if definition
        .value
        .get("rust_presence_unverified")
        .and_then(|value| value.as_bool())
        != Some(false)
        || definition.evidence.len() != 1
    {
        return Err("invalid_rust_symbol_evidence");
    }
    let location = entity
        .source
        .as_ref()
        .ok_or("invalid_rust_symbol_evidence")?;
    let (Some(start), Some(end)) = (location.line_start, location.line_end) else {
        return Err("invalid_rust_symbol_evidence");
    };
    if start == 0
        || start > end
        || definition.evidence[0].line_start != Some(start)
        || definition.evidence[0].line_end != Some(end)
    {
        return Err("invalid_rust_symbol_evidence");
    }
    let visibility = entity
        .payload
        .get("visibility")
        .and_then(|value| value.as_str())
        .filter(|value| {
            !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
        })
        .ok_or("invalid_rust_symbol_evidence")?;
    Ok(ProvenDefinition {
        source_qualified_name: qualified.to_string(),
        symbol_kind: kind.to_string(),
        visibility: visibility.to_string(),
        definition_fact: definition.id.0.clone(),
        line_start: start,
        line_end: end,
    })
}

fn verified_direct_symbol(
    entity: &Entity,
    path: &str,
    module: &DocumentationModulePlan,
    scope: &VerifiedSymbolScope,
    evidence: &EvidenceIndex<'_>,
) -> Result<DocumentationRustSymbolPlan, &'static str> {
    let proof = proven_definition(entity, path, module, evidence)?;
    if !matches!(
        (&entity.kind, proof.symbol_kind.as_str()),
        (EntityKind::Function, "function")
            | (EntityKind::TestCase, "test_case")
            | (
                EntityKind::Symbol,
                "struct" | "enum" | "union" | "trait" | "type_alias" | "constant" | "static"
            )
    ) {
        // Associated items require a separate type/impl provenance proof;
        // module declarations are already represented by module facts/rows.
        return Err(
            if matches!(proof.symbol_kind.as_str(), "method" | "trait_method") {
                "unverified_rust_associated_symbol"
            } else {
                "unsupported_rust_symbol_kind"
            },
        );
    }
    let (parent, leaf) = proof
        .source_qualified_name
        .rsplit_once("::")
        .ok_or("invalid_rust_symbol_scope")?;
    if leaf == "_" || leaf != entity.name || !scope.modules.contains(parent) {
        return Err("invalid_rust_symbol_scope");
    }
    let relative = parent
        .strip_prefix(&scope.local_root)
        .ok_or("invalid_rust_symbol_scope")?;
    if !relative.is_empty() && !relative.starts_with("::") {
        return Err("invalid_rust_symbol_scope");
    }
    let logical_module = format!("{}{}", module.module, relative);
    Ok(DocumentationRustSymbolPlan {
        manifest: module.manifest.clone(),
        target: module.target.clone(),
        source_file: path.to_string(),
        module: logical_module.clone(),
        name: entity.name.clone(),
        qualified_name: format!("{logical_module}::{}", entity.name),
        source_qualified_name: proof.source_qualified_name,
        symbol_kind: proof.symbol_kind,
        visibility: proof.visibility,
        stable_key: entity.stable_key.0.clone(),
        definition_fact: proof.definition_fact,
        owner_symbol: None,
        owner_definition_fact: None,
        association_fact: None,
        line_start: proof.line_start,
        line_end: proof.line_end,
    })
}

fn verified_inherent_method(
    entity: &Entity,
    path: &str,
    module: &DocumentationModulePlan,
    types: &BTreeMap<(String, String), DocumentationRustSymbolPlan>,
    evidence: &EvidenceIndex<'_>,
) -> Result<DocumentationRustSymbolPlan, &'static str> {
    let proof = proven_definition(entity, path, module, evidence)?;
    if entity.kind != EntityKind::Function || proof.symbol_kind != "method" {
        return Err("unsupported_rust_symbol_kind");
    }
    let (parent, leaf) = proof
        .source_qualified_name
        .rsplit_once("::")
        .ok_or("invalid_rust_symbol_scope")?;
    if leaf != entity.name {
        return Err("invalid_rust_symbol_scope");
    }
    let owner = types
        .get(&(path.to_string(), parent.to_string()))
        .filter(|owner| owner.manifest == module.manifest && owner.target == module.target)
        .ok_or("unverified_rust_associated_owner")?;
    let owner_entity = evidence
        .symbol_by_key(&owner.stable_key)
        .ok_or("unverified_rust_associated_owner")?;
    let association = evidence
        .inherent_method_fact(entity, owner_entity, path)
        .ok_or("invalid_rust_inherent_method_fact")?;
    Ok(DocumentationRustSymbolPlan {
        manifest: module.manifest.clone(),
        target: module.target.clone(),
        source_file: path.to_string(),
        module: owner.module.clone(),
        name: entity.name.clone(),
        qualified_name: format!("{}::{}", owner.qualified_name, entity.name),
        source_qualified_name: proof.source_qualified_name,
        symbol_kind: proof.symbol_kind,
        visibility: proof.visibility,
        stable_key: entity.stable_key.0.clone(),
        definition_fact: proof.definition_fact,
        owner_symbol: Some(owner.stable_key.clone()),
        owner_definition_fact: Some(owner.definition_fact.clone()),
        association_fact: Some(association.id.0.clone()),
        line_start: proof.line_start,
        line_end: proof.line_end,
    })
}

fn verified_trait_method(
    entity: &Entity,
    path: &str,
    module: &DocumentationModulePlan,
    traits: &BTreeMap<(String, String), DocumentationRustSymbolPlan>,
    evidence: &EvidenceIndex<'_>,
) -> Result<DocumentationRustSymbolPlan, &'static str> {
    let proof = proven_definition(entity, path, module, evidence)?;
    if entity.kind != EntityKind::Function || proof.symbol_kind != "trait_method" {
        return Err("unsupported_rust_symbol_kind");
    }
    let (parent, leaf) = proof
        .source_qualified_name
        .rsplit_once("::")
        .ok_or("invalid_rust_symbol_scope")?;
    if leaf != entity.name {
        return Err("invalid_rust_symbol_scope");
    }
    let owner = traits
        .get(&(path.to_string(), parent.to_string()))
        .filter(|owner| owner.manifest == module.manifest && owner.target == module.target)
        .ok_or("unverified_rust_associated_owner")?;
    Ok(DocumentationRustSymbolPlan {
        manifest: module.manifest.clone(),
        target: module.target.clone(),
        source_file: path.to_string(),
        module: owner.module.clone(),
        name: entity.name.clone(),
        qualified_name: format!("{}::{}", owner.qualified_name, entity.name),
        source_qualified_name: proof.source_qualified_name,
        symbol_kind: proof.symbol_kind,
        visibility: proof.visibility,
        stable_key: entity.stable_key.0.clone(),
        definition_fact: proof.definition_fact,
        owner_symbol: Some(owner.stable_key.clone()),
        owner_definition_fact: Some(owner.definition_fact.clone()),
        association_fact: None,
        line_start: proof.line_start,
        line_end: proof.line_end,
    })
}

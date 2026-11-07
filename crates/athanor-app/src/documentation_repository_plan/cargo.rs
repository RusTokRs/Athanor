//! Cargo package and target evidence from canonical entities and exact File rows.
//! Neither Cargo metadata nor the live filesystem is consulted at planning time.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityKind, EvidenceStatus, Fact, FactKind};
use athanor_extractor_basic::stable_hash;
use serde::Deserialize;

use super::{
    DocumentationPackagePlan, DocumentationPlanIssue, DocumentationTargetPlan, FileInventory,
    foreign_root, inside, join, package_manifest_claims, package_root, portable_fold,
    portable_path, portable_unique_path, reserved_component,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetEvidence {
    version: u8,
    autolib: Option<bool>,
    autobins: Option<bool>,
    autoexamples: Option<bool>,
    autotests: Option<bool>,
    autobenches: Option<bool>,
    workspace: Option<String>,
    edition_inherited: bool,
    lib: Option<TargetSpec>,
    bins: Vec<TargetSpec>,
    examples: Vec<TargetSpec>,
    tests: Vec<TargetSpec>,
    benches: Vec<TargetSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetSpec {
    name: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipEvidence {
    version: u8,
    members: Vec<String>,
    exclude: Vec<String>,
    edition: Option<String>,
}

struct Workspace {
    manifest: String,
    root: String,
    members: BTreeSet<String>,
    excluded: BTreeSet<String>,
    edition: Option<String>,
    valid: bool,
}

pub(super) fn discover(
    snapshot: &CanonicalSnapshot,
    files: &FileInventory,
    issues: &mut Vec<DocumentationPlanIssue>,
) -> (Vec<DocumentationPackagePlan>, Vec<DocumentationTargetPlan>) {
    let mut definitions = HashMap::<&str, Vec<&Fact>>::new();
    let mut fact_ids = HashMap::<&str, usize>::new();
    let mut entity_ids = HashMap::<&str, usize>::new();
    let mut entity_keys = HashMap::<&str, usize>::new();
    for fact in &snapshot.facts {
        *fact_ids.entry(fact.id.0.as_str()).or_default() += 1;
        if fact.kind == FactKind::SymbolDefined {
            definitions
                .entry(fact.subject.0.as_str())
                .or_default()
                .push(fact);
        }
    }
    for entity in &snapshot.entities {
        *entity_ids.entry(entity.id.0.as_str()).or_default() += 1;
        *entity_keys.entry(entity.stable_key.0.as_str()).or_default() += 1;
    }
    let mut workspaces = Vec::new();
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    for entity in &snapshot.entities {
        if entity.kind != EntityKind::Package {
            continue;
        }
        let claims = package_manifest_claims(entity)
            .into_iter()
            .filter(|path| path.as_str() == "Cargo.toml" || path.ends_with("/Cargo.toml"))
            .collect::<BTreeSet<_>>();
        let kind = entity
            .payload
            .get("package_kind")
            .and_then(|kind| kind.as_str());
        if entity.language.as_ref().map(|language| language.0.as_str()) != Some("cargo")
            || !matches!(kind, Some("cargo_package" | "cargo_workspace"))
        {
            // A malformed or incompatible Package row cannot prove ownership,
            // but its manifest-shaped source, ownership or key may still prove
            // that a would-be Cargo root is shared or unverified.
            for claim in claims {
                blocked.insert(claim.clone());
                issues.push(DocumentationPlanIssue::new(
                    "unverified_cargo_manifest_claim",
                    claim,
                    "another Package row claims this Cargo manifest without verified Cargo ownership",
                ));
            }
            continue;
        }
        let Some(kind) = kind else {
            continue;
        };
        let Some(manifest) = validated_manifest(
            entity,
            snapshot,
            files,
            &definitions,
            &fact_ids,
            &entity_ids,
            &entity_keys,
        ) else {
            for claim in claims {
                blocked.insert(claim.clone());
                issues.push(DocumentationPlanIssue::new(
                    "unverified_cargo_manifest_claim",
                    claim,
                    "invalid Cargo evidence cannot establish exclusive ownership of this manifest",
                ));
            }
            issues.push(DocumentationPlanIssue::new(
                "invalid_cargo_evidence",
                entity.source.as_ref().map_or("unknown", |source| source.path.as_str()),
                "package/workspace entity lacks matching File, source, stable key or definition fact",
            ));
            continue;
        };
        if !seen.insert((kind.to_string(), manifest.to_string())) {
            issues.push(DocumentationPlanIssue::new(
                "duplicate_cargo_owner",
                manifest,
                "multiple canonical entities describe the same package/workspace manifest",
            ));
            continue;
        }
        if kind == "cargo_workspace" {
            workspaces.push(workspace_from_entity(entity, manifest, files, issues));
        } else {
            candidates.push((manifest.to_string(), entity));
        }
    }

    // A duplicate entity must not silently supply a usable owner.
    candidates.retain(|(manifest, _)| {
        !blocked.contains(manifest)
            && !issues
                .iter()
                .any(|issue| issue.kind == "duplicate_cargo_owner" && issue.path == *manifest)
    });
    workspaces.retain(|workspace| {
        !blocked.contains(&workspace.manifest)
            && !issues.iter().any(|issue| {
                issue.kind == "duplicate_cargo_owner" && issue.path == workspace.manifest
            })
    });
    workspaces.sort_by(|left, right| left.manifest.cmp(&right.manifest));
    candidates.sort_by(|left, right| left.0.cmp(&right.0));

    let mut packages = Vec::new();
    let mut evidence = BTreeMap::new();
    for (manifest, entity) in candidates {
        let name = entity.payload.get("name").and_then(|name| name.as_str());
        if name != Some(entity.name.as_str())
            || entity.name.is_empty()
            || entity.name.len() > 128
            || !entity
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            issues.push(DocumentationPlanIssue::new(
                "invalid_cargo_package_name",
                &manifest,
                "package name does not agree with the snapshot's canonical entity",
            ));
            continue;
        }
        if entity
            .payload
            .get("edition")
            .and_then(|value| value.as_str())
            .is_some_and(|edition| !matches!(edition, "2015" | "2018" | "2021" | "2024"))
        {
            issues.push(DocumentationPlanIssue::new(
                "unsupported_cargo_edition",
                &manifest,
                "Cargo edition is not a recognized, extractable release",
            ));
            continue;
        }
        let targets = parse_target_evidence(entity, &manifest, issues);
        let (mut membership, workspace_manifest) = if let Some(targets) = &targets {
            membership_for(
                &manifest,
                targets.workspace.as_deref(),
                &workspaces,
                files,
                issues,
            )
        } else {
            ("unresolved", None)
        };
        if targets
            .as_ref()
            .is_some_and(|config| config.edition_inherited)
            && !workspace_manifest
                .as_deref()
                .and_then(|manifest| {
                    workspaces
                        .iter()
                        .find(|workspace| workspace.manifest == manifest)
                })
                .is_some_and(|workspace| workspace.edition.is_some())
        {
            membership = "unresolved";
            issues.push(DocumentationPlanIssue::new(
                "unresolved_inherited_edition",
                &manifest,
                "[package].edition.workspace requires a verified [workspace.package].edition",
            ));
        }
        let modern_auto_discovery = if targets
            .as_ref()
            .is_some_and(|config| config.edition_inherited)
        {
            workspace_manifest
                .as_deref()
                .and_then(|manifest| {
                    workspaces
                        .iter()
                        .find(|workspace| workspace.manifest == manifest)
                })
                .and_then(|workspace| workspace.edition.as_deref())
                .is_some_and(|edition| matches!(edition, "2018" | "2021" | "2024"))
        } else {
            matches!(
                entity
                    .payload
                    .get("edition")
                    .and_then(|value| value.as_str()),
                Some("2018" | "2021" | "2024")
            )
        };
        let root = package_root(&manifest);
        // A manifest with both [workspace] and [package] shares its docs root
        // with workspace-level material, regardless of its repository depth.
        let shared_workspace_root = workspace_manifest.as_deref() == Some(manifest.as_str());
        if shared_workspace_root {
            issues.push(DocumentationPlanIssue::new(
                "shared_workspace_documentation_root", &manifest,
                "a Cargo manifest is both workspace and package; shared docs need explicit ownership",
            ));
        }
        packages.push(DocumentationPackagePlan {
            manifest: manifest.clone(),
            name: entity.name.clone(),
            membership: membership.to_string(),
            workspace_manifest,
            docs_dir: join(root, "docs"),
            index_candidate: (membership != "unresolved" && !shared_workspace_root)
                .then(|| join(root, "docs/index.athanor.md")),
        });
        if let Some(targets) = targets {
            evidence.insert(manifest, (targets, modern_auto_discovery));
        }
    }
    let mut targets = Vec::new();
    for package in &packages {
        if package.membership == "unresolved"
            || package.workspace_manifest.as_deref() == Some(package.manifest.as_str())
        {
            continue;
        }
        let Some((config, modern_auto_discovery)) = evidence.get(&package.manifest) else {
            continue;
        };
        targets.extend(targets_for(
            package,
            config,
            *modern_auto_discovery,
            files,
            &packages,
            issues,
        ));
    }
    (packages, targets)
}

fn validated_manifest<'a>(
    entity: &'a Entity,
    snapshot: &CanonicalSnapshot,
    files: &FileInventory,
    definitions: &HashMap<&str, Vec<&Fact>>,
    fact_ids: &HashMap<&str, usize>,
    entity_ids: &HashMap<&str, usize>,
    entity_keys: &HashMap<&str, usize>,
) -> Option<&'a str> {
    let manifest = entity.source.as_ref()?.path.as_str();
    let kind = entity.payload.get("package_kind")?.as_str()?;
    let expected_key = if kind == "cargo_workspace" {
        format!("cargo-workspace://{manifest}")
    } else {
        format!("package://{manifest}#{}", entity.name)
    };
    let line = entity.source.as_ref()?.line_start?;
    if !portable_path(manifest)
        || manifest != "Cargo.toml" && !manifest.ends_with("/Cargo.toml")
        || !portable_unique_path(manifest, files)
        || entity.stable_key.0 != expected_key
        || kind == "cargo_workspace" && entity.name != "workspace"
        || entity.id.0
            != format!(
                "ent_package_{:016x}",
                stable_hash(entity.stable_key.0.as_bytes())
            )
        || entity_ids.get(entity.id.0.as_str()) != Some(&1)
        || entity_keys.get(entity.stable_key.0.as_str()) != Some(&1)
        || line == 0
        || entity.source.as_ref()?.line_end != Some(line)
        || entity.ownership.len() != 1
        || entity.ownership[0].source_file != manifest
    {
        return None;
    }
    let file_id = files.ids.get(manifest)?;
    let facts = definitions.get(entity.id.0.as_str())?;
    if facts.len() != 1 {
        return None;
    }
    let fact = facts[0];
    let evidence = fact.evidence.first()?;
    (fact.id.0
        == format!(
            "fact_package_defined_{:016x}",
            stable_hash(entity.stable_key.0.as_bytes())
        )
        && fact_ids.get(fact.id.0.as_str()) == Some(&1)
        && fact.subject == entity.id
        && fact.object.as_ref() == Some(file_id)
        && snapshot.snapshot.as_ref() == Some(&fact.snapshot)
        && fact.extractor == "operations"
        && fact.confidence.is_finite()
        && fact.confidence > 0.0
        && fact.ownership.len() == 1
        && fact.ownership[0].source_file == manifest
        && fact
            .value
            .get("stable_key")
            .and_then(|value| value.as_str())
            == Some(entity.stable_key.0.as_str())
        && fact.value.get("path").and_then(|value| value.as_str()) == Some(manifest)
        && fact
            .value
            .get("source_kind")
            .and_then(|value| value.as_str())
            == Some("cargo_manifest")
        && fact
            .value
            .get("package_kind")
            .and_then(|value| value.as_str())
            == Some(kind)
        && fact.evidence.len() == 1
        && evidence.source_file.as_deref() == Some(manifest)
        && evidence.extractor.as_deref() == Some("operations")
        && evidence.status == EvidenceStatus::Verified
        && evidence.confidence.is_finite()
        && evidence.confidence > 0.0
        && evidence.line_start == Some(line)
        && evidence.line_end == Some(line))
    .then_some(manifest)
}

fn workspace_from_entity(
    entity: &Entity,
    manifest: &str,
    files: &FileInventory,
    issues: &mut Vec<DocumentationPlanIssue>,
) -> Workspace {
    let root = package_root(manifest).to_string();
    let mut workspace = Workspace {
        manifest: manifest.to_string(),
        root,
        members: BTreeSet::new(),
        excluded: BTreeSet::new(),
        edition: None,
        valid: true,
    };
    let parsed = entity
        .payload
        .get("membership_evidence")
        .cloned()
        .and_then(|value| serde_json::from_value::<MembershipEvidence>(value).ok());
    let Some(parsed) = parsed.filter(|parsed| parsed.version == 1) else {
        workspace.valid = false;
        issues.push(DocumentationPlanIssue::new(
            "workspace_membership_unavailable",
            manifest,
            "workspace members/excludes cannot be proven from versioned snapshot evidence",
        ));
        return workspace;
    };
    if entity
        .payload
        .get("members")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .as_ref()
        != Some(&parsed.members)
    {
        workspace.valid = false;
        issues.push(DocumentationPlanIssue::new(
            "workspace_membership_mismatch",
            manifest,
            "workspace entity members disagree with its versioned membership evidence",
        ));
        return workspace;
    }
    workspace.edition = parsed.edition;
    if workspace
        .edition
        .as_deref()
        .is_some_and(|edition| !matches!(edition, "2015" | "2018" | "2021" | "2024"))
    {
        workspace.valid = false;
        issues.push(DocumentationPlanIssue::new(
            "unsupported_workspace_edition",
            manifest,
            "[workspace.package].edition is not a recognized Cargo edition",
        ));
    }
    for (members, values) in [(true, parsed.members), (false, parsed.exclude)] {
        for value in values {
            let Some(manifests) = member_manifests(&workspace.root, &value, files) else {
                workspace.valid = false;
                issues.push(DocumentationPlanIssue::new(
                    "unsupported_workspace_member", manifest,
                    "workspace pattern is not a portable literal or supported one-level directory glob",
                ));
                continue;
            };
            if members {
                workspace.members.extend(manifests);
            } else {
                workspace.excluded.extend(manifests);
            }
        }
    }
    workspace
}

fn member_manifests(root: &str, member: &str, files: &FileInventory) -> Option<Vec<String>> {
    if member == "." {
        return Some(vec![join(root, "Cargo.toml")]);
    }
    let member = member.strip_prefix("./").unwrap_or(member);
    let has_glob = member
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']'));
    if !has_glob {
        return (portable_path(member) && !member.ends_with("/Cargo.toml"))
            .then(|| vec![join(&join(root, member), "Cargo.toml")]);
    }
    // The common Cargo `members = ["crates/*"]` form matches one directory
    // component. Never approximate **, ? or character-class patterns.
    let prefix = if member == "*" {
        ""
    } else {
        member.strip_suffix("/*")?
    };
    if !prefix.is_empty() && !portable_path(prefix) {
        return None;
    }
    if prefix
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']'))
    {
        return None;
    }
    let directory = if prefix.is_empty() {
        root.to_string()
    } else {
        join(root, prefix)
    };
    let search = if directory.is_empty() {
        String::new()
    } else {
        format!("{directory}/")
    };
    Some(
        files
            .paths
            .range(search.clone()..)
            .take_while(|path| path.starts_with(&search))
            .filter_map(|path| {
                let relative = path.strip_prefix(&search)?;
                let (component, filename) = relative.split_once('/')?;
                (filename == "Cargo.toml"
                    && !component.starts_with('.')
                    && portable_path(component))
                .then(|| path.clone())
            })
            .collect(),
    )
}

fn parse_target_evidence(
    entity: &Entity,
    manifest: &str,
    issues: &mut Vec<DocumentationPlanIssue>,
) -> Option<TargetEvidence> {
    let parsed = entity
        .payload
        .get("target_evidence")
        .cloned()
        .and_then(|value| {
            let object = value.as_object()?;
            let required = [
                "version",
                "autolib",
                "autobins",
                "autoexamples",
                "autotests",
                "autobenches",
                "workspace",
                "edition_inherited",
                "lib",
                "bins",
                "examples",
                "tests",
                "benches",
            ];
            if required.iter().any(|name| !object.contains_key(*name)) {
                return None;
            }
            serde_json::from_value::<TargetEvidence>(value).ok()
        });
    let Some(parsed) = parsed.filter(|parsed| parsed.version == 1) else {
        issues.push(DocumentationPlanIssue::new(
            "cargo_target_evidence_unavailable",
            manifest,
            "target roots require versioned manifest evidence from a newly indexed snapshot",
        ));
        return None;
    };
    // The extractor stores a literal edition in the Package entity only for
    // `edition = "..."`, never for `edition.workspace = true`. Contradictory
    // forms cannot supply a Cargo target or an inherited default rule.
    if parsed.edition_inherited
        && entity
            .payload
            .get("edition")
            .is_some_and(|edition| !edition.is_null())
    {
        issues.push(DocumentationPlanIssue::new(
            "cargo_edition_evidence_mismatch",
            manifest,
            "package edition is both literal and inherited in the same snapshot",
        ));
        return None;
    }
    Some(parsed)
}

fn membership_for(
    manifest: &str,
    explicit: Option<&str>,
    workspaces: &[Workspace],
    files: &FileInventory,
    issues: &mut Vec<DocumentationPlanIssue>,
) -> (&'static str, Option<String>) {
    let root = package_root(manifest);
    let enclosing = workspaces
        .iter()
        .filter(|workspace| inside(root, &workspace.root))
        .collect::<Vec<_>>();
    let selected = if let Some(path) = explicit {
        let Some(expected) = resolve_workspace_pointer(root, path) else {
            issues.push(DocumentationPlanIssue::new(
                "invalid_workspace_pointer",
                manifest,
                "[package].workspace does not resolve to an in-repository workspace root",
            ));
            return ("unresolved", None);
        };
        let selected = workspaces
            .iter()
            .find(|workspace| workspace.manifest == expected);
        if selected.is_none() {
            issues.push(DocumentationPlanIssue::new(
                "workspace_not_found",
                manifest,
                "[package].workspace has no matching snapshot workspace entity",
            ));
        }
        selected
    } else if enclosing.len() == 1 {
        enclosing.first().copied()
    } else if enclosing.is_empty() {
        // A repository-root package has no parent manifest to inspect; its
        // own verified Cargo.toml is not an unverified enclosing workspace.
        if root.is_empty() {
            return ("standalone", None);
        }
        // An ancestor manifest that did not produce a verified workspace row
        // might still own a nested package. Do not call it standalone by
        // absence of parsed evidence alone.
        let mut parent = root.rsplit_once('/').map_or("", |(parent, _)| parent);
        loop {
            if files
                .folded
                .contains_key(&portable_fold(&join(parent, "Cargo.toml")))
            {
                issues.push(DocumentationPlanIssue::new(
                    "unverified_enclosing_manifest",
                    manifest,
                    "an ancestor Cargo.toml lacks a verified workspace ownership declaration",
                ));
                return ("unresolved", None);
            }
            if parent.is_empty() {
                break;
            }
            parent = parent.rsplit_once('/').map_or("", |(prefix, _)| prefix);
        }
        return ("standalone", None);
    } else {
        issues.push(DocumentationPlanIssue::new(
            "overlapping_workspaces",
            manifest,
            "several nested workspaces can claim the same package",
        ));
        return ("unresolved", None);
    };
    let Some(workspace) = selected else {
        return ("unresolved", None);
    };
    if !workspace.valid || workspace.excluded.contains(manifest) {
        issues.push(DocumentationPlanIssue::new(
            "workspace_membership_unresolved",
            manifest,
            "workspace evidence is invalid or explicitly excludes this package",
        ));
        return ("unresolved", Some(workspace.manifest.clone()));
    }
    if workspace.root == root || workspace.members.contains(manifest) || explicit.is_some() {
        ("member", Some(workspace.manifest.clone()))
    } else {
        issues.push(DocumentationPlanIssue::new(
            "not_a_workspace_member",
            manifest,
            "package is nested under a workspace but no supported member declaration resolves to its manifest",
        ));
        ("unresolved", Some(workspace.manifest.clone()))
    }
}

fn resolve_workspace_pointer(root: &str, value: &str) -> Option<String> {
    if value.is_empty() || value.len() > 512 || value.starts_with('/') || value.contains('\\') {
        return None;
    }
    let mut parts = root
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for part in value.split('/') {
        match part {
            "." => {}
            ".." => {
                parts.pop()?;
            }
            "" => return None,
            part if portable_path(part) => parts.push(part.to_string()),
            _ => return None,
        }
    }
    Some(join(&parts.join("/"), "Cargo.toml"))
}

fn targets_for(
    package: &DocumentationPackagePlan,
    evidence: &TargetEvidence,
    modern_auto_discovery: bool,
    files: &FileInventory,
    packages: &[DocumentationPackagePlan],
    issues: &mut Vec<DocumentationPlanIssue>,
) -> Vec<DocumentationTargetPlan> {
    let mut candidates = Vec::new();
    let root = package_root(&package.manifest);
    let manually_declared = evidence.lib.is_some()
        || !evidence.bins.is_empty()
        || !evidence.examples.is_empty()
        || !evidence.tests.is_empty()
        || !evidence.benches.is_empty();
    // Cargo 2015 disables implicit discovery by default when any target is
    // declared manually. An unresolved inherited edition takes the same
    // conservative path; explicit auto*=true remains authoritative.
    let default_auto = modern_auto_discovery || !manually_declared;
    if !default_auto {
        issues.push(DocumentationPlanIssue::new(
            "legacy_auto_discovery_unverified", &package.manifest,
            "implicit target discovery is disabled without a verified modern edition or an explicit auto*=true",
        ));
    }
    let auto_enabled = |configured: Option<bool>| configured.unwrap_or(default_auto);
    if let Some(spec) = &evidence.lib {
        let name = spec
            .name
            .clone()
            .unwrap_or_else(|| package.name.replace('-', "_"));
        let path = spec.path.as_deref().unwrap_or("src/lib.rs");
        add_target(
            &mut candidates,
            package,
            files,
            packages,
            issues,
            "lib",
            &name,
            path,
            "declared",
        );
    } else if auto_enabled(evidence.autolib) && files.paths.contains(&join(root, "src/lib.rs")) {
        add_target(
            &mut candidates,
            package,
            files,
            packages,
            issues,
            "lib",
            &package.name.replace('-', "_"),
            "src/lib.rs",
            "cargo_default",
        );
    }
    let mut declared_names = BTreeMap::<(String, String), usize>::new();
    for (kind, specs) in [
        ("bin", &evidence.bins),
        ("example", &evidence.examples),
        ("test", &evidence.tests),
        ("bench", &evidence.benches),
    ] {
        for spec in specs {
            let Some(name) = &spec.name else {
                issues.push(DocumentationPlanIssue::new(
                    "incomplete_cargo_target",
                    &package.manifest,
                    "explicit non-library target has no declared name",
                ));
                continue;
            };
            *declared_names
                .entry((kind.to_string(), name.to_ascii_lowercase()))
                .or_default() += 1;
            let path = match &spec.path {
                Some(path) => path.clone(),
                None => {
                    let inferred = infer_declared_root(kind, name, package, files);
                    let Some(path) = inferred else {
                        issues.push(DocumentationPlanIssue::new(
                            "ambiguous_declared_target_root",
                            &package.manifest,
                            "a named target without a path has no unique Cargo-default File root",
                        ));
                        continue;
                    };
                    path
                }
            };
            add_target(
                &mut candidates,
                package,
                files,
                packages,
                issues,
                kind,
                name,
                &path,
                "declared",
            );
        }
    }
    for (kind, enabled, dir) in [
        ("bin", auto_enabled(evidence.autobins), "src/bin"),
        ("example", auto_enabled(evidence.autoexamples), "examples"),
        ("test", auto_enabled(evidence.autotests), "tests"),
        ("bench", auto_enabled(evidence.autobenches), "benches"),
    ] {
        if !enabled {
            continue;
        }
        if kind == "bin" && files.paths.contains(&join(root, "src/main.rs")) {
            add_target(
                &mut candidates,
                package,
                files,
                packages,
                issues,
                kind,
                &package.name,
                "src/main.rs",
                "cargo_default",
            );
        }
        let prefix = format!("{}/", join(root, dir));
        for file in files
            .paths
            .range(prefix.clone()..)
            .take_while(|path| path.starts_with(&prefix))
        {
            let relative = &file[prefix.len()..];
            let name = if relative.ends_with(".rs") && !relative.contains('/') {
                relative.strip_suffix(".rs")
            } else if relative.ends_with("/main.rs") && relative.split('/').count() == 2 {
                relative.strip_suffix("/main.rs")
            } else {
                None
            };
            if let Some(name) = name {
                add_target(
                    &mut candidates,
                    package,
                    files,
                    packages,
                    issues,
                    kind,
                    name,
                    file.strip_prefix(&format!("{root}/")).unwrap_or(file),
                    "cargo_default",
                );
            }
        }
    }
    // An explicit declaration can confirm the same name and default root.
    // Repeated explicit declarations, or distinct spellings that alias on a
    // portable filesystem, cannot be silently collapsed even with one root.
    let mut unique: BTreeMap<(String, String), DocumentationTargetPlan> = BTreeMap::new();
    let mut conflicts = BTreeMap::<(String, String), &'static str>::new();
    for target in candidates {
        let key = (target.kind.clone(), target.name.to_ascii_lowercase());
        if let Some(previous) = unique.get(&key) {
            if previous.root != target.root {
                conflicts.insert(key.clone(), "ambiguous_target_root");
            } else if previous.name != target.name || previous.origin == target.origin {
                conflicts
                    .entry(key.clone())
                    .or_insert("ambiguous_target_identity");
            } else if target.origin == "declared" {
                unique.insert(key, target);
            }
        } else {
            unique.insert(key, target);
        }
    }
    // An explicit name remains duplicated even if one of its paths was
    // unverified and therefore never made it into the candidate list.
    for (key, count) in declared_names {
        if count > 1 {
            conflicts.entry(key).or_insert("ambiguous_target_identity");
        }
    }
    for (key, blocker) in conflicts {
        issues.push(DocumentationPlanIssue::new(
            "ambiguous_cargo_target",
            &package.manifest,
            if blocker == "ambiguous_target_root" {
                "two target roots claim the same kind/name in the exact snapshot"
            } else {
                "repeated or portability-aliased target declarations claim the same kind/name"
            },
        ));
        if let Some(target) = unique.get_mut(&key) {
            target.output_candidate = None;
            target.output_blocker = Some(blocker.to_string());
        }
    }
    let mut targets = unique.into_values().collect::<Vec<_>>();
    let mut by_root: BTreeMap<String, usize> = BTreeMap::new();
    for target in &targets {
        *by_root.entry(target.root.clone()).or_default() += 1;
    }
    for target in &mut targets {
        if by_root.get(&target.root).copied().unwrap_or(0) > 1 {
            target.output_candidate = None;
            target.output_blocker = Some("shared_target_root".to_string());
            issues.push(DocumentationPlanIssue::new(
                "shared_cargo_target_root",
                &target.root,
                "several targets share a source root; Rust symbol ownership is ambiguous",
            ));
        }
        let implicit_enabled = match target.kind.as_str() {
            "bin" => auto_enabled(evidence.autobins),
            "example" => auto_enabled(evidence.autoexamples),
            "test" => auto_enabled(evidence.autotests),
            "bench" => auto_enabled(evidence.autobenches),
            _ => false,
        };
        if implicit_enabled
            && claimed_default_roots(&target.kind, &target.name, package, files).len() > 1
        {
            // Auto discovery can name an additional target even when its File
            // identity is rejected. It must not disappear from the ambiguity
            // check merely because the other root has a valid File row.
            target.output_candidate = None;
            target.output_blocker = Some("ambiguous_target_root".to_string());
            issues.push(DocumentationPlanIssue::new(
                "ambiguous_cargo_target",
                &package.manifest,
                "multiple claimed Cargo-default roots have the same kind/name",
            ));
        }
    }
    targets
}

fn infer_declared_root(
    kind: &str,
    name: &str,
    package: &DocumentationPackagePlan,
    files: &FileInventory,
) -> Option<String> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || reserved_component(name)
    {
        return None;
    }
    let mut claimed = claimed_default_roots(kind, name, package, files).into_iter();
    let only = claimed.next()?;
    (claimed.next().is_none()
        && files
            .paths
            .contains(&join(package_root(&package.manifest), &only)))
    .then_some(only)
}

fn claimed_default_roots(
    kind: &str,
    name: &str,
    package: &DocumentationPackagePlan,
    files: &FileInventory,
) -> Vec<String> {
    let directory = match kind {
        "bin" => "src/bin",
        "example" => "examples",
        "test" => "tests",
        "bench" => "benches",
        _ => return Vec::new(),
    };
    let mut possible = vec![
        format!("{directory}/{name}.rs"),
        format!("{directory}/{name}/main.rs"),
    ];
    if kind == "bin" && name == package.name {
        possible.push("src/main.rs".to_string());
    }
    let root = package_root(&package.manifest);
    possible
        .into_iter()
        .filter(|path| files.folded.contains_key(&portable_fold(&join(root, path))))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn add_target(
    targets: &mut Vec<DocumentationTargetPlan>,
    package: &DocumentationPackagePlan,
    files: &FileInventory,
    packages: &[DocumentationPackagePlan],
    issues: &mut Vec<DocumentationPlanIssue>,
    kind: &str,
    name: &str,
    path: &str,
    origin: &str,
) {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || reserved_component(name)
        || !portable_path(path)
        || path.starts_with("docs/")
    {
        issues.push(DocumentationPlanIssue::new(
            "invalid_cargo_target",
            &package.manifest,
            "target name or path cannot be represented by a portable package-local page",
        ));
        return;
    }
    let root = join(package_root(&package.manifest), path);
    if !files.paths.contains(&root) {
        issues.push(DocumentationPlanIssue::new(
            "missing_cargo_target_root",
            &root,
            "declared or Cargo-default target root is not a File in this exact snapshot",
        ));
        return;
    }
    if !portable_unique_path(&root, files) {
        issues.push(DocumentationPlanIssue::new(
            "ambiguous_cargo_target_root",
            &root,
            "target File or a parent directory has a portable case/normalization alias",
        ));
        return;
    }
    if nearest_package(&root, packages).map(|owner| owner.manifest.as_str())
        != Some(package.manifest.as_str())
        || crosses_unverified_manifest(&root, &package.manifest, files)
    {
        issues.push(DocumentationPlanIssue::new(
            "overlapping_cargo_target",
            &root,
            "another package is the nearest manifest owner of this target root",
        ));
        return;
    }
    // A declared Cargo root can physically be inside another ecosystem's
    // project. Keep the Cargo target visible, but never claim exclusive
    // documentation or symbol ownership of that foreign File subtree.
    let foreign = foreign_root(&root, &files.foreign_roots)
        .is_some_and(|foreign_root| foreign_root.len() >= package_root(&package.manifest).len());
    if foreign {
        issues.push(DocumentationPlanIssue::new(
            "foreign_cargo_target",
            &root,
            "target root lies in a non-Cargo project's directory or a shared project root",
        ));
    }
    targets.push(DocumentationTargetPlan {
        manifest: package.manifest.clone(),
        kind: kind.to_string(),
        name: name.to_string(),
        root,
        origin: origin.to_string(),
        output_candidate: (!foreign).then(|| {
            join(
                package_root(&package.manifest),
                &format!("docs/modules/{kind}-{name}.athanor.md"),
            )
        }),
        output_blocker: foreign.then(|| "foreign_target_project".to_string()),
        // File-qualified Rust keys prevent coalescing, but neither the Cargo
        // target's full module graph nor cross-file links are proven here.
        symbol_omission: Some(
            if foreign {
                "foreign_target_project"
            } else {
                "rust_target_module_scope_unverified"
            }
            .to_string(),
        ),
    });
}

pub(super) fn crosses_unverified_manifest(
    path: &str,
    owner_manifest: &str,
    files: &FileInventory,
) -> bool {
    let root = package_root(owner_manifest);
    let mut directory = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    while directory != root && inside(directory, root) {
        if files
            .folded
            .contains_key(&portable_fold(&join(directory, "Cargo.toml")))
        {
            return true;
        }
        directory = directory.rsplit_once('/').map_or("", |(parent, _)| parent);
    }
    false
}

pub(super) fn nearest_package<'a>(
    path: &str,
    packages: &'a [DocumentationPackagePlan],
) -> Option<&'a DocumentationPackagePlan> {
    packages
        .iter()
        .filter(|package| inside(path, package_root(&package.manifest)))
        .max_by_key(|package| package_root(&package.manifest).len())
}

pub(super) fn nearest_member<'a>(
    path: &str,
    packages: &'a [DocumentationPackagePlan],
) -> Option<&'a DocumentationPackagePlan> {
    nearest_package(path, packages).filter(|owner| owner.membership != "unresolved")
}

//! Snapshot-only ownership inventory for future package-local documentation.
//!
//! This is deliberately not a publisher. Paths in this report are proposals from
//! an immutable canonical snapshot, not permission to write to the working tree.

mod cargo;
mod modules;
mod outputs;
mod symbols;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityId, EntityKind};
use athanor_extractor_basic::stable_hash;
use serde::{Deserialize, Serialize};
use unicase::UniCase;
use unicode_normalization::UnicodeNormalization;

use crate::DocumentationContractError;

pub const DOCUMENTATION_REPOSITORY_PLAN_REQUEST_SCHEMA_V1: &str =
    "athanor.documentation_repository_plan_request.v1";
pub const DOCUMENTATION_REPOSITORY_PLAN_SCHEMA_V1: &str =
    "athanor.documentation_repository_plan.v1";
pub const DOCUMENTATION_REPOSITORY_PLAN_MAX_LIMIT: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentationRepositoryPlanRequest {
    pub schema: String,
    pub snapshot: String,
    pub limit: usize,
}

impl DocumentationRepositoryPlanRequest {
    pub const SCHEMA: &'static str = DOCUMENTATION_REPOSITORY_PLAN_REQUEST_SCHEMA_V1;

    pub fn new(snapshot: impl Into<String>, limit: usize) -> Self {
        Self {
            schema: Self::SCHEMA.to_string(),
            snapshot: snapshot.into(),
            limit,
        }
    }

    pub fn validate(&self) -> Result<(), DocumentationContractError> {
        if self.schema != Self::SCHEMA {
            return Err(invalid(
                "unsupported repository documentation plan request schema",
            ));
        }
        if !portable_snapshot_id(&self.snapshot) {
            return Err(invalid(
                "repository documentation snapshot is not a portable identity",
            ));
        }
        if !(1..=DOCUMENTATION_REPOSITORY_PLAN_MAX_LIMIT).contains(&self.limit) {
            return Err(invalid(format!(
                "repository documentation plan limit must be between 1 and {DOCUMENTATION_REPOSITORY_PLAN_MAX_LIMIT}"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationRepositoryPlan {
    pub schema: String,
    pub request_schema: String,
    pub snapshot: String,
    pub limit: usize,
    /// Always false: the report has no live-file or output-ownership proof.
    pub writes_allowed: bool,
    pub packages: Vec<DocumentationPackagePlan>,
    pub targets: Vec<DocumentationTargetPlan>,
    pub sources: Vec<DocumentationSourcePlan>,
    /// Target-reachable Rust Files, not a claim that the full crate compiles.
    pub modules: Vec<DocumentationModulePlan>,
    /// Individually proven syntax definitions; not a complete target API.
    pub rust_symbols: Vec<DocumentationRustSymbolPlan>,
    pub rust_files: Vec<DocumentationRustFilePlan>,
    pub issues: Vec<DocumentationPlanIssue>,
    pub totals: DocumentationPlanCounts,
    pub omitted: DocumentationPlanCounts,
}

impl DocumentationRepositoryPlan {
    pub const SCHEMA: &'static str = DOCUMENTATION_REPOSITORY_PLAN_SCHEMA_V1;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationPlanCounts {
    pub packages: usize,
    pub targets: usize,
    pub sources: usize,
    pub modules: usize,
    pub rust_symbols: usize,
    pub rust_files: usize,
    pub issues: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationPackagePlan {
    pub manifest: String,
    pub name: String,
    /// `member`, `standalone`, or `unresolved`.
    pub membership: String,
    pub workspace_manifest: Option<String>,
    pub docs_dir: String,
    pub index_candidate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationTargetPlan {
    pub manifest: String,
    pub kind: String,
    pub name: String,
    pub root: String,
    /// `declared` or `cargo_default`, never a filesystem guess.
    pub origin: String,
    pub output_candidate: Option<String>,
    /// Why the path proposal was withheld, if any.
    pub output_blocker: Option<String>,
    /// Why Rust symbol details cannot be projected, independently of the path.
    pub symbol_omission: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationSourcePlan {
    pub path: String,
    /// `markdown`, filename-based `readme` (not necessarily Markdown),
    /// `cargo_manifest`, or `ecosystem_manifest`.
    pub kind: String,
    /// Shared and unresolved material remains visible, never hidden by suffix.
    pub scope: String,
    pub owner_manifest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationModulePlan {
    pub manifest: String,
    pub target: String,
    pub path: String,
    /// Logical target-relative path; `crate` is the verified Cargo root.
    pub module: String,
    /// None for a Cargo root, otherwise the source of one validated `mod` fact.
    pub declared_in: Option<String>,
    pub declaration_fact: Option<String>,
    /// `verified` parser facts, `opaque` macro/attribute syntax, or `unavailable`.
    /// This is syntax evidence, not compilation or complete feature coverage.
    pub syntax: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationRustSymbolPlan {
    pub manifest: String,
    pub target: String,
    pub source_file: String,
    /// Module path anchored to the Cargo target, including verified inline parents.
    pub module: String,
    pub name: String,
    pub qualified_name: String,
    /// Parser-local name, before target/module resolution.
    pub source_qualified_name: String,
    pub symbol_kind: String,
    pub visibility: String,
    pub stable_key: String,
    pub definition_fact: String,
    /// A proven same-File owning type/trait, when this is an associated item.
    pub owner_symbol: Option<String>,
    pub owner_definition_fact: Option<String>,
    /// A separately validated impl-declaration fact, if required by the kind.
    pub association_fact: Option<String>,
    pub line_start: u32,
    pub line_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationRustFilePlan {
    pub path: String,
    pub owner_manifest: Option<String>,
    pub target: Option<String>,
    pub module: Option<String>,
    pub omission: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentationPlanIssue {
    pub kind: String,
    pub path: String,
    pub detail: String,
}

impl DocumentationPlanIssue {
    fn new(kind: &str, path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.to_string(),
            path: path.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Default)]
struct FileInventory {
    paths: BTreeSet<String>,
    ids: BTreeMap<String, EntityId>,
    // All portable File path claims (name, source, ownership and file key) and
    // manifest-shaped Package source/ownership/key claims, including rejected identities:
    // untrusted rows cannot become owners, but still fence nested projects
    // and occupied output paths.
    folded: BTreeMap<String, BTreeSet<String>>,
    directories: BTreeMap<String, BTreeSet<String>>,
    foreign_roots: BTreeSet<String>,
}

impl FileInventory {
    fn observe_path(&mut self, path: &str) {
        self.folded
            .entry(portable_fold(path))
            .or_default()
            .insert(path.to_string());
        let mut parent = String::new();
        let mut components = path.split('/').peekable();
        while let Some(component) = components.next() {
            if components.peek().is_none() {
                break;
            }
            parent = join(&parent, component);
            self.directories
                .entry(portable_fold(&parent))
                .or_default()
                .insert(parent.clone());
        }
        if source_kind(path) == Some("ecosystem_manifest") {
            self.foreign_roots.insert(
                path.rsplit_once('/')
                    .map_or("", |(directory, _)| directory)
                    .to_string(),
            );
        }
    }

    fn from_snapshot(
        snapshot: &CanonicalSnapshot,
        issues: &mut Vec<DocumentationPlanIssue>,
    ) -> Self {
        let mut groups: BTreeMap<String, Vec<&Entity>> = BTreeMap::new();
        let mut entity_ids = BTreeMap::<&str, usize>::new();
        let mut entity_keys = BTreeMap::<&str, usize>::new();
        // Count distinct File *rows* claiming each portable path. A rejected
        // row can point its source, ownership or file key at a real File using
        // exactly the same spelling, so a set of path spellings alone cannot
        // prove the real File has a unique claimant.
        let mut file_claimants = BTreeMap::<String, BTreeSet<usize>>::new();
        for (index, entity) in snapshot.entities.iter().enumerate() {
            *entity_ids.entry(entity.id.0.as_str()).or_default() += 1;
            *entity_keys.entry(entity.stable_key.0.as_str()).or_default() += 1;
            if entity.kind == EntityKind::File {
                groups.entry(entity.name.clone()).or_default().push(entity);
                for path in std::iter::once(entity.name.as_str())
                    .chain(entity.source.iter().map(|source| source.path.as_str()))
                    .chain(
                        entity
                            .ownership
                            .iter()
                            .map(|owner| owner.source_file.as_str()),
                    )
                    .chain(entity.stable_key.0.strip_prefix("file://"))
                {
                    if portable_path(path) && !is_internal_path(path) {
                        file_claimants
                            .entry(portable_fold(path))
                            .or_default()
                            .insert(index);
                    }
                }
            }
        }
        let mut inventory = Self::default();
        for (path, entities) in groups {
            if !portable_path(&path) || is_internal_path(&path) {
                issues.push(DocumentationPlanIssue::new(
                    "invalid_snapshot_path",
                    path,
                    "file path is not a portable, non-internal repository-relative path",
                ));
                continue;
            }
            // Even a duplicate or malformed File row can identify an occupied
            // path or nested manifest. Never promote it into `paths`/`ids`, but
            // retain its spelling as a conservative boundary for valid owners.
            inventory.observe_path(&path);
            if entities.len() != 1 {
                issues.push(DocumentationPlanIssue::new(
                    "ambiguous_file_identity",
                    path,
                    "several canonical file entities claim the same path",
                ));
                continue;
            }
            let entity = entities[0];
            if entity.source.as_ref().map(|source| source.path.as_str()) != Some(path.as_str())
                || entity.stable_key.0 != format!("file://{path}")
                || entity_ids.get(entity.id.0.as_str()) != Some(&1)
                || entity_keys.get(entity.stable_key.0.as_str()) != Some(&1)
                || file_claimants
                    .get(&portable_fold(&path))
                    .is_none_or(|claimants| claimants.len() != 1)
                || entity.id.0
                    != format!(
                        "ent_file_{:016x}",
                        stable_hash(entity.stable_key.0.as_bytes())
                    )
                || entity
                    .payload
                    .get("snapshot")
                    .and_then(|value| value.as_str())
                    != snapshot.snapshot.as_ref().map(|id| id.0.as_str())
                || entity.ownership.len() != 1
                || entity.ownership[0].source_file != path
            {
                issues.push(DocumentationPlanIssue::new(
                    "invalid_file_identity",
                    path,
                    "canonical file path, unique identity/path claimant, snapshot, source and ownership disagree",
                ));
                continue;
            }
            inventory.ids.insert(path.clone(), entity.id.clone());
            inventory.paths.insert(path);
        }
        // Rejected File identities may disagree on name, source, ownership
        // and file key. None proves a File, but every portable path they claim
        // must still fence occupied outputs and nested project boundaries.
        // A Package without a valid File likewise supplies only a manifest
        // boundary, never a verified source or owner.
        for entity in &snapshot.entities {
            if entity.kind == EntityKind::File {
                for path in entity
                    .source
                    .iter()
                    .map(|source| source.path.as_str())
                    .chain(
                        entity
                            .ownership
                            .iter()
                            .map(|owner| owner.source_file.as_str()),
                    )
                    .chain(entity.stable_key.0.strip_prefix("file://"))
                {
                    if portable_path(path) && !is_internal_path(path) {
                        inventory.observe_path(path);
                    }
                }
            }
            if entity.kind == EntityKind::Package {
                for path in package_manifest_claims(entity) {
                    inventory.observe_path(&path);
                }
            }
        }
        for (alias, names) in &inventory.folded {
            if names.len() > 1 {
                issues.push(DocumentationPlanIssue::new(
                    "portable_case_collision",
                    names.iter().next().cloned().unwrap_or_default(),
                    "snapshot contains distinct File or manifest claims with the same portable path alias",
                ));
            }
            if inventory.directories.contains_key(alias) {
                issues.push(DocumentationPlanIssue::new(
                    "snapshot_file_directory_conflict",
                    names.iter().next().cloned().unwrap_or_default(),
                    "snapshot path is claimed both as a File/manifest and as a directory or portable alias",
                ));
            }
        }
        for names in inventory.directories.values() {
            if names.len() > 1 {
                issues.push(DocumentationPlanIssue::new(
                    "portable_directory_case_collision",
                    names.iter().next().cloned().unwrap_or_default(),
                    "snapshot path claims contain directory spellings that alias after normalization or case folding",
                ));
            }
        }
        inventory
    }
}

/// A deterministic, bounded report from one exact canonical snapshot; no I/O.
pub fn build_documentation_repository_plan(
    request: &DocumentationRepositoryPlanRequest,
    snapshot: &CanonicalSnapshot,
) -> Result<DocumentationRepositoryPlan, DocumentationContractError> {
    request.validate()?;
    if snapshot.snapshot.as_ref().map(|id| id.0.as_str()) != Some(request.snapshot.as_str()) {
        return Err(invalid(
            "repository documentation plan requires the exact requested snapshot",
        ));
    }
    let mut issues = Vec::new();
    let inventory = FileInventory::from_snapshot(snapshot, &mut issues);
    let (mut packages, mut targets) = cargo::discover(snapshot, &inventory, &mut issues);
    outputs::preflight_candidates(&inventory, &mut packages, &mut targets, &mut issues);
    let module_resolution =
        modules::discover(snapshot, &inventory, &packages, &targets, &mut issues);
    let rust_symbols = symbols::project(snapshot, &inventory, &module_resolution, &mut issues);
    for target in &mut targets {
        if module_resolution.rows.iter().any(|row| {
            row.manifest == target.manifest
                && row.target == format!("{}:{}", target.kind, target.name)
                && row.syntax == "verified"
        }) {
            // Verified definitions are still only an incomplete syntax inventory:
            // methods, conditional items and whole-file macro effects are omitted.
            target.symbol_omission = Some("rust_symbol_inventory_partial".to_string());
        }
    }
    let package_roots = packages
        .iter()
        .map(|package| (package_root(&package.manifest), package))
        .collect::<BTreeMap<_, _>>();

    let sources = inventory
        .paths
        .iter()
        .filter_map(|path| {
            let kind = source_kind(path)?;
            let nearest = nearest_package_by_root(path, &package_roots);
            let owner = nearest.filter(|owner| {
                owner.membership != "unresolved"
                    && portable_unique_path(path, &inventory)
                    && !cargo::crosses_unverified_manifest(path, &owner.manifest, &inventory)
                    && foreign_root(path, &inventory.foreign_roots)
                        .is_none_or(|root| package_root(&owner.manifest).len() > root.len())
            });
            Some(DocumentationSourcePlan {
                path: path.clone(),
                kind: kind.to_string(),
                scope: match owner {
                    Some(_) => "package",
                    None if nearest.is_some() => "unresolved",
                    None => "repository",
                }
                .to_string(),
                owner_manifest: owner.map(|owner| owner.manifest.clone()),
            })
        })
        .collect::<Vec<_>>();

    let rust_files = inventory
        .paths
        .iter()
        .filter(|path| path.ends_with(".rs"))
        .map(|path| {
            let owner = nearest_package_by_root(path, &package_roots).filter(|owner| {
                owner.membership != "unresolved"
                    && portable_unique_path(path, &inventory)
                    && !cargo::crosses_unverified_manifest(path, &owner.manifest, &inventory)
                    && foreign_root(path, &inventory.foreign_roots)
                        .is_none_or(|root| package_root(&owner.manifest).len() > root.len())
            });
            let reachability = owner.and_then(|owner| {
                module_resolution
                    .by_file
                    .get(path)
                    .filter(|module| module.manifest == owner.manifest)
            });
            DocumentationRustFilePlan {
                path: path.clone(),
                owner_manifest: owner.map(|owner| owner.manifest.clone()),
                target: reachability.map(|module| module.target.clone()),
                module: reachability.map(|module| module.module.clone()),
                omission: Some(
                    if owner.is_none() {
                        "cargo_owner_unresolved"
                    } else if module_resolution.ambiguous.contains(path) {
                        "ambiguous_module_owner"
                    } else if reachability.is_none() {
                        "module_graph_unverified"
                    } else if reachability.is_some_and(|module| module.syntax == "unavailable") {
                        "rust_symbol_evidence_unavailable"
                    } else if reachability.is_some_and(|module| module.syntax == "opaque") {
                        "rust_module_syntax_opaque"
                    } else {
                        // Some source symbols can be proven individually; the
                        // full Rust API and its target context are not proven.
                        "rust_symbol_inventory_partial"
                    }
                    .to_string(),
                ),
            }
        })
        .collect::<Vec<_>>();

    let mut report = DocumentationRepositoryPlan {
        schema: DOCUMENTATION_REPOSITORY_PLAN_SCHEMA_V1.to_string(),
        request_schema: request.schema.clone(),
        snapshot: request.snapshot.clone(),
        limit: request.limit,
        writes_allowed: false,
        packages,
        targets,
        sources,
        modules: module_resolution.rows,
        rust_symbols,
        rust_files,
        issues,
        totals: DocumentationPlanCounts::default(),
        omitted: DocumentationPlanCounts::default(),
    };
    report.packages.sort_by(|a, b| a.manifest.cmp(&b.manifest));
    report.targets.sort_by(|a, b| {
        (&a.manifest, &a.kind, &a.name, &a.root).cmp(&(&b.manifest, &b.kind, &b.name, &b.root))
    });
    report
        .issues
        .sort_by(|a, b| (&a.kind, &a.path, &a.detail).cmp(&(&b.kind, &b.path, &b.detail)));
    report.totals = DocumentationPlanCounts {
        packages: report.packages.len(),
        targets: report.targets.len(),
        sources: report.sources.len(),
        modules: report.modules.len(),
        rust_symbols: report.rust_symbols.len(),
        rust_files: report.rust_files.len(),
        issues: report.issues.len(),
    };
    report.packages.truncate(request.limit);
    report.targets.truncate(request.limit);
    report.sources.truncate(request.limit);
    report.modules.truncate(request.limit);
    report.rust_symbols.truncate(request.limit);
    report.rust_files.truncate(request.limit);
    report.issues.truncate(request.limit);
    report.omitted = DocumentationPlanCounts {
        packages: report.totals.packages - report.packages.len(),
        targets: report.totals.targets - report.targets.len(),
        sources: report.totals.sources - report.sources.len(),
        modules: report.totals.modules - report.modules.len(),
        rust_symbols: report.totals.rust_symbols - report.rust_symbols.len(),
        rust_files: report.totals.rust_files - report.rust_files.len(),
        issues: report.totals.issues - report.issues.len(),
    };
    Ok(report)
}

fn nearest_package_by_root<'a>(
    path: &str,
    packages: &BTreeMap<&str, &'a DocumentationPackagePlan>,
) -> Option<&'a DocumentationPackagePlan> {
    let mut component = path;
    loop {
        if let Some(package) = packages.get(component) {
            return Some(package);
        }
        match component.rsplit_once('/') {
            Some((parent, _)) => component = parent,
            None => return packages.get("").copied(),
        }
    }
}

fn foreign_root<'a>(path: &str, roots: &'a BTreeSet<String>) -> Option<&'a str> {
    let mut component = path;
    loop {
        if let Some(root) = roots.get(component) {
            return Some(root.as_str());
        }
        match component.rsplit_once('/') {
            Some((parent, _)) => component = parent,
            None => return roots.get("").map(String::as_str),
        }
    }
}

fn source_kind(path: &str) -> Option<&'static str> {
    // A compatibility-spelled manifest is still a possible ownership
    // boundary, even if no extractor could establish its exact semantics.
    let name = portable_fold(path.rsplit('/').next().unwrap_or(path));
    if name == "cargo.toml" {
        Some("cargo_manifest")
    } else if name.ends_with(".md") || name.ends_with(".mdx") || name.ends_with(".markdown") {
        Some("markdown")
    } else if name.starts_with("readme") {
        // A README.rs or README.rst is a filename-based documentation input,
        // not evidence that its contents are Markdown.
        Some("readme")
    } else if matches!(
        name.as_str(),
        "package.json"
            | "pnpm-workspace.yaml"
            | "go.mod"
            | "go.work"
            | "pyproject.toml"
            | "setup.py"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "gemfile"
            | "composer.json"
            | "package.swift"
            | "cmakelists.txt"
    ) || name.ends_with(".csproj")
        || name.ends_with(".fsproj")
        || name.ends_with(".sln")
    {
        Some("ecosystem_manifest")
    } else {
        None
    }
}

/// Manifest-shaped Package claims are boundaries, not ownership proofs. The
/// path in a key may itself contain `#`; a malformed *extra* fragment must not
/// hide the preceding Cargo.toml (or foreign manifest) from either the File
/// inventory or the Cargo owner verifier.
fn package_manifest_claims(entity: &Entity) -> BTreeSet<String> {
    let mut claims = BTreeSet::new();
    let mut add = |path: &str| {
        if portable_path(path)
            && !is_internal_path(path)
            && matches!(
                source_kind(path),
                Some("cargo_manifest" | "ecosystem_manifest")
            )
        {
            claims.insert(path.to_string());
        }
    };
    for path in entity
        .source
        .iter()
        .map(|source| source.path.as_str())
        .chain(
            entity
                .ownership
                .iter()
                .map(|owner| owner.source_file.as_str()),
        )
    {
        add(path);
    }
    if let Some(key) = entity
        .stable_key
        .0
        .strip_prefix("cargo-workspace://")
        .or_else(|| entity.stable_key.0.strip_prefix("package://"))
    {
        add(key);
        for (end, _) in key.match_indices('#') {
            if end > 4096 {
                break;
            }
            add(&key[..end]);
        }
    }
    claims
}

// Conservative portable alias key: compatibility normalization plus full
// Unicode case folding catches e.g. composed/decomposed accents and ß/SS.
// It is only a snapshot preflight; a publisher still needs live FS checks.
fn portable_fold(value: &str) -> String {
    let normalized = value.nfkc().collect::<String>();
    UniCase::new(normalized).to_folded_case().nfkc().collect()
}

fn portable_unique_path(path: &str, files: &FileInventory) -> bool {
    let alias = portable_fold(path);
    if files.directories.contains_key(&alias)
        || !files
            .folded
            .get(&alias)
            .is_some_and(|names| names.len() == 1 && names.contains(path))
    {
        // A File cannot itself be a directory, even if both claims use the
        // same spelling. Do not turn a contradictory snapshot into an owner.
        return false;
    }
    let mut parent = String::new();
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        parent = join(&parent, component);
        let parent_alias = portable_fold(&parent);
        if files.folded.contains_key(&parent_alias)
            || files
                .directories
                .get(&parent_alias)
                .is_some_and(|names| names.len() != 1 || !names.contains(&parent))
        {
            return false;
        }
    }
    true
}

fn portable_snapshot_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        && !reserved_component(value)
}

fn portable_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.split('/').all(|component| {
            let normalized = component.nfkc().collect::<String>();
            !normalized.is_empty()
                && !matches!(normalized.as_str(), "." | "..")
                && !normalized.ends_with('.')
                && !normalized.ends_with(' ')
                && !normalized.chars().any(|ch| {
                    ch.is_control()
                        || matches!(ch, ':' | '/' | '\\' | '<' | '>' | '"' | '|' | '?' | '*')
                })
                && !reserved_component(component)
        })
}

fn reserved_component(component: &str) -> bool {
    let normalized = portable_fold(component);
    let stem = normalized.split('.').next().unwrap_or("");
    matches!(stem, "con" | "prn" | "aux" | "nul" | "clock$")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

fn is_internal_path(path: &str) -> bool {
    path.split('/')
        .any(|component| portable_fold(component) == ".athanor")
}

fn package_root(manifest: &str) -> &str {
    manifest.strip_suffix("/Cargo.toml").unwrap_or("")
}

fn inside(path: &str, root: &str) -> bool {
    root.is_empty()
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn join(root: &str, suffix: &str) -> String {
    if root.is_empty() {
        suffix.to_string()
    } else {
        format!("{root}/{suffix}")
    }
}

fn invalid(message: impl Into<String>) -> DocumentationContractError {
    DocumentationContractError(message.into())
}

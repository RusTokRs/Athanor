//! Evidence-backed, deliberately partial Rust module reachability.
//!
//! A source file's path or qualified name is not a Cargo target. Start at a
//! verified Cargo target root, follow only unconditional, conventional `mod`
//! declarations with matching Rust facts and File entities, and preserve all
//! failures as omissions. This never reads source files or runs Cargo.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use athanor_core::{CanonicalSnapshot, rust_identity::rust_file_symbol_identity};
use athanor_domain::{Entity, EntityKind, EvidenceStatus, Fact, FactKind};
use athanor_extractor_basic::stable_hash;
use serde::Deserialize;

use super::{
    DocumentationModulePlan, DocumentationPackagePlan, DocumentationPlanIssue,
    DocumentationTargetPlan, FileInventory, cargo, foreign_root, join, nearest_package_by_root,
    package_root, portable_fold, portable_path, portable_unique_path,
};

const MAX_MODULE_FILES: usize = 20_000;
const MAX_DECLARATIONS_PER_FILE: usize = 4_096;
const MAX_METHOD_FACT_ORDINAL: usize = 100_000;
const MAX_MODULE_DEPTH: usize = 128;

#[derive(Default)]
pub(super) struct ModuleResolution {
    pub(super) rows: Vec<DocumentationModulePlan>,
    pub(super) by_file: BTreeMap<String, DocumentationModulePlan>,
    pub(super) scopes: BTreeMap<String, VerifiedSymbolScope>,
    pub(super) ambiguous: BTreeSet<String>,
}

/// Only names syntactically declared in this particular File. The logical
/// target path is in `DocumentationModulePlan`, not the extractor's path guess.
pub(super) struct VerifiedSymbolScope {
    pub(super) local_root: String,
    pub(super) modules: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleScan {
    version: u8,
    declarations: usize,
    opaque_items: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleDeclaration {
    version: u8,
    ordinal: usize,
    parent: String,
    name: String,
    qualified_name: String,
    form: String,
    inline_ancestors: Vec<String>,
    conditional: bool,
    custom_path: bool,
}

struct VerifiedDeclaration {
    value: ModuleDeclaration,
    fact_id: String,
}

struct VerifiedSource {
    root: String,
    declarations: Vec<VerifiedDeclaration>,
    opaque: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InherentMethodEvidence {
    version: u8,
    ordinal: usize,
    source_file: String,
    owner_qualified_name: String,
    method_qualified_name: String,
    owner_stable_key: String,
    method_stable_key: String,
    conditional: bool,
    form: String,
}

pub(super) struct EvidenceIndex<'a> {
    snapshot: &'a CanonicalSnapshot,
    files: &'a FileInventory,
    entities: HashMap<&'a str, Vec<&'a Entity>>,
    symbols_by_key: HashMap<&'a str, Vec<&'a Entity>>,
    definitions: HashMap<&'a str, Vec<&'a Fact>>,
    declarations: HashMap<&'a str, Vec<&'a Fact>>,
    inherent_methods: HashMap<&'a str, Vec<&'a Fact>>,
    fact_ids: HashMap<&'a str, usize>,
    roots: HashMap<&'a str, Vec<&'a Entity>>,
}

impl<'a> EvidenceIndex<'a> {
    pub(super) fn new(snapshot: &'a CanonicalSnapshot, files: &'a FileInventory) -> Self {
        let mut index = Self {
            snapshot,
            files,
            entities: HashMap::new(),
            symbols_by_key: HashMap::new(),
            definitions: HashMap::new(),
            declarations: HashMap::new(),
            inherent_methods: HashMap::new(),
            fact_ids: HashMap::new(),
            roots: HashMap::new(),
        };
        for entity in &snapshot.entities {
            index
                .entities
                .entry(entity.id.0.as_str())
                .or_default()
                .push(entity);
            index
                .symbols_by_key
                .entry(entity.stable_key.0.as_str())
                .or_default()
                .push(entity);
            if entity.kind == EntityKind::Module
                && entity.payload.get("rust_module_scan").is_some()
                && let Some((path, _)) = rust_file_symbol_identity(entity)
            {
                index.roots.entry(path).or_default().push(entity);
            }
        }
        for fact in &snapshot.facts {
            *index.fact_ids.entry(fact.id.0.as_str()).or_default() += 1;
            if fact.kind == FactKind::SymbolDefined {
                index
                    .definitions
                    .entry(fact.subject.0.as_str())
                    .or_default()
                    .push(fact);
            }
            if matches!(&fact.kind, FactKind::Other(kind) if kind == "rust_module_declaration") {
                index
                    .declarations
                    .entry(fact.subject.0.as_str())
                    .or_default()
                    .push(fact);
            }
            if matches!(&fact.kind, FactKind::Other(kind) if kind == "rust_inherent_method") {
                index
                    .inherent_methods
                    .entry(fact.subject.0.as_str())
                    .or_default()
                    .push(fact);
            }
        }
        index
    }

    fn fact_origin(&self, fact: &Fact, path: &str) -> bool {
        self.snapshot.snapshot.as_ref() == Some(&fact.snapshot)
            && fact.extractor == "rust"
            && fact.confidence.is_finite()
            && fact.confidence > 0.0
            && fact.ownership.len() == 1
            && fact.ownership[0].source_file == path
            && fact.evidence.iter().any(|evidence| {
                evidence.source_file.as_deref() == Some(path)
                    && evidence.extractor.as_deref() == Some("rust")
                    && evidence.status == EvidenceStatus::Verified
                    && evidence.confidence.is_finite()
                    && evidence.confidence > 0.0
                    && evidence.line_start.is_some()
                    && evidence.line_end.is_some()
            })
    }

    pub(super) fn symbol_by_key(&self, key: &str) -> Option<&Entity> {
        let candidates = self.symbols_by_key.get(key)?;
        (candidates.len() == 1).then_some(candidates[0])
    }

    pub(super) fn definition_fact(&self, entity: &Entity, path: &str) -> Option<&Fact> {
        let file_id = self.files.ids.get(path)?;
        let facts = self.definitions.get(entity.id.0.as_str())?;
        let prefix = match entity.kind {
            EntityKind::Module => "ent_rust_module_",
            EntityKind::Function => "ent_rust_function_",
            EntityKind::TestCase => "ent_rust_test_case_",
            EntityKind::Symbol => "ent_rust_symbol_",
            _ => return None,
        };
        if facts.len() != 1
            || self.entities.get(entity.id.0.as_str())?.len() != 1
            || self.symbol_by_key(entity.stable_key.0.as_str())? != entity
            || rust_file_symbol_identity(entity).map(|(source, _)| source) != Some(path)
            || entity.id.0
                != format!(
                    "{}{:016x}",
                    prefix,
                    stable_hash(entity.stable_key.0.as_bytes())
                )
        {
            return None;
        }
        let fact = facts[0];
        (self.fact_ids.get(fact.id.0.as_str()) == Some(&1)
            && fact.kind == FactKind::SymbolDefined
            && fact.id.0
                == format!(
                    "fact_symbol_defined_{:016x}",
                    stable_hash(entity.stable_key.0.as_bytes())
                )
            && fact.subject == entity.id
            && fact.object.as_ref() == Some(file_id)
            && self.fact_origin(fact, path)
            && fact
                .value
                .get("stable_key")
                .and_then(|value| value.as_str())
                == Some(entity.stable_key.0.as_str())
            && fact.value.get("path").and_then(|value| value.as_str()) == Some(path)
            && fact
                .value
                .get("symbol_kind")
                .and_then(|value| value.as_str())
                == entity
                    .payload
                    .get("symbol_kind")
                    .and_then(|value| value.as_str()))
        .then_some(fact)
    }

    /// A separately sourced inherent-impl declaration with exactly one method
    /// and owner claim. The parser's last-segment method name alone is never a
    /// type/impl ownership proof.
    pub(super) fn inherent_method_fact(
        &self,
        method: &Entity,
        owner: &Entity,
        path: &str,
    ) -> Option<&Fact> {
        let facts = self.inherent_methods.get(method.id.0.as_str())?;
        if facts.len() != 1 {
            return None;
        }
        let fact = facts[0];
        let value = serde_json::from_value::<InherentMethodEvidence>(fact.value.clone()).ok()?;
        let (method_path, method_name) = rust_file_symbol_identity(method)?;
        let (owner_path, owner_name) = rust_file_symbol_identity(owner)?;
        let source = method.source.as_ref()?;
        let (start, end) = (source.line_start?, source.line_end?);
        let method_span = fact.evidence.first()?;
        let impl_span = fact.evidence.get(1)?;
        let (impl_start, impl_end) = (impl_span.line_start?, impl_span.line_end?);
        (value.version == 1
            && value.ordinal < MAX_METHOD_FACT_ORDINAL
            && value.form == "simple_inherent"
            && !value.conditional
            && value.source_file == path
            && value.owner_qualified_name == owner_name
            && value.method_qualified_name == method_name
            && method_name == format!("{owner_name}::{}", method.name)
            && value.owner_stable_key == owner.stable_key.0
            && value.method_stable_key == method.stable_key.0
            && method_path == path
            && owner_path == path
            && fact.kind == FactKind::Other("rust_inherent_method".to_string())
            && fact.id.0
                == format!(
                    "fact_rust_inherent_method_{:016x}",
                    stable_hash(
                        format!("{path}\0{owner_name}\0{method_name}\0{}", value.ordinal)
                            .as_bytes()
                    )
                )
            && self.fact_ids.get(fact.id.0.as_str()) == Some(&1)
            && fact.subject == method.id
            && fact.object.as_ref() == Some(&owner.id)
            && self.fact_origin(fact, path)
            && fact.evidence.len() == 2
            && fact.evidence.iter().all(|span| {
                span.source_file.as_deref() == Some(path)
                    && span.extractor.as_deref() == Some("rust")
                    && span.status == EvidenceStatus::Verified
                    && span.confidence.is_finite()
                    && span.confidence > 0.0
            })
            && start > 0
            && start <= end
            && method_span.line_start == Some(start)
            && method_span.line_end == Some(end)
            && impl_start > 0
            && impl_start <= start
            && end <= impl_end)
            .then_some(fact)
    }

    fn symbol_definition(&self, entity: &Entity, path: &str) -> bool {
        self.definition_fact(entity, path).is_some()
    }

    fn source(&self, path: &str) -> Result<VerifiedSource, &'static str> {
        let Some(roots) = self.roots.get(path).filter(|roots| roots.len() == 1) else {
            return Err("rust_source_evidence_unavailable");
        };
        let root = roots[0];
        let definition = self
            .definition_fact(root, path)
            .ok_or("invalid_rust_source_evidence")?;
        let root_source = root.source.as_ref().ok_or("invalid_rust_source_evidence")?;
        if self
            .entities
            .get(root.id.0.as_str())
            .is_none_or(|entities| entities.len() != 1)
            || root
                .payload
                .get("symbol_kind")
                .and_then(|value| value.as_str())
                != Some("module")
            || root
                .payload
                .get("definitions")
                .and_then(|value| value.as_u64())
                != Some(1)
            || definition.evidence.len() != 1
            || definition.evidence[0]
                .line_start
                .is_none_or(|line| line == 0)
            || definition.evidence[0].line_start > definition.evidence[0].line_end
            || root_source.line_start != definition.evidence[0].line_start
            || root_source.line_end != definition.evidence[0].line_end
        {
            return Err("invalid_rust_source_evidence");
        }
        let root_name = root
            .payload
            .get("qualified_name")
            .and_then(|value| value.as_str())
            .filter(|name| !name.is_empty())
            .ok_or("invalid_rust_source_evidence")?;
        let scan = root
            .payload
            .get("rust_module_scan")
            .cloned()
            .and_then(|value| serde_json::from_value::<ModuleScan>(value).ok())
            .filter(|scan| scan.version == 1 && scan.declarations <= MAX_DECLARATIONS_PER_FILE)
            .ok_or("invalid_rust_source_evidence")?;
        if root
            .payload
            .get("rust_presence_unverified")
            .and_then(|value| value.as_bool())
            != Some(false)
            && !scan.opaque_items
        {
            return Err("invalid_rust_source_evidence");
        }
        let file_id = self
            .files
            .ids
            .get(path)
            .ok_or("invalid_rust_source_evidence")?;
        let facts = self.declarations.get(file_id.0.as_str());
        let facts = facts.map(Vec::as_slice).unwrap_or(&[]);
        if facts.len() != scan.declarations {
            return Err("incomplete_rust_module_facts");
        }
        let mut ordinals = BTreeSet::new();
        let mut declarations = Vec::new();
        for fact in facts {
            let value = serde_json::from_value::<ModuleDeclaration>(fact.value.clone())
                .map_err(|_| "invalid_rust_module_fact")?;
            if value.version != 1
                || value.ordinal >= scan.declarations
                || !ordinals.insert(value.ordinal)
                || module_file_name(&value.name).is_none()
                || value
                    .inline_ancestors
                    .iter()
                    .any(|name| module_file_name(name).is_none())
                || !matches!(value.form.as_str(), "inline" | "external")
                || value.qualified_name != format!("{}::{}", value.parent, value.name)
                || value.parent != qualified_parent(root_name, &value.inline_ancestors)
                || self.fact_ids.get(fact.id.0.as_str()) != Some(&1)
                || fact.id.0
                    != format!(
                        "fact_rust_module_decl_{:016x}",
                        stable_hash(
                            format!(
                                "{path}\0{}\0{}\0{}",
                                value.parent, value.name, value.ordinal
                            )
                            .as_bytes()
                        )
                    )
                || fact.subject != *file_id
                || !self.fact_origin(fact, path)
            {
                return Err("invalid_rust_module_fact");
            }
            let Some(entity) = fact
                .object
                .as_ref()
                .and_then(|id| self.entities.get(id.0.as_str()))
                .filter(|entities| entities.len() == 1)
                .map(|entities| entities[0])
            else {
                return Err("invalid_rust_module_fact");
            };
            if entity.kind != EntityKind::Module
                || entity.name != value.name
                || rust_file_symbol_identity(entity) != Some((path, value.qualified_name.as_str()))
                || !self.symbol_definition(entity, path)
                || (!value.conditional
                    && !value.custom_path
                    && entity
                        .payload
                        .get("rust_presence_unverified")
                        .and_then(|value| value.as_bool())
                        != Some(false))
                || fact.evidence.len() != 1
                || fact.evidence[0].line_start.is_none_or(|line| line == 0)
                || fact.evidence[0].line_start > fact.evidence[0].line_end
                || entity.source.as_ref().and_then(|source| source.line_start)
                    != fact.evidence[0].line_start
                || entity.source.as_ref().and_then(|source| source.line_end)
                    != fact.evidence[0].line_end
            {
                return Err("invalid_rust_module_fact");
            }
            declarations.push(VerifiedDeclaration {
                value,
                fact_id: fact.id.0.clone(),
            });
        }
        for declaration in &declarations {
            let mut parent = root_name.to_string();
            for ancestor in &declaration.value.inline_ancestors {
                let qualified = format!("{parent}::{ancestor}");
                let Some(inline) = declarations.iter().find(|item| {
                    item.value.qualified_name == qualified && item.value.form == "inline"
                }) else {
                    return Err("invalid_rust_module_fact");
                };
                if inline.value.conditional && !declaration.value.conditional
                    || inline.value.custom_path && !declaration.value.custom_path
                {
                    return Err("invalid_rust_module_fact");
                }
                parent = qualified;
            }
        }
        declarations.sort_by_key(|item| item.value.ordinal);
        Ok(VerifiedSource {
            root: root_name.to_string(),
            declarations,
            opaque: scan.opaque_items,
        })
    }
}

fn module_file_name(value: &str) -> Option<&str> {
    // `mod r#type;` declares `type.rs`, while the syntactic qualified name
    // retains the raw-identifier spelling from syn.
    let name = value.strip_prefix("r#").unwrap_or(value);
    let mut characters = name.chars();
    let first = characters.next()?;
    (name != "_"
        && (first == '_' || first.is_alphabetic())
        && characters.all(|ch| ch == '_' || ch.is_alphanumeric())
        && portable_path(name))
    .then_some(name)
}

fn qualified_parent(root: &str, inline_ancestors: &[String]) -> String {
    inline_ancestors
        .iter()
        .fold(root.to_string(), |parent, name| format!("{parent}::{name}"))
}

struct Pending {
    path: String,
    module: String,
    declared_in: Option<String>,
    declaration_fact: Option<String>,
    depth: usize,
    target_root: bool,
}

pub(super) fn discover(
    snapshot: &CanonicalSnapshot,
    files: &FileInventory,
    packages: &[DocumentationPackagePlan],
    targets: &[DocumentationTargetPlan],
    issues: &mut Vec<DocumentationPlanIssue>,
) -> ModuleResolution {
    let index = EvidenceIndex::new(snapshot, files);
    let package_roots = packages
        .iter()
        .map(|package| (package_root(&package.manifest), package))
        .collect::<BTreeMap<_, _>>();
    let mut by_root = BTreeMap::<&str, usize>::new();
    for target in targets {
        *by_root.entry(target.root.as_str()).or_default() += 1;
    }
    let mut rows = Vec::new();
    let mut scopes = BTreeMap::new();
    let mut visits = 0;
    for target in targets {
        if matches!(
            target.output_blocker.as_deref(),
            Some("ambiguous_target_root" | "ambiguous_target_identity" | "shared_target_root")
        ) || by_root.get(target.root.as_str()) != Some(&1)
            || !admissible(&target.root, &target.manifest, files, &package_roots)
        {
            continue;
        }
        let mut pending = VecDeque::from([Pending {
            path: target.root.clone(),
            module: "crate".to_string(),
            declared_in: None,
            declaration_fact: None,
            depth: 0,
            target_root: true,
        }]);
        let mut seen = BTreeSet::new();
        while let Some(node) = pending.pop_front() {
            if !seen.insert((node.path.clone(), node.module.clone())) {
                continue;
            }
            if visits >= MAX_MODULE_FILES {
                issues.push(DocumentationPlanIssue::new(
                    "rust_module_graph_limit",
                    &target.root,
                    "the bounded module graph visit limit was reached; remaining files are omitted",
                ));
                break;
            }
            visits += 1;
            let source = index.source(&node.path);
            let syntax = match &source {
                Ok(source) if source.opaque => "opaque",
                Ok(_) => "verified",
                Err(_) => "unavailable",
            };
            if let Err(kind) = &source {
                issues.push(DocumentationPlanIssue::new(
                    kind,
                    &node.path,
                    "Rust file has no complete, same-snapshot parser and definition evidence",
                ));
            }
            if syntax == "opaque" {
                issues.push(DocumentationPlanIssue::new(
                    "rust_module_macro_opaque",
                    &node.path,
                    "macro or attribute syntax may declare modules not visible to the parser",
                ));
            }
            rows.push(DocumentationModulePlan {
                manifest: target.manifest.clone(),
                target: format!("{}:{}", target.kind, target.name),
                path: node.path.clone(),
                module: node.module.clone(),
                declared_in: node.declared_in,
                declaration_fact: node.declaration_fact,
                syntax: syntax.to_string(),
            });
            let Ok(source) = source else {
                continue;
            };
            if source.opaque {
                // A macro or procedural attribute can contribute competing
                // declarations; syntax-only reachability is not conclusive.
                continue;
            }
            let mut counts = BTreeMap::<&str, usize>::new();
            for declaration in &source.declarations {
                *counts
                    .entry(declaration.value.qualified_name.as_str())
                    .or_default() += 1;
            }
            let mut inline_modules = BTreeSet::from([source.root.clone()]);
            for declaration in &source.declarations {
                let value = &declaration.value;
                if value.form == "inline"
                    && !value.conditional
                    && !value.custom_path
                    && counts.get(value.qualified_name.as_str()) == Some(&1)
                    && inline_modules.contains(&value.parent)
                {
                    inline_modules.insert(value.qualified_name.clone());
                }
            }
            scopes.insert(
                node.path.clone(),
                VerifiedSymbolScope {
                    local_root: source.root.clone(),
                    modules: inline_modules,
                },
            );
            for declaration in &source.declarations {
                let value = &declaration.value;
                if value.form != "external" {
                    continue;
                }
                if counts.get(value.qualified_name.as_str()) != Some(&1)
                    || !inline_modules.contains(&value.parent)
                {
                    issues.push(DocumentationPlanIssue::new(
                        "unverified_rust_module_scope",
                        &node.path,
                        "external module declaration has a duplicate or unverified inline parent",
                    ));
                    continue;
                }
                if value.conditional || value.custom_path {
                    issues.push(DocumentationPlanIssue::new(
                        if value.conditional { "rust_conditional_module" } else { "rust_custom_module_path" },
                        &node.path,
                        "conditional or custom-path module declaration has no confirmed default File",
                    ));
                    continue;
                }
                let Some(base) = module_base(&node.path, node.target_root, &value.inline_ancestors)
                else {
                    issues.push(DocumentationPlanIssue::new(
                        "rust_module_path_unresolved",
                        &node.path,
                        "module declaration cannot be resolved to a portable default File path",
                    ));
                    continue;
                };
                let Some(name) = module_file_name(&value.name) else {
                    continue;
                };
                let candidates = [
                    join(&base, &format!("{name}.rs")),
                    join(&join(&base, name), "mod.rs"),
                ];
                // An invalid File row cannot prove a module, but it can claim
                // the competing conventional path (or a portable alias). Do
                // not call the other, valid File unambiguous by ignoring it.
                let claimed = candidates
                    .iter()
                    .filter(|path| files.folded.contains_key(&portable_fold(path)))
                    .collect::<Vec<_>>();
                if claimed.len() != 1 {
                    issues.push(DocumentationPlanIssue::new(
                        if claimed.is_empty() {
                            "missing_rust_module_file"
                        } else {
                            "ambiguous_rust_module_file"
                        },
                        &node.path,
                        "external mod does not name exactly one claimed conventional File",
                    ));
                    continue;
                }
                let child = claimed[0].as_str();
                if !files.ids.contains_key(child) {
                    issues.push(DocumentationPlanIssue::new(
                        "unverified_rust_module_file",
                        &node.path,
                        "the only claimed conventional module path has no verified File identity",
                    ));
                    continue;
                }
                if !admissible(child, &target.manifest, files, &package_roots) {
                    issues.push(DocumentationPlanIssue::new(
                        "rust_module_crosses_owner", child,
                        "module File crosses a package, nested manifest or foreign-project boundary",
                    ));
                    continue;
                }
                if node.depth >= MAX_MODULE_DEPTH {
                    issues.push(DocumentationPlanIssue::new(
                        "rust_module_depth_limit",
                        child,
                        "module traversal depth limit reached; child is omitted",
                    ));
                    continue;
                }
                let logical = format!(
                    "{}::{}",
                    qualified_parent(&node.module, &value.inline_ancestors),
                    value.name
                );
                pending.push_back(Pending {
                    path: child.to_string(),
                    module: logical,
                    declared_in: Some(node.path.clone()),
                    declaration_fact: Some(declaration.fact_id.clone()),
                    depth: node.depth + 1,
                    target_root: false,
                });
            }
        }
    }

    let mut claims = BTreeMap::<String, BTreeSet<(String, String, String, Option<String>)>>::new();
    for row in &rows {
        claims.entry(row.path.clone()).or_default().insert((
            row.manifest.clone(),
            row.target.clone(),
            row.module.clone(),
            row.declaration_fact.clone(),
        ));
    }
    let mut ambiguous = claims
        .iter()
        .filter(|(_, claims)| claims.len() > 1)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let mut changed = false;
        for row in &rows {
            if row
                .declared_in
                .as_ref()
                .is_some_and(|path| ambiguous.contains(path))
            {
                changed |= ambiguous.insert(row.path.clone());
            }
        }
        if !changed {
            break;
        }
    }
    for path in &ambiguous {
        issues.push(DocumentationPlanIssue::new(
            "ambiguous_rust_module_owner",
            path,
            "multiple target/module paths claim this File, or its parent File is ambiguous",
        ));
    }
    rows.retain(|row| !ambiguous.contains(&row.path));
    rows.sort_by(|left, right| {
        (&left.manifest, &left.target, &left.module, &left.path).cmp(&(
            &right.manifest,
            &right.target,
            &right.module,
            &right.path,
        ))
    });
    let by_file = rows
        .iter()
        .map(|row| (row.path.clone(), row.clone()))
        .collect::<BTreeMap<_, _>>();
    scopes.retain(|path, _| by_file.contains_key(path));
    ModuleResolution {
        rows,
        by_file,
        scopes,
        ambiguous,
    }
}

fn admissible(
    path: &str,
    manifest: &str,
    files: &FileInventory,
    package_roots: &BTreeMap<&str, &DocumentationPackagePlan>,
) -> bool {
    files.ids.contains_key(path)
        && portable_unique_path(path, files)
        && nearest_package_by_root(path, package_roots)
            .is_some_and(|owner| owner.membership != "unresolved" && owner.manifest == manifest)
        && !cargo::crosses_unverified_manifest(path, manifest, files)
        && foreign_root(path, &files.foreign_roots)
            .is_none_or(|root| package_root(manifest).len() > root.len())
}

fn module_base(path: &str, target_root: bool, inline_ancestors: &[String]) -> Option<String> {
    let (directory, file) = path.rsplit_once('/').unwrap_or(("", path));
    let stem = file.strip_suffix(".rs")?;
    let mut base = if target_root || stem == "mod" {
        directory.to_string()
    } else {
        join(directory, stem)
    };
    for ancestor in inline_ancestors {
        base = join(&base, module_file_name(ancestor)?);
    }
    portable_path(&join(&base, "probe.rs")).then_some(base)
}

//! Import and call resolution over the tree-sitter pack payloads.
//!
//! Import resolution reads the `style` the engine recorded, so the module layout stays a property of
//! the language pack: `dotted` and `from` statements resolve through dotted module paths (including
//! relative imports), while `spec` statements resolve import paths by the longest package-directory
//! suffix and skip standard-library paths. Call resolution is shared and ordered from the most
//! precise evidence to the least, and every name-based step requires exactly one candidate.

use std::collections::BTreeMap;

use athanor_domain::{Entity, RelationStatus};
use serde_json::Value;

use crate::index::{Index, is_exported, layout, module_key, qualified_name, symbol_key, unit_of};
use crate::{
    IMPORTED_EXACT_CONFIDENCE, IMPORTED_NAME_CONFIDENCE, SAME_UNIT_EXACT_CONFIDENCE,
    SAME_UNIT_OWN_CONFIDENCE, UNIQUE_NAME_CONFIDENCE,
};

/// One local name an import statement binds.
pub(crate) struct Binding<'a> {
    /// The name the importing module can use.
    pub(crate) local: String,
    /// The imported name, when the statement imports a specific name instead of a module.
    pub(crate) symbol: Option<String>,
    /// The recorded import payload the binding came from.
    pub(crate) import: &'a Value,
}

/// Resolved import targets with the evidence class of the match.
pub(crate) struct ImportResolution<'a> {
    pub(crate) targets: Vec<&'a Entity>,
    pub(crate) reason: &'static str,
    pub(crate) status: RelationStatus,
    pub(crate) confidence: f32,
}

impl<'a> ImportResolution<'a> {
    fn empty() -> Self {
        Self {
            targets: Vec::new(),
            reason: "unresolved",
            status: RelationStatus::Missing,
            confidence: 0.0,
        }
    }
}

pub(crate) fn call_sites(entity: &Entity) -> impl Iterator<Item = &Value> {
    entity
        .payload
        .get("calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

pub(crate) fn import_payloads(entity: &Entity) -> impl Iterator<Item = &Value> {
    entity
        .payload
        .get("imports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// Resolves the module entities one recorded import statement refers to.
pub(crate) fn import_targets<'a>(
    index: &Index<'a>,
    module: &Entity,
    import: &Value,
) -> ImportResolution<'a> {
    let Some(text) = import.get("module").and_then(Value::as_str) else {
        return ImportResolution::empty();
    };
    if text.is_empty() {
        return ImportResolution::empty();
    }
    let Some((language, importer_path)) = module_key(&module.stable_key.0) else {
        return ImportResolution::empty();
    };
    match import.get("style").and_then(Value::as_str) {
        Some("dotted" | "from") => dotted_resolution(index, language, importer_path, text),
        Some("spec") => {
            if import
                .get("stdlib")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return ImportResolution::empty();
            }
            package_resolution(index, language, text)
        }
        _ => ImportResolution::empty(),
    }
}

/// Dotted module text (`a.b.c`, or `..pkg` for a relative import) to an existing module.
fn dotted_resolution<'a>(
    index: &Index<'a>,
    language: &str,
    importer_path: &str,
    text: &str,
) -> ImportResolution<'a> {
    let relative = split_relative(text);
    let reason = if relative.is_some() {
        "relative_module_path"
    } else {
        "dotted_module_path"
    };
    let candidates = match relative {
        Some((dots, body)) => {
            let mut base = crate::index::directory_of(importer_path).to_string();
            for _ in 1..dots {
                base = crate::index::directory_of(&base).to_string();
            }
            dotted_candidates(&base, body, language)
        }
        None => dotted_candidates("", text, language),
    };
    let targets = candidates
        .iter()
        .find_map(|candidate| index.module(language, candidate))
        .into_iter()
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return ImportResolution::empty();
    }
    ImportResolution {
        targets,
        reason,
        status: RelationStatus::Verified,
        confidence: 1.0,
    }
}

fn split_relative(text: &str) -> Option<(usize, &str)> {
    let dots = text.len() - text.trim_start_matches('.').len();
    if dots == 0 {
        return None;
    }
    Some((dots, &text[dots..]))
}

fn dotted_candidates(base: &str, dotted: &str, language: &str) -> Vec<String> {
    let layout = layout(language);
    let stem = match (base.is_empty(), dotted.is_empty()) {
        (true, _) => dotted.replace('.', "/"),
        (false, true) => base.to_string(),
        (false, false) => format!("{base}/{}", dotted.replace('.', "/")),
    };
    if stem.is_empty() {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    for extension in layout.module_extensions {
        candidates.push(format!("{stem}.{extension}"));
    }
    for package_file in layout.package_files {
        candidates.push(format!("{stem}/{package_file}"));
    }
    if candidates.is_empty() {
        candidates.push(stem);
    }
    candidates
}

/// Import path to the package directory with the longest matching suffix.
fn package_resolution<'a>(
    index: &Index<'a>,
    language: &str,
    import_path: &str,
) -> ImportResolution<'a> {
    let wanted = import_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let mut matches = BTreeMap::<String, Vec<&'a Entity>>::new();
    for module in index.modules() {
        let Some((module_language, path)) = module_key(&module.stable_key.0) else {
            continue;
        };
        if module_language != language {
            continue;
        }
        let directory = crate::index::directory_of(path);
        if directory.is_empty() {
            continue;
        }
        let segments = directory
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.len() > wanted.len() {
            continue;
        }
        if wanted[wanted.len() - segments.len()..] != segments[..] {
            continue;
        }
        matches
            .entry(directory.to_string())
            .or_default()
            .push(module);
    }
    let Some((_, mut targets)) = matches
        .into_iter()
        .max_by_key(|(directory, _)| directory.matches('/').count())
    else {
        return ImportResolution::empty();
    };
    targets.sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
    ImportResolution {
        targets,
        reason: "package_suffix",
        status: RelationStatus::Inferred,
        confidence: 0.8,
    }
}

/// Resolves one statically named callee to a canonical declaration.
pub(crate) fn resolve_call<'a>(
    index: &Index<'a>,
    caller: &'a Entity,
    caller_module: &'a Entity,
    callee: &str,
) -> Option<(&'a Entity, &'static str, f32)> {
    let (language, caller_path, _) = symbol_key(&caller.stable_key.0)?;
    let unit = unit_of(language, caller_path);
    let same_unit = index.declarations_in(language, &unit);
    let last_segment = callee.rsplit('.').next().unwrap_or(callee);
    let tail = callee.contains('.').then_some(last_segment);

    // 1. A member called on the language's own instance (`self.encode()` in `Client.send`).
    if let Some(own) = layout(language).own_receiver
        && let Some(member) = callee
            .strip_prefix(own)
            .and_then(|rest| rest.strip_prefix('.'))
        && let Some(owner) = owner_of(caller)
    {
        let qualified = format!("{owner}.{member}");
        if let Some(target) = find_first(same_unit, |candidate| {
            qualified_name(candidate) == Some(qualified.as_str())
        }) {
            return Some((target, "same_unit_own_receiver", SAME_UNIT_OWN_CONFIDENCE));
        }
    }

    // 2. The exact qualified name inside the caller's resolution unit.
    if let Some(target) = find_first(same_unit, |candidate| {
        qualified_name(candidate) == Some(callee)
    }) {
        return Some((
            target,
            "same_unit_exact_name",
            SAME_UNIT_EXACT_CONFIDENCE,
        ));
    }

    // 3. Import bindings of the calling module.
    if let Some(resolved) = through_bindings(index, caller_module, callee, tail) {
        return Some(resolved);
    }

    if callee.contains('.') {
        // 4. Member call with an unknown receiver: a unique member name inside the unit.
        let suffix = format!(".{last_segment}");
        if let Some(target) = unique_match(same_unit, |candidate| {
            candidate.name == last_segment
                && qualified_name(candidate).is_some_and(|qualified| {
                    qualified == last_segment || qualified.ends_with(&suffix)
                })
        }) {
            return Some((target, "same_unit_member_name", UNIQUE_NAME_CONFIDENCE));
        }
    } else if let Some(target) =
        unique_match(index.declarations_named(language, callee), |candidate| {
            candidate.id != caller.id && is_exported(candidate)
        })
    {
        // 5. Bare call without a local binding: a unique exported declaration elsewhere.
        return Some((target, "unique_exported_name", UNIQUE_NAME_CONFIDENCE));
    }

    None
}

fn through_bindings<'a>(
    index: &Index<'a>,
    caller_module: &Entity,
    callee: &str,
    tail: Option<&str>,
) -> Option<(&'a Entity, &'static str, f32)> {
    let (language, importer_path, _) = symbol_key(&caller_module.stable_key.0)?;
    let wanted = tail.unwrap_or(callee);
    let local = if tail.is_some() {
        callee.split('.').next().unwrap_or(callee)
    } else {
        callee
    };
    for binding in import_bindings(caller_module) {
        if binding.local != local {
            continue;
        }
        let text = binding
            .import
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        let text = match (&binding.symbol, tail) {
            (Some(symbol), Some(_)) => format!("{text}.{symbol}"),
            _ => text.to_string(),
        };
        let resolution = if binding.import.get("style").and_then(Value::as_str) == Some("spec") {
            package_resolution(index, language, &text)
        } else {
            dotted_resolution(index, language, importer_path, &text)
        };
        for target_module in resolution.targets {
            let Some((_, target_path)) = module_key(&target_module.stable_key.0) else {
                continue;
            };
            let target_unit = unit_of(language, target_path);
            let candidates = index.declarations_in(language, &target_unit);
            if let Some(target) = find_first(candidates, |candidate| {
                qualified_name(candidate) == Some(wanted)
            }) {
                return Some((target, "imported_exact_name", IMPORTED_EXACT_CONFIDENCE));
            }
            if let Some(target) = find_first(candidates, |candidate| candidate.name == wanted) {
                return Some((target, "imported_symbol_name", IMPORTED_NAME_CONFIDENCE));
            }
        }
    }
    None
}

/// The local names one module can use, read from its recorded import payloads.
pub(crate) fn import_bindings<'a>(module: &'a Entity) -> Vec<Binding<'a>> {
    let mut bindings = Vec::new();
    for import in import_payloads(module) {
        let text = import
            .get("module")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        match import.get("style").and_then(Value::as_str) {
            Some("dotted" | "spec") => {
                let style = import.get("style").and_then(Value::as_str);
                let local = import.get("alias").and_then(Value::as_str).map_or_else(
                    || {
                        if style == Some("dotted") {
                            text.split('.').next().unwrap_or(text).to_string()
                        } else {
                            text.rsplit('/').next().unwrap_or(text).to_string()
                        }
                    },
                    str::to_string,
                );
                if !local.is_empty() {
                    bindings.push(Binding {
                        local,
                        symbol: None,
                        import,
                    });
                }
            }
            Some("from") => {
                let aliases = import.get("aliases").and_then(Value::as_object);
                let names = import.get("names").and_then(Value::as_array);
                for name in names.into_iter().flatten().filter_map(Value::as_str) {
                    if name == "*" {
                        continue;
                    }
                    let local = aliases
                        .and_then(|aliases| aliases.get(name))
                        .and_then(Value::as_str)
                        .map_or_else(|| name.to_string(), str::to_string);
                    bindings.push(Binding {
                        local,
                        symbol: Some(name.to_string()),
                        import,
                    });
                }
            }
            _ => {}
        }
    }
    bindings
}

fn owner_of(entity: &Entity) -> Option<&str> {
    qualified_name(entity)?.split_once('.').map(|(owner, _)| owner)
}

fn find_first<'a>(
    candidates: &[&'a Entity],
    predicate: impl Fn(&Entity) -> bool,
) -> Option<&'a Entity> {
    candidates
        .iter()
        .copied()
        .find(|candidate| predicate(candidate))
}

fn unique_match<'a>(
    candidates: &[&'a Entity],
    predicate: impl Fn(&Entity) -> bool,
) -> Option<&'a Entity> {
    let mut found: Option<&'a Entity> = None;
    for candidate in candidates {
        if predicate(candidate) {
            if found.is_some() {
                return None;
            }
            found = Some(candidate);
        }
    }
    found
}

/// Per-language resolution coverage: recorded references against resolved ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCoverage {
    pub language: String,
    pub modules: usize,
    pub declarations: usize,
    pub imports_declared: usize,
    pub imports_resolved: usize,
    pub call_sites: usize,
    pub calls_resolved: usize,
    pub module_call_sites: usize,
}

impl LanguageCoverage {
    fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            modules: 0,
            declarations: 0,
            imports_declared: 0,
            imports_resolved: 0,
            call_sites: 0,
            calls_resolved: 0,
            module_call_sites: 0,
        }
    }

    /// Resolved call sites per ten thousand recorded call sites.
    pub fn calls_basis_points(&self) -> u32 {
        basis_points(self.calls_resolved, self.call_sites)
    }

    /// Resolved import statements per ten thousand recorded import statements.
    pub fn imports_basis_points(&self) -> u32 {
        basis_points(self.imports_resolved, self.imports_declared)
    }
}

fn basis_points(resolved: usize, recorded: usize) -> u32 {
    if recorded == 0 {
        return 0;
    }
    ((resolved.saturating_mul(10_000)) / recorded) as u32
}

/// Measures resolution coverage for every tree-sitter language in `entities`.
///
/// The denominator is the evidence the extractor recorded: import statements and call sites inside
/// declaration bodies. Module-level call sites are counted separately because no declaration owns
/// them, so they stay out of the denominator instead of silently lowering it.
pub fn resolution_coverage(entities: &[Entity]) -> Vec<LanguageCoverage> {
    let index = Index::build(entities);
    let mut coverage = BTreeMap::<String, LanguageCoverage>::new();

    for module in index.modules() {
        let Some((language, _)) = module_key(&module.stable_key.0) else {
            continue;
        };
        let entry = coverage
            .entry(language.to_string())
            .or_insert_with(|| LanguageCoverage::new(language));
        entry.modules += 1;
        for import in import_payloads(module) {
            entry.imports_declared += 1;
            if !import_targets(&index, module, import).targets.is_empty() {
                entry.imports_resolved += 1;
            }
        }
        entry.module_call_sites += call_sites(module).count();
    }

    for declaration in index.declarations() {
        let Some((language, path, _)) = symbol_key(&declaration.stable_key.0) else {
            continue;
        };
        let entry = coverage
            .entry(language.to_string())
            .or_insert_with(|| LanguageCoverage::new(language));
        entry.declarations += 1;
        let Some(module) = index.module(language, path) else {
            continue;
        };
        for call in call_sites(declaration) {
            let Some(callee) = call.get("callee").and_then(Value::as_str) else {
                continue;
            };
            entry.call_sites += 1;
            if resolve_call(&index, declaration, module, callee).is_some() {
                entry.calls_resolved += 1;
            }
        }
    }

    coverage.into_values().collect()
}

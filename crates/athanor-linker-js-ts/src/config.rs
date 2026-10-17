//! Bounded JavaScript/TypeScript module-resolution configuration.
//!
//! The JS/TS extractor records two kinds of resolution evidence: the `paths`
//! mappings of `tsconfig.json`/`jsconfig.json` and the workspace membership and
//! entry point declared by `package.json`. This module turns that evidence into
//! one snapshot-scoped configuration the import and call linkers share, so a
//! non-relative specifier such as `@/lib/api` or `@scope/ui/button` resolves to
//! an existing module entity instead of staying unresolved.
//!
//! The configuration is deliberately fail-closed. A specifier only resolves
//! through a mapping declared in a configuration file that applies to the
//! importing module, or through a package that is really a member of a declared
//! workspace. Package managers, `node_modules` contents, `exports` maps,
//! conditional exports, and `pnpm-workspace.yaml` are never read, so an
//! unprovable specifier stays unresolved rather than becoming a guessed fact.

use std::collections::{BTreeMap, HashMap};

use athanor_domain::{Entity, EntityKind};
use serde_json::Value;

use crate::paths::{
    import_candidates, normalize_path, normalize_relative, resolve_local_import,
};

/// Upper bound on `extends` hops followed for one configuration file.
const MAX_EXTENDS_DEPTH: usize = 8;

/// Upper bound on workspace glob patterns considered for one declaration owner.
const MAX_WORKSPACE_GLOBS: usize = 64;

/// Upper bound on `exports` rows considered for one package.
const MAX_EXPORT_ROWS: usize = 64;

/// Upper bound on condition keys honoured for one `exports` row.
const MAX_EXPORT_CONDITIONS: usize = 4;

/// Export conditions Athanor can evaluate without running the package manager.
///
/// A row guarded by any other condition — `production`, `deno`, `worker`, or a
/// custom key — is skipped rather than guessed, because the snapshot cannot
/// prove which branch a real resolution would take.
const SUPPORTED_EXPORT_CONDITIONS: [&str; 5] = ["default", "import", "require", "node", "browser"];

/// Upper bound on `rootDirs` entries considered for one configuration file.
const MAX_ROOT_DIRS: usize = 8;

/// Configuration file names that apply to a directory without being named by
/// `extends`, matching TypeScript's own nearest-ancestor discovery.
const DISCOVERED_CONFIG_FILES: [&str; 2] = ["tsconfig.json", "jsconfig.json"];

/// How one module specifier was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecifierOrigin {
    /// A `./` or `../` specifier resolved against the importing module.
    Relative,
    /// A TypeScript `paths` alias resolved through a configuration file.
    PathAlias,
    /// A bare workspace-package specifier resolved through `package.json`.
    WorkspacePackage,
    /// A relative specifier resolved through another `rootDirs` root.
    RootDirectory,
}

impl SpecifierOrigin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::PathAlias => "path_alias",
            Self::WorkspacePackage => "workspace_package",
            Self::RootDirectory => "root_directory",
        }
    }
}

/// Snapshot-scoped module-resolution configuration.
#[derive(Debug, Default)]
pub(crate) struct ModuleResolutionConfig {
    configs: Vec<TypeScriptConfig>,
    packages: Vec<WorkspacePackage>,
    layouts: Vec<WorkspaceLayout>,
}

#[derive(Debug)]
struct TypeScriptConfig {
    path: String,
    base_dir: Option<String>,
    mappings: Vec<PathMapping>,
    extends: Option<String>,
    root_dirs: Vec<String>,
}

/// One declared row of a package `exports` map.
#[derive(Debug)]
struct ExportRow {
    subpath: String,
    conditions: Vec<String>,
    target: String,
}

/// Workspace membership declared by a package-manager manifest.
#[derive(Debug)]
struct WorkspaceLayout {
    dir: String,
    patterns: Vec<String>,
}

#[derive(Debug)]
struct PathMapping {
    pattern: String,
    targets: Vec<String>,
}

#[derive(Debug)]
struct WorkspacePackage {
    dir: String,
    name: String,
    main: Option<String>,
    workspaces: Vec<String>,
    exports: Vec<ExportRow>,
    workspace_member: bool,
}

/// Builds the configuration from the entities of one snapshot.
///
/// Entities are normalized in a deterministic order, so the same snapshot always
/// produces the same resolution result.
pub(crate) fn build_module_resolution_config(entities: &[Entity]) -> ModuleResolutionConfig {
    let mut configs = Vec::new();
    let mut packages = Vec::new();
    let mut layouts = Vec::new();
    for entity in entities {
        if is_typescript_config(entity)
            && let Some(config) = parse_typescript_config(entity)
        {
            configs.push(config);
            continue;
        }
        if is_package_entity(entity)
            && let Some(package) = parse_workspace_package(entity)
        {
            packages.push(package);
            continue;
        }
        if is_workspace_layout(entity)
            && let Some(layout) = parse_workspace_layout(entity)
        {
            layouts.push(layout);
        }
    }
    configs.sort_by(|left, right| left.path.cmp(&right.path));
    packages.sort_by(|left, right| left.dir.cmp(&right.dir));
    layouts.sort_by(|left, right| left.dir.cmp(&right.dir));
    let mut config = ModuleResolutionConfig {
        configs,
        packages,
        layouts,
    };
    config.mark_workspace_members();
    config
}

impl ModuleResolutionConfig {
    /// Marks the packages that a declared workspace glob really owns.
    ///
    /// A `package.json` only makes its directory importable when a workspace
    /// declaration covers it; a fixture or vendored `package.json` with the same
    /// name must not silently become a resolution target. A name declared by two
    /// members is ambiguous, so neither member resolves.
    fn mark_workspace_members(&mut self) {
        let declarations = self
            .packages
            .iter()
            .map(|package| (package.dir.clone(), package.workspaces.clone()))
            .chain(
                self.layouts
                    .iter()
                    .map(|layout| (layout.dir.clone(), layout.patterns.clone())),
            )
            .collect::<Vec<_>>();
        for package in &mut self.packages {
            let named = !package.name.is_empty();
            package.workspace_member = named && is_workspace_member(&declarations, &package.dir);
        }
        let mut counts = BTreeMap::<String, usize>::new();
        for package in &self.packages {
            if package.workspace_member {
                *counts.entry(package.name.clone()).or_insert(0) += 1;
            }
        }
        for package in &mut self.packages {
            let ambiguous = counts.get(&package.name).is_some_and(|count| *count > 1);
            if package.workspace_member && ambiguous {
                package.workspace_member = false;
            }
        }
    }

    /// The nearest configuration file that applies to one module path.
    fn nearest_config(&self, source_path: &str) -> Option<&TypeScriptConfig> {
        let mut directory = parent_directory(source_path);
        loop {
            for file_name in DISCOVERED_CONFIG_FILES {
                let candidate = if directory.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{directory}/{file_name}")
                };
                if let Some(config) = self.configs.iter().find(|config| config.path == candidate) {
                    return Some(config);
                }
            }
            if directory.is_empty() {
                return None;
            }
            directory = parent_directory(&directory);
        }
    }

    /// The configuration whose `paths` apply to one module.
    ///
    /// TypeScript does not merge path mappings across an `extends` chain: the
    /// nearest configuration that declares mappings wins, and a configuration
    /// without mappings inherits the first ancestor that declares them.
    fn effective_config(&self, source_path: &str) -> Option<&TypeScriptConfig> {
        self.effective_config_where(source_path, |config| !config.mappings.is_empty())
    }

    /// The nearest configuration that declares more than one `rootDirs` root,
    /// because a single root merges nothing.
    fn effective_root_config(&self, source_path: &str) -> Option<&TypeScriptConfig> {
        self.effective_config_where(source_path, |config| config.root_dirs.len() > 1)
    }

    /// The nearest configuration that satisfies one predicate, following the
    /// relative `extends` chain of every configuration that does not.
    fn effective_config_where(
        &self,
        source_path: &str,
        satisfied: impl Fn(&TypeScriptConfig) -> bool,
    ) -> Option<&TypeScriptConfig> {
        let mut current = self.nearest_config(source_path);
        let mut depth = 0usize;
        while let Some(config) = current {
            if satisfied(config) {
                return Some(config);
            }
            let extends = config.extends.as_deref()?;
            if !extends.starts_with('.') || depth >= MAX_EXTENDS_DEPTH {
                return None;
            }
            depth += 1;
            let inherited = join_relative(&parent_directory(&config.path), extends)?;
            current = self.configs.iter().find(|config| config.path == inherited);
        }
        None
    }
}

/// Resolves one module specifier to an existing JS/TS module entity.
///
/// Relative specifiers keep the existing directory-based resolution. Every other
/// specifier is tried against the configuration that applies to the importing
/// module and then against the declared workspace packages.
pub(crate) fn resolve_module_specifier<'a>(
    source: &Entity,
    specifier: &str,
    modules: &HashMap<String, &'a Entity>,
    config: &ModuleResolutionConfig,
) -> Option<(&'a Entity, SpecifierOrigin)> {
    if specifier.starts_with('.') {
        if let Some(target) = resolve_local_import(source, specifier, modules) {
            return Some((target, SpecifierOrigin::Relative));
        }
        return resolve_through_root_dirs(source, specifier, modules, config)
            .map(|target| (target, SpecifierOrigin::RootDirectory));
    }
    if let Some(target) = resolve_path_alias(source, specifier, modules, config) {
        return Some((target, SpecifierOrigin::PathAlias));
    }
    resolve_workspace_package(specifier, modules, config)
        .map(|target| (target, SpecifierOrigin::WorkspacePackage))
}

fn resolve_path_alias<'a>(
    source: &Entity,
    specifier: &str,
    modules: &HashMap<String, &'a Entity>,
    config: &ModuleResolutionConfig,
) -> Option<&'a Entity> {
    let source_path = normalize_path(&source.source.as_ref()?.path);
    let config = config.effective_config(&source_path)?;
    let base_dir = config.base_dir.as_deref()?;
    for mapping in &config.mappings {
        let Some(captured) = match_alias(&mapping.pattern, specifier) else {
            continue;
        };
        for target in &mapping.targets {
            let substituted = substitute_star(target, captured);
            let Some(joined) = join_relative(base_dir, &substituted) else {
                continue;
            };
            if let Some(found) = lookup_candidate(&joined, modules) {
                return Some(found);
            }
        }
    }
    None
}

fn resolve_workspace_package<'a>(
    specifier: &str,
    modules: &HashMap<String, &'a Entity>,
    config: &ModuleResolutionConfig,
) -> Option<&'a Entity> {
    let members = config
        .packages
        .iter()
        .filter(|package| package.workspace_member)
        .collect::<Vec<_>>();
    for package in members {
        if specifier == package.name {
            if !package.exports.is_empty() {
                if let Some(found) = resolve_exports_subpath(package, ".", modules) {
                    return Some(found);
                }
                continue;
            }
            for candidate in package_entry_candidates(package) {
                if let Some(found) = lookup_candidate(&candidate, modules) {
                    return Some(found);
                }
            }
            continue;
        }
        let Some(subpath) = strip_package_prefix(specifier, &package.name) else {
            continue;
        };
        if !package.exports.is_empty() {
            if let Some(found) =
                resolve_exports_subpath(package, &format!("./{subpath}"), modules)
            {
                return Some(found);
            }
            continue;
        }
        let Some(joined) = join_relative(&package.dir, subpath) else {
            continue;
        };
        if let Some(found) = lookup_candidate(&joined, modules) {
            return Some(found);
        }
    }
    None
}

/// Resolves one subpath through the `exports` map a package declares.
///
/// Node's encapsulation rule is kept: once `exports` is declared, a subpath it
/// does not declare is unreachable even when the file exists. Among matching
/// rows the longest static prefix wins; rows of one subpath are then tried in
/// declaration order, which is the package's own fallback order, and a row
/// guarded by a condition Athanor cannot evaluate is skipped rather than
/// guessed. A target that leaves the package directory stays unresolved.
fn resolve_exports_subpath<'a>(
    package: &WorkspacePackage,
    subpath: &str,
    modules: &HashMap<String, &'a Entity>,
) -> Option<&'a Entity> {
    let mut rows: Vec<&ExportRow> = Vec::new();
    let mut best_prefix = 0usize;
    for row in package.exports.iter().take(MAX_EXPORT_ROWS) {
        if match_alias(&row.subpath, subpath).is_none() {
            continue;
        }
        if row.conditions.len() > MAX_EXPORT_CONDITIONS
            || row
                .conditions
                .iter()
                .any(|condition| !SUPPORTED_EXPORT_CONDITIONS.contains(&condition.as_str()))
        {
            continue;
        }
        let prefix = row
            .subpath
            .split_once('*')
            .map_or(row.subpath.len(), |(prefix, _)| prefix.len());
        if rows.is_empty() || prefix > best_prefix {
            rows.clear();
            best_prefix = prefix;
        }
        if prefix == best_prefix {
            rows.push(row);
        }
    }

    for row in rows {
        let Some(captured) = match_alias(&row.subpath, subpath) else {
            continue;
        };
        let target = substitute_star(&row.target, captured);
        if !target.starts_with("./") {
            continue;
        }
        let Some(joined) = join_relative(&package.dir, &target) else {
            continue;
        };
        if let Some(found) = lookup_candidate(&joined, modules) {
            return Some(found);
        }
    }
    None
}

/// Resolves a relative specifier through the merged roots of `rootDirs`.
///
/// TypeScript presents the recorded roots as one virtual directory, so an
/// import that misses in the importing file's own root may resolve to the same
/// relative path in another root. Two roots that both hold the path are
/// ambiguous, and ambiguity stays unresolved.
fn resolve_through_root_dirs<'a>(
    source: &Entity,
    specifier: &str,
    modules: &HashMap<String, &'a Entity>,
    config: &ModuleResolutionConfig,
) -> Option<&'a Entity> {
    let source_path = normalize_path(&source.source.as_ref()?.path);
    let config = config.effective_root_config(&source_path)?;
    let root = config
        .root_dirs
        .iter()
        .take(MAX_ROOT_DIRS)
        .find(|root| is_under_root(&source_path, root))?;
    let remainder = source_path
        .strip_prefix(root.as_str())?
        .trim_start_matches('/');
    let directory = remainder.rsplit_once('/').map_or("", |(parent, _)| parent);
    let base = normalize_relative(directory, specifier)?;

    let mut found: Option<&'a Entity> = None;
    for candidate_root in config.root_dirs.iter().take(MAX_ROOT_DIRS) {
        let Some(joined) = join_relative(candidate_root, &base) else {
            continue;
        };
        let Some(target) = lookup_candidate(&joined, modules) else {
            continue;
        };
        if found.is_some_and(|existing| existing.id != target.id) {
            return None;
        }
        found = Some(target);
    }
    found
}

/// Whether one repository-relative path lies directly inside a root directory.
fn is_under_root(path: &str, root: &str) -> bool {
    !root.is_empty()
        && path
            .strip_prefix(root.as_str())
            .is_some_and(|remainder| remainder.starts_with('/'))
}

fn is_workspace_member(declarations: &[(String, Vec<String>)], package_dir: &str) -> bool {
    for (dir, patterns) in declarations {
        if dir.as_str() == package_dir {
            continue;
        }
        let Some(relative) = relative_dir(dir, package_dir) else {
            continue;
        };
        for pattern in patterns.iter().take(MAX_WORKSPACE_GLOBS) {
            if glob_matches(pattern, &relative) {
                return true;
            }
        }
    }
    false
}

fn package_entry_candidates(package: &WorkspacePackage) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(main) = package.main.as_deref()
        && let Some(joined) = join_relative(&package.dir, main)
    {
        candidates.extend(import_candidates(&joined));
    }
    let index = if package.dir.is_empty() {
        "index".to_string()
    } else {
        format!("{}/index", package.dir)
    };
    candidates.extend(import_candidates(&index));
    candidates
}

fn lookup_candidate<'a>(base: &str, modules: &HashMap<String, &'a Entity>) -> Option<&'a Entity> {
    import_candidates(base)
        .into_iter()
        .find_map(|candidate| modules.get(&candidate).copied())
}

/// Captures the text matched by the single `*` of one alias pattern.
fn match_alias<'a>(pattern: &str, specifier: &'a str) -> Option<&'a str> {
    match pattern.split_once('*') {
        Some((prefix, suffix)) => {
            let remainder = specifier.strip_prefix(prefix)?;
            remainder.strip_suffix(suffix)
        }
        None => (pattern == specifier).then_some(""),
    }
}

/// Replaces the single `*` of one substitution target with the captured text.
fn substitute_star(target: &str, captured: &str) -> String {
    match target.split_once('*') {
        Some((prefix, suffix)) => format!("{prefix}{captured}{suffix}"),
        None => target.to_string(),
    }
}

/// Joins a repository-relative directory with a configuration-relative path.
///
/// A path that walks above the repository root has no canonical target, so the
/// join fails closed instead of producing `../` segments.
fn join_relative(base: &str, relative: &str) -> Option<String> {
    let mut parts = base
        .split('/')
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for part in relative.replace('\\', "/").split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other.to_string()),
        }
    }
    Some(parts.join("/"))
}

fn parent_directory(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(String::new(), |(parent, _)| parent.to_string())
}

fn relative_dir(base: &str, candidate: &str) -> Option<String> {
    if base.is_empty() {
        if candidate.is_empty() {
            return None;
        }
        return Some(candidate.to_string());
    }
    let remainder = candidate.strip_prefix(&format!("{base}/"))?;
    if remainder.is_empty() {
        return None;
    }
    Some(remainder.to_string())
}

/// Matches one workspace glob against a repository-relative directory.
///
/// `*` matches exactly one path segment and `**` matches any number of them,
/// including none. A segment with more than one `*` is not supported and never
/// matches, so an exotic pattern stays unresolved instead of over-matching.
fn glob_matches(pattern: &str, candidate: &str) -> bool {
    let pattern_parts = pattern
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let candidate_parts = candidate
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match_segments(&pattern_parts, &candidate_parts)
}

fn match_segments(pattern: &[&str], candidate: &[&str]) -> bool {
    let Some((segment, rest)) = pattern.split_first() else {
        return candidate.is_empty();
    };
    if *segment == "**" {
        return (0..=candidate.len()).any(|index| match_segments(rest, &candidate[index..]));
    }
    let Some((head, tail)) = candidate.split_first() else {
        return false;
    };
    segment_matches(segment, head) && match_segments(rest, tail)
}

fn segment_matches(pattern: &str, candidate: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == candidate;
    };
    if suffix.contains('*') {
        return false;
    }
    candidate.len() >= prefix.len() + suffix.len()
        && candidate.starts_with(prefix)
        && candidate.ends_with(suffix)
}

fn strip_package_prefix<'a>(specifier: &'a str, package_name: &str) -> Option<&'a str> {
    if package_name.is_empty() {
        return None;
    }
    let remainder = specifier.strip_prefix(package_name)?;
    remainder.strip_prefix('/')
}

fn is_typescript_config(entity: &Entity) -> bool {
    entity.kind == EntityKind::Feature
        && entity.payload.get("config_kind").and_then(Value::as_str)
            == Some("js_ts_module_resolution")
}

fn parse_typescript_config(entity: &Entity) -> Option<TypeScriptConfig> {
    let path = normalize_path(&entity.source.as_ref()?.path);
    let mappings = entity
        .payload
        .get("paths")
        .and_then(Value::as_array)
        .map(|mappings| read_path_mappings(mappings))
        .unwrap_or_default();
    Some(TypeScriptConfig {
        path,
        base_dir: entity
            .payload
            .get("base_dir")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        mappings,
        extends: entity
            .payload
            .get("extends")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        root_dirs: entity
            .payload
            .get("root_dirs")
            .and_then(Value::as_array)
            .map(|roots| read_patterns(roots))
            .unwrap_or_default(),
    })
}

/// Workspace membership declared by a package-manager manifest is read exactly
/// like the `workspaces` field of a `package.json`: a manifest without a
/// readable package list declares nothing.
fn is_workspace_layout(entity: &Entity) -> bool {
    entity.kind == EntityKind::Feature
        && entity.payload.get("config_kind").and_then(Value::as_str)
            == Some("js_ts_workspace_layout")
}

fn parse_workspace_layout(entity: &Entity) -> Option<WorkspaceLayout> {
    let path = normalize_path(&entity.source.as_ref()?.path);
    Some(WorkspaceLayout {
        dir: parent_directory(&path),
        patterns: entity
            .payload
            .get("packages")
            .and_then(Value::as_array)
            .map(|patterns| read_patterns(patterns))
            .unwrap_or_default(),
    })
}

fn read_path_mappings(mappings: &[Value]) -> Vec<PathMapping> {
    let mut recorded = Vec::new();
    for mapping in mappings {
        let Some(pattern) = mapping.get("pattern").and_then(Value::as_str) else {
            continue;
        };
        let Some(targets) = mapping.get("targets").and_then(Value::as_array) else {
            continue;
        };
        let targets = read_patterns(targets);
        if pattern.trim().is_empty() || targets.is_empty() {
            continue;
        }
        recorded.push(PathMapping {
            pattern: pattern.trim().to_string(),
            targets,
        });
    }
    recorded
}

fn is_package_entity(entity: &Entity) -> bool {
    entity.kind == EntityKind::Package && entity.stable_key.0.starts_with("package://npm:")
}

fn parse_workspace_package(entity: &Entity) -> Option<WorkspacePackage> {
    let path = normalize_path(&entity.source.as_ref()?.path);
    let name = entity
        .stable_key
        .0
        .strip_prefix("package://npm:")?
        .to_string();
    if name.is_empty() || name == "(anonymous)" {
        return None;
    }
    Some(WorkspacePackage {
        dir: parent_directory(&path),
        name,
        main: entity
            .payload
            .get("main")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        exports: entity
            .payload
            .get("exports")
            .and_then(Value::as_array)
            .map(|rows| read_export_rows(rows))
            .unwrap_or_default(),
        workspaces: entity
            .payload
            .get("workspaces")
            .and_then(Value::as_array)
            .map(|patterns| read_patterns(patterns))
            .unwrap_or_default(),
        workspace_member: false,
    })
}

/// Reads the ordered `exports` rows one package declares.
fn read_export_rows(rows: &[Value]) -> Vec<ExportRow> {
    let mut recorded = Vec::new();
    for row in rows {
        let Some(subpath) = row.get("subpath").and_then(Value::as_str) else {
            continue;
        };
        let Some(target) = row.get("target").and_then(Value::as_str) else {
            continue;
        };
        let conditions = row
            .get("conditions")
            .and_then(Value::as_array)
            .map(|conditions| read_patterns(conditions))
            .unwrap_or_default();
        if subpath.is_empty() || target.is_empty() || recorded.len() == MAX_EXPORT_ROWS {
            continue;
        }
        recorded.push(ExportRow {
            subpath: subpath.to_string(),
            conditions,
            target: target.to_string(),
        });
    }
    recorded
}

fn read_patterns(patterns: &[Value]) -> Vec<String> {
    patterns
        .iter()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use athanor_domain::{EntityId, LanguageCode, SourceLocation, StableKey};
    use serde_json::json;

    use super::*;

    #[test]
    fn alias_resolution_prefers_the_nearest_configuration() {
        let root = tsconfig(
            "tsconfig.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": "src",
                "paths": [{"pattern": "@/*", "targets": ["app/*"]}],
            }),
        );
        let nested = tsconfig(
            "packages/app/tsconfig.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": "packages/app/src",
                "paths": [{"pattern": "@/*", "targets": ["lib/*"]}],
            }),
        );
        let root_target = module("src/app/api.ts");
        let nested_target = module("packages/app/src/lib/api.ts");
        let mut modules = HashMap::new();
        modules.insert("src/app/api.ts".to_string(), &root_target);
        modules.insert("packages/app/src/lib/api.ts".to_string(), &nested_target);
        let config = build_module_resolution_config(&[root, nested]);

        let source = module("packages/app/src/routes/home.ts");
        let (name, origin) = resolve_module_specifier(&source, "@/api", &modules, &config)
            .map(|(entity, origin)| (entity.name.clone(), origin))
            .expect("nested alias must resolve");
        assert_eq!(name, "packages/app/src/lib/api.ts");
        assert_eq!(origin, SpecifierOrigin::PathAlias);

        let source = module("src/routes/home.ts");
        let (name, origin) = resolve_module_specifier(&source, "@/api", &modules, &config)
            .map(|(entity, origin)| (entity.name.clone(), origin))
            .expect("root alias must resolve");
        assert_eq!(name, "src/app/api.ts");
        assert_eq!(origin, SpecifierOrigin::PathAlias);
    }

    #[test]
    fn inherited_mappings_resolve_through_a_relative_extends_chain() {
        let base = tsconfig(
            "tsconfig.base.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": "src",
                "paths": [{"pattern": "@/*", "targets": ["core/*"]}],
            }),
        );
        let child = tsconfig(
            "tsconfig.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": "",
                "extends": "./tsconfig.base.json",
                "paths": [],
            }),
        );
        let target = module("src/core/api.ts");
        let mut modules = HashMap::new();
        modules.insert("src/core/api.ts".to_string(), &target);
        let config = build_module_resolution_config(&[base, child]);

        let source = module("src/routes/home.ts");
        let (name, origin) = resolve_module_specifier(&source, "@/api", &modules, &config)
            .map(|(entity, origin)| (entity.name.clone(), origin))
            .expect("inherited alias must resolve");
        assert_eq!(name, "src/core/api.ts");
        assert_eq!(origin, SpecifierOrigin::PathAlias);
    }

    #[test]
    fn alias_resolution_fails_closed_without_a_base_directory() {
        let unsupported = tsconfig(
            "tsconfig.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": null,
                "paths": [{"pattern": "@/*", "targets": ["src/*"]}],
            }),
        );
        let target = module("src/api.ts");
        let mut modules = HashMap::new();
        modules.insert("src/api.ts".to_string(), &target);
        let config = build_module_resolution_config(&[unsupported]);

        let source = module("src/routes/home.ts");
        assert!(resolve_module_specifier(&source, "@/api", &modules, &config).is_none());
    }

    #[test]
    fn resolves_workspace_packages_and_their_subpaths() {
        let root = package(
            "package.json",
            "root",
            &json!({"workspaces": ["packages/*"]}),
        );
        let member = package(
            "packages/ui/package.json",
            "@scope/ui",
            &json!({"main": "src/index.ts"}),
        );
        let orphan = package("examples/demo/package.json", "@scope/demo", &json!({}));
        let entry = module("packages/ui/src/index.ts");
        let button = module("packages/ui/button.ts");
        let orphan_entry = module("examples/demo/index.ts");
        let mut modules = HashMap::new();
        modules.insert("packages/ui/src/index.ts".to_string(), &entry);
        modules.insert("packages/ui/button.ts".to_string(), &button);
        modules.insert("examples/demo/index.ts".to_string(), &orphan_entry);
        let config = build_module_resolution_config(&[root, member, orphan]);

        let source = module("apps/web/src/app.ts");
        let (name, origin) = resolve_module_specifier(&source, "@scope/ui", &modules, &config)
            .map(|(entity, origin)| (entity.name.clone(), origin))
            .expect("workspace package must resolve");
        assert_eq!(name, "packages/ui/src/index.ts");
        assert_eq!(origin, SpecifierOrigin::WorkspacePackage);

        let subpath = resolve_module_specifier(&source, "@scope/ui/button", &modules, &config)
            .map(|(entity, _)| entity.name.clone());
        assert_eq!(subpath, Some("packages/ui/button.ts".to_string()));
        assert!(resolve_module_specifier(&source, "@scope/demo", &modules, &config).is_none());
        assert!(resolve_module_specifier(&source, "react", &modules, &config).is_none());
    }

    #[test]
    fn duplicate_workspace_package_names_stay_unresolved() {
        let root = package(
            "package.json",
            "root",
            &json!({"workspaces": ["packages/*"]}),
        );
        let first = package("packages/a/package.json", "@scope/ui", &json!({}));
        let second = package("packages/b/package.json", "@scope/ui", &json!({}));
        let first_entry = module("packages/a/index.ts");
        let mut modules = HashMap::new();
        modules.insert("packages/a/index.ts".to_string(), &first_entry);
        let config = build_module_resolution_config(&[root, first, second]);

        let source = module("apps/web/src/app.ts");
        assert!(resolve_module_specifier(&source, "@scope/ui", &modules, &config).is_none());
    }

    #[test]
    fn workspace_glob_matching_stays_segment_scoped() {
        assert!(glob_matches("packages/*", "packages/ui"));
        assert!(!glob_matches("packages/*", "packages/ui/nested"));
        assert!(glob_matches("packages/**", "packages/ui/nested"));
        assert!(glob_matches("**", "packages/ui"));
        assert!(!glob_matches("apps/*", "packages/ui"));
        assert!(!glob_matches("*-*", "a-b"));
    }

    #[test]
    fn alias_substitution_keeps_prefixes_and_suffixes() {
        assert_eq!(match_alias("@/*", "@/lib/api"), Some("lib/api"));
        assert_eq!(match_alias("config", "other"), None);
        assert_eq!(match_alias("config", "config"), Some(""));
        assert_eq!(substitute_star("app/*", "lib/api"), "app/lib/api");
        assert_eq!(substitute_star("v1/*.gen", "api"), "v1/api.gen");
    }

    #[test]
    fn resolves_declared_exports_subpaths_in_declaration_order() {
        let root = package(
            "package.json",
            "root",
            &json!({"workspaces": ["packages/*"]}),
        );
        let member = package(
            "packages/ui/package.json",
            "@scope/ui",
            &json!({
                "exports": [
                    {"subpath": ".", "conditions": ["import"], "target": "./dist/index.mjs"},
                    {"subpath": ".", "conditions": ["require"], "target": "./dist/index.cjs"},
                    {"subpath": "./button", "conditions": [], "target": "./src/button.ts"}
                ]
            }),
        );
        let esm = module("packages/ui/dist/index.mjs");
        let cjs = module("packages/ui/dist/index.cjs");
        let button = module("packages/ui/src/button.ts");
        let hidden = module("packages/ui/src/tools.ts");
        let mut modules = HashMap::new();
        modules.insert("packages/ui/dist/index.mjs".to_string(), &esm);
        modules.insert("packages/ui/dist/index.cjs".to_string(), &cjs);
        modules.insert("packages/ui/src/button.ts".to_string(), &button);
        modules.insert("packages/ui/src/tools.ts".to_string(), &hidden);
        let config = build_module_resolution_config(&[root, member]);
        let source = module("apps/web/src/app.ts");

        let (name, origin) = resolve_module_specifier(&source, "@scope/ui", &modules, &config)
            .map(|(entity, origin)| (entity.name.clone(), origin))
            .expect("declared package root");
        assert_eq!(name, "packages/ui/dist/index.mjs");
        assert_eq!(origin, SpecifierOrigin::WorkspacePackage);

        let subpath = resolve_module_specifier(&source, "@scope/ui/button", &modules, &config)
            .map(|(entity, _)| entity.name.clone());
        assert_eq!(subpath, Some("packages/ui/src/button.ts".to_string()));

        assert!(resolve_module_specifier(&source, "@scope/ui/tools", &modules, &config).is_none());
    }

    #[test]
    fn export_rows_guarded_by_unknown_conditions_stay_unresolved() {
        let root = package(
            "package.json",
            "root",
            &json!({"workspaces": ["packages/*"]}),
        );
        let member = package(
            "packages/ui/package.json",
            "@scope/ui",
            &json!({
                "exports": [
                    {"subpath": ".", "conditions": ["production"], "target": "./dist/prod.js"},
                    {"subpath": ".", "conditions": [], "target": "./dist/base.js"}
                ]
            }),
        );
        let production = module("packages/ui/dist/prod.js");
        let base = module("packages/ui/dist/base.js");
        let mut modules = HashMap::new();
        modules.insert("packages/ui/dist/prod.js".to_string(), &production);
        modules.insert("packages/ui/dist/base.js".to_string(), &base);
        let config = build_module_resolution_config(&[root, member]);
        let source = module("apps/web/src/app.ts");

        let resolved = resolve_module_specifier(&source, "@scope/ui", &modules, &config)
            .map(|(entity, _)| entity.name.clone());
        assert_eq!(resolved, Some("packages/ui/dist/base.js".to_string()));
    }

    #[test]
    fn pnpm_workspace_layout_grants_workspace_membership() {
        let layout_entity = layout(
            "pnpm-workspace.yaml",
            &json!({
                "config_kind": "js_ts_workspace_layout",
                "packages": ["packages/*"]
            }),
        );
        let member = package(
            "packages/ui/package.json",
            "@scope/ui",
            &json!({"main": "src/index.ts"}),
        );
        let entry = module("packages/ui/src/index.ts");
        let mut modules = HashMap::new();
        modules.insert("packages/ui/src/index.ts".to_string(), &entry);
        let source = module("apps/web/src/app.ts");

        let config = build_module_resolution_config(&[layout_entity, member.clone()]);
        let resolved = resolve_module_specifier(&source, "@scope/ui", &modules, &config)
            .map(|(entity, _)| entity.name.clone());
        assert_eq!(resolved, Some("packages/ui/src/index.ts".to_string()));

        let without_layout = build_module_resolution_config(&[member]);
        assert!(
            resolve_module_specifier(&source, "@scope/ui", &modules, &without_layout).is_none()
        );
    }

    #[test]
    fn relative_imports_resolve_through_merged_root_directories() {
        let config_entity = tsconfig(
            "tsconfig.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "base_dir": "src",
                "root_dirs": ["src/en", "src/de"]
            }),
        );
        let source = module("src/en/widget/index.ts");
        let translated = module("src/de/widget/shared/api.ts");
        let mut modules = HashMap::new();
        modules.insert("src/de/widget/shared/api.ts".to_string(), &translated);
        let config = build_module_resolution_config(&[config_entity]);

        let (name, origin) = resolve_module_specifier(&source, "./shared/api", &modules, &config)
            .map(|(entity, origin)| (entity.name.clone(), origin))
            .expect("merged root directory");
        assert_eq!(name, "src/de/widget/shared/api.ts");
        assert_eq!(origin, SpecifierOrigin::RootDirectory);
    }

    #[test]
    fn ambiguous_merged_root_directories_stay_unresolved() {
        let config_entity = tsconfig(
            "tsconfig.json",
            &json!({
                "config_kind": "js_ts_module_resolution",
                "root_dirs": ["src/en", "src/de", "src/fr"]
            }),
        );
        let source = module("src/en/widget/index.ts");
        let german = module("src/de/widget/shared/api.ts");
        let french = module("src/fr/widget/shared/api.ts");
        let mut modules = HashMap::new();
        modules.insert("src/de/widget/shared/api.ts".to_string(), &german);
        modules.insert("src/fr/widget/shared/api.ts".to_string(), &french);
        let config = build_module_resolution_config(&[config_entity]);

        assert!(resolve_module_specifier(&source, "./shared/api", &modules, &config).is_none());
    }

    fn tsconfig(path: &str, payload: &serde_json::Value) -> Entity {
        Entity {
            id: EntityId(format!("ent_config_{path}")),
            stable_key: StableKey(format!("config://{path}#js-ts-module-resolution")),
            kind: EntityKind::Feature,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: None,
            }),
            language: Some(LanguageCode("json".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: payload.clone(),
        }
    }

    fn layout(path: &str, payload: &serde_json::Value) -> Entity {
        Entity {
            id: EntityId(format!("ent_layout_{path}")),
            stable_key: StableKey(format!("config://{path}#js-ts-workspace-layout")),
            kind: EntityKind::Feature,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: None,
            }),
            language: Some(LanguageCode("yaml".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: payload.clone(),
        }
    }

    fn package(path: &str, name: &str, payload: &serde_json::Value) -> Entity {
        Entity {
            id: EntityId(format!("ent_package_{path}")),
            stable_key: StableKey(format!("package://npm:{name}")),
            kind: EntityKind::Package,
            name: name.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: None,
            }),
            language: Some(LanguageCode("json".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: payload.clone(),
        }
    }

    fn module(path: &str) -> Entity {
        Entity {
            id: EntityId(format!("ent_module_{path}")),
            stable_key: StableKey(format!("module://js-ts:{path}")),
            kind: EntityKind::Module,
            name: path.to_string(),
            title: None,
            source: Some(SourceLocation {
                path: path.to_string(),
                line_start: Some(1),
                line_end: Some(1),
            }),
            language: Some(LanguageCode("typescript".to_string())),
            aliases: Vec::new(),
            ownership: Vec::new(),
            payload: json!({"imports": [], "exports": []}),
        }
    }
}

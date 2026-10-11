//! Pack data and the generic tree-sitter walk behind the `Extractor` port.
//!
//! A language pack is data: node kinds, field names, and the mapping from matched nodes onto
//! canonical entity kinds. The engine owns traversal, bounded collection, and the emission of
//! entities, facts, and diagnostics, so adding a language is a pack module plus a fixture rather
//! than a new crate.
//!
//! Nothing is guessed. Only statically named callees, syntactically present imports, and
//! declarations whose node shape a pack classifies are recorded; every other construct is left
//! out, and each bound that truncates a collection is disclosed as a diagnostic.

use std::collections::{BTreeSet, HashSet};

use athanor_core::{CoreError, CoreResult, ExtractInput, ExtractOutput};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind, Fact,
    FactId, FactKind, LanguageCode, Severity, SnapshotId, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, ownership_for_file, stable_hash};
use serde_json::{Map, Value, json};
use tree_sitter::{Language, Node, Parser};

/// Upper bound on recorded declarations per file.
pub const MAX_DECLARATIONS_PER_FILE: usize = 512;
/// Upper bound on recorded call sites per declaration.
pub const MAX_CALL_SITES_PER_DECLARATION: usize = 256;
/// Upper bound on recorded call sites per file across all declarations.
pub const MAX_CALL_SITES_PER_FILE: usize = 4_096;
/// Upper bound on recorded import statements per file.
pub const MAX_IMPORTS_PER_FILE: usize = 256;
/// Upper bound on recorded exported names per file.
pub const MAX_EXPORTS_PER_FILE: usize = 256;
/// Upper bound on parse-error diagnostics per file.
pub const MAX_PARSE_ERROR_DIAGNOSTICS: usize = 32;
/// Upper bound on duplicate-declaration diagnostics per file.
pub const MAX_DUPLICATE_DIAGNOSTICS: usize = 8;
/// Upper bound on the traversal depth of one file.
pub const MAX_TRAVERSAL_DEPTH: usize = 256;
/// Upper bound on a recorded receiver type name.
const MAX_TYPE_NAME_LENGTH: usize = 128;

/// Canonical classification of one matched declaration node kind.
#[derive(Clone)]
pub struct DeclarationKindRule {
    /// Node kind this rule classifies.
    pub node_kind: &'static str,
    /// Canonical entity kind recorded for the declaration.
    pub entity_kind: EntityKind,
    /// Fine-grained symbol kind recorded in the entity payload and facts.
    pub symbol_kind: &'static str,
}

/// One declaration shape of a language.
pub struct DeclarationRule {
    /// Node kind that matches the declaration, or its container when `name_child` is set.
    pub node_kind: &'static str,
    /// Wrapper node kinds that are unwrapped before matching (`decorated_definition`).
    pub wrappers: &'static [&'static str],
    /// When set, every named child of this kind is one declaration of the matched container.
    pub name_child: Option<&'static str>,
    /// Field holding the declaration name.
    pub name_field: &'static str,
    /// Field holding the body scanned for nested declarations and call sites.
    pub body_field: Option<&'static str>,
    /// Classification by the matched node kind.
    pub matched_kinds: &'static [DeclarationKindRule],
    /// Field holding the type node that classifies a `name_child` declaration.
    pub type_field: Option<&'static str>,
    /// Classification by the `type_field` node kind; takes precedence over `matched_kinds`.
    pub type_kinds: &'static [DeclarationKindRule],
    /// Classification used when `type_kinds` does not cover the declared type node.
    pub fallback_kind: Option<DeclarationKindRule>,
    /// Ancestor declaration kinds that turn a nested declaration into a method.
    pub method_ancestors: &'static [&'static str],
    /// Classification applied when the nearest ancestor declaration is a `method_ancestors` kind.
    pub method_kind: Option<DeclarationKindRule>,
    /// Field holding a receiver (`method_declaration.receiver`).
    pub receiver_field: Option<&'static str>,
    /// Node kinds inside the receiver that name the receiver type.
    pub receiver_type_kinds: &'static [&'static str],
}

/// Member access inside a callee expression (`attribute`, `selector_expression`).
pub struct MemberRule {
    /// Node kind of the member expression.
    pub node_kind: &'static str,
    /// Field holding the object the member is read from.
    pub object_field: &'static str,
    /// Field holding the member being read.
    pub member_field: &'static str,
}

/// The call shape of a language.
pub struct CallRule {
    /// Node kind of a call expression.
    pub node_kind: &'static str,
    /// Field holding the callee expression.
    pub function_field: &'static str,
    /// Field holding the argument list.
    pub arguments_field: &'static str,
    /// Node kinds that name a value and terminate a callee path.
    pub atom_kinds: &'static [&'static str],
    /// Member access rules used to build dotted callee paths.
    pub members: &'static [MemberRule],
}

/// The import shapes of a language.
pub enum ImportRule {
    /// `import a.b`, `import a.b as c`: matching child nodes carry one module name each.
    Dotted {
        /// Node kind of the import statement.
        node_kind: &'static str,
        /// Child node kinds that carry a dotted module name.
        module_kinds: &'static [&'static str],
    },
    /// `from a.b import c, d as e`.
    From {
        /// Node kind of the import statement.
        node_kind: &'static str,
        /// Field holding the module being imported from.
        module_field: &'static str,
        /// Child node kinds that carry an imported name.
        imported_kinds: &'static [&'static str],
    },
    /// `import f "fmt"` inside a spec container; the module is a quoted path.
    Spec {
        /// Node kind of the import statement.
        node_kind: &'static str,
        /// Node kind of one import specification.
        spec_kind: &'static str,
        /// Field holding the quoted import path.
        path_field: &'static str,
        /// Field holding the local alias.
        alias_field: &'static str,
    },
}

/// How a language marks exported declarations.
pub enum ExportRule {
    /// Every declaration is reported with `exported: false`.
    None,
    /// A declaration is exported when its name starts with an upper-case letter.
    Capitalized,
    /// A named list assigned in the same file lists the exported names.
    NameList {
        /// Node kind of the assignment statement.
        node_kind: &'static str,
        /// Left-hand name that holds the list.
        name: &'static str,
        /// Field holding the left-hand side.
        name_field: &'static str,
        /// Field holding the right-hand side.
        value_field: &'static str,
        /// Node kinds inside the right-hand side that carry one exported name.
        value_kinds: &'static [&'static str],
    },
}

/// One grammar plus the data that maps its nodes onto canonical evidence.
pub struct LanguagePack {
    /// Stable language id used in stable keys and entity ids.
    pub id: &'static str,
    /// Language code recorded on entities.
    pub hint: &'static str,
    /// Grammar crate recorded in module payloads.
    pub parser: &'static str,
    /// Lower-case file extensions owned by this pack.
    pub extensions: &'static [&'static str],
    /// Grammar entry point.
    pub language: fn() -> Language,
    /// Declaration rules, applied in order.
    pub declarations: &'static [DeclarationRule],
    /// Call shape.
    pub call: CallRule,
    /// Import shapes.
    pub imports: &'static [ImportRule],
    /// Export marking.
    pub exports: ExportRule,
    /// Anonymous first-token kind that marks an asynchronous declaration.
    pub async_prefix: Option<&'static str>,
}

impl LanguagePack {
    /// Reports whether this pack owns the file path.
    pub fn supports(&self, path: &str) -> bool {
        path.rsplit_once('.').is_some_and(|(_, extension)| {
            self.extensions
                .contains(&extension.to_ascii_lowercase().as_str())
        })
    }
}

#[derive(Clone, Default)]
struct Scope {
    qualified_prefix: String,
    enclosing_kind: Option<&'static str>,
}

struct FileExtraction<'a> {
    pack: &'a LanguagePack,
    extractor: &'a str,
    path: &'a str,
    snapshot: &'a SnapshotId,
    bytes: &'a [u8],
    module_id: EntityId,
    exports: Vec<String>,
    entities: Vec<Entity>,
    facts: Vec<Fact>,
    diagnostics: Vec<Diagnostic>,
    truncated: BTreeSet<&'static str>,
    call_sites: usize,
    declaration_keys: HashSet<String>,
    duplicates: usize,
}

/// Extracts one file with the pack that owns its extension.
pub(crate) fn extract_pack(
    pack: &LanguagePack,
    extractor: &str,
    input: &ExtractInput,
    content: &str,
) -> CoreResult<ExtractOutput> {
    let mut parser = Parser::new();
    parser
        .set_language(&(pack.language)())
        .map_err(|error| CoreError::Adapter(error.to_string()))?;
    let Some(tree) = parser.parse(content, None) else {
        return Ok(ExtractOutput::default());
    };

    let mut extraction = FileExtraction {
        pack,
        extractor,
        path: input.source.path.as_str(),
        snapshot: &input.snapshot,
        bytes: content.as_bytes(),
        module_id: EntityId(String::new()),
        exports: Vec::new(),
        entities: Vec::new(),
        facts: Vec::new(),
        diagnostics: Vec::new(),
        truncated: BTreeSet::new(),
        call_sites: 0,
        declaration_keys: HashSet::new(),
        duplicates: 0,
    };

    Ok(extraction.run(tree.root_node(), input))
}

impl FileExtraction<'_> {
    fn run(&mut self, root: Node, input: &ExtractInput) -> ExtractOutput {
        if root.has_error() {
            self.collect_parse_errors(root);
        }
        self.exports = self.collect_exports(root);
        let imports = self.collect_imports(root);
        let module_calls = self.collect_module_calls(root);
        self.emit_module(root, input, imports, module_calls);

        let scope = Scope::default();
        self.walk(root, &scope, 0);
        let declaration_count = self.entities.len().saturating_sub(1);
        if let Some(module) = self
            .entities
            .iter_mut()
            .find(|entity| entity.id == self.module_id)
        {
            module.payload["declaration_count"] = json!(declaration_count);
        }
        self.emit_truncation_diagnostics(root);

        self.entities
            .sort_by(|left, right| left.stable_key.0.cmp(&right.stable_key.0));
        self.facts.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        self.diagnostics
            .sort_by(|left, right| left.id.0.cmp(&right.id.0));

        ExtractOutput {
            entities: std::mem::take(&mut self.entities),
            facts: std::mem::take(&mut self.facts),
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    fn emit_module(
        &mut self,
        root: Node,
        input: &ExtractInput,
        imports: Vec<Value>,
        calls: Vec<Value>,
    ) {
        let stable_key = StableKey(format!("module://{}:{}", self.pack.id, self.path));
        let id = EntityId(format!(
            "ent_{}_module_{:016x}",
            self.pack.id,
            stable_hash(stable_key.0.as_bytes())
        ));
        let range = (1, root.end_position().row as u32 + 1);
        self.module_id = id.clone();
        self.entities.push(Entity {
            id: id.clone(),
            stable_key: stable_key.clone(),
            kind: EntityKind::Module,
            name: self
                .path
                .rsplit('/')
                .next()
                .unwrap_or(self.path)
                .to_string(),
            title: None,
            source: Some(SourceLocation {
                path: self.path.to_string(),
                line_start: Some(range.0),
                line_end: Some(range.1),
            }),
            language: Some(LanguageCode(self.pack.hint.to_string())),
            aliases: Vec::new(),
            ownership: ownership_for_file(self.path),
            payload: json!({
                "language_hint": input.source.language_hint,
                "parser": self.pack.parser,
                "imports": imports,
                "exports": self.exports,
                "calls": calls,
                "declaration_count": 0,
            }),
        });
        self.facts
            .push(self.symbol_fact(&id, None, &stable_key, "module", range));
    }

    fn walk(&mut self, node: Node, scope: &Scope, depth: usize) {
        if depth > MAX_TRAVERSAL_DEPTH {
            self.truncated.insert("declaration_depth");
            return;
        }
        let effective = unwrap_wrappers(self.pack, node);
        let matched = self.match_declarations(effective);
        if !matched.is_empty() {
            for (rule, declaration) in matched {
                self.emit_declaration(rule, declaration, scope, depth);
            }
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.walk(child, scope, depth + 1);
        }
    }

    /// Returns the rule and node of every declaration carried by a matched node.
    fn match_declarations(&self, node: Node) -> Vec<(&'static DeclarationRule, Node)> {
        let kind = node.kind();
        let Some(rule) = self
            .pack
            .declarations
            .iter()
            .find(|rule| rule.node_kind == kind)
        else {
            return Vec::new();
        };
        match rule.name_child {
            None => vec![(rule, node)],
            Some(child_kind) => {
                let mut matched = Vec::new();
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == child_kind {
                        matched.push((rule, child));
                    }
                }
                matched
            }
        }
    }

    fn emit_declaration(
        &mut self,
        rule: &'static DeclarationRule,
        node: Node,
        scope: &Scope,
        depth: usize,
    ) {
        if self.entities.len().saturating_sub(1) >= MAX_DECLARATIONS_PER_FILE {
            self.truncated.insert("declarations");
            return;
        }
        let Some(name) = node
            .child_by_field_name(rule.name_field)
            .and_then(|name| self.text(name))
        else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let is_method = rule.method_kind.is_some()
            && scope
                .enclosing_kind
                .is_some_and(|kind| rule.method_ancestors.contains(&kind));
        let Some((entity_kind, symbol_kind)) = classify(rule, node, is_method) else {
            return;
        };
        let qualified_name = if scope.qualified_prefix.is_empty() {
            name
        } else {
            format!("{}.{}", scope.qualified_prefix, name)
        };
        let stable_key = StableKey(format!(
            "symbol://{}:{}#{}",
            self.pack.id, self.path, qualified_name
        ));
        if !self.declaration_keys.insert(stable_key.0.clone()) {
            self.duplicates += 1;
            if self.duplicates <= MAX_DUPLICATE_DIAGNOSTICS {
                let range = line_range(node);
                let diagnostic = self.diagnostic(
                    &format!("{}_duplicate_declaration", self.pack.id),
                    &stable_key.0,
                    "Declaration recorded once",
                    format!(
                        "`{qualified_name}` is already defined in this file; the later definition \
                         is not recorded because both share the stable key `{}`",
                        stable_key.0
                    ),
                    range,
                    json!({
                        "language": self.pack.id,
                        "stable_key": stable_key.0,
                        "symbol_kind": symbol_kind,
                    }),
                );
                self.diagnostics.push(diagnostic);
            }
            return;
        }

        let prefix = match entity_kind {
            EntityKind::Function => "function",
            EntityKind::Class => "class",
            _ => "symbol",
        };
        let id = EntityId(format!(
            "ent_{}_{}_{:016x}",
            self.pack.id,
            prefix,
            stable_hash(stable_key.0.as_bytes())
        ));
        let range = line_range(node);
        let calls = self.collect_declaration_calls(rule, node);
        let mut payload = json!({
            "symbol_kind": symbol_kind,
            "qualified_name": qualified_name,
            "exported": self.is_exported(&qualified_name),
            "async": self
                .pack
                .async_prefix
                .is_some_and(|prefix| node.child(0).is_some_and(|child| child.kind() == prefix)),
            "source_node": node.kind(),
            "calls": calls,
        });
        if let Some(receiver) = receiver_type(rule, node, self.bytes) {
            payload["receiver"] = json!(receiver);
        }

        self.entities.push(Entity {
            id: id.clone(),
            stable_key: stable_key.clone(),
            kind: entity_kind,
            name: qualified_name.clone(),
            title: None,
            source: Some(SourceLocation {
                path: self.path.to_string(),
                line_start: Some(range.0),
                line_end: Some(range.1),
            }),
            language: Some(LanguageCode(self.pack.hint.to_string())),
            aliases: Vec::new(),
            ownership: ownership_for_file(self.path),
            payload,
        });
        let module_id = self.module_id.clone();
        self.facts
            .push(self.symbol_fact(&id, Some(&module_id), &stable_key, symbol_kind, range));

        let Some(body) = rule
            .body_field
            .and_then(|field| node.child_by_field_name(field))
        else {
            return;
        };
        let nested = Scope {
            qualified_prefix: qualified_name,
            enclosing_kind: Some(rule.node_kind),
        };
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            self.walk(child, &nested, depth + 1);
        }
    }

    fn symbol_fact(
        &self,
        subject: &EntityId,
        object: Option<&EntityId>,
        stable_key: &StableKey,
        symbol_kind: &str,
        range: (u32, u32),
    ) -> Fact {
        Fact {
            id: FactId(format!(
                "fact_{}_symbol_defined_{:016x}",
                self.pack.id,
                stable_hash(stable_key.0.as_bytes())
            )),
            kind: FactKind::SymbolDefined,
            subject: subject.clone(),
            object: object.cloned(),
            value: json!({
                "stable_key": stable_key.0,
                "path": self.path,
                "symbol_kind": symbol_kind,
            }),
            evidence: vec![evidence_for_file(
                self.path,
                self.extractor,
                Some(range.0),
                Some(range.1),
            )],
            ownership: ownership_for_file(self.path),
            snapshot: self.snapshot.clone(),
            extractor: self.extractor.to_string(),
            confidence: 1.0,
        }
    }

    fn is_exported(&self, qualified_name: &str) -> bool {
        let name = qualified_name.rsplit('.').next().unwrap_or(qualified_name);
        match &self.pack.exports {
            ExportRule::None => false,
            ExportRule::Capitalized => name
                .chars()
                .next()
                .is_some_and(|first| first.is_uppercase()),
            ExportRule::NameList { .. } => self.exports.iter().any(|export| export == name),
        }
    }

    /// Collects the exported-name list of an `ExportRule::NameList` language.
    fn collect_exports(&mut self, root: Node) -> Vec<String> {
        let ExportRule::NameList {
            node_kind,
            name,
            name_field,
            value_field,
            value_kinds,
        } = &self.pack.exports
        else {
            return Vec::new();
        };
        let mut exports = Vec::new();
        let mut cursor = root.walk();
        for statement in root.named_children(&mut cursor) {
            let mut inner = statement.walk();
            for node in statement.named_children(&mut inner) {
                if node.kind() != *node_kind {
                    continue;
                }
                let left = node
                    .child_by_field_name(name_field)
                    .and_then(|l| self.text(l));
                if left.as_deref() != Some(*name) {
                    continue;
                }
                let Some(value) = node.child_by_field_name(value_field) else {
                    continue;
                };
                collect_name_list(value, value_kinds, self.bytes, &mut exports);
            }
        }
        if exports.len() > MAX_EXPORTS_PER_FILE {
            exports.truncate(MAX_EXPORTS_PER_FILE);
            self.truncated.insert("exports");
        }
        exports
    }

    fn collect_imports(&mut self, root: Node) -> Vec<Value> {
        let mut imports = Vec::new();
        self.scan_imports(root, &mut imports, 0);
        if imports.len() > MAX_IMPORTS_PER_FILE {
            imports.truncate(MAX_IMPORTS_PER_FILE);
            self.truncated.insert("imports");
        }
        imports
    }

    fn scan_imports(&self, node: Node, imports: &mut Vec<Value>, depth: usize) {
        if depth > MAX_TRAVERSAL_DEPTH || imports.len() >= MAX_IMPORTS_PER_FILE {
            return;
        }
        for rule in self.pack.imports {
            if rule.node_kind() == node.kind() {
                imports.extend(import_payloads(rule, node, self.bytes));
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.scan_imports(child, imports, depth + 1);
        }
    }

    fn collect_module_calls(&mut self, root: Node) -> Vec<Value> {
        let mut calls = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = root.walk();
        for child in root.named_children(&mut cursor) {
            self.collect_calls(child, &mut calls, &mut seen, true, 0);
        }
        calls
    }

    fn collect_declaration_calls(&mut self, rule: &DeclarationRule, node: Node) -> Vec<Value> {
        let Some(body) = rule
            .body_field
            .and_then(|field| node.child_by_field_name(field))
        else {
            return Vec::new();
        };
        let mut calls = Vec::new();
        let mut seen = HashSet::new();
        self.collect_calls(body, &mut calls, &mut seen, true, 0);
        calls
    }

    fn collect_calls(
        &mut self,
        node: Node,
        calls: &mut Vec<Value>,
        seen: &mut HashSet<(String, u32, u32)>,
        stop_at_declarations: bool,
        depth: usize,
    ) {
        if depth > MAX_TRAVERSAL_DEPTH {
            self.truncated.insert("declaration_depth");
            return;
        }
        if calls.len() >= MAX_CALL_SITES_PER_DECLARATION {
            self.truncated.insert("declaration_call_sites");
            return;
        }
        if self.call_sites >= MAX_CALL_SITES_PER_FILE {
            self.truncated.insert("file_call_sites");
            return;
        }
        if stop_at_declarations && self.is_declaration(node) {
            return;
        }
        if node.kind() == self.pack.call.node_kind {
            if let Some(payload) = self.call_site(node) {
                let key = (
                    payload
                        .get("callee")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    payload.get("line").and_then(Value::as_u64).unwrap_or(0) as u32,
                    payload.get("column").and_then(Value::as_u64).unwrap_or(0) as u32,
                );
                if seen.insert(key) {
                    self.call_sites += 1;
                    calls.push(payload);
                }
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.collect_calls(child, calls, seen, stop_at_declarations, depth + 1);
        }
    }

    fn is_declaration(&self, node: Node) -> bool {
        let effective = unwrap_wrappers(self.pack, node);
        self.pack
            .declarations
            .iter()
            .any(|rule| rule.node_kind == effective.kind())
    }

    /// Records one call site, or `None` when the callee cannot be named statically.
    fn call_site(&self, node: Node) -> Option<Value> {
        if self.call_sites >= MAX_CALL_SITES_PER_FILE {
            return None;
        }
        let rule = &self.pack.call;
        let callee_node = node.child_by_field_name(rule.function_field)?;
        let callee = callee_path(callee_node, rule, self.bytes, 0)?;
        let (name, receiver) = match callee.rsplit_once('.') {
            Some((receiver, name)) => (name.to_string(), Some(receiver.to_string())),
            None => (callee.clone(), None),
        };
        let start = callee_node.start_position();
        let arg_count = node
            .child_by_field_name(rule.arguments_field)
            .map_or(0, |arguments| arguments.named_child_count());

        Some(json!({
            "callee": callee,
            "name": name,
            "receiver": receiver,
            "kind": "call",
            "line": start.row as u32 + 1,
            "column": start.column as u32 + 1,
            "arg_count": arg_count,
        }))
    }

    fn collect_parse_errors(&mut self, root: Node) {
        let mut found: Vec<(u32, String, bool)> = Vec::new();
        self.scan_parse_errors(root, 0, &mut found);
        for (line, node_kind, missing) in found {
            let diagnostic = self.diagnostic(
                &format!("{}_parse_error", self.pack.id),
                &format!("{node_kind}:{line}"),
                "File contains a parse error",
                format!(
                    "the {} parser reports a {} node on line {line}; declarations and call sites \
                     inside the damaged region may be missing",
                    self.pack.parser,
                    if missing { "missing" } else { "damaged" }
                ),
                (line, line),
                json!({
                    "language": self.pack.id,
                    "node_kind": node_kind,
                    "missing": missing,
                }),
            );
            self.diagnostics.push(diagnostic);
        }
    }

    fn scan_parse_errors(&self, node: Node, depth: usize, found: &mut Vec<(u32, String, bool)>) {
        if depth > MAX_TRAVERSAL_DEPTH || found.len() >= MAX_PARSE_ERROR_DIAGNOSTICS {
            return;
        }
        if node.is_error() || node.is_missing() {
            let line = node.start_position().row as u32 + 1;
            let node_kind = node.kind().to_string();
            if !found
                .iter()
                .any(|(known_line, known_kind, _)| *known_line == line && *known_kind == node_kind)
            {
                found.push((line, node_kind, node.is_missing()));
            }
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.scan_parse_errors(child, depth + 1, found);
        }
    }

    fn emit_truncation_diagnostics(&mut self, root: Node) {
        let range = (1, root.end_position().row as u32 + 1);
        let truncated: Vec<&'static str> = self.truncated.iter().copied().collect();
        for collection in truncated {
            let limit = match collection {
                "declarations" => MAX_DECLARATIONS_PER_FILE,
                "declaration_call_sites" => MAX_CALL_SITES_PER_DECLARATION,
                "file_call_sites" => MAX_CALL_SITES_PER_FILE,
                "imports" => MAX_IMPORTS_PER_FILE,
                "exports" => MAX_EXPORTS_PER_FILE,
                "declaration_depth" => MAX_TRAVERSAL_DEPTH,
                _ => MAX_TRAVERSAL_DEPTH,
            };
            let diagnostic = self.diagnostic(
                &format!("{}_truncation", self.pack.id),
                collection,
                "Extraction was truncated by a bound",
                format!(
                    "`{collection}` reached its bound of {limit} in this file; the recorded \
                     evidence is incomplete and every count derived from it is a lower bound"
                ),
                range,
                json!({
                    "language": self.pack.id,
                    "collection": collection,
                    "limit": limit,
                }),
            );
            self.diagnostics.push(diagnostic);
        }
    }

    fn diagnostic(
        &self,
        kind: &str,
        key: &str,
        title: &str,
        message: String,
        range: (u32, u32),
        payload: Value,
    ) -> Diagnostic {
        Diagnostic {
            id: DiagnosticId(format!(
                "diag_{}_{:016x}",
                kind,
                stable_hash(format!("{}:{}:{key}:{range:?}", self.path, kind).as_bytes())
            )),
            kind: DiagnosticKind::Other(kind.to_string()),
            severity: Severity::Low,
            status: DiagnosticStatus::Open,
            title: title.to_string(),
            message,
            entities: Vec::new(),
            evidence: vec![evidence_for_file(
                self.path,
                self.extractor,
                Some(range.0),
                Some(range.1),
            )],
            ownership: ownership_for_file(self.path),
            snapshot: self.snapshot.clone(),
            suggested_fix: None,
            payload,
        }
    }

    fn text(&self, node: Node) -> Option<String> {
        node.utf8_text(self.bytes).ok().map(ToString::to_string)
    }
}

fn classify(
    rule: &DeclarationRule,
    node: Node,
    is_method: bool,
) -> Option<(EntityKind, &'static str)> {
    if is_method && let Some(kind) = &rule.method_kind {
        return Some((kind.entity_kind.clone(), kind.symbol_kind));
    }
    if let Some(type_field) = rule.type_field {
        if let Some(type_node) = node.child_by_field_name(type_field) {
            if let Some(kind) = rule
                .type_kinds
                .iter()
                .find(|kind| kind.node_kind == type_node.kind())
            {
                return Some((kind.entity_kind.clone(), kind.symbol_kind));
            }
            return rule
                .fallback_kind
                .as_ref()
                .map(|kind| (kind.entity_kind.clone(), kind.symbol_kind));
        }
        return rule
            .fallback_kind
            .as_ref()
            .map(|kind| (kind.entity_kind.clone(), kind.symbol_kind));
    }
    rule.matched_kinds
        .iter()
        .find(|kind| kind.node_kind == node.kind())
        .map(|kind| (kind.entity_kind.clone(), kind.symbol_kind))
}

/// Unwraps wrapper nodes such as `decorated_definition` onto their declaration node.
fn unwrap_wrappers<'pack>(pack: &'pack LanguagePack, node: Node<'pack>) -> Node<'pack> {
    let mut current = node;
    for _ in 0..4 {
        let wrapper_kind = current.kind();
        let mut next = None;
        if pack
            .declarations
            .iter()
            .any(|rule| rule.wrappers.contains(&wrapper_kind))
        {
            let mut cursor = current.walk();
            for child in current.named_children(&mut cursor) {
                if pack.declarations.iter().any(|rule| {
                    rule.wrappers.contains(&wrapper_kind) && rule.node_kind == child.kind()
                }) {
                    next = Some(child);
                    break;
                }
            }
        }
        match next {
            Some(child) => current = child,
            None => break,
        }
    }
    current
}

/// Builds a dotted callee path from a statically named call target.
fn callee_path(node: Node, rule: &CallRule, bytes: &[u8], depth: usize) -> Option<String> {
    if depth > 32 {
        return None;
    }
    if rule.atom_kinds.contains(&node.kind()) {
        return node.utf8_text(bytes).ok().map(ToString::to_string);
    }
    let member = rule
        .members
        .iter()
        .find(|member| member.node_kind == node.kind())?;
    let object = node.child_by_field_name(member.object_field)?;
    let name = node
        .child_by_field_name(member.member_field)
        .and_then(|member_node| member_node.utf8_text(bytes).ok())?;
    let object_path = callee_path(object, rule, bytes, depth + 1)?;
    Some(format!("{object_path}.{name}"))
}

/// Records the receiver type of a method declaration.
fn receiver_type(rule: &DeclarationRule, node: Node, bytes: &[u8]) -> Option<String> {
    let field = rule.receiver_field?;
    let receiver = node.child_by_field_name(field)?;
    type_name(receiver, rule.receiver_type_kinds, bytes, 6)
}

fn type_name(node: Node, kinds: &[&str], bytes: &[u8], depth: usize) -> Option<String> {
    if kinds.contains(&node.kind()) {
        return node
            .utf8_text(bytes)
            .ok()
            .filter(|name| name.len() <= MAX_TYPE_NAME_LENGTH)
            .map(ToString::to_string);
    }
    if depth == 0 {
        return None;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = type_name(child, kinds, bytes, depth - 1) {
            return Some(found);
        }
    }
    None
}

/// Collects imported names listed in a name-list export assignment.
fn collect_name_list(node: Node, value_kinds: &[&str], bytes: &[u8], exports: &mut Vec<String>) {
    if exports.len() >= MAX_EXPORTS_PER_FILE {
        return;
    }
    if value_kinds.contains(&node.kind())
        && let Ok(text) = node.utf8_text(bytes)
    {
        let name = text
            .trim_start_matches(['"', '\'', 'r', 'b', 'f', 'u'])
            .trim_end_matches(['"', '\''])
            .to_string();
        if !name.is_empty() {
            exports.push(name);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_name_list(child, value_kinds, bytes, exports);
    }
}

impl ImportRule {
    fn node_kind(&self) -> &'static str {
        match self {
            ImportRule::Dotted { node_kind, .. }
            | ImportRule::From { node_kind, .. }
            | ImportRule::Spec { node_kind, .. } => node_kind,
        }
    }
}

fn import_payloads(rule: &ImportRule, node: Node, bytes: &[u8]) -> Vec<Value> {
    match rule {
        ImportRule::Dotted { module_kinds, .. } => {
            let mut payloads = Vec::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let Some((module, alias)) = module_name(child, module_kinds, bytes) else {
                    continue;
                };
                payloads.push(json!({
                    "style": "dotted",
                    "module": module,
                    "names": [],
                    "aliases": {},
                    "alias": alias,
                    "line": child.start_position().row as u32 + 1,
                }));
            }
            payloads
        }
        ImportRule::From {
            module_field,
            imported_kinds,
            ..
        } => {
            let module_node = node.child_by_field_name(module_field);
            let module = module_node
                .and_then(|module_node| module_node.utf8_text(bytes).ok())
                .unwrap_or_default()
                .to_string();
            let mut names = Vec::new();
            let mut aliases = Map::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if module_node.is_some_and(|module_node| module_node.id() == child.id()) {
                    continue;
                }
                if child.kind() == "wildcard_import" {
                    names.push("*".to_string());
                    continue;
                }
                if child.kind() == "aliased_import" {
                    let Some(name_node) = child.child_by_field_name("name") else {
                        continue;
                    };
                    let Some(name) = name_node.utf8_text(bytes).ok() else {
                        continue;
                    };
                    if let Some(alias) = child
                        .child_by_field_name("alias")
                        .and_then(|alias| alias.utf8_text(bytes).ok())
                    {
                        aliases.insert(name.to_string(), Value::String(alias.to_string()));
                    }
                    names.push(name.to_string());
                    continue;
                }
                if imported_kinds.contains(&child.kind())
                    && let Ok(name) = child.utf8_text(bytes)
                {
                    names.push(name.to_string());
                }
            }
            vec![json!({
                "style": "from",
                "module": module,
                "names": names,
                "aliases": aliases,
                "line": node.start_position().row as u32 + 1,
            })]
        }
        ImportRule::Spec {
            spec_kind,
            path_field,
            alias_field,
            ..
        } => {
            let mut payloads = Vec::new();
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let mut specs = Vec::new();
                if child.kind() == *spec_kind {
                    specs.push(child);
                } else {
                    let mut inner = child.walk();
                    for grandchild in child.named_children(&mut inner) {
                        if grandchild.kind() == *spec_kind {
                            specs.push(grandchild);
                        }
                    }
                }
                for spec in specs {
                    let Some(path) = spec
                        .child_by_field_name(path_field)
                        .and_then(|path| path.utf8_text(bytes).ok())
                    else {
                        continue;
                    };
                    let module = path.trim_matches(['"', '`']).to_string();
                    let alias = spec
                        .child_by_field_name(alias_field)
                        .and_then(|alias| alias.utf8_text(bytes).ok())
                        .map(ToString::to_string);
                    payloads.push(json!({
                        "style": "spec",
                        "module": module,
                        "names": [],
                        "aliases": {},
                        "alias": alias,
                        "stdlib": is_standard_library_path(&module),
                        "line": spec.start_position().row as u32 + 1,
                    }));
                }
            }
            payloads
        }
    }
}

/// Resolves one module name written by a dotted import statement.
fn module_name(
    node: Node,
    module_kinds: &[&str],
    bytes: &[u8],
) -> Option<(String, Option<String>)> {
    if module_kinds.contains(&node.kind()) {
        return node
            .utf8_text(bytes)
            .ok()
            .map(|text| (text.to_string(), None));
    }
    let name = node.child_by_field_name("name")?;
    if !module_kinds.contains(&name.kind()) {
        return None;
    }
    let module = name.utf8_text(bytes).ok()?.to_string();
    let alias = node
        .child_by_field_name("alias")
        .and_then(|alias| alias.utf8_text(bytes).ok())
        .map(ToString::to_string);
    Some((module, alias))
}

/// Reports whether a Go-style import path belongs to the standard library.
///
/// Standard-library paths never carry a dot in their first segment.
fn is_standard_library_path(module: &str) -> bool {
    let first = module.split('/').next().unwrap_or(module);
    !first.is_empty() && !first.contains('.')
}

fn line_range(node: Node) -> (u32, u32) {
    (
        node.start_position().row as u32 + 1,
        node.end_position().row as u32 + 1,
    )
}

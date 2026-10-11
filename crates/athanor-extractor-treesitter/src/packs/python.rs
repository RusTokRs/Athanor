//! The Python pack: functions, classes, imports, calls, and `__all__` exports.
//!
//! Node kinds and field names are taken from `tree-sitter-python` 0.25.0: a decorated definition
//! wraps `function_definition`/`class_definition` in `decorated_definition`, a function inside a
//! class body is a method, calls are `call` nodes over `attribute` chains, imports are
//! `import_statement`/`import_from_statement`, and `__all__ = [...]` lists the exported names.

use athanor_domain::EntityKind;
use tree_sitter::Language;

use crate::engine::{
    CallRule, DeclarationKindRule, DeclarationRule, ExportRule, ImportRule, LanguagePack,
    MemberRule,
};

fn language() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

/// The Python language pack.
pub static PYTHON: LanguagePack = LanguagePack {
    id: "python",
    hint: "python",
    parser: "tree-sitter-python",
    extensions: &["py", "pyi"],
    language,
    declarations: &[
        DeclarationRule {
            node_kind: "function_definition",
            wrappers: &["decorated_definition"],
            name_child: None,
            name_field: "name",
            body_field: Some("body"),
            matched_kinds: &[DeclarationKindRule {
                node_kind: "function_definition",
                entity_kind: EntityKind::Function,
                symbol_kind: "function",
            }],
            type_field: None,
            type_kinds: &[],
            fallback_kind: None,
            method_ancestors: &["class_definition"],
            method_kind: Some(DeclarationKindRule {
                node_kind: "function_definition",
                entity_kind: EntityKind::Function,
                symbol_kind: "method",
            }),
            receiver_field: None,
            receiver_type_kinds: &[],
        },
        DeclarationRule {
            node_kind: "class_definition",
            wrappers: &["decorated_definition"],
            name_child: None,
            name_field: "name",
            body_field: Some("body"),
            matched_kinds: &[DeclarationKindRule {
                node_kind: "class_definition",
                entity_kind: EntityKind::Class,
                symbol_kind: "class",
            }],
            type_field: None,
            type_kinds: &[],
            fallback_kind: None,
            method_ancestors: &[],
            method_kind: None,
            receiver_field: None,
            receiver_type_kinds: &[],
        },
    ],
    call: CallRule {
        node_kind: "call",
        function_field: "function",
        arguments_field: "arguments",
        atom_kinds: &["identifier"],
        members: &[MemberRule {
            node_kind: "attribute",
            object_field: "object",
            member_field: "attribute",
        }],
    },
    imports: &[
        ImportRule::Dotted {
            node_kind: "import_statement",
            module_kinds: &["dotted_name"],
        },
        ImportRule::From {
            node_kind: "import_from_statement",
            module_field: "module_name",
            imported_kinds: &["dotted_name", "identifier"],
        },
    ],
    exports: ExportRule::NameList {
        node_kind: "assignment",
        name: "__all__",
        name_field: "left",
        value_field: "right",
        value_kinds: &["string"],
    },
    async_prefix: Some("async"),
};

#[cfg(test)]
mod tests {
    use athanor_core::Extractor;
    use athanor_domain::{DiagnosticKind, EntityKind, FactKind};

    use crate::{TreeSitterExtractor, input};

    const CLIENT: &str = "\
import os
import os.path as osp
from typing import List, Optional as Maybe

__all__ = [\"Client\", \"connect\"]


class Client:
    def __init__(self, session):
        self.session = session

    async def send(self, payload):
        return self.session.post(payload, timeout=5)


@decorator(query=\"connect\")
def connect(url):
    client = Client(build_session())
    return client.send(url)


handlers[key]()
";

    #[tokio::test]
    async fn records_declarations_imports_exports_and_calls() {
        let output = TreeSitterExtractor
            .extract(input("api/client.py", CLIENT, "python"))
            .await
            .unwrap();

        let module = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Module)
            .expect("module entity");
        assert_eq!(module.stable_key.0, "module://python:api/client.py");
        assert_eq!(module.payload["parser"], "tree-sitter-python");
        assert_eq!(module.payload["declaration_count"], 4);
        assert_eq!(
            module.payload["exports"],
            serde_json::json!(["Client", "connect"])
        );
        assert_eq!(module.payload["imports"].as_array().unwrap().len(), 3);
        assert_eq!(module.payload["imports"][0]["module"], "os");
        assert_eq!(module.payload["imports"][1]["module"], "os.path");
        assert_eq!(module.payload["imports"][1]["alias"], "osp");
        assert_eq!(module.payload["imports"][2]["module"], "typing");
        assert_eq!(
            module.payload["imports"][2]["names"],
            serde_json::json!(["List", "Optional"])
        );
        assert_eq!(
            module.payload["imports"][2]["aliases"],
            serde_json::json!({"Optional": "Maybe"})
        );
        // A subscript call target cannot be named statically.
        assert_eq!(module.payload["calls"], serde_json::json!([]));

        let class = entity(&output, "symbol://python:api/client.py#Client");
        assert_eq!(class.kind, EntityKind::Class);
        assert_eq!(class.payload["symbol_kind"], "class");
        assert_eq!(class.payload["exported"], true);
        assert_eq!(class.source.as_ref().unwrap().line_start, Some(8));

        let method = entity(&output, "symbol://python:api/client.py#Client.send");
        assert_eq!(method.kind, EntityKind::Function);
        assert_eq!(method.payload["symbol_kind"], "method");
        assert_eq!(method.payload["async"], true);
        assert_eq!(method.payload["exported"], false);
        assert_eq!(
            method.payload["calls"],
            serde_json::json!([{
                "callee": "self.session.post",
                "name": "post",
                "receiver": "self.session",
                "kind": "call",
                "line": 13,
                "column": 16,
                "arg_count": 2,
            }])
        );

        let function = entity(&output, "symbol://python:api/client.py#connect");
        assert_eq!(function.payload["symbol_kind"], "function");
        assert_eq!(function.payload["source_node"], "function_definition");
        assert_eq!(function.payload["async"], false);
        assert_eq!(
            function.payload["calls"],
            serde_json::json!([
                {
                    "callee": "Client",
                    "name": "Client",
                    "receiver": null,
                    "kind": "call",
                    "line": 18,
                    "column": 14,
                    "arg_count": 1,
                },
                {
                    "callee": "build_session",
                    "name": "build_session",
                    "receiver": null,
                    "kind": "call",
                    "line": 18,
                    "column": 21,
                    "arg_count": 0,
                },
                {
                    "callee": "client.send",
                    "name": "send",
                    "receiver": "client",
                    "kind": "call",
                    "line": 19,
                    "column": 12,
                    "arg_count": 1,
                },
            ])
        );

        assert!(
            output
                .facts
                .iter()
                .any(|fact| fact.kind == FactKind::SymbolDefined
                    && fact.value["stable_key"] == "symbol://python:api/client.py#Client.send")
        );
        assert!(output.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn records_a_nested_declaration_with_its_qualified_name() {
        let output = TreeSitterExtractor
            .extract(input(
                "pkg/tools.py",
                "class Registry:\n    def register(self):\n        def normalize(name):\n            return name.strip()\n\n        return normalize\n",
                "python",
            ))
            .await
            .unwrap();

        let nested = entity(
            &output,
            "symbol://python:pkg/tools.py#Registry.register.normalize",
        );
        assert_eq!(nested.kind, EntityKind::Function);
        assert_eq!(nested.payload["symbol_kind"], "function");
        assert_eq!(
            nested.payload["qualified_name"],
            "Registry.register.normalize"
        );
    }

    #[tokio::test]
    async fn reports_parse_errors_and_keeps_the_damaged_file_visible() {
        let output = TreeSitterExtractor
            .extract(input("broken.py", "def broken(:\n    pass\n", "python"))
            .await
            .unwrap();

        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.kind == DiagnosticKind::Other("python_parse_error".to_string())
            })
            .expect("parse error diagnostic");
        assert_eq!(
            diagnostic.evidence[0].source_file.as_deref(),
            Some("broken.py")
        );
        assert_eq!(diagnostic.payload["language"], "python");
    }

    #[tokio::test]
    async fn records_a_redefined_name_once_and_discloses_the_duplicate() {
        let output = TreeSitterExtractor
            .extract(input(
                "platform_tools.py",
                "def process():\n    return 1\n\n\ndef process():\n    return 2\n",
                "python",
            ))
            .await
            .unwrap();

        let declared = output
            .entities
            .iter()
            .filter(|entity| entity.stable_key.0 == "symbol://python:platform_tools.py#process")
            .count();
        assert_eq!(declared, 1);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::Other("python_duplicate_declaration".to_string())
        }));
    }

    #[tokio::test]
    async fn discloses_call_site_truncation_at_the_declaration_bound() {
        let mut source = String::from("def busy():\n");
        for index in 0..300 {
            source.push_str(&format!("    step_{index}()\n"));
        }
        let output = TreeSitterExtractor
            .extract(input("pkg/busy.py", &source, "python"))
            .await
            .unwrap();

        let busy = entity(&output, "symbol://python:pkg/busy.py#busy");
        assert_eq!(
            busy.payload["calls"].as_array().unwrap().len(),
            crate::MAX_CALL_SITES_PER_DECLARATION
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == DiagnosticKind::Other("python_truncation".to_string())
                && diagnostic.payload["collection"] == "declaration_call_sites"
        }));
    }

    fn entity<'a>(
        output: &'a athanor_core::ExtractOutput,
        stable_key: &str,
    ) -> &'a athanor_domain::Entity {
        output
            .entities
            .iter()
            .find(|entity| entity.stable_key.0 == stable_key)
            .unwrap_or_else(|| panic!("missing entity {stable_key}"))
    }
}

//! The Go pack: functions, methods, named types, imports, and calls.
//!
//! Node kinds and field names are taken from `tree-sitter-go` 0.25.0: `type_declaration` carries
//! one `type_spec` per named type (an alias is a separate `type_alias` kind and stays out of the
//! graph), struct and interface types classify the spec, methods are identified by their receiver
//! type and record the receiver variable when it is written, calls are `call_expression` nodes over
//! `selector_expression` chains, and imports are `import_spec` nodes whose `path` is a quoted string.
//! Export is a name property: a leading upper-case letter.

use athanor_domain::EntityKind;
use tree_sitter::Language;

use crate::engine::{
    CallRule, DeclarationKindRule, DeclarationRule, ExportRule, ImportRule, LanguagePack,
    MemberRule,
};

fn language() -> Language {
    tree_sitter_go::LANGUAGE.into()
}

/// The Go language pack.
pub static GO: LanguagePack = LanguagePack {
    id: "go",
    hint: "go",
    parser: "tree-sitter-go",
    extensions: &["go"],
    language,
    declarations: &[
        DeclarationRule {
            node_kind: "function_declaration",
            wrappers: &[],
            name_child: None,
            name_field: "name",
            body_field: Some("body"),
            matched_kinds: &[DeclarationKindRule {
                node_kind: "function_declaration",
                entity_kind: EntityKind::Function,
                symbol_kind: "function",
            }],
            type_field: None,
            type_kinds: &[],
            fallback_kind: None,
            method_ancestors: &[],
            method_kind: None,
            receiver_field: None,
            receiver_type_kinds: &[],
        },
        DeclarationRule {
            node_kind: "method_declaration",
            wrappers: &[],
            name_child: None,
            name_field: "name",
            body_field: Some("body"),
            matched_kinds: &[DeclarationKindRule {
                node_kind: "method_declaration",
                entity_kind: EntityKind::Function,
                symbol_kind: "method",
            }],
            type_field: None,
            type_kinds: &[],
            fallback_kind: None,
            method_ancestors: &[],
            method_kind: None,
            receiver_field: Some("receiver"),
            receiver_type_kinds: &["type_identifier"],
        },
        DeclarationRule {
            node_kind: "type_declaration",
            wrappers: &[],
            name_child: Some("type_spec"),
            name_field: "name",
            body_field: None,
            matched_kinds: &[],
            type_field: Some("type"),
            type_kinds: &[
                DeclarationKindRule {
                    node_kind: "struct_type",
                    entity_kind: EntityKind::Class,
                    symbol_kind: "struct",
                },
                DeclarationKindRule {
                    node_kind: "interface_type",
                    entity_kind: EntityKind::Symbol,
                    symbol_kind: "interface",
                },
            ],
            fallback_kind: Some(DeclarationKindRule {
                node_kind: "type_spec",
                entity_kind: EntityKind::Symbol,
                symbol_kind: "type",
            }),
            method_ancestors: &[],
            method_kind: None,
            receiver_field: None,
            receiver_type_kinds: &[],
        },
    ],
    call: CallRule {
        node_kind: "call_expression",
        function_field: "function",
        arguments_field: "arguments",
        atom_kinds: &["identifier", "field_identifier"],
        members: &[MemberRule {
            node_kind: "selector_expression",
            object_field: "operand",
            member_field: "field",
        }],
    },
    imports: &[ImportRule::Spec {
        node_kind: "import_declaration",
        spec_kind: "import_spec",
        path_field: "path",
        alias_field: "name",
    }],
    exports: ExportRule::Capitalized,
    async_prefix: None,
};

#[cfg(test)]
mod tests {
    use athanor_core::Extractor;
    use athanor_domain::{DiagnosticKind, EntityKind, FactKind};

    use crate::{TreeSitterExtractor, input};

    const SERVER: &str = "\
package api

import (
\"fmt\"
cfg \"example.com/project/config\"
)

type Server struct {
name string
}

type Handler interface {
Handle()
}

type ServerAlias = Server

func NewServer(name string) *Server {
return &Server{name: name}
}

func (s *Server) Serve() error {
fmt.Println(s.name)
return cfg.Load(s.name)
}

func run() {
handlers[key]()
}
";

    #[tokio::test]
    async fn records_types_imports_exports_and_calls() {
        let output = TreeSitterExtractor
            .extract(input("api/server.go", SERVER, "go"))
            .await
            .unwrap();

        let module = output
            .entities
            .iter()
            .find(|entity| entity.kind == EntityKind::Module)
            .expect("module entity");
        assert_eq!(module.stable_key.0, "module://go:api/server.go");
        assert_eq!(module.payload["parser"], "tree-sitter-go");
        // Type aliases are not declarations of this pack.
        assert_eq!(module.payload["declaration_count"], 5);
        assert_eq!(module.payload["imports"].as_array().unwrap().len(), 2);
        assert_eq!(module.payload["imports"][0]["module"], "fmt");
        assert_eq!(module.payload["imports"][0]["stdlib"], true);
        assert_eq!(
            module.payload["imports"][0]["alias"],
            serde_json::Value::Null
        );
        assert_eq!(
            module.payload["imports"][1]["module"],
            "example.com/project/config"
        );
        assert_eq!(module.payload["imports"][1]["stdlib"], false);
        assert_eq!(module.payload["imports"][1]["alias"], "cfg");
        assert_eq!(module.payload["calls"], serde_json::json!([]));

        let struct_type = entity(&output, "symbol://go:api/server.go#Server");
        assert_eq!(struct_type.kind, EntityKind::Class);
        assert_eq!(struct_type.payload["symbol_kind"], "struct");
        assert_eq!(struct_type.payload["exported"], true);
        assert_eq!(struct_type.source.as_ref().unwrap().line_start, Some(8));

        let interface = entity(&output, "symbol://go:api/server.go#Handler");
        assert_eq!(interface.kind, EntityKind::Symbol);
        assert_eq!(interface.payload["symbol_kind"], "interface");

        let constructor = entity(&output, "symbol://go:api/server.go#NewServer");
        assert_eq!(constructor.payload["symbol_kind"], "function");
        assert_eq!(constructor.payload["exported"], true);
        assert_eq!(constructor.payload["calls"], serde_json::json!([]));

        let method = entity(&output, "symbol://go:api/server.go#Server.Serve");
        assert_eq!(method.payload["symbol_kind"], "method");
        assert_eq!(method.payload["qualified_name"], "Server.Serve");
        assert_eq!(method.payload["receiver"], "Server");
        assert_eq!(method.payload["receiver_binding"], "s");
        assert_eq!(method.payload["exported"], true);
        assert_eq!(method.payload["async"], false);
        assert_eq!(
            method.payload["calls"],
            serde_json::json!([
                {
                    "callee": "fmt.Println",
                    "name": "Println",
                    "receiver": "fmt",
                    "kind": "call",
                    "line": 23,
                    "column": 1,
                    "arg_count": 1,
                },
                {
                    "callee": "cfg.Load",
                    "name": "Load",
                    "receiver": "cfg",
                    "kind": "call",
                    "line": 24,
                    "column": 8,
                    "arg_count": 1,
                },
            ])
        );

        let function = entity(&output, "symbol://go:api/server.go#run");
        assert_eq!(function.payload["exported"], false);
        assert_eq!(function.payload["calls"], serde_json::json!([]));

        assert!(output.facts.iter().any(|fact| {
            fact.kind == FactKind::SymbolDefined
                && fact.value["stable_key"] == "symbol://go:api/server.go#Server.Serve"
        }));
        assert!(output.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn keeps_unknown_named_types_with_a_disclosed_symbol_kind() {
        let output = TreeSitterExtractor
            .extract(input(
                "api/types.go",
                "package api\n\ntype IDs map[string]int\n\ntype Alias = IDs\n",
                "go",
            ))
            .await
            .unwrap();

        let named = entity(&output, "symbol://go:api/types.go#IDs");
        assert_eq!(named.kind, EntityKind::Symbol);
        assert_eq!(named.payload["symbol_kind"], "type");
        assert!(
            output
                .entities
                .iter()
                .all(|entity| entity.stable_key.0 != "symbol://go:api/types.go#Alias")
        );
    }

    #[tokio::test]
    async fn identifies_methods_by_receiver_type_and_keeps_same_named_methods_apart() {
        let output = TreeSitterExtractor
            .extract(input(
                "api/resources.go",
                "package api\n\ntype Server struct{}\n\ntype Client struct{}\n\nfunc (s *Server) Close() {}\n\nfunc (c *Client) Close() {}\n",
                "go",
            ))
            .await
            .unwrap();

        assert_eq!(
            entity(&output, "symbol://go:api/resources.go#Server.Close").payload["receiver_binding"],
            "s"
        );
        assert_eq!(
            entity(&output, "symbol://go:api/resources.go#Client.Close").payload["receiver_binding"],
            "c"
        );
        assert!(
            !output.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == DiagnosticKind::Other("go_duplicate_declaration".to_string())
            }),
            "receiver-qualified methods of different types do not collide"
        );
    }

    #[tokio::test]
    async fn reports_parse_errors_for_damaged_go_files() {
        let output = TreeSitterExtractor
            .extract(input(
                "api/broken.go",
                "package api\n\nfunc broken( {\n",
                "go",
            ))
            .await
            .unwrap();

        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.kind == DiagnosticKind::Other("go_parse_error".to_string())
            })
            .expect("parse error diagnostic");
        assert_eq!(diagnostic.payload["language"], "go");
        assert_eq!(
            diagnostic.evidence[0].source_file.as_deref(),
            Some("api/broken.go")
        );
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

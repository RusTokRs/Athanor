use std::collections::HashMap;

use async_trait::async_trait;
use athanor_core::{
    CoreResult, ExtractInput, ExtractOutput, Extractor, InvalidationPolicy, SourceFile,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind, Fact,
    FactId, FactKind, LanguageCode, Severity, SourceLocation, StableKey,
};
use athanor_extractor_basic::{evidence_for_file, file_entity, ownership_for_file, stable_hash};
use serde_json::{Value, json};

#[derive(Debug, Clone, Default)]
pub struct GraphQlExtractor;

#[async_trait]
impl Extractor for GraphQlExtractor {
    fn name(&self) -> &'static str {
        "graphql"
    }

    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::FILE_LOCAL
    }

    fn supports(&self, source: &SourceFile) -> bool {
        matches!(source.language_hint.as_deref(), Some("graphql"))
            || source.path.ends_with(".graphql")
            || source.path.ends_with(".gql")
            || is_graphql_introspection_path(&source.path)
            || (matches!(source.language_hint.as_deref(), Some("json"))
                && source
                    .content
                    .as_deref()
                    .is_some_and(has_graphql_introspection_marker))
    }

    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput> {
        let Some(content) = input.source.content.as_deref() else {
            return Ok(ExtractOutput::default());
        };

        let file_id = file_entity(&input.source, &input.snapshot.0).id;
        let mut entities = Vec::new();
        let mut facts = Vec::new();
        let mut diagnostics = Vec::new();

        if is_graphql_introspection_source(&input.source, content) {
            match parse_introspection_document(content, &input.source.path) {
                Ok(document) => {
                    if document.root_schema.is_none()
                        && document.schema_types.is_empty()
                        && document.directives.is_empty()
                    {
                        diagnostics.push(graphql_diagnostic(
                            self.name(),
                            &input,
                            "graphql_introspection_empty",
                            "GraphQL introspection contains no extractable schema declarations",
                            "The GraphQL introspection JSON did not contain root operation types, non-built-in schema types under __schema.types, or directive definitions under __schema.directives.",
                            1,
                            json!({
                                "source_kind": "graphql_introspection",
                                "expected_paths": [
                                    "data.__schema.queryType",
                                    "data.__schema.mutationType",
                                    "data.__schema.subscriptionType",
                                    "data.__schema.types",
                                    "data.__schema.directives"
                                ],
                            }),
                        ));
                    }
                    for schema_type in document
                        .root_schema
                        .into_iter()
                        .chain(document.schema_types)
                    {
                        let entity = schema_entity(
                            &input.source.path,
                            &schema_type.schema_kind,
                            &schema_type.name,
                            1,
                            schema_type.fields,
                            schema_type.arguments,
                            schema_type.argument_definitions,
                            None,
                            schema_type.deprecated_members,
                            Vec::new(),
                            schema_type.member_types,
                            Vec::new(),
                        );
                        facts.push(declaration_fact(
                            self.name(),
                            &input,
                            &entity,
                            &file_id,
                            FactKind::Other("api_schema_declared".to_string()),
                            "graphql_introspection_schema",
                            1,
                        ));
                        entities.push(entity);
                    }
                    for directive in document.directives {
                        let entity = directive_entity(
                            &input.source.path,
                            &directive.name,
                            1,
                            directive.locations,
                            directive.arguments,
                            directive.argument_definitions,
                        );
                        facts.push(declaration_fact(
                            self.name(),
                            &input,
                            &entity,
                            &file_id,
                            FactKind::Other("graphql_directive_declared".to_string()),
                            "graphql_directive",
                            1,
                        ));
                        entities.push(entity);
                    }
                }
                Err(message) => diagnostics.push(graphql_diagnostic(
                    self.name(),
                    &input,
                    "graphql_introspection_parse_error",
                    "GraphQL introspection JSON could not be parsed",
                    &message,
                    1,
                    json!({
                        "source_kind": "graphql_introspection",
                    }),
                )),
            }

            return Ok(ExtractOutput {
                entities,
                facts,
                diagnostics,
            });
        }

        let declarations = parse_graphql_declarations(content);

        let mut validation_diagnostics =
            validate_graphql_declarations(self.name(), &input, &declarations, &input.source.path);
        diagnostics.append(&mut validation_diagnostics);

        if declarations.is_empty() {
            diagnostics.push(graphql_diagnostic(
                self.name(),
                &input,
                "graphql_no_declarations",
                "GraphQL source contains no supported declarations",
                "The GraphQL extractor did not find a top-level operation, fragment, directive, or SDL type declaration in this source.",
                1,
                json!({
                    "source_kind": "graphql_sdl_or_operation",
                    "supported_declarations": [
                        "query",
                        "mutation",
                        "subscription",
                        "type",
                        "input",
                        "interface",
                        "enum",
                        "scalar",
                        "union",
                        "schema",
                        "fragment",
                        "directive"
                    ],
                }),
            ));
        }

        for declaration in declarations {
            let entity = match &declaration.kind {
                GraphQlDeclarationKind::Operation { operation_type } => operation_entity(
                    &input.source.path,
                    operation_type,
                    &declaration.name,
                    declaration.line,
                    declaration.variables.clone(),
                    declaration.variable_definitions.clone(),
                    declaration.fields.clone(),
                    declaration.fragment_spreads.clone(),
                    declaration.inline_type_conditions.clone(),
                    declaration.arguments.clone(),
                    declaration.directives.clone(),
                ),
                GraphQlDeclarationKind::Schema { schema_kind } => schema_entity(
                    &input.source.path,
                    schema_kind,
                    &declaration.name,
                    declaration.line,
                    declaration.fields.clone(),
                    declaration.arguments.clone(),
                    declaration.member_argument_definitions.clone(),
                    declaration.deprecation_reason.clone(),
                    declaration.deprecated_members.clone(),
                    declaration.directives.clone(),
                    declaration.member_types.clone(),
                    declaration.member_directives.clone(),
                ),
                GraphQlDeclarationKind::Fragment { type_condition } => fragment_entity(
                    &input.source.path,
                    &declaration.name,
                    type_condition.clone(),
                    declaration.line,
                    declaration.fields.clone(),
                    declaration.fragment_spreads.clone(),
                    declaration.directives.clone(),
                ),
                GraphQlDeclarationKind::Directive { locations } => directive_entity(
                    &input.source.path,
                    &declaration.name,
                    declaration.line,
                    locations.clone(),
                    declaration.arguments.clone(),
                    declaration.argument_definitions.clone(),
                ),
            };

            let fact_kind = match &declaration.kind {
                GraphQlDeclarationKind::Operation { .. } => FactKind::RouteDeclared,
                GraphQlDeclarationKind::Schema { .. } => {
                    FactKind::Other("api_schema_declared".to_string())
                }
                GraphQlDeclarationKind::Fragment { .. } => {
                    FactKind::Other("graphql_fragment_declared".to_string())
                }
                GraphQlDeclarationKind::Directive { .. } => {
                    FactKind::Other("graphql_directive_declared".to_string())
                }
            };
            let fact_scope = match &declaration.kind {
                GraphQlDeclarationKind::Operation { .. } => "graphql_operation",
                GraphQlDeclarationKind::Schema { .. } => "graphql_schema",
                GraphQlDeclarationKind::Fragment { .. } => "graphql_fragment",
                GraphQlDeclarationKind::Directive { .. } => "graphql_directive",
            };

            facts.push(declaration_fact(
                self.name(),
                &input,
                &entity,
                &file_id,
                fact_kind,
                fact_scope,
                declaration.line,
            ));
            entities.push(entity);
        }

        Ok(ExtractOutput {
            entities,
            facts,
            diagnostics,
        })
    }
}

#[derive(Debug, Clone)]
enum GraphQlDeclarationKind<'a> {
    Operation { operation_type: &'a str },
    Schema { schema_kind: &'a str },
    Fragment { type_condition: String },
    Directive { locations: Vec<String> },
}

#[derive(Debug, Clone)]
struct GraphQlDeclaration<'a> {
    kind: GraphQlDeclarationKind<'a>,
    name: String,
    line: u32,
    variables: Vec<String>,
    variable_definitions: Vec<VariableDefinition>,
    variable_references: Vec<String>,
    fields: Vec<String>,
    arguments: Vec<String>,
    argument_definitions: Vec<ArgumentDefinition>,
    fragment_spreads: Vec<String>,
    inline_type_conditions: Vec<String>,
    directives: Vec<String>,
    deprecation_reason: Option<String>,
    deprecated_members: Vec<DeprecatedMember>,
    member_types: Vec<MemberType>,
    member_argument_definitions: Vec<MemberArgumentDefinitions>,
    member_directives: Vec<MemberDirectives>,
}

#[derive(Debug, Clone)]
struct VariableDefinition {
    name: String,
    type_name: String,
}

#[derive(Debug, Clone)]
struct ArgumentDefinition {
    name: String,
    type_name: String,
}

#[derive(Debug, Clone)]
struct IntrospectionType {
    name: String,
    schema_kind: String,
    fields: Vec<String>,
    arguments: Vec<String>,
    argument_definitions: Vec<MemberArgumentDefinitions>,
    deprecated_members: Vec<DeprecatedMember>,
    member_types: Vec<MemberType>,
}

#[derive(Debug, Clone, Default)]
struct IntrospectionDocument {
    root_schema: Option<IntrospectionType>,
    schema_types: Vec<IntrospectionType>,
    directives: Vec<IntrospectionDirective>,
}

#[derive(Debug, Clone)]
struct IntrospectionDirective {
    name: String,
    locations: Vec<String>,
    arguments: Vec<String>,
    argument_definitions: Vec<ArgumentDefinition>,
}

#[derive(Debug, Clone)]
struct DeprecatedMember {
    name: String,
    reason: Option<String>,
}

#[derive(Debug, Clone)]
struct MemberType {
    name: String,
    type_name: String,
}

#[derive(Debug, Clone)]
struct MemberArgumentDefinitions {
    member: String,
    arguments: Vec<ArgumentDefinition>,
}

#[derive(Debug, Clone)]
struct MemberDirectives {
    name: String,
    directives: Vec<String>,
    directive_args: Vec<Vec<String>>,
}

fn is_graphql_introspection_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".graphql.json")
        || lower.ends_with(".graphqls.json")
        || lower.ends_with(".gql.json")
        || lower.ends_with("graphql.schema.json")
        || lower.ends_with("graphql-introspection.json")
        || lower.ends_with("introspection.json")
}

fn has_graphql_introspection_marker(content: &str) -> bool {
    content.contains("\"__schema\"")
        && (content.contains("\"types\"")
            || content.contains("\"directives\"")
            || content.contains("\"queryType\"")
            || content.contains("\"mutationType\"")
            || content.contains("\"subscriptionType\""))
}

fn is_graphql_introspection_source(source: &SourceFile, content: &str) -> bool {
    is_graphql_introspection_path(&source.path)
        || (matches!(source.language_hint.as_deref(), Some("json"))
            && has_graphql_introspection_marker(content))
}

fn parse_introspection_document(
    content: &str,
    path: &str,
) -> Result<IntrospectionDocument, String> {
    let value: Value = serde_json::from_str(content)
        .map_err(|error| format!("failed to parse GraphQL introspection JSON {path}: {error}"))?;

    let Some(schema) = value
        .get("data")
        .and_then(|data| data.get("__schema"))
        .or_else(|| value.get("__schema"))
    else {
        return Ok(IntrospectionDocument::default());
    };

    let schema_types = schema
        .get("types")
        .and_then(Value::as_array)
        .map(|types| parse_introspection_types(types))
        .unwrap_or_default();
    let root_schema = parse_introspection_root_schema(schema);
    let directives = schema
        .get("directives")
        .and_then(Value::as_array)
        .map(|directives| parse_introspection_directives(directives))
        .unwrap_or_default();

    Ok(IntrospectionDocument {
        root_schema,
        schema_types,
        directives,
    })
}

fn parse_introspection_root_schema(schema: &Value) -> Option<IntrospectionType> {
    let mut fields = Vec::new();
    for (field, key) in [
        ("query", "queryType"),
        ("mutation", "mutationType"),
        ("subscription", "subscriptionType"),
    ] {
        if schema
            .get(key)
            .and_then(|root| root.get("name"))
            .and_then(Value::as_str)
            .is_some_and(|name| !name.is_empty() && !name.starts_with("__"))
        {
            fields.push(field.to_string());
        }
    }
    if fields.is_empty() {
        return None;
    }
    Some(IntrospectionType {
        name: "schema".to_string(),
        schema_kind: "schema".to_string(),
        fields,
        arguments: Vec::new(),
        argument_definitions: Vec::new(),
        deprecated_members: Vec::new(),
        member_types: Vec::new(),
    })
}

fn parse_introspection_types(types: &[Value]) -> Vec<IntrospectionType> {
    let mut output = Vec::new();
    for schema_type in types {
        let Some(name) = schema_type.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.starts_with("__") {
            continue;
        }
        let Some(kind) = schema_type.get("kind").and_then(Value::as_str) else {
            continue;
        };
        let schema_kind = kind.to_ascii_lowercase();
        if !matches!(
            schema_kind.as_str(),
            "object" | "input_object" | "interface" | "enum" | "scalar" | "union"
        ) {
            continue;
        }
        output.push(IntrospectionType {
            name: name.to_string(),
            schema_kind,
            fields: introspection_member_names(schema_type),
            arguments: introspection_argument_names(schema_type),
            argument_definitions: introspection_member_argument_definitions(schema_type),
            deprecated_members: introspection_deprecated_members(schema_type),
            member_types: introspection_member_types(schema_type),
        });
    }
    output
}

fn parse_introspection_directives(directives: &[Value]) -> Vec<IntrospectionDirective> {
    directives
        .iter()
        .filter_map(|directive| {
            let name = directive.get("name").and_then(Value::as_str)?;
            if name.is_empty() || name.starts_with("__") {
                return None;
            }
            Some(IntrospectionDirective {
                name: name.to_string(),
                locations: introspection_string_array(directive, &["locations"]),
                arguments: introspection_named_array(directive, &["args", "arguments"]),
                argument_definitions: introspection_argument_definitions(directive),
            })
        })
        .take(128)
        .collect()
}

fn introspection_string_array(value: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|name| !name.is_empty())
                .take(128)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn introspection_named_array(value: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .filter(|name| !name.is_empty())
                .take(128)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn introspection_member_arrays(schema_type: &Value) -> Vec<&Vec<Value>> {
    let mut arrays = Vec::new();
    for key in [
        "fields",
        "inputFields",
        "input_fields",
        "enumValues",
        "enum_values",
        "possibleTypes",
        "possible_types",
    ] {
        if let Some(members) = schema_type.get(key).and_then(Value::as_array) {
            arrays.push(members);
        }
    }
    arrays
}

fn introspection_field_arrays(schema_type: &Value) -> Vec<&Vec<Value>> {
    let mut arrays = Vec::new();
    for key in ["fields", "inputFields", "input_fields"] {
        if let Some(members) = schema_type.get(key).and_then(Value::as_array) {
            arrays.push(members);
        }
    }
    arrays
}

fn introspection_member_names(schema_type: &Value) -> Vec<String> {
    unique_bounded_names(
        introspection_member_arrays(schema_type)
            .into_iter()
            .flat_map(|members| members.iter())
            .filter_map(|member| member.get("name").and_then(Value::as_str)),
        128,
    )
}

fn introspection_argument_names(schema_type: &Value) -> Vec<String> {
    unique_bounded_names(
        introspection_field_arrays(schema_type)
            .into_iter()
            .flat_map(|members| members.iter())
            .flat_map(|member| introspection_named_array(member, &["args", "arguments"])),
        128,
    )
}

fn introspection_argument_definitions(value: &Value) -> Vec<ArgumentDefinition> {
    ["args", "arguments"]
        .iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name").and_then(Value::as_str)?;
                    let type_name = introspection_type_ref_name(item.get("type")?)?;
                    Some(ArgumentDefinition {
                        name: name.to_string(),
                        type_name,
                    })
                })
                .take(64)
                .collect()
        })
        .unwrap_or_default()
}

fn introspection_member_argument_definitions(
    schema_type: &Value,
) -> Vec<MemberArgumentDefinitions> {
    introspection_field_arrays(schema_type)
        .into_iter()
        .flat_map(|members| members.iter())
        .filter_map(|member| {
            let member_name = member.get("name").and_then(Value::as_str)?;
            let arguments = introspection_argument_definitions(member);
            if arguments.is_empty() {
                return None;
            }
            Some(MemberArgumentDefinitions {
                member: member_name.to_string(),
                arguments,
            })
        })
        .take(128)
        .collect()
}

fn introspection_member_types(schema_type: &Value) -> Vec<MemberType> {
    introspection_field_arrays(schema_type)
        .into_iter()
        .flat_map(|members| members.iter())
        .filter_map(|member| {
            let name = member.get("name").and_then(Value::as_str)?;
            let type_name = introspection_type_ref_name(member.get("type")?)?;
            Some(MemberType {
                name: name.to_string(),
                type_name,
            })
        })
        .take(128)
        .collect()
}

fn introspection_type_ref_name(type_ref: &Value) -> Option<String> {
    let kind = type_ref.get("kind").and_then(Value::as_str)?;
    match kind {
        "NON_NULL" => {
            let inner = introspection_type_ref_name(type_ref.get("ofType")?)?;
            Some(format!("{inner}!"))
        }
        "LIST" => {
            let inner = introspection_type_ref_name(type_ref.get("ofType")?)?;
            Some(format!("[{inner}]"))
        }
        _ => type_ref
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
    }
}

fn unique_bounded_names<I, S>(names: I, limit: usize) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut output = Vec::new();
    for name in names {
        let name = name.as_ref();
        if !name.is_empty() && !output.iter().any(|existing| existing == name) {
            output.push(name.to_string());
            if output.len() >= limit {
                break;
            }
        }
    }
    output
}

fn introspection_deprecated_members(schema_type: &Value) -> Vec<DeprecatedMember> {
    introspection_field_arrays(schema_type)
        .into_iter()
        .flat_map(|members| members.iter())
        .filter(|member| {
            member
                .get("isDeprecated")
                .or_else(|| member.get("is_deprecated"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|member| {
            let name = member.get("name").and_then(Value::as_str)?;
            Some(DeprecatedMember {
                name: name.to_string(),
                reason: member
                    .get("deprecationReason")
                    .or_else(|| member.get("deprecation_reason"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .take(128)
        .collect()
}

fn parse_graphql_declarations(content: &str) -> Vec<GraphQlDeclaration<'_>> {
    let mut declarations = Vec::new();
    let mut current_body: Option<BodyCapture> = None;

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = (index + 1) as u32;
        let line = strip_graphql_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(body) = current_body.as_mut() {
            body.capture_line(line);
            if body.is_complete() {
                if let Some(mut declaration) = declarations.pop() {
                    apply_body_capture(&mut declaration, body);
                    declarations.push(declaration);
                }
                current_body = None;
            }
            continue;
        }

        if let Some((operation_type, name)) = parse_operation_header(line) {
            declarations.push(GraphQlDeclaration {
                kind: GraphQlDeclarationKind::Operation { operation_type },
                name: name
                    .unwrap_or_else(|| anonymous_operation_name(operation_type, line_number, line)),
                line: line_number,
                variables: parse_operation_variables(line),
                variable_definitions: parse_operation_variable_definitions(line),
                variable_references: Vec::new(),
                fields: Vec::new(),
                arguments: parse_operation_arguments(line),
                argument_definitions: Vec::new(),
                fragment_spreads: Vec::new(),
                inline_type_conditions: Vec::new(),
                directives: graphql_directive_names(line),
                deprecation_reason: None,
                deprecated_members: Vec::new(),
                member_types: Vec::new(),
                member_argument_definitions: Vec::new(),
                member_directives: Vec::new(),
            });
            start_body_capture(line, &mut declarations, &mut current_body);
            continue;
        }

        if let Some((name, locations, arguments)) = parse_directive_header(line) {
            declarations.push(GraphQlDeclaration {
                kind: GraphQlDeclarationKind::Directive { locations },
                name,
                line: line_number,
                variables: Vec::new(),
                variable_definitions: Vec::new(),
                variable_references: Vec::new(),
                fields: Vec::new(),
                arguments,
                argument_definitions: graphql_argument_definitions(line),
                fragment_spreads: Vec::new(),
                inline_type_conditions: Vec::new(),
                directives: Vec::new(),
                deprecation_reason: None,
                deprecated_members: Vec::new(),
                member_types: Vec::new(),
                member_argument_definitions: Vec::new(),
                member_directives: Vec::new(),
            });
            continue;
        }

        if let Some((schema_kind, name)) = parse_schema_header(line) {
            declarations.push(GraphQlDeclaration {
                kind: GraphQlDeclarationKind::Schema { schema_kind },
                name: name.to_string(),
                line: line_number,
                variables: Vec::new(),
                variable_definitions: Vec::new(),
                variable_references: Vec::new(),
                fields: schema_root_fields(schema_kind, line),
                arguments: Vec::new(),
                argument_definitions: Vec::new(),
                fragment_spreads: Vec::new(),
                inline_type_conditions: Vec::new(),
                directives: graphql_directive_names(line),
                deprecation_reason: parse_deprecation_reason(line),
                deprecated_members: Vec::new(),
                member_types: Vec::new(),
                member_argument_definitions: Vec::new(),
                member_directives: Vec::new(),
            });
            start_body_capture(line, &mut declarations, &mut current_body);
            continue;
        }

        if let Some((name, type_condition)) = parse_fragment_header(line) {
            declarations.push(GraphQlDeclaration {
                kind: GraphQlDeclarationKind::Fragment { type_condition },
                name,
                line: line_number,
                variables: Vec::new(),
                variable_definitions: Vec::new(),
                variable_references: Vec::new(),
                fields: Vec::new(),
                arguments: Vec::new(),
                argument_definitions: Vec::new(),
                fragment_spreads: Vec::new(),
                inline_type_conditions: Vec::new(),
                directives: graphql_directive_names(line),
                deprecation_reason: None,
                deprecated_members: Vec::new(),
                member_types: Vec::new(),
                member_argument_definitions: Vec::new(),
                member_directives: Vec::new(),
            });
            start_body_capture(line, &mut declarations, &mut current_body);
        }
    }

    declarations
}

fn validate_graphql_declarations(
    extractor: &str,
    input: &ExtractInput,
    declarations: &[GraphQlDeclaration<'_>],
    path: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let declared_fragments: Vec<&str> = declarations
        .iter()
        .filter_map(|decl| match &decl.kind {
            GraphQlDeclarationKind::Fragment { .. } => Some(decl.name.as_str()),
            _ => None,
        })
        .collect();

    let declared_schema_types: Vec<&str> = declarations
        .iter()
        .filter_map(|decl| match &decl.kind {
            GraphQlDeclarationKind::Schema { .. } => Some(decl.name.as_str()),
            _ => None,
        })
        .collect();

    let schema_type_names: std::collections::HashSet<&str> =
        declared_schema_types.iter().copied().collect();

    let directive_arg_defs: HashMap<&str, Vec<&str>> = declarations
        .iter()
        .filter_map(|decl| match &decl.kind {
            GraphQlDeclarationKind::Directive { .. } => Some((
                decl.name.as_str(),
                decl.argument_definitions
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect(),
            )),
            _ => None,
        })
        .collect();

    let deprecated_fields: Vec<(&str, &str)> = declarations
        .iter()
        .filter(|decl| matches!(decl.kind, GraphQlDeclarationKind::Schema { .. }))
        .flat_map(|decl| {
            decl.deprecated_members
                .iter()
                .map(move |dm| (decl.name.as_str(), dm.name.as_str()))
        })
        .collect();

    let declared_directives: HashMap<&str, Vec<&str>> = declarations
        .iter()
        .filter_map(|decl| match &decl.kind {
            GraphQlDeclarationKind::Directive { locations } => Some((
                decl.name.as_str(),
                locations.iter().map(|s| s.as_str()).collect(),
            )),
            _ => None,
        })
        .collect();

    for declaration in declarations {
        for spread in &declaration.fragment_spreads {
            if !declared_fragments.contains(&spread.as_str()) {
                let identity = format!(
                    "{}\0{}\0graphql_unresolved_fragment_spread\0{}\0{path}",
                    extractor, input.source.path, spread
                );
                let hash = stable_hash(identity.as_bytes());
                diagnostics.push(Diagnostic {
                    id: DiagnosticId(format!("diag_graphql_unresolved_fragment_spread_{hash:016x}")),
                    kind: DiagnosticKind::Other("graphql_unresolved_fragment_spread".to_string()),
                    severity: Severity::Medium,
                    status: DiagnosticStatus::Open,
                    title: "Unresolved GraphQL fragment spread".to_string(),
                    message: format!(
                        "Fragment spread `...{spread}` does not reference a fragment declared in this file"
                    ),
                    entities: Vec::new(),
                    evidence: vec![evidence_for_file(path, extractor, Some(declaration.line), Some(declaration.line))],
                    ownership: ownership_for_file(path),
                    snapshot: input.snapshot.clone(),
                    suggested_fix: Some(format!(
                        "Declare a fragment named `{spread}` in this file or correct the spread name"
                    )),
                    payload: json!({
                        "spread_name": spread,
                        "source_kind": "graphql_sdl_or_operation",
                        "declared_fragments": declared_fragments,
                    }),
                });
            }
        }

        for type_condition in &declaration.inline_type_conditions {
            if !declared_schema_types.contains(&type_condition.as_str()) {
                let identity = format!(
                    "{}\0{}\0graphql_unresolved_type_condition\0{}\0{path}",
                    extractor, input.source.path, type_condition
                );
                let hash = stable_hash(identity.as_bytes());
                diagnostics.push(Diagnostic {
                    id: DiagnosticId(format!("diag_graphql_unresolved_type_condition_{hash:016x}")),
                    kind: DiagnosticKind::Other("graphql_unresolved_type_condition".to_string()),
                    severity: Severity::Medium,
                    status: DiagnosticStatus::Open,
                    title: "Unresolved GraphQL inline type condition".to_string(),
                    message: format!(
                        "Inline fragment type condition `... on {type_condition}` does not reference a type declared in this file"
                    ),
                    entities: Vec::new(),
                    evidence: vec![evidence_for_file(path, extractor, Some(declaration.line), Some(declaration.line))],
                    ownership: ownership_for_file(path),
                    snapshot: input.snapshot.clone(),
                    suggested_fix: Some(format!(
                        "Declare type `{type_condition}` in this file or correct the type condition"
                    )),
                    payload: json!({
                        "type_condition": type_condition,
                        "source_kind": "graphql_sdl_or_operation",
                        "declared_schema_types": declared_schema_types,
                    }),
                });
            }
        }

        if matches!(declaration.kind, GraphQlDeclarationKind::Operation { .. }) {
            for field in &declaration.fields {
                if let Some((type_name, reason)) = deprecated_fields
                    .iter()
                    .find(|(_, member)| *member == field.as_str())
                {
                    let deprecated_reason = reason.to_string();
                    let identity = format!(
                        "{}\0{}\0graphql_deprecated_field_used\0{}\0{path}",
                        extractor, input.source.path, field
                    );
                    let hash = stable_hash(identity.as_bytes());
                    diagnostics.push(Diagnostic {
                        id: DiagnosticId(format!("diag_graphql_deprecated_field_used_{hash:016x}")),
                        kind: DiagnosticKind::Other("graphql_deprecated_field_used".to_string()),
                        severity: Severity::Low,
                        status: DiagnosticStatus::Open,
                        title: "Deprecated GraphQL field used in operation".to_string(),
                        message: format!(
                            "Operation `{}` selects deprecated field `{field}` from type `{type_name}`",
                            declaration.name
                        ),
                        entities: Vec::new(),
                        evidence: vec![evidence_for_file(path, extractor, Some(declaration.line), Some(declaration.line))],
                        ownership: ownership_for_file(path),
                        snapshot: input.snapshot.clone(),
                        suggested_fix: Some(format!(
                            "Replace `{field}` with a non-deprecated alternative: {deprecated_reason}"
                        )),
                        payload: json!({
                            "field_name": field,
                            "type_name": type_name,
                            "deprecation_reason": deprecated_reason,
                            "operation_name": declaration.name,
                            "source_kind": "graphql_sdl_or_operation",
                        }),
                    });
                }
            }
        }

        if let Some(location) = declaration_location(declaration) {
            for directive in &declaration.directives {
                if !directive_allowed_at(directive, location, &declared_directives) {
                    let identity = format!(
                        "{}\0{}\0graphql_invalid_directive_location\0{}\0{}\0{path}",
                        extractor, input.source.path, declaration.name, directive
                    );
                    let hash = stable_hash(identity.as_bytes());
                    diagnostics.push(Diagnostic {
                        id: DiagnosticId(format!("diag_graphql_invalid_directive_location_{hash:016x}")),
                        kind: DiagnosticKind::Other("graphql_invalid_directive_location".to_string()),
                        severity: Severity::Medium,
                        status: DiagnosticStatus::Open,
                        title: "GraphQL directive used at invalid location".to_string(),
                        message: format!(
                            "Directive `@{directive}` is used on `{}` but its declared location is {location}",
                            declaration.name
                        ),
                        entities: Vec::new(),
                        evidence: vec![evidence_for_file(path, extractor, Some(declaration.line), Some(declaration.line))],
                        ownership: ownership_for_file(path),
                        snapshot: input.snapshot.clone(),
                        suggested_fix: Some(format!(
                            "Remove `@{directive}` from `{}` or update the directive's location declaration in the SDL",
                            declaration.name
                        )),
                        payload: json!({
                            "directive_name": directive,
                            "declaration_name": declaration.name,
                            "location": location,
                            "source_kind": "graphql_sdl_or_operation",
                        }),
                    });
                }
            }

            if let Some(member_directives) = declaration_to_member_directives(declaration) {
                for member in member_directives {
                    for (di, directive) in member.directives.iter().enumerate() {
                        if !directive_allowed_at(
                            directive,
                            "FIELD_DEFINITION",
                            &declared_directives,
                        ) {
                            let identity = format!(
                                "{}\0{}\0graphql_invalid_directive_location\0{}@{}\0{path}",
                                extractor, input.source.path, declaration.name, member.name
                            );
                            let hash = stable_hash(identity.as_bytes());
                            diagnostics.push(Diagnostic {
                                id: DiagnosticId(format!("diag_graphql_invalid_directive_location_{hash:016x}")),
                                kind: DiagnosticKind::Other("graphql_invalid_directive_location".to_string()),
                                severity: Severity::Medium,
                                status: DiagnosticStatus::Open,
                                title: "GraphQL directive used at invalid location".to_string(),
                                message: format!(
                                    "Directive `@{directive}` is used on field `{}` of `{}` but its declared location is FIELD_DEFINITION",
                                    member.name, declaration.name
                                ),
                                entities: Vec::new(),
                                evidence: vec![evidence_for_file(path, extractor, Some(declaration.line), Some(declaration.line))],
                                ownership: ownership_for_file(path),
                                snapshot: input.snapshot.clone(),
                                suggested_fix: Some(format!(
                                    "Remove `@{directive}` from field `{}` of `{}` or update the directive's location declaration",
                                    member.name, declaration.name
                                )),
                                payload: json!({
                                    "directive_name": directive,
                                    "declaration_name": declaration.name,
                                    "member_name": member.name,
                                    "location": "FIELD_DEFINITION",
                                    "source_kind": "graphql_sdl_or_operation",
                                }),
                            });
                        }

                        if let Some(defined_arg_names) = directive_arg_defs.get(directive.as_str())
                            && let Some(applied_arg_names) = member.directive_args.get(di)
                        {
                            for arg_name in applied_arg_names {
                                if !defined_arg_names.contains(&arg_name.as_str()) {
                                    let identity = format!(
                                        "{}\0{}\0graphql_invalid_directive_argument\0{}@{}:{}\0{path}",
                                        extractor,
                                        input.source.path,
                                        declaration.name,
                                        directive,
                                        arg_name
                                    );
                                    let hash = stable_hash(identity.as_bytes());
                                    diagnostics.push(Diagnostic {
                                        id: DiagnosticId(format!(
                                            "diag_graphql_invalid_directive_argument_{hash:016x}"
                                        )),
                                        kind: DiagnosticKind::Other(
                                            "graphql_invalid_directive_argument".to_string(),
                                        ),
                                        severity: Severity::Medium,
                                        status: DiagnosticStatus::Open,
                                        title: "GraphQL directive has unknown argument".to_string(),
                                        message: format!(
                                            "Directive `@{directive}` on `{}` of `{}` has argument `{arg_name}` which is not defined in the directive declaration",
                                            member.name, declaration.name
                                        ),
                                        entities: Vec::new(),
                                        evidence: vec![evidence_for_file(
                                            path,
                                            extractor,
                                            Some(declaration.line),
                                            Some(declaration.line),
                                        )],
                                        ownership: ownership_for_file(path),
                                        snapshot: input.snapshot.clone(),
                                        suggested_fix: Some(format!(
                                            "Remove argument `{arg_name}` from `@{directive}` or add it to the directive's declaration"
                                        )),
                                        payload: json!({
                                            "directive_name": directive,
                                            "argument_name": arg_name,
                                            "declaration_name": declaration.name,
                                            "member_name": member.name,
                                            "source_kind": "graphql_sdl_or_operation",
                                        }),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if matches!(declaration.kind, GraphQlDeclarationKind::Operation { .. }) {
            for var_def in &declaration.variable_definitions {
                if !is_valid_graphql_type_syntax(&var_def.type_name) {
                    let identity = format!(
                        "{}\0{}\0graphql_invalid_variable_type\0{}\0{}\0{path}",
                        extractor, input.source.path, declaration.name, var_def.type_name
                    );
                    let hash = stable_hash(identity.as_bytes());
                    diagnostics.push(Diagnostic {
                        id: DiagnosticId(format!("diag_graphql_invalid_variable_type_{hash:016x}")),
                        kind: DiagnosticKind::Other("graphql_invalid_variable_type".to_string()),
                        severity: Severity::Medium,
                        status: DiagnosticStatus::Open,
                        title: "Invalid GraphQL variable type syntax".to_string(),
                        message: format!(
                            "Variable `${}` in operation `{}` has invalid type syntax `{}`",
                            var_def.name, declaration.name, var_def.type_name
                        ),
                        entities: Vec::new(),
                        evidence: vec![evidence_for_file(
                            path,
                            extractor,
                            Some(declaration.line),
                            Some(declaration.line),
                        )],
                        ownership: ownership_for_file(path),
                        snapshot: input.snapshot.clone(),
                        suggested_fix: Some(format!(
                            "Fix the type syntax for `${}`. Valid examples: `ID`, `String!`, `[Int]`, `[User!]!`",
                            var_def.name
                        )),
                        payload: json!({
                            "variable_name": var_def.name,
                            "type_name": var_def.type_name,
                            "operation_name": declaration.name,
                            "source_kind": "graphql_sdl_or_operation",
                        }),
                    });
                }
            }
        }

        if matches!(declaration.kind, GraphQlDeclarationKind::Operation { .. }) {
            let declared_var_names: Vec<&str> = declaration
                .variable_definitions
                .iter()
                .map(|v| v.name.as_str())
                .collect();

            for var_ref in &declaration.variable_references {
                if !declared_var_names.contains(&var_ref.as_str()) {
                    let identity = format!(
                        "{}\0{}\0graphql_undeclared_variable_reference\0{}\0{var_ref}\0{path}",
                        extractor, input.source.path, declaration.name
                    );
                    let hash = stable_hash(identity.as_bytes());
                    diagnostics.push(Diagnostic {
                        id: DiagnosticId(format!(
                            "diag_graphql_undeclared_variable_reference_{hash:016x}"
                        )),
                        kind: DiagnosticKind::Other(
                            "graphql_undeclared_variable_reference".to_string(),
                        ),
                        severity: Severity::Medium,
                        status: DiagnosticStatus::Open,
                        title: "Undeclared GraphQL variable reference".to_string(),
                        message: format!(
                            "Variable `${var_ref}` is used in operation `{}` but not declared in the operation's variable definitions",
                            declaration.name
                        ),
                        entities: Vec::new(),
                        evidence: vec![evidence_for_file(
                            path,
                            extractor,
                            Some(declaration.line),
                            Some(declaration.line),
                        )],
                        ownership: ownership_for_file(path),
                        snapshot: input.snapshot.clone(),
                        suggested_fix: Some(format!(
                            "Add `${var_ref}: <Type>` to the operation's variable definitions or remove the usage of `${var_ref}`"
                        )),
                        payload: json!({
                            "variable_name": var_ref,
                            "operation_name": declaration.name,
                            "declared_variables": declared_var_names,
                            "source_kind": "graphql_sdl_or_operation",
                        }),
                    });
                }
            }

            for declared_var in &declared_var_names {
                if !declaration
                    .variable_references
                    .iter()
                    .any(|r| r == declared_var)
                {
                    let identity = format!(
                        "{}\0{}\0graphql_unused_variable\0{}\0{declared_var}\0{path}",
                        extractor, input.source.path, declaration.name
                    );
                    let hash = stable_hash(identity.as_bytes());
                    diagnostics.push(Diagnostic {
                        id: DiagnosticId(format!("diag_graphql_unused_variable_{hash:016x}")),
                        kind: DiagnosticKind::Other("graphql_unused_variable".to_string()),
                        severity: Severity::Low,
                        status: DiagnosticStatus::Open,
                        title: "Unused GraphQL variable".to_string(),
                        message: format!(
                            "Variable `${declared_var}` is declared in operation `{}` but never used in the operation body",
                            declaration.name
                        ),
                        entities: Vec::new(),
                        evidence: vec![evidence_for_file(
                            path,
                            extractor,
                            Some(declaration.line),
                            Some(declaration.line),
                        )],
                        ownership: ownership_for_file(path),
                        snapshot: input.snapshot.clone(),
                        suggested_fix: Some(format!(
                            "Remove `${declared_var}` from the operation's variable definitions or use it in the operation body"
                        )),
                        payload: json!({
                            "variable_name": declared_var,
                            "operation_name": declaration.name,
                            "declared_variables": declared_var_names,
                            "source_kind": "graphql_sdl_or_operation",
                        }),
                    });
                }
            }

            for var_def in &declaration.variable_definitions {
                if !var_def.type_name.is_empty()
                    && !graphql_type_exists(&var_def.type_name, &schema_type_names)
                {
                    let identity = format!(
                        "{}\0{}\0graphql_variable_type_not_found\0{}\0{}\0{path}",
                        extractor, input.source.path, declaration.name, var_def.type_name
                    );
                    let hash = stable_hash(identity.as_bytes());
                    diagnostics.push(Diagnostic {
                        id: DiagnosticId(format!(
                            "diag_graphql_variable_type_not_found_{hash:016x}"
                        )),
                        kind: DiagnosticKind::Other("graphql_variable_type_not_found".to_string()),
                        severity: Severity::Medium,
                        status: DiagnosticStatus::Open,
                        title: "GraphQL variable references undeclared type".to_string(),
                        message: format!(
                            "Variable `${}` in operation `{}` has type `{}` which is not a declared schema type or built-in scalar",
                            var_def.name, declaration.name, var_def.type_name
                        ),
                        entities: Vec::new(),
                        evidence: vec![evidence_for_file(
                            path,
                            extractor,
                            Some(declaration.line),
                            Some(declaration.line),
                        )],
                        ownership: ownership_for_file(path),
                        snapshot: input.snapshot.clone(),
                        suggested_fix: Some(format!(
                            "Declare type `{}` in the schema or use a built-in scalar (Int, Float, String, Boolean, ID)",
                            extract_base_type_name(&var_def.type_name)
                        )),
                        payload: json!({
                            "variable_name": var_def.name,
                            "type_name": var_def.type_name,
                            "operation_name": declaration.name,
                            "source_kind": "graphql_sdl_or_operation",
                        }),
                    });
                }
            }
        }
    }

    diagnostics
}

fn declaration_location(declaration: &GraphQlDeclaration<'_>) -> Option<&'static str> {
    match &declaration.kind {
        GraphQlDeclarationKind::Operation { operation_type } => match *operation_type {
            "query" => Some("QUERY"),
            "mutation" => Some("MUTATION"),
            "subscription" => Some("SUBSCRIPTION"),
            _ => None,
        },
        GraphQlDeclarationKind::Schema { schema_kind } => match *schema_kind {
            "type" => Some("OBJECT"),
            "interface" => Some("INTERFACE"),
            "input" => Some("INPUT_OBJECT"),
            "enum" => Some("ENUM"),
            "scalar" => Some("SCALAR"),
            "union" => Some("UNION"),
            _ => None,
        },
        GraphQlDeclarationKind::Fragment { .. } => Some("FRAGMENT_DEFINITION"),
        GraphQlDeclarationKind::Directive { .. } => None,
    }
}

fn directive_allowed_at(
    directive: &str,
    location: &str,
    declared_directives: &HashMap<&str, Vec<&str>>,
) -> bool {
    match directive {
        "skip" | "include" => ["FIELD", "FRAGMENT_SPREAD", "INLINE_FRAGMENT"].contains(&location),
        "deprecated" => ["FIELD_DEFINITION", "ENUM_VALUE"].contains(&location),
        "specifiedBy" => location == "SCALAR",
        _ => declared_directives
            .get(directive)
            .is_none_or(|locations| locations.contains(&location)),
    }
}

fn declaration_to_member_directives<'a>(
    declaration: &'a GraphQlDeclaration<'a>,
) -> Option<&'a [MemberDirectives]> {
    if declaration.member_directives.is_empty() {
        return None;
    }
    Some(&declaration.member_directives)
}

fn start_body_capture<'a>(
    line: &str,
    declarations: &mut Vec<GraphQlDeclaration<'a>>,
    current_body: &mut Option<BodyCapture>,
) {
    let Some(body) = BodyCapture::from_header(line) else {
        return;
    };
    if body.is_complete() {
        if let Some(mut declaration) = declarations.pop() {
            apply_body_capture(&mut declaration, &body);
            declarations.push(declaration);
        }
    } else {
        *current_body = Some(body);
    }
}

fn apply_body_capture(declaration: &mut GraphQlDeclaration<'_>, body: &BodyCapture) {
    if !body.fields.is_empty() {
        declaration.fields = body.fields.clone();
    }
    declaration.arguments = body.arguments.clone();
    declaration.variable_references = body.variable_references.clone();
    declaration.fragment_spreads = body.fragment_spreads.clone();
    declaration.inline_type_conditions = body.inline_type_conditions.clone();
    declaration.deprecated_members = body.deprecated_members.clone();
    declaration.member_types = body.member_types.clone();
    declaration.member_directives = body.member_directives.clone();
}

fn parse_operation_header(line: &str) -> Option<(&'static str, Option<String>)> {
    for operation_type in ["query", "mutation", "subscription"] {
        let Some(rest) = keyword_rest(line, operation_type) else {
            continue;
        };
        let name = leading_graphql_name(rest)
            .filter(|name| !name.is_empty() && *name != "{")
            .map(str::to_string);
        return Some((operation_type, name));
    }
    None
}

fn parse_operation_variables(line: &str) -> Vec<String> {
    let before_body = line.split_once('{').map_or(line, |(before, _)| before);
    let Some((_, after_open)) = before_body.split_once('(') else {
        return Vec::new();
    };
    let Some((variables, _)) = after_open.rsplit_once(')') else {
        return Vec::new();
    };
    variables
        .split('$')
        .skip(1)
        .filter_map(leading_graphql_name)
        .filter(|name| !name.is_empty())
        .take(64)
        .map(str::to_string)
        .collect()
}

fn parse_operation_variable_definitions(line: &str) -> Vec<VariableDefinition> {
    let before_body = line.split_once('{').map_or(line, |(before, _)| before);
    let Some((_, after_open)) = before_body.split_once('(') else {
        return Vec::new();
    };
    let Some((variables, _)) = after_open.rsplit_once(')') else {
        return Vec::new();
    };
    variables
        .split(',')
        .filter_map(|variable| {
            let variable = variable.trim().strip_prefix('$')?;
            let (name, after_name) = variable.split_once(':')?;
            let name = leading_graphql_name(name)?;
            let type_name = graphql_variable_type_name(after_name)?;
            Some(VariableDefinition {
                name: name.to_string(),
                type_name,
            })
        })
        .take(64)
        .collect()
}

fn graphql_variable_type_name(input: &str) -> Option<String> {
    let mut output = String::new();
    for ch in input.trim_start().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '!' || ch == '[' || ch == ']' {
            output.push(ch);
            continue;
        }
        break;
    }
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn is_valid_graphql_type_syntax(type_name: &str) -> bool {
    if type_name.is_empty() {
        return false;
    }
    let mut chars = type_name.chars().peekable();
    let mut bracket_depth = 0;
    let mut prev_was_bracket_close = false;

    while let Some(ch) = chars.next() {
        match ch {
            '[' => {
                bracket_depth += 1;
                prev_was_bracket_close = false;
            }
            ']' => {
                if bracket_depth == 0 {
                    return false;
                }
                bracket_depth -= 1;
                prev_was_bracket_close = true;
                if chars.peek() == Some(&'!') {
                    chars.next();
                }
            }
            '!' => {
                if prev_was_bracket_close {
                    prev_was_bracket_close = false;
                    continue;
                }
                if chars.peek().is_some() {
                    return false;
                }
            }
            _ => {
                if bracket_depth > 0 && !prev_was_bracket_close {
                    // Inside brackets, before base type: expect uppercase start
                    if !ch.is_ascii_uppercase() {
                        return false;
                    }
                    // Consume the rest of the type name
                    while chars
                        .peek()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
                    {
                        chars.next();
                    }
                    if chars.peek() == Some(&'!') {
                        chars.next();
                    }
                    prev_was_bracket_close = false;
                } else if bracket_depth == 0 && !prev_was_bracket_close {
                    // Top-level type: must start with uppercase
                    if !ch.is_ascii_uppercase() {
                        return false;
                    }
                    while chars
                        .peek()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
                    {
                        chars.next();
                    }
                    if chars.peek() == Some(&'!') {
                        chars.next();
                    }
                    prev_was_bracket_close = false;
                } else {
                    return false;
                }
            }
        }
    }
    bracket_depth == 0 && !type_name.ends_with('!')
        || (bracket_depth == 0 && type_name.ends_with('!') && !type_name.ends_with("!!"))
}

fn builtin_scalar_names() -> &'static [&'static str] {
    &["Int", "Float", "String", "Boolean", "ID"]
}

fn extract_base_type_name(type_syntax: &str) -> &str {
    let mut name = type_syntax;
    while let Some(rest) = name.strip_prefix('[') {
        name = rest;
    }
    let mut end = name.len();
    while end > 0 && (name.as_bytes()[end - 1] == b']' || name.as_bytes()[end - 1] == b'!') {
        end -= 1;
    }
    &name[..end]
}

fn graphql_type_exists(
    type_syntax: &str,
    declared_types: &std::collections::HashSet<&str>,
) -> bool {
    let base = extract_base_type_name(type_syntax);
    if base.is_empty() {
        return false;
    }
    let first_char = base.as_bytes()[0];
    if !first_char.is_ascii_uppercase() {
        return false;
    }
    builtin_scalar_names().contains(&base) || declared_types.contains(base)
}

fn is_graphql_body_keyword(name: &str) -> bool {
    matches!(
        name,
        "type"
            | "input"
            | "interface"
            | "enum"
            | "union"
            | "query"
            | "mutation"
            | "subscription"
            | "fragment"
            | "schema"
            | "on"
    )
}

fn strip_graphql_directives(line: &str) -> &str {
    line.split('@').next().unwrap_or(line)
}

fn schema_field_definition_prefix(line: &str) -> Option<&str> {
    let closing = line.find("):")?;
    Some(&line[..closing + 1])
}

fn graphql_field_argument_names(line: &str) -> Vec<String> {
    let line = strip_graphql_directives(line).trim();
    if let Some(schema_part) = schema_field_definition_prefix(line) {
        return graphql_argument_names(schema_part);
    }
    graphql_argument_names(line)
}

fn graphql_member_type_name(line: &str) -> Option<String> {
    let line = strip_graphql_directives(line).trim();
    let after_colon = if let Some(schema_part) = schema_field_definition_prefix(line) {
        line[schema_part.len()..].trim_start().strip_prefix(':')?
    } else {
        line.split_once(':')?.1.trim_start()
    };
    let mut output = String::new();
    for ch in after_colon.trim_start().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '!' || ch == '[' || ch == ']' {
            output.push(ch);
            continue;
        }
        break;
    }
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn parse_operation_arguments(line: &str) -> Vec<String> {
    let before_body = line.split_once('{').map_or(line, |(before, _)| before);
    graphql_argument_names(before_body)
}

fn graphql_argument_names(line: &str) -> Vec<String> {
    graphql_argument_definitions(line)
        .into_iter()
        .map(|definition| definition.name)
        .collect()
}

fn graphql_argument_definitions(line: &str) -> Vec<ArgumentDefinition> {
    let mut output = Vec::new();
    let mut rest = line;
    while let Some((_, after_open)) = rest.split_once('(') {
        let Some((inside, after_close)) = after_open.split_once(')') else {
            break;
        };
        for argument in inside.split(',') {
            let Some((name_part, type_part)) = argument.split_once(':') else {
                continue;
            };
            let name = name_part.trim().trim_start_matches('$');
            let Some(name) = leading_graphql_name(name) else {
                continue;
            };
            let type_name = match graphql_variable_type_name(type_part) {
                Some(type_name) => type_name,
                None => {
                    let trimmed = type_part.trim();
                    if trimmed.starts_with('$') {
                        trimmed.to_string()
                    } else {
                        continue;
                    }
                }
            };
            if !name.is_empty()
                && !output
                    .iter()
                    .any(|existing: &ArgumentDefinition| existing.name == name)
            {
                output.push(ArgumentDefinition {
                    name: name.to_string(),
                    type_name,
                });
                if output.len() >= 64 {
                    return output;
                }
            }
        }
        rest = after_close;
    }
    output
}

fn graphql_directive_names(line: &str) -> Vec<String> {
    line.split('@')
        .skip(1)
        .filter_map(|after_at| leading_graphql_name(after_at.trim_start()))
        .filter(|name| !name.is_empty())
        .take(64)
        .map(str::to_string)
        .fold(Vec::new(), |mut output, directive| {
            if !output.iter().any(|existing| existing == &directive) {
                output.push(directive);
            }
            output
        })
}

fn graphql_directive_applications(line: &str) -> Vec<(String, Vec<String>)> {
    line.split('@')
        .skip(1)
        .filter_map(|after_at| {
            let trimmed = after_at.trim_start();
            let name = leading_graphql_name(trimmed)?;
            let rest = &trimmed[name.len()..];
            let args = if let Some(after_open) = rest.trim_start().strip_prefix('(') {
                if let Some((inside, _)) = after_open.split_once(')') {
                    inside
                        .split(',')
                        .filter_map(|arg| {
                            let arg = arg.trim();
                            if arg.is_empty() {
                                return None;
                            }
                            let name = arg.split_once(':').map_or(arg, |(n, _)| n);
                            let name = name.trim();
                            if name.is_empty() || name.starts_with('$') {
                                None
                            } else {
                                Some(name.to_string())
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
            Some((name.to_string(), args))
        })
        .collect()
}

fn parse_schema_header(line: &str) -> Option<(&'static str, &str)> {
    let line = line.strip_prefix("extend ").unwrap_or(line);
    if keyword_rest(line, "schema").is_some() {
        return Some(("schema", "schema"));
    }
    for schema_kind in ["type", "input", "interface", "enum", "scalar", "union"] {
        let Some(rest) = keyword_rest(line, schema_kind) else {
            continue;
        };
        if let Some(name) = leading_graphql_name(rest) {
            return Some((schema_kind, name));
        }
    }
    None
}

fn schema_root_fields(schema_kind: &str, line: &str) -> Vec<String> {
    if schema_kind != "schema" {
        return Vec::new();
    }
    let Some((_, after_open)) = line.split_once('{') else {
        return Vec::new();
    };
    let inside = after_open
        .rsplit_once('}')
        .map_or(after_open, |(inside, _)| inside);
    ["query", "mutation", "subscription"]
        .into_iter()
        .filter(|&root| {
            inside
                .split_once(root)
                .is_some_and(|(_, after_root)| after_root.trim_start().starts_with(':'))
        })
        .map(|root| root.to_string())
        .collect()
}

fn parse_fragment_header(line: &str) -> Option<(String, String)> {
    let rest = keyword_rest(line, "fragment")?;
    let name = leading_graphql_name(rest)?;
    let rest = rest[name.len()..].trim_start();
    let rest = keyword_rest(rest, "on")?;
    let type_condition = leading_graphql_name(rest)?;
    Some((name.to_string(), type_condition.to_string()))
}

fn parse_directive_header(line: &str) -> Option<(String, Vec<String>, Vec<String>)> {
    let rest = keyword_rest(line, "directive")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('@')?.trim_start();
    let name = leading_graphql_name(rest)?;
    let after_name = &rest[name.len()..];
    let arguments = graphql_argument_names(after_name);
    let (_, after_on) = after_name.split_once(" on ")?;
    let locations = after_on
        .split('|')
        .filter_map(|location| leading_graphql_name(location.trim()))
        .filter(|location| !location.is_empty())
        .take(64)
        .map(str::to_string)
        .collect();
    Some((name.to_string(), locations, arguments))
}

fn parse_deprecation_reason(line: &str) -> Option<String> {
    let (_, after) = line.split_once("@deprecated")?;
    let Some((_, args)) = after.split_once('(') else {
        return Some("No longer supported".to_string());
    };
    let (args, _) = args.split_once(')').unwrap_or((args, ""));
    let Some((_, reason)) = args.split_once("reason") else {
        return Some("No longer supported".to_string());
    };
    let (_, reason) = reason.split_once(':')?;
    let reason = reason.trim();
    if let Some(stripped) = reason.strip_prefix('"') {
        let (value, _) = stripped.split_once('"').unwrap_or((stripped, ""));
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    if reason.is_empty() {
        None
    } else {
        Some(reason.trim_matches('"').to_string())
    }
}

fn keyword_rest<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if rest.is_empty() || rest.chars().next().is_some_and(char::is_whitespace) {
        Some(rest.trim_start())
    } else {
        None
    }
}

fn leading_graphql_name(input: &str) -> Option<&str> {
    let input = input.trim_start();
    let end = input
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_')
        .last()
        .map(|(index, ch)| index + ch.len_utf8())?;
    Some(&input[..end])
}

fn strip_graphql_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(before, _)| before)
}

fn anonymous_operation_name(operation_type: &str, line: u32, header: &str) -> String {
    let hash = stable_hash(header.as_bytes());
    format!("anonymous_{operation_type}_{line}_{hash:016x}")
}

#[derive(Debug, Clone)]
struct BodyCapture {
    depth: i32,
    fields: Vec<String>,
    arguments: Vec<String>,
    variable_references: Vec<String>,
    fragment_spreads: Vec<String>,
    inline_type_conditions: Vec<String>,
    deprecated_members: Vec<DeprecatedMember>,
    member_types: Vec<MemberType>,
    member_directives: Vec<MemberDirectives>,
    is_header_line: bool,
}

impl BodyCapture {
    fn from_header(line: &str) -> Option<Self> {
        let mut capture = Self {
            depth: 0,
            fields: Vec::new(),
            arguments: Vec::new(),
            variable_references: Vec::new(),
            fragment_spreads: Vec::new(),
            inline_type_conditions: Vec::new(),
            deprecated_members: Vec::new(),
            member_types: Vec::new(),
            member_directives: Vec::new(),
            is_header_line: true,
        };
        capture.capture_line(line);
        capture.is_header_line = false;
        Some(capture)
    }

    fn capture_line(&mut self, line: &str) {
        for spread in fragment_spreads_in_line(line) {
            if !self
                .fragment_spreads
                .iter()
                .any(|existing| existing == &spread)
            {
                self.fragment_spreads.push(spread);
            }
        }
        for var_ref in variable_refs_in_line(line) {
            if !self.is_header_line
                && !self
                    .variable_references
                    .iter()
                    .any(|existing| existing == &var_ref)
            {
                self.variable_references.push(var_ref);
            }
        }
        for type_condition in inline_type_conditions_in_line(line) {
            if !self
                .inline_type_conditions
                .iter()
                .any(|existing| existing == &type_condition)
            {
                self.inline_type_conditions.push(type_condition);
            }
        }
        if let Some(field) = leading_graphql_name(line.trim_start_matches('{').trim()) {
            let is_keyword = is_graphql_body_keyword(field);
            if !is_keyword && !self.fields.iter().any(|existing| existing == field) {
                self.fields.push(field.to_string());
            }
            if !is_keyword {
                for argument in graphql_field_argument_names(line) {
                    if !self.arguments.iter().any(|existing| existing == &argument) {
                        self.arguments.push(argument);
                    }
                }
                if let Some(type_name) = graphql_member_type_name(line)
                    && !self.member_types.iter().any(|member| member.name == field)
                {
                    self.member_types.push(MemberType {
                        name: field.to_string(),
                        type_name,
                    });
                }
                let directives = graphql_directive_names(line);
                if !directives.is_empty()
                    && !self
                        .member_directives
                        .iter()
                        .any(|member| member.name == field)
                {
                    let apps = graphql_directive_applications(line);
                    let directive_args: Vec<Vec<String>> = directives
                        .iter()
                        .map(|dn| {
                            apps.iter()
                                .find(|(app_name, _)| app_name == dn)
                                .map(|(_, args)| args.clone())
                                .unwrap_or_default()
                        })
                        .collect();
                    self.member_directives.push(MemberDirectives {
                        name: field.to_string(),
                        directives,
                        directive_args,
                    });
                }
                if let Some(reason) = parse_deprecation_reason(line)
                    && !self
                        .deprecated_members
                        .iter()
                        .any(|member| member.name == field)
                {
                    self.deprecated_members.push(DeprecatedMember {
                        name: field.to_string(),
                        reason: Some(reason),
                    });
                }
            }
        }
        self.depth += brace_delta(line);
    }

    fn is_complete(&self) -> bool {
        self.depth <= 0
    }
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

fn fragment_spreads_in_line(line: &str) -> Vec<String> {
    line.split("...")
        .skip(1)
        .filter_map(|after_spread| {
            let trimmed = after_spread.trim_start();
            if keyword_rest(trimmed, "on").is_some() {
                return None;
            }
            leading_graphql_name(trimmed).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .take(64)
        .collect()
}

fn inline_type_conditions_in_line(line: &str) -> Vec<String> {
    line.split("...")
        .skip(1)
        .filter_map(|after_spread| {
            let trimmed = after_spread.trim_start();
            let rest = keyword_rest(trimmed, "on")?;
            leading_graphql_name(rest).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .take(64)
        .collect()
}

fn variable_refs_in_line(line: &str) -> Vec<String> {
    line.split('$')
        .skip(1)
        .filter_map(|after_dollar| leading_graphql_name(after_dollar))
        .filter(|name| !name.is_empty())
        .take(64)
        .fold(Vec::new(), |mut output, name| {
            if !output.iter().any(|existing| existing == name) {
                output.push(name.to_string());
            }
            output
        })
}

#[allow(clippy::too_many_arguments)]
fn operation_entity(
    path: &str,
    operation_type: &str,
    name: &str,
    line: u32,
    variables: Vec<String>,
    variable_definitions: Vec<VariableDefinition>,
    selections: Vec<String>,
    fragment_spreads: Vec<String>,
    inline_type_conditions: Vec<String>,
    arguments: Vec<String>,
    directives: Vec<String>,
) -> Entity {
    let normalized_type = operation_type.to_ascii_uppercase();
    let stable_key = StableKey(format!("api://GRAPHQL_{normalized_type}:{name}"));
    let hash = stable_hash(stable_key.0.as_bytes());
    Entity {
        id: EntityId(format!("ent_graphql_operation_{hash:016x}")),
        stable_key,
        kind: EntityKind::ApiEndpoint,
        name: format!("{normalized_type} {name}"),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(line),
            line_end: Some(line),
        }),
        language: Some(LanguageCode("graphql".to_string())),
        aliases: vec![name.to_string()],
        ownership: ownership_for_file(path),
        payload: json!({
            "schema": "athanor.graphql_operation.v1",
            "protocol": "graphql",
            "operation_type": operation_type,
            "operation_name": name,
            "variables": variables,
            "variable_definitions": variable_definitions_payload(variable_definitions),
            "arguments": arguments,
            "selection_roots": selections,
            "fragment_spreads": fragment_spreads,
            "inline_type_conditions": inline_type_conditions,
            "directives": directives,
        }),
    }
}

fn variable_definitions_payload(definitions: Vec<VariableDefinition>) -> Vec<Value> {
    definitions
        .into_iter()
        .map(|definition| {
            json!({
                "name": definition.name,
                "type": definition.type_name,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn schema_entity(
    path: &str,
    schema_kind: &str,
    name: &str,
    line: u32,
    fields: Vec<String>,
    arguments: Vec<String>,
    member_argument_definitions: Vec<MemberArgumentDefinitions>,
    deprecation_reason: Option<String>,
    deprecated_members: Vec<DeprecatedMember>,
    directives: Vec<String>,
    member_types: Vec<MemberType>,
    member_directives: Vec<MemberDirectives>,
) -> Entity {
    let stable_key = StableKey(format!("api-schema://graphql:{path}#{name}"));
    let hash = stable_hash(stable_key.0.as_bytes());
    Entity {
        id: EntityId(format!("ent_graphql_schema_{hash:016x}")),
        stable_key,
        kind: EntityKind::ApiSchema,
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(line),
            line_end: Some(line),
        }),
        language: Some(LanguageCode("graphql".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(path),
        payload: json!({
            "schema": "athanor.graphql_schema.v1",
            "protocol": "graphql",
            "schema_kind": schema_kind,
            "fields": fields,
            "field_arguments": arguments,
            "member_argument_definitions": member_argument_definitions_payload(
                member_argument_definitions,
            ),
            "member_types": member_types_payload(member_types),
            "directives": directives,
            "member_directives": member_directives_payload(member_directives),
            "deprecation_reason": deprecation_reason,
            "deprecated_members": deprecated_members_payload(deprecated_members),
        }),
    }
}

fn member_types_payload(members: Vec<MemberType>) -> Vec<Value> {
    members
        .into_iter()
        .map(|member| {
            json!({
                "name": member.name,
                "type": member.type_name,
            })
        })
        .collect()
}

fn argument_definitions_payload(definitions: Vec<ArgumentDefinition>) -> Vec<Value> {
    definitions
        .into_iter()
        .map(|definition| {
            json!({
                "name": definition.name,
                "type": definition.type_name,
            })
        })
        .collect()
}

fn member_argument_definitions_payload(definitions: Vec<MemberArgumentDefinitions>) -> Vec<Value> {
    definitions
        .into_iter()
        .map(|definition| {
            json!({
                "member": definition.member,
                "arguments": argument_definitions_payload(definition.arguments),
            })
        })
        .collect()
}

fn deprecated_members_payload(members: Vec<DeprecatedMember>) -> Vec<Value> {
    members
        .into_iter()
        .map(|member| {
            json!({
                "name": member.name,
                "reason": member.reason,
            })
        })
        .collect()
}

fn member_directives_payload(members: Vec<MemberDirectives>) -> Vec<Value> {
    members
        .into_iter()
        .map(|member| {
            json!({
                "name": member.name,
                "directives": member.directives,
                "directive_args": member.directive_args,
            })
        })
        .collect()
}

fn fragment_entity(
    path: &str,
    name: &str,
    type_condition: String,
    line: u32,
    selections: Vec<String>,
    fragment_spreads: Vec<String>,
    directives: Vec<String>,
) -> Entity {
    let stable_key = StableKey(format!("api-fragment://graphql:{path}#{name}"));
    let hash = stable_hash(stable_key.0.as_bytes());
    Entity {
        id: EntityId(format!("ent_graphql_fragment_{hash:016x}")),
        stable_key,
        kind: EntityKind::Other("graphql_fragment".to_string()),
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(line),
            line_end: Some(line),
        }),
        language: Some(LanguageCode("graphql".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(path),
        payload: json!({
            "schema": "athanor.graphql_fragment.v1",
            "protocol": "graphql",
            "fragment_name": name,
            "type_condition": type_condition,
            "selection_roots": selections,
            "fragment_spreads": fragment_spreads,
            "directives": directives,
        }),
    }
}

fn directive_entity(
    path: &str,
    name: &str,
    line: u32,
    locations: Vec<String>,
    arguments: Vec<String>,
    argument_definitions: Vec<ArgumentDefinition>,
) -> Entity {
    let stable_key = StableKey(format!("api-directive://graphql:{path}#{name}"));
    let hash = stable_hash(stable_key.0.as_bytes());
    Entity {
        id: EntityId(format!("ent_graphql_directive_{hash:016x}")),
        stable_key,
        kind: EntityKind::Other("graphql_directive".to_string()),
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(line),
            line_end: Some(line),
        }),
        language: Some(LanguageCode("graphql".to_string())),
        aliases: Vec::new(),
        ownership: ownership_for_file(path),
        payload: json!({
            "schema": "athanor.graphql_directive.v1",
            "protocol": "graphql",
            "directive_name": name,
            "locations": locations,
            "arguments": arguments,
            "argument_definitions": argument_definitions_payload(argument_definitions),
        }),
    }
}

fn declaration_fact(
    extractor: &str,
    input: &ExtractInput,
    entity: &Entity,
    file_id: &EntityId,
    kind: FactKind,
    declaration_kind: &str,
    line: u32,
) -> Fact {
    let identity = format!(
        "{}\0{}\0{}",
        extractor, entity.stable_key.0, declaration_kind
    );
    let hash = stable_hash(identity.as_bytes());
    Fact {
        id: FactId(format!("fact_graphql_declared_{hash:016x}")),
        kind,
        subject: entity.id.clone(),
        object: Some(file_id.clone()),
        value: json!({
            "declaration_kind": declaration_kind,
            "stable_key": &entity.stable_key.0,
            "source_file": &input.source.path,
        }),
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(line),
            Some(line),
        )],
        ownership: ownership_for_file(&input.source.path),
        snapshot: input.snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    }
}

fn graphql_diagnostic(
    extractor: &str,
    input: &ExtractInput,
    kind: &str,
    title: &str,
    message: &str,
    line: u32,
    payload: Value,
) -> Diagnostic {
    let identity = format!("{}\0{}\0{}\0{line}", extractor, input.source.path, kind);
    let hash = stable_hash(identity.as_bytes());
    Diagnostic {
        id: DiagnosticId(format!("diag_{kind}_{hash:016x}")),
        kind: DiagnosticKind::Other(kind.to_string()),
        severity: Severity::Low,
        status: DiagnosticStatus::Open,
        title: title.to_string(),
        message: message.to_string(),
        entities: Vec::new(),
        evidence: vec![evidence_for_file(
            &input.source.path,
            extractor,
            Some(line),
            Some(line),
        )],
        ownership: ownership_for_file(&input.source.path),
        snapshot: input.snapshot.clone(),
        suggested_fix: None,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{RepoId, SnapshotId};

    use super::*;

    #[tokio::test]
    async fn extracts_graphql_schema_and_operation_declarations() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
directive @auth(requires: Role = USER) on FIELD_DEFINITION | OBJECT

type User @key(fields: "id") {
  id: ID!
  name: String!
  friends(first: Int, after: String): [User!]! @cacheControl(maxAge: 60)
  oldName: String @deprecated(reason: "Use name")
}

query GetUser($id: ID!) @auth {
  user(id: $id) {
    id
    ...UserFields
    ... on Admin {
      permissions
    }
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 3);
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::Other("graphql_directive".to_string())
                && entity.stable_key.0 == "api-directive://graphql:schema.graphql#auth"
                && entity.payload["locations"] == json!(["FIELD_DEFINITION", "OBJECT"])
                && entity.payload["arguments"] == json!(["requires"])
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ApiSchema
                && entity.stable_key.0 == "api-schema://graphql:schema.graphql#User"
                && entity.payload["field_arguments"] == json!(["first", "after"])
                && entity.payload["member_types"]
                    == json!([
                        { "name": "id", "type": "ID!" },
                        { "name": "name", "type": "String!" },
                        { "name": "friends", "type": "[User!]!" },
                        { "name": "oldName", "type": "String" }
                    ])
                && entity.payload["directives"] == json!(["key"])
                && entity.payload["member_directives"]
                    == json!([
                        { "name": "friends", "directives": ["cacheControl"], "directive_args": [["maxAge"]] },
                        { "name": "oldName", "directives": ["deprecated"], "directive_args": [["reason"]] }
                    ])
                && entity.payload["deprecated_members"]
                    == json!([{ "name": "oldName", "reason": "Use name" }])
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.kind == EntityKind::ApiEndpoint
                && entity.stable_key.0 == "api://GRAPHQL_QUERY:GetUser"
                && entity.payload["variables"] == json!(["id"])
                && entity.payload["variable_definitions"]
                    == json!([{ "name": "id", "type": "ID!" }])
                && entity.payload["arguments"] == json!(["id"])
                && entity.payload["directives"] == json!(["auth"])
                && entity.payload["fragment_spreads"] == json!(["UserFields"])
                && entity.payload["inline_type_conditions"] == json!(["Admin"])
        }));
        assert_eq!(output.facts.len(), 3);
        assert!(output.facts.iter().all(|fact| !fact.evidence.is_empty()));
        assert!(output.facts.iter().all(|fact| !fact.ownership.is_empty()));
    }

    #[tokio::test]
    async fn extracts_graphql_introspection_schema_types() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql.json".to_string(),
                    language_hint: Some("json".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"{
  "data": {
    "__schema": {
      "queryType": { "name": "Query" },
      "directives": [
        { "name": "auth", "locations": ["FIELD_DEFINITION"], "args": [{ "name": "requires" }] }
      ],
      "types": [
        {
          "kind": "OBJECT",
          "name": "User",
          "fields": [
            {
              "name": "id",
              "args": [{ "name": "format" }],
              "type": { "kind": "NON_NULL", "ofType": { "kind": "SCALAR", "name": "ID" } }
            },
            {
              "name": "name",
              "isDeprecated": true,
              "deprecationReason": "Use displayName",
              "type": { "kind": "SCALAR", "name": "String" }
            }
          ]
        },
        {
          "kind": "ENUM",
          "name": "Role",
          "enumValues": [
            { "name": "ADMIN" },
            { "name": "USER" }
          ]
        },
        {
          "kind": "OBJECT",
          "name": "__Schema",
          "fields": [{ "name": "types" }]
        }
      ]
    }
  }
}"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 4);
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "api-schema://graphql:schema.graphql.json#schema"
                && entity.payload["schema_kind"] == json!("schema")
                && entity.payload["fields"] == json!(["query"])
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "api-schema://graphql:schema.graphql.json#User"
                && entity.payload["fields"] == json!(["id", "name"])
                && entity.payload["field_arguments"] == json!(["format"])
                && entity.payload["member_types"]
                    == json!([
                        { "name": "id", "type": "ID!" },
                        { "name": "name", "type": "String" }
                    ])
                && entity.payload["deprecated_members"]
                    == json!([{ "name": "name", "reason": "Use displayName" }])
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "api-schema://graphql:schema.graphql.json#Role"
                && entity.payload["schema_kind"] == json!("enum")
        }));
        assert!(output.entities.iter().any(|entity| {
            entity.stable_key.0 == "api-directive://graphql:schema.graphql.json#auth"
                && entity.payload["locations"] == json!(["FIELD_DEFINITION"])
                && entity.payload["arguments"] == json!(["requires"])
        }));
        assert_eq!(output.facts.len(), 4);
        assert!(
            output
                .facts
                .iter()
                .all(|fact| !fact.evidence.is_empty() && !fact.ownership.is_empty())
        );
    }

    #[tokio::test]
    async fn extracts_graphql_fragment_declarations() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "fragments.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
fragment UserFields on User @client {
  id
  name
  ...AuditFields
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        let fragment = &output.entities[0];
        assert_eq!(
            fragment.stable_key.0,
            "api-fragment://graphql:fragments.graphql#UserFields"
        );
        assert_eq!(
            fragment.kind,
            EntityKind::Other("graphql_fragment".to_string())
        );
        assert_eq!(fragment.payload["type_condition"], json!("User"));
        assert_eq!(fragment.payload["directives"], json!(["client"]));
        assert_eq!(fragment.payload["selection_roots"], json!(["id", "name"]));
        assert_eq!(fragment.payload["fragment_spreads"], json!(["AuditFields"]));
        assert_eq!(output.facts.len(), 1);
        assert_eq!(
            output.facts[0].kind,
            FactKind::Other("graphql_fragment_declared".to_string())
        );
    }

    #[tokio::test]
    async fn reports_invalid_introspection_json_as_diagnostic() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql.json".to_string(),
                    language_hint: Some("json".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("{ invalid json".to_string()),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics[0].kind,
            DiagnosticKind::Other("graphql_introspection_parse_error".to_string())
        );
        assert!(!output.diagnostics[0].evidence.is_empty());
        assert!(!output.diagnostics[0].ownership.is_empty());
    }

    #[tokio::test]
    async fn extracts_graphql_schema_root_declaration() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("schema { query: Query }".to_string()),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        let schema = &output.entities[0];
        assert_eq!(
            schema.stable_key.0,
            "api-schema://graphql:schema.graphql#schema"
        );
        assert_eq!(schema.kind, EntityKind::ApiSchema);
        assert_eq!(schema.payload["schema_kind"], json!("schema"));
        assert_eq!(schema.payload["fields"], json!(["query"]));
        assert_eq!(output.facts.len(), 1);
        assert!(output.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn reports_graphql_file_without_supported_declarations() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some("# only a comment".to_string()),
                },
            })
            .await
            .unwrap();

        assert!(output.entities.is_empty());
        assert!(output.facts.is_empty());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics[0].kind,
            DiagnosticKind::Other("graphql_no_declarations".to_string())
        );
    }

    #[test]
    fn supports_graphql_sources() {
        assert!(GraphQlExtractor.supports(&SourceFile {
            path: "src/query.gql".to_string(),
            language_hint: None,
            content_hash: None,
            content: Some("query Ping { ping }".to_string()),
        }));
        assert!(!GraphQlExtractor.supports(&SourceFile {
            path: "src/query.ts".to_string(),
            language_hint: Some("typescript".to_string()),
            content_hash: None,
            content: Some("const query = `query Ping { ping }`;".to_string()),
        }));
        assert!(GraphQlExtractor.supports(&SourceFile {
            path: "schema.graphql.json".to_string(),
            language_hint: Some("json".to_string()),
            content_hash: None,
            content: Some(r#"{ "data": { "__schema": { "types": [] } } }"#.to_string()),
        }));
        assert!(GraphQlExtractor.supports(&SourceFile {
            path: "schema.json".to_string(),
            language_hint: Some("json".to_string()),
            content_hash: None,
            content: Some(r#"{ "data": { "__schema": { "directives": [] } } }"#.to_string()),
        }));
        assert!(GraphQlExtractor.supports(&SourceFile {
            path: "schema.json".to_string(),
            language_hint: Some("json".to_string()),
            content_hash: None,
            content: Some(
                r#"{ "data": { "__schema": { "queryType": { "name": "Query" } } } }"#.to_string()
            ),
        }));
    }

    #[tokio::test]
    async fn reports_unresolved_fragment_spread() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "query.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser {
  user {
    id
    ...NonExistentFragment
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.diagnostics.len(), 1);
        let diag = &output.diagnostics[0];
        assert_eq!(
            diag.kind,
            DiagnosticKind::Other("graphql_unresolved_fragment_spread".to_string())
        );
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("NonExistentFragment"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
        assert!(!diag.ownership.is_empty());
    }

    #[tokio::test]
    async fn reports_unresolved_type_condition() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "query.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser {
  user {
    ... on UnknownType {
      id
    }
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 1);
        assert_eq!(output.diagnostics.len(), 1);
        let diag = &output.diagnostics[0];
        assert_eq!(
            diag.kind,
            DiagnosticKind::Other("graphql_unresolved_type_condition".to_string())
        );
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("UnknownType"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
        assert!(!diag.ownership.is_empty());
    }

    #[tokio::test]
    async fn reports_deprecated_field_usage_in_operation() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
type User {
  id: ID!
  name: String!
  oldName: String @deprecated(reason: "Use name")
}

query GetUser {
  user {
    oldName
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 2);
        let deprecated_diag = output
            .diagnostics
            .iter()
            .find(|d| d.kind == DiagnosticKind::Other("graphql_deprecated_field_used".to_string()));
        assert!(
            deprecated_diag.is_some(),
            "expected deprecated field diagnostic"
        );
        let diag = deprecated_diag.unwrap();
        assert_eq!(diag.severity, Severity::Low);
        assert!(diag.message.contains("oldName"));
        assert!(diag.message.contains("deprecated"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
        assert!(!diag.ownership.is_empty());
    }

    #[tokio::test]
    async fn no_diagnostics_when_fragments_and_types_resolve() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
type User {
  id: ID!
  name: String!
}

fragment UserFields on User {
  id
  name
}

query GetUser {
  user {
    ...UserFields
    ... on User {
      name
    }
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        assert_eq!(output.entities.len(), 3);
        assert!(
            output.diagnostics.is_empty(),
            "expected no validation diagnostics, got: {:?}",
            output.diagnostics
        );
    }

    #[tokio::test]
    async fn reports_invalid_directive_location_on_operation() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
directive @auth on FIELD_DEFINITION

query GetUser @auth {
  user
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_invalid_directive_location".to_string())
        });
        assert!(
            diag.is_some(),
            "expected invalid directive location diagnostic, got diagnostics: {:?}",
            output.diagnostics
        );
        let diag = diag.unwrap();
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("@auth"));
        assert!(diag.message.contains("QUERY"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
    }

    #[tokio::test]
    async fn allows_deprecated_on_field_definition() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
type User {
  id: ID!
  name: String @deprecated(reason: "Use displayName")
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_invalid_directive_location".to_string())
        });
        assert!(
            diag.is_none(),
            "expected no invalid directive location diagnostic for @deprecated on FIELD_DEFINITION"
        );
    }

    #[tokio::test]
    async fn allows_unknown_directive_without_sdl_declaration() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "query.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser @cacheControl(maxAge: 60) {
  user
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_invalid_directive_location".to_string())
        });
        assert!(
            diag.is_none(),
            "expected no diagnostic for unknown directive @cacheControl"
        );
    }

    #[tokio::test]
    async fn reports_undeclared_variable_reference() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser {
  user(id: $id) {
    id
    name
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_undeclared_variable_reference".to_string())
        });
        assert!(
            diag.is_some(),
            "expected undeclared variable reference diagnostic"
        );
        let diag = diag.unwrap();
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("$id"));
        assert!(diag.message.contains("GetUser"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
    }

    #[tokio::test]
    async fn reports_unused_variable() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser($id: ID!, $name: String) {
  user(id: $id) {
    id
    name
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output
            .diagnostics
            .iter()
            .find(|d| d.kind == DiagnosticKind::Other("graphql_unused_variable".to_string()));
        assert!(diag.is_some(), "expected unused variable diagnostic");
        let diag = diag.unwrap();
        assert_eq!(diag.severity, Severity::Low);
        assert!(diag.message.contains("$name"));
        assert!(diag.message.contains("GetUser"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
    }

    #[tokio::test]
    async fn allows_valid_variable_usage() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser($id: ID!) {
  user(id: $id) {
    id
    name
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let undeclared = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_undeclared_variable_reference".to_string())
        });
        assert!(
            undeclared.is_none(),
            "expected no undeclared variable reference diagnostic"
        );
        let unused = output
            .diagnostics
            .iter()
            .find(|d| d.kind == DiagnosticKind::Other("graphql_unused_variable".to_string()));
        assert!(unused.is_none(), "expected no unused variable diagnostic");
    }

    #[tokio::test]
    async fn reports_invalid_variable_type_syntax() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser($id: id!) {
  user(id: $id) {
    id
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output
            .diagnostics
            .iter()
            .find(|d| d.kind == DiagnosticKind::Other("graphql_invalid_variable_type".to_string()));
        assert!(
            diag.is_some(),
            "expected invalid variable type diagnostic, got: {:?}",
            output.diagnostics
        );
        let diag = diag.unwrap();
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("$id"));
        assert!(diag.message.contains("id!"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
    }

    #[tokio::test]
    async fn allows_valid_variable_type_syntax() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser($id: ID!, $name: String, $tags: [String!]!, $count: Int) {
  user(id: $id, name: $name) {
    id
    name
    tags
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output
            .diagnostics
            .iter()
            .find(|d| d.kind == DiagnosticKind::Other("graphql_invalid_variable_type".to_string()));
        assert!(
            diag.is_none(),
            "expected no invalid variable type diagnostic, got: {:?}",
            output.diagnostics
        );
    }

    #[tokio::test]
    async fn reports_variable_type_not_found() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
type User {
  id: ID!
  name: String!
}

query GetUser($id: Nonexistent!) {
  user(id: $id) {
    id
    name
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_variable_type_not_found".to_string())
        });
        assert!(
            diag.is_some(),
            "expected variable type not found diagnostic, got: {:?}",
            output.diagnostics
        );
        let diag = diag.unwrap();
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("$id"));
        assert!(diag.message.contains("Nonexistent!"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
    }

    #[tokio::test]
    async fn allows_variable_type_builtin_scalar() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetUser($id: ID!, $name: String, $count: Int, $flag: Boolean, $score: Float) {
  user(id: $id) {
    id
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_variable_type_not_found".to_string())
        });
        assert!(
            diag.is_none(),
            "expected no variable type not found diagnostic, got: {:?}",
            output.diagnostics
        );
    }

    #[tokio::test]
    async fn allows_variable_type_declared_schema_type() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
type User {
  id: ID!
  name: String!
}

type Post {
  id: ID!
  title: String!
  author: User!
}

query GetUser($id: ID!, $postFilter: Post) {
  user(id: $id) {
    id
    name
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_variable_type_not_found".to_string())
        });
        assert!(
            diag.is_none(),
            "expected no variable type not found diagnostic, got: {:?}",
            output.diagnostics
        );
    }

    #[tokio::test]
    async fn reports_variable_type_not_found_for_list_type() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
query GetItems($ids: [UnknownType!]!) {
  items(ids: $ids) {
    id
  }
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_variable_type_not_found".to_string())
        });
        assert!(
            diag.is_some(),
            "expected variable type not found diagnostic for list type, got: {:?}",
            output.diagnostics
        );
        let diag = diag.unwrap();
        assert!(diag.message.contains("UnknownType"));
    }

    #[tokio::test]
    async fn reports_invalid_directive_argument() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
directive @auth(role: String!) on FIELD_DEFINITION

type Query {
  user: String @auth(role: "admin", unknown: "value")
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_invalid_directive_argument".to_string())
        });
        assert!(
            diag.is_some(),
            "expected invalid directive argument diagnostic, got: {:?}",
            output.diagnostics
        );
        let diag = diag.unwrap();
        assert_eq!(diag.severity, Severity::Medium);
        assert!(diag.message.contains("@auth"));
        assert!(diag.message.contains("unknown"));
        assert!(diag.suggested_fix.is_some());
        assert!(!diag.evidence.is_empty());
    }

    #[tokio::test]
    async fn allows_valid_directive_arguments() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
directive @auth(role: String!) on FIELD_DEFINITION

type Query {
  user: String @auth(role: "admin")
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let diag = output.diagnostics.iter().find(|d| {
            d.kind == DiagnosticKind::Other("graphql_invalid_directive_argument".to_string())
        });
        assert!(
            diag.is_none(),
            "expected no invalid directive argument diagnostic, got: {:?}",
            output.diagnostics
        );
    }

    #[tokio::test]
    async fn reports_invalid_directive_argument_on_multiple_directives() {
        let output = GraphQlExtractor
            .extract(ExtractInput {
                repo: RepoId("repo_test".to_string()),
                snapshot: SnapshotId("snap_test".to_string()),
                source: SourceFile {
                    path: "schema.graphql".to_string(),
                    language_hint: Some("graphql".to_string()),
                    content_hash: Some("hash".to_string()),
                    content: Some(
                        r#"
directive @auth(role: String!) on FIELD_DEFINITION
directive @cacheControl(maxAge: Int) on FIELD_DEFINITION

type Query {
  user: String @auth(role: "admin") @cacheControl(maxAge: 60, badArg: true)
}
"#
                        .to_string(),
                    ),
                },
            })
            .await
            .unwrap();

        let auth_diag = output.diagnostics.iter().filter(|d| {
            d.kind == DiagnosticKind::Other("graphql_invalid_directive_argument".to_string())
        });
        let auth_diags: Vec<_> = auth_diag.collect();
        assert_eq!(
            auth_diags.len(),
            1,
            "expected one invalid argument diagnostic for badArg, got: {:?}",
            output.diagnostics
        );
        assert!(auth_diags[0].message.contains("badArg"));
        assert!(auth_diags[0].message.contains("@cacheControl"));
    }
}

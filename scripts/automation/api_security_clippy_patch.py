from pathlib import Path

path = Path("crates/athanor-extractor-openapi/src/implementation.rs")
source = path.read_text(encoding="utf-8")

replacements = [
    (
        '''        let root_security = root.get("security");

        if let Some(paths) = root.get("paths").and_then(Value::as_object) {
''',
        '''        let root_security = root.get("security");
        let document_context = OpenApiDocumentContext {
            component_parameters,
            root_security,
            security_schemes,
        };

        if let Some(paths) = root.get("paths").and_then(Value::as_object) {
''',
    ),
    (
        '''                        component_parameters,
                        root_security,
                        security_schemes,
                        line,
''',
        '''                        document_context,
                        line,
''',
    ),
    (
        '''struct OpenApiSourceContext<'a> {
    path: &'a str,
    version: &'a str,
    parser_backend: &'a str,
}

fn endpoint_entity(
''',
        '''struct OpenApiSourceContext<'a> {
    path: &'a str,
    version: &'a str,
    parser_backend: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct OpenApiDocumentContext<'a> {
    component_parameters: Option<&'a Map<String, Value>>,
    root_security: Option<&'a Value>,
    security_schemes: Option<&'a Map<String, Value>>,
}

fn endpoint_entity(
''',
    ),
    (
        '''    path_parameters: Option<&Value>,
    component_parameters: Option<&Map<String, Value>>,
    root_security: Option<&Value>,
    security_schemes: Option<&Map<String, Value>>,
    line: Option<u32>,
''',
        '''    path_parameters: Option<&Value>,
    document: OpenApiDocumentContext<'_>,
    line: Option<u32>,
''',
    ),
    (
        '''    let parameters = operation_parameters(path_parameters, operation, component_parameters);
    let path_parameter_count = array_len(path_parameters);
    let effective_security = operation.get("security").or(root_security);
    let security_requirements =
        normalize_security_requirements(effective_security, security_schemes);
''',
        '''    let parameters = operation_parameters(
        path_parameters,
        operation,
        document.component_parameters,
    );
    let path_parameter_count = array_len(path_parameters);
    let effective_security = operation.get("security").or(document.root_security);
    let security_requirements =
        normalize_security_requirements(effective_security, document.security_schemes);
''',
    ),
]

for old, new in replacements:
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"implementation context anchor count {count}: {old[:80]!r}")
    source = source.replace(old, new)

path.write_text(source, encoding="utf-8")

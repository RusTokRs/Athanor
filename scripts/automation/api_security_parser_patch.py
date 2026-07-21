from pathlib import Path

path = Path("crates/athanor-extractor-openapi/src/parser.rs")
source = path.read_text(encoding="utf-8")

old = '''    let (backend, root) = if version.starts_with("3.1.") {
        (ParserBackend::Oas3, Oas3Parser.parse(content, path)?)
    } else if version.starts_with("3.0.") {
        (
            ParserBackend::LegacyValue,
            LegacyValueParser.parse(content, path)?,
        )
    } else {
        return Err(CoreError::Adapter(format!(
            "OpenAPI document {path} uses unsupported version {version}; expected 3.0.x or 3.1.x"
        )));
    };

    Ok(NormalizedOpenApiDocument {
        root,
'''
new = '''    let (backend, mut normalized_root) = if version.starts_with("3.1.") {
        (ParserBackend::Oas3, Oas3Parser.parse(content, path)?)
    } else if version.starts_with("3.0.") {
        (
            ParserBackend::LegacyValue,
            LegacyValueParser.parse(content, path)?,
        )
    } else {
        return Err(CoreError::Adapter(format!(
            "OpenAPI document {path} uses unsupported version {version}; expected 3.0.x or 3.1.x"
        )));
    };
    restore_explicit_security_overrides(&preflight, &mut normalized_root);

    Ok(NormalizedOpenApiDocument {
        root: normalized_root,
'''
if source.count(old) != 1:
    raise SystemExit(f"parser dispatch anchor count: {source.count(old)}")
source = source.replace(old, new)

marker = '''pub(crate) fn has_openapi_root_marker(content: &str) -> bool {
'''
addition = '''fn restore_explicit_security_overrides(source: &Value, normalized: &mut Value) {
    if let Some(security) = source.get("security")
        && let Some(root) = normalized.as_object_mut()
    {
        root.insert("security".to_string(), security.clone());
    }

    let Some(source_paths) = source.get("paths").and_then(Value::as_object) else {
        return;
    };
    let Some(normalized_paths) = normalized.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };
    for (path, source_item) in source_paths {
        let Some(source_item) = source_item.as_object() else {
            continue;
        };
        let Some(normalized_item) = normalized_paths.get_mut(path).and_then(Value::as_object_mut)
        else {
            continue;
        };
        for method in [
            "get", "put", "post", "delete", "options", "head", "patch", "trace",
        ] {
            let Some(security) = source_item
                .get(method)
                .and_then(Value::as_object)
                .and_then(|operation| operation.get("security"))
            else {
                continue;
            };
            if let Some(operation) = normalized_item
                .get_mut(method)
                .and_then(Value::as_object_mut)
            {
                operation.insert("security".to_string(), security.clone());
            }
        }
    }
}

'''
if source.count(marker) != 1:
    raise SystemExit(f"parser helper marker count: {source.count(marker)}")
source = source.replace(marker, addition + marker)
path.write_text(source, encoding="utf-8")

use athanor_core::{CoreError, CoreResult};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParserBackend {
    Oas3,
    LegacyValue,
}

impl ParserBackend {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Oas3 => "oas3",
            Self::LegacyValue => "legacy-value",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NormalizedOpenApiDocument {
    pub(crate) root: Value,
    pub(crate) version: String,
    pub(crate) backend: ParserBackend,
}

trait OpenApiDocumentParser {
    fn name(&self) -> &'static str;
    fn parse(&self, content: &str, path: &str) -> CoreResult<Value>;
}

#[derive(Debug, Clone, Copy)]
struct Oas3Parser;

impl OpenApiDocumentParser for Oas3Parser {
    fn name(&self) -> &'static str {
        "oas3"
    }

    fn parse(&self, content: &str, path: &str) -> CoreResult<Value> {
        let spec = if serde_json::from_str::<Value>(content).is_ok() {
            oas3::from_json(content).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to parse OpenAPI 3.1 document {path} with {}: {error}",
                    self.name()
                ))
            })?
        } else {
            oas3::from_yaml(content).map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to parse OpenAPI 3.1 document {path} with {}: {error}",
                    self.name()
                ))
            })?
        };

        serde_json::to_value(spec).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to normalize OpenAPI 3.1 document {path} from {}: {error}",
                self.name()
            ))
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct LegacyValueParser;

impl OpenApiDocumentParser for LegacyValueParser {
    fn name(&self) -> &'static str {
        "legacy-value"
    }

    fn parse(&self, content: &str, path: &str) -> CoreResult<Value> {
        parse_untyped(content).map_err(|error| {
            CoreError::Adapter(format!(
                "failed to parse OpenAPI 3.0 document {path} with {}: {error}",
                self.name()
            ))
        })
    }
}

pub(crate) fn parse_openapi_document(
    content: &str,
    path: &str,
) -> CoreResult<NormalizedOpenApiDocument> {
    let preflight = parse_untyped(content).map_err(|error| {
        CoreError::Adapter(format!("failed to parse OpenAPI document {path}: {error}"))
    })?;
    let root = preflight.as_object().ok_or_else(|| {
        CoreError::Adapter(format!("OpenAPI document {path} must have an object root"))
    })?;
    let version = root.get("openapi").and_then(Value::as_str).ok_or_else(|| {
        if let Some(swagger) = root.get("swagger").and_then(Value::as_str) {
            CoreError::Adapter(format!(
                "OpenAPI document {path} uses unsupported Swagger version {swagger}; expected OpenAPI 3.x"
            ))
        } else {
            CoreError::Adapter(format!(
                "OpenAPI document {path} is missing the openapi version"
            ))
        }
    })?;

    let (backend, root) = if version.starts_with("3.1.") {
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
        version: version.to_string(),
        backend,
    })
}

pub(crate) fn has_openapi_root_marker(content: &str) -> bool {
    parse_untyped(content).ok().is_some_and(|value| {
        value
            .as_object()
            .is_some_and(|root| root.get("openapi").and_then(Value::as_str).is_some())
    })
}

fn parse_untyped(content: &str) -> Result<Value, String> {
    if let Ok(json) = serde_json::from_str::<Value>(content) {
        return Ok(json);
    }
    let yaml = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content)
        .map_err(|error| error.to_string())?;
    serde_json::to_value(yaml).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_openapi_versions_to_expected_backends() {
        let cases = [
            (
                "3.0.3",
                "openapi: 3.0.3\ninfo: { title: API, version: '1' }\npaths: {}",
                ParserBackend::LegacyValue,
            ),
            (
                "3.1.0",
                "openapi: 3.1.0\ninfo: { title: API, version: '1' }\npaths: {}",
                ParserBackend::Oas3,
            ),
            (
                "3.1.1",
                r#"{"openapi":"3.1.1","info":{"title":"API","version":"1"},"paths":{}}"#,
                ParserBackend::Oas3,
            ),
        ];

        for (version, content, backend) in cases {
            let document = parse_openapi_document(content, "openapi.yaml").unwrap();
            assert_eq!(document.version, version);
            assert_eq!(document.backend, backend);
            assert_eq!(document.root["openapi"], version);
        }
    }

    #[test]
    fn rejects_unsupported_versions_with_selected_contract() {
        let error = parse_openapi_document(
            "openapi: 3.2.0\ninfo: { title: API, version: '1' }\npaths: {}",
            "openapi.yaml",
        )
        .unwrap_err();

        assert!(error.to_string().contains("expected 3.0.x or 3.1.x"));
    }
}

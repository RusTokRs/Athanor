//! Inventory for JSON boundaries that are not public application reports.
//!
//! Public report ownership lives in `json_contract`. This module separately records
//! schema-bearing persisted/generated/interchange/embedded documents and the
//! intentionally schema-less external-process protocols.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde_json::Value;

use crate::json_contract::{JsonContractDescriptor, validate_schema_id};

pub const PROCESS_ADAPTER_FRAMING: &str =
    "newline-terminated-json-stdin/single-json-stdout";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonBoundaryClass {
    Persisted,
    Generated,
    Interchange,
    Embedded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryLifecycle {
    Current,
    LegacyInput,
    Historical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonShape {
    Object,
    Array,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonPublicJsonContractDescriptor {
    pub schema: &'static str,
    pub rust_owner: &'static str,
    pub class: JsonBoundaryClass,
    pub lifecycle: BoundaryLifecycle,
    pub required_fields: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessProtocolDescriptor {
    pub name: &'static str,
    pub request_type: &'static str,
    pub response_type: &'static str,
    pub request_shape: JsonShape,
    pub response_shape: JsonShape,
    pub framing: &'static str,
}

macro_rules! boundary {
    ($schema:expr, $owner:literal, $class:ident, $lifecycle:ident, [$($field:literal),* $(,)?]) => {
        NonPublicJsonContractDescriptor {
            schema: $schema,
            rust_owner: $owner,
            class: JsonBoundaryClass::$class,
            lifecycle: BoundaryLifecycle::$lifecycle,
            required_fields: &[$($field),*],
        }
    };
}

pub const NON_PUBLIC_JSON_CONTRACTS: &[NonPublicJsonContractDescriptor] = &[
    boundary!(
        crate::project_registry::PROJECT_REGISTRY_STATE_SCHEMA,
        "ProjectRegistry",
        Persisted,
        Current,
        ["schema", "projects"]
    ),
    boundary!(
        crate::daemon::DAEMON_ENDPOINT_SCHEMA,
        "DaemonEndpoint",
        Persisted,
        Current,
        [
            "schema",
            "protocol_version",
            "athanor_version",
            "runtime_id",
            "token_path",
            "project_id",
            "root",
            "registry_path",
            "address",
            "pid",
            "started_at_unix_ms",
            "max_concurrent_requests",
            "max_job_history"
        ]
    ),
    boundary!(
        crate::daemon::DAEMON_ENDPOINT_SCHEMA_V2,
        "DaemonEndpoint",
        Persisted,
        LegacyInput,
        ["schema"]
    ),
    boundary!(
        crate::daemon::DAEMON_ENDPOINT_SCHEMA_V1,
        "DaemonEndpoint",
        Persisted,
        Historical,
        ["schema"]
    ),
    boundary!(
        crate::index_current::INDEX_CURRENT_SCHEMA,
        "IndexCurrent",
        Persisted,
        Current,
        [
            "schema",
            "generation",
            "snapshot",
            "read_model",
            "index_state",
            "read_model_manifest_sha256",
            "index_state_sha256"
        ]
    ),
    boundary!(
        crate::index_current::INDEX_CURRENT_SCHEMA_V1,
        "IndexCurrent",
        Persisted,
        LegacyInput,
        ["schema", "generation", "snapshot", "read_model", "index_state"]
    ),
    boundary!(
        "athanor.index_state.v46",
        "IndexState",
        Persisted,
        Current,
        ["schema", "snapshot", "files"]
    ),
    boundary!(
        "athanor.index_state.v46-js-ts-precision-v1",
        "IndexState",
        Persisted,
        Current,
        ["schema", "snapshot", "files"]
    ),
    boundary!(
        crate::index_publication_journal::INDEX_PUBLICATION_JOURNAL_SCHEMA_V3,
        "IndexPublicationJournal",
        Persisted,
        Current,
        ["schema", "prepared", "generation", "id", "read_model", "index_state"]
    ),
    boundary!(
        crate::index_publication_journal::INDEX_PUBLICATION_JOURNAL_SCHEMA_V2,
        "IndexPublicationJournal",
        Persisted,
        LegacyInput,
        ["schema"]
    ),
    boundary!(
        crate::index_publication_journal::INDEX_PUBLICATION_JOURNAL_SCHEMA_V1,
        "IndexPublicationJournal",
        Persisted,
        LegacyInput,
        ["schema"]
    ),
    boundary!(
        "athanor.index_current_publication.v1",
        "IndexCurrentPublicationJournal",
        Persisted,
        Current,
        ["schema", "generation", "snapshot"]
    ),
    boundary!(
        "athanor.canonical_snapshot.v1",
        "CanonicalSnapshotManifest",
        Persisted,
        Current,
        ["schema", "snapshot"]
    ),
    boundary!(
        "athanor.canonical_latest.v1",
        "LatestIdentityDocument",
        Persisted,
        Current,
        ["schema", "snapshot", "generation"]
    ),
    boundary!(
        "athanor.canonical_commit.v2",
        "SnapshotCommitV2",
        Persisted,
        Current,
        ["schema", "snapshot", "generation"]
    ),
    boundary!(
        "athanor.canonical_commit.v1",
        "SnapshotCommitV1",
        Persisted,
        LegacyInput,
        ["schema", "snapshot"]
    ),
    boundary!(
        crate::index::VALIDATION_RESULT_SCHEMA,
        "ValidationResultFile",
        Generated,
        Current,
        [
            "schema",
            "status",
            "snapshot",
            "files_indexed",
            "changed_files",
            "unchanged_files",
            "removed_files",
            "entities",
            "facts",
            "relations",
            "diagnostics"
        ]
    ),
    boundary!(
        crate::generation::GENERATED_GENERATION_SCHEMA,
        "GenerationManifest",
        Generated,
        Current,
        [
            "schema",
            "status",
            "generation",
            "snapshot",
            "athanor_version",
            "entities",
            "facts",
            "relations",
            "diagnostics",
            "outputs"
        ]
    ),
    boundary!(
        crate::generation::GENERATED_CURRENT_SCHEMA,
        "CurrentGeneration",
        Generated,
        Current,
        ["schema", "generation", "snapshot", "path", "manifest"]
    ),
    boundary!(
        crate::api::API_CONTRACT_SNAPSHOT_SCHEMA,
        "ApiContractSnapshot",
        Generated,
        Current,
        ["schema", "snapshot", "endpoints", "schemas", "examples"]
    ),
    boundary!(
        crate::api::API_CONTRACT_LATEST_SCHEMA,
        "ApiContractLatest",
        Generated,
        Current,
        ["schema", "snapshot", "path"]
    ),
    boundary!(
        crate::read_model::JSONL_MANIFEST_SCHEMA,
        "JsonlReadModelManifest",
        Generated,
        Current,
        [
            "schema",
            "snapshot",
            "generation",
            "files_indexed",
            "changed_files",
            "unchanged_files",
            "removed_files",
            "entities",
            "facts",
            "relations",
            "diagnostics"
        ]
    ),
    boundary!(
        "athanor.wiki_manifest.v1",
        "WikiManifest",
        Generated,
        Current,
        [
            "schema",
            "wiki_format_version",
            "snapshot",
            "status",
            "entities",
            "facts",
            "relations",
            "open_diagnostics"
        ]
    ),
    boundary!(
        "athanor.html_report_manifest.v1",
        "HtmlReportManifest",
        Generated,
        Current,
        [
            "schema",
            "report_format_version",
            "snapshot",
            "status",
            "entities",
            "entity_pages",
            "facts",
            "relations",
            "open_diagnostics"
        ]
    ),
    boundary!(
        crate::docs::DOCS_PATCH_SCHEMA,
        "DocsPatchProposal",
        Interchange,
        Current,
        ["schema", "id", "snapshot", "operations"]
    ),
    boundary!(
        crate::projection::WIKI_PROJECTION_SCHEMA,
        "CanonicalProjectionPayload",
        Interchange,
        Current,
        ["schema", "entities", "facts", "relations", "diagnostics"]
    ),
    boundary!(
        crate::projection::HTML_REPORT_PROJECTION_SCHEMA,
        "CanonicalProjectionPayload",
        Interchange,
        Current,
        ["schema", "entities", "facts", "relations", "diagnostics"]
    ),
    boundary!(
        "athanor.index_metrics.v1",
        "IndexPipelineMetrics",
        Embedded,
        Current,
        ["schema"]
    ),
    boundary!(
        crate::index::INDEX_REPORT_METRICS_SCHEMA,
        "IndexReportMetrics",
        Embedded,
        Current,
        ["schema", "total_ms", "pipeline"]
    ),
    boundary!(
        "athanor.generation_metrics.v1",
        "GenerationMetrics",
        Embedded,
        Current,
        ["schema", "total_ms"]
    ),
];

pub const PROCESS_PROTOCOL_CONTRACTS: &[ProcessProtocolDescriptor] = &[
    ProcessProtocolDescriptor {
        name: "source-discover",
        request_type: "SourceDiscoverInput",
        response_type: "Vec<SourceFile>",
        request_shape: JsonShape::Object,
        response_shape: JsonShape::Array,
        framing: PROCESS_ADAPTER_FRAMING,
    },
    ProcessProtocolDescriptor {
        name: "extractor",
        request_type: "ExtractInput",
        response_type: "ExtractOutput",
        request_shape: JsonShape::Object,
        response_shape: JsonShape::Object,
        framing: PROCESS_ADAPTER_FRAMING,
    },
    ProcessProtocolDescriptor {
        name: "linker",
        request_type: "LinkInput",
        response_type: "Vec<Relation>",
        request_shape: JsonShape::Object,
        response_shape: JsonShape::Array,
        framing: PROCESS_ADAPTER_FRAMING,
    },
    ProcessProtocolDescriptor {
        name: "checker",
        request_type: "CheckInput",
        response_type: "Vec<Diagnostic>",
        request_shape: JsonShape::Object,
        response_shape: JsonShape::Array,
        framing: PROCESS_ADAPTER_FRAMING,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryContractError(pub String);

impl fmt::Display for BoundaryContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for BoundaryContractError {}

pub fn validate_boundary_contract_inventory(
    public_contracts: &[JsonContractDescriptor],
) -> Result<(), BoundaryContractError> {
    let public_schemas = public_contracts
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let mut non_public_schemas = BTreeSet::new();

    for contract in NON_PUBLIC_JSON_CONTRACTS {
        validate_schema_id(contract.schema)
            .map_err(|error| BoundaryContractError(error.to_string()))?;
        if public_schemas.contains(contract.schema) {
            return Err(BoundaryContractError(format!(
                "non-public schema {} is also registered as a public report",
                contract.schema
            )));
        }
        if !non_public_schemas.insert(contract.schema) {
            return Err(BoundaryContractError(format!(
                "duplicate non-public schema {}",
                contract.schema
            )));
        }
        if contract.rust_owner.is_empty() || contract.required_fields.is_empty() {
            return Err(BoundaryContractError(format!(
                "non-public schema {} has incomplete ownership metadata",
                contract.schema
            )));
        }
    }

    let mut protocol_names = BTreeSet::new();
    for protocol in PROCESS_PROTOCOL_CONTRACTS {
        if !protocol_names.insert(protocol.name) {
            return Err(BoundaryContractError(format!(
                "duplicate process protocol {}",
                protocol.name
            )));
        }
        if protocol.name.starts_with("athanor.") {
            return Err(BoundaryContractError(format!(
                "schema-less process protocol {} must not masquerade as a schema id",
                protocol.name
            )));
        }
        if protocol.request_type.is_empty()
            || protocol.response_type.is_empty()
            || protocol.framing != PROCESS_ADAPTER_FRAMING
        {
            return Err(BoundaryContractError(format!(
                "process protocol {} has incomplete framing metadata",
                protocol.name
            )));
        }
    }

    Ok(())
}

pub fn validate_non_public_contract_value(
    descriptor: &NonPublicJsonContractDescriptor,
    value: &Value,
) -> Result<(), BoundaryContractError> {
    let object = value.as_object().ok_or_else(|| {
        BoundaryContractError(format!("{} document must be an object", descriptor.schema))
    })?;
    let actual = object.get("schema").and_then(Value::as_str).unwrap_or("<missing>");
    if actual != descriptor.schema {
        return Err(BoundaryContractError(format!(
            "document schema {actual} does not match {}",
            descriptor.schema
        )));
    }
    for field in descriptor.required_fields {
        if !object.contains_key(*field) {
            return Err(BoundaryContractError(format!(
                "{} document is missing required field {field}",
                descriptor.schema
            )));
        }
    }
    Ok(())
}

pub fn validate_process_protocol_value(
    descriptor: &ProcessProtocolDescriptor,
    request: &Value,
    response: &Value,
) -> Result<(), BoundaryContractError> {
    validate_shape(descriptor.name, "request", descriptor.request_shape, request)?;
    validate_shape(descriptor.name, "response", descriptor.response_shape, response)
}

fn validate_shape(
    protocol: &str,
    side: &str,
    expected: JsonShape,
    value: &Value,
) -> Result<(), BoundaryContractError> {
    let valid = match expected {
        JsonShape::Object => value.is_object(),
        JsonShape::Array => value.is_array(),
    };
    if valid {
        Ok(())
    } else {
        Err(BoundaryContractError(format!(
            "process protocol {protocol} {side} must be a {expected:?}"
        )))
    }
}

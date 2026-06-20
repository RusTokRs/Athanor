use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FactId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosticId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextPackId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConceptId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StableKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    File,
    Symbol,
    Function,
    Class,
    Module,
    ApiEndpoint,
    ApiSchema,
    ApiExample,
    DocumentationPage,
    DocumentationSection,
    Script,
    ScriptCommand,
    EnvVar,
    DbTable,
    DbMigration,
    TestCase,
    CiJob,
    DockerService,
    Feature,
    Package,
    Dependency,
    Concept,
    Runbook,
    OperationStep,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    FileDiscovered,
    SymbolDefined,
    RouteDeclared,
    DocSectionFound,
    DocMentionsSymbol,
    EnvVarUsed,
    ScriptReferencesFile,
    MigrationCreatesTable,
    FunctionQueriesTable,
    TestCoversSymbol,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Defines,
    Contains,
    Imports,
    Calls,
    Implements,
    Documents,
    DocumentsApi,
    DocumentsOperation,
    UsesEnv,
    QueriesTable,
    CoveredByTest,
    ChangedWith,
    IntroducedIn,
    OutdatedAgainst,
    Contradicts,
    BrokenReference,
    TranslationOf,
    TranslationOutdatedAgainst,
    ImplementedBy,
    DeclaredInOpenapi,
    TestedBy,
    ExampleFor,
    SchemaForRequest,
    SchemaForResponse,
    RequiresAuth,
    RequiresPermission,
    StaleAgainst,
    MissingAgainst,
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationStatus {
    Verified,
    Inferred,
    Suspected,
    Broken,
    Missing,
    Conflicting,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Verified,
    Inferred,
    Suspected,
    Missing,
    Conflicting,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    EmptyDocumentationPage,
    DocumentationPageMissingTitle,
    MissingDocumentation,
    StaleDocumentation,
    OpenapiMismatch,
    BrokenScriptReference,
    MissingEnvVar,
    OrphanDoc,
    DeadEndpoint,
    UncoveredSymbol,
    MigrationCodeMismatch,
    ApiEndpointMissingInOpenapi,
    ApiEndpointDocumentedButNotImplemented,
    ApiEndpointImplementedButNotDocumented,
    ApiMethodMismatch,
    ApiPathMismatch,
    ApiRequestSchemaMismatch,
    ApiResponseSchemaMismatch,
    ApiStatusCodeUndocumented,
    ApiAuthRequirementMismatch,
    ApiPermissionMismatch,
    ApiExampleInvalid,
    ApiErrorModelMismatch,
    ApiBreakingChangeDetected,
    TranslationOutdated,
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Open,
    Resolved,
    Suppressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextLevel {
    Summary,
    Normal,
    Deep,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguageCode(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub path: String,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ownership {
    pub source_file: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub source_file: Option<String>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub extractor: Option<String>,
    pub commit_hash: Option<String>,
    pub confidence: f32,
    pub status: EvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub stable_key: StableKey,
    pub kind: EntityKind,
    pub name: String,
    pub title: Option<String>,
    pub source: Option<SourceLocation>,
    pub language: Option<LanguageCode>,
    pub aliases: Vec<String>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub kind: FactKind,
    pub subject: EntityId,
    pub object: Option<EntityId>,
    pub value: Value,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub snapshot: SnapshotId,
    pub extractor: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub id: RelationId,
    pub kind: RelationKind,
    pub from: EntityId,
    pub to: EntityId,
    pub status: RelationStatus,
    pub confidence: f32,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub snapshot: SnapshotId,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub id: DiagnosticId,
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub status: DiagnosticStatus,
    pub title: String,
    pub message: String,
    pub entities: Vec<EntityId>,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub ownership: Vec<Ownership>,
    pub snapshot: SnapshotId,
    pub suggested_fix: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPack {
    pub id: ContextPackId,
    pub task: String,
    pub scope: Vec<String>,
    pub level: ContextLevel,
    pub language: Option<LanguageCode>,
    pub summary: String,
    pub entities: Vec<EntityId>,
    pub files: Vec<String>,
    pub diagnostics: Vec<DiagnosticId>,
    pub suggested_checks: Vec<String>,
    pub confidence: f32,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Concept {
    pub id: ConceptId,
    pub canonical_name: String,
    pub terms: Vec<LocalizedTerms>,
    pub related_entities: Vec<EntityId>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedTerms {
    pub language: LanguageCode,
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotBase {
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub parent_snapshot: Option<SnapshotId>,
    pub working_tree: bool,
}

use serde::Serialize;

use super::VersionedJsonContract;
use crate::{ApiSnapshotReport, DocsProposeFixReport};

pub const API_SNAPSHOT_SCHEMA_V1: &str = "athanor.api_snapshot.v1";
pub const DOCS_PROPOSE_FIX_SCHEMA_V1: &str = "athanor.docs_propose_fix.v1";

#[derive(Debug, Clone, Serialize)]
pub struct VersionedApiSnapshotReport {
    pub schema: &'static str,
    #[serde(flatten)]
    pub report: ApiSnapshotReport,
}

impl VersionedApiSnapshotReport {
    pub fn new(report: ApiSnapshotReport) -> Self {
        Self {
            schema: API_SNAPSHOT_SCHEMA_V1,
            report,
        }
    }
}

impl From<ApiSnapshotReport> for VersionedApiSnapshotReport {
    fn from(report: ApiSnapshotReport) -> Self {
        Self::new(report)
    }
}

impl VersionedJsonContract for VersionedApiSnapshotReport {
    const SCHEMA: &'static str = API_SNAPSHOT_SCHEMA_V1;

    fn schema(&self) -> &str {
        self.schema
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionedDocsProposeFixReport {
    pub schema: &'static str,
    #[serde(flatten)]
    pub report: DocsProposeFixReport,
}

impl VersionedDocsProposeFixReport {
    pub fn new(report: DocsProposeFixReport) -> Self {
        Self {
            schema: DOCS_PROPOSE_FIX_SCHEMA_V1,
            report,
        }
    }
}

impl From<DocsProposeFixReport> for VersionedDocsProposeFixReport {
    fn from(report: DocsProposeFixReport) -> Self {
        Self::new(report)
    }
}

impl VersionedJsonContract for VersionedDocsProposeFixReport {
    const SCHEMA: &'static str = DOCS_PROPOSE_FIX_SCHEMA_V1;

    fn schema(&self) -> &str {
        self.schema
    }
}

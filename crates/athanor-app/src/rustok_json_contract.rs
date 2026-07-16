use std::ops::Deref;

use serde::Serialize;

use crate::graph::{
    RUSTOK_FFA_SURFACE_GRAPH_SCHEMA, RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA, RustokFfaGraph,
};

/// Stable public owner for the FFA surface graph JSON contract.
///
/// The transparent wrapper preserves the established JSON object shape while
/// separating this schema from the violations graph that shares the internal
/// `RustokFfaGraph` calculation type.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokFfaSurfaceGraphReport(RustokFfaGraph);

impl RustokFfaSurfaceGraphReport {
    pub fn new(mut graph: RustokFfaGraph) -> Self {
        graph.schema = RUSTOK_FFA_SURFACE_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokFfaGraph {
        self.0
    }
}

impl Deref for RustokFfaSurfaceGraphReport {
    type Target = RustokFfaGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokFfaGraph> for RustokFfaSurfaceGraphReport {
    fn as_ref(&self) -> &RustokFfaGraph {
        &self.0
    }
}

/// Stable public owner for the FFA violations graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokFfaViolationsGraphReport(RustokFfaGraph);

impl RustokFfaViolationsGraphReport {
    pub fn new(mut graph: RustokFfaGraph) -> Self {
        graph.schema = RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokFfaGraph {
        self.0
    }
}

impl Deref for RustokFfaViolationsGraphReport {
    type Target = RustokFfaGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokFfaGraph> for RustokFfaViolationsGraphReport {
    fn as_ref(&self) -> &RustokFfaGraph {
        &self.0
    }
}

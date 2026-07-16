use std::ops::Deref;

use serde::Serialize;

use crate::graph::{
    RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA, RUSTOK_FBA_MODULE_GRAPH_SCHEMA,
    RUSTOK_FBA_PORT_GRAPH_SCHEMA, RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA,
    RUSTOK_FFA_SURFACE_GRAPH_SCHEMA, RUSTOK_FFA_VIOLATIONS_GRAPH_SCHEMA,
    RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA, RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA,
    RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA, RustokFbaGraph, RustokFfaGraph,
    RustokPageBuilderGraph,
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

/// Stable public owner for the FBA module graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokFbaModuleGraphReport(RustokFbaGraph);

impl RustokFbaModuleGraphReport {
    pub fn new(mut graph: RustokFbaGraph) -> Self {
        graph.schema = RUSTOK_FBA_MODULE_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokFbaGraph {
        self.0
    }
}

impl Deref for RustokFbaModuleGraphReport {
    type Target = RustokFbaGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokFbaGraph> for RustokFbaModuleGraphReport {
    fn as_ref(&self) -> &RustokFbaGraph {
        &self.0
    }
}

/// Stable public owner for the FBA port graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokFbaPortGraphReport(RustokFbaGraph);

impl RustokFbaPortGraphReport {
    pub fn new(mut graph: RustokFbaGraph) -> Self {
        graph.schema = RUSTOK_FBA_PORT_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokFbaGraph {
        self.0
    }
}

impl Deref for RustokFbaPortGraphReport {
    type Target = RustokFbaGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokFbaGraph> for RustokFbaPortGraphReport {
    fn as_ref(&self) -> &RustokFbaGraph {
        &self.0
    }
}

/// Stable public owner for the FBA dependencies graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokFbaDependenciesGraphReport(RustokFbaGraph);

impl RustokFbaDependenciesGraphReport {
    pub fn new(mut graph: RustokFbaGraph) -> Self {
        graph.schema = RUSTOK_FBA_DEPENDENCIES_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokFbaGraph {
        self.0
    }
}

impl Deref for RustokFbaDependenciesGraphReport {
    type Target = RustokFbaGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokFbaGraph> for RustokFbaDependenciesGraphReport {
    fn as_ref(&self) -> &RustokFbaGraph {
        &self.0
    }
}

/// Stable public owner for the FBA violations graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokFbaViolationsGraphReport(RustokFbaGraph);

impl RustokFbaViolationsGraphReport {
    pub fn new(mut graph: RustokFbaGraph) -> Self {
        graph.schema = RUSTOK_FBA_VIOLATIONS_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokFbaGraph {
        self.0
    }
}

impl Deref for RustokFbaViolationsGraphReport {
    type Target = RustokFbaGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokFbaGraph> for RustokFbaViolationsGraphReport {
    fn as_ref(&self) -> &RustokFbaGraph {
        &self.0
    }
}

/// Stable public owner for the Page Builder provider graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokPageBuilderProviderGraphReport(RustokPageBuilderGraph);

impl RustokPageBuilderProviderGraphReport {
    pub fn new(mut graph: RustokPageBuilderGraph) -> Self {
        graph.schema = RUSTOK_PAGE_BUILDER_PROVIDER_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokPageBuilderGraph {
        self.0
    }
}

impl Deref for RustokPageBuilderProviderGraphReport {
    type Target = RustokPageBuilderGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokPageBuilderGraph> for RustokPageBuilderProviderGraphReport {
    fn as_ref(&self) -> &RustokPageBuilderGraph {
        &self.0
    }
}

/// Stable public owner for the Page Builder consumer graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokPageBuilderConsumerGraphReport(RustokPageBuilderGraph);

impl RustokPageBuilderConsumerGraphReport {
    pub fn new(mut graph: RustokPageBuilderGraph) -> Self {
        graph.schema = RUSTOK_PAGE_BUILDER_CONSUMER_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokPageBuilderGraph {
        self.0
    }
}

impl Deref for RustokPageBuilderConsumerGraphReport {
    type Target = RustokPageBuilderGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokPageBuilderGraph> for RustokPageBuilderConsumerGraphReport {
    fn as_ref(&self) -> &RustokPageBuilderGraph {
        &self.0
    }
}

/// Stable public owner for the Page Builder violations graph JSON contract.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(transparent)]
pub struct RustokPageBuilderViolationsGraphReport(RustokPageBuilderGraph);

impl RustokPageBuilderViolationsGraphReport {
    pub fn new(mut graph: RustokPageBuilderGraph) -> Self {
        graph.schema = RUSTOK_PAGE_BUILDER_VIOLATIONS_GRAPH_SCHEMA.to_string();
        Self(graph)
    }

    pub fn into_inner(self) -> RustokPageBuilderGraph {
        self.0
    }
}

impl Deref for RustokPageBuilderViolationsGraphReport {
    type Target = RustokPageBuilderGraph;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<RustokPageBuilderGraph> for RustokPageBuilderViolationsGraphReport {
    fn as_ref(&self) -> &RustokPageBuilderGraph {
        &self.0
    }
}

mod content;
mod narrative;
mod update;

pub(super) const MANAGED_START_PREFIX: &str = "<!-- athanor:api-doc:start ";
pub(super) const MANAGED_END: &str = "<!-- athanor:api-doc:end -->";
pub(super) const COORDINATION_START_PREFIX: &str = "<!-- athanor:api-docs-coordination:start ";
pub(super) const COORDINATION_END: &str = "<!-- athanor:api-docs-coordination:end -->";
pub(super) const NARRATIVE_REVIEW_START: &str = "<!-- athanor:api-narrative-review:start -->";
pub(super) const NARRATIVE_REVIEW_END: &str = "<!-- athanor:api-narrative-review:end -->";

pub(super) use content::{api_doc_content, api_doc_path};
pub(super) use narrative::{stale_api_route_mentions, upsert_api_narrative_review_section};
pub(super) use update::{
    documented_api_pages, explicit_api_entity_reference_change, is_api_documentation_page,
    upsert_api_doc_managed_section, upsert_api_docs_coordination_section,
};

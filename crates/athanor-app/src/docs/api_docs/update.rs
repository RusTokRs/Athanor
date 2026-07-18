use std::collections::{BTreeMap, HashMap};

use athanor_domain::{Entity, EntityKind, Relation, RelationKind};
use serde_json::Value;

use super::super::DocsFrontmatterChange;
use super::super::check::page_path;
use super::content::push_api_contract_lines;
use super::{COORDINATION_END, COORDINATION_START_PREFIX, MANAGED_END, MANAGED_START_PREFIX};

pub(crate) fn documented_api_pages<'a>(
    endpoint: &Entity,
    pages_by_path: &'a BTreeMap<String, &'a Entity>,
    entities: &'a [Entity],
    relations: &[Relation],
) -> Vec<&'a Entity> {
    let by_id = entities
        .iter()
        .map(|entity| (&entity.id, entity))
        .collect::<HashMap<_, _>>();
    let mut pages = BTreeMap::<String, &'a Entity>::new();

    for page in pages_by_path.values() {
        if !is_api_documentation_page(page) {
            continue;
        }
        if page.payload["entities"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|stable_key| stable_key == endpoint.stable_key.0)
            && let Some(path) = page_path(page)
        {
            pages.insert(path, *page);
        }
    }

    for relation in relations.iter().filter(|relation| {
        relation.to == endpoint.id
            && matches!(
                relation.kind,
                RelationKind::Documents
                    | RelationKind::DocumentsApi
                    | RelationKind::DocumentsOperation
            )
    }) {
        let Some(document) = by_id.get(&relation.from).copied() else {
            continue;
        };
        if !matches!(
            document.kind,
            EntityKind::DocumentationPage | EntityKind::DocumentationSection
        ) {
            continue;
        }
        let Some(source_path) = document.source.as_ref().map(|source| source.path.clone()) else {
            continue;
        };
        let Some(page) = pages_by_path.get(&source_path).copied() else {
            continue;
        };
        if is_api_documentation_page(page) {
            pages.insert(source_path, page);
        }
    }
    pages.into_values().collect()
}

pub(crate) fn is_api_documentation_page(page: &Entity) -> bool {
    page.payload["documentation_kind"].as_str() == Some("api_documentation")
        || page.payload["kind"].as_str() == Some("api_documentation")
}

pub(crate) fn explicit_api_entity_reference_change(
    page: &Entity,
    endpoint: &Entity,
) -> Option<DocsFrontmatterChange> {
    let mut entities = page
        .payload
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if entities
        .iter()
        .any(|stable_key| stable_key == endpoint.stable_key.0.as_str())
    {
        return None;
    }
    entities.push(endpoint.stable_key.0.clone());
    entities.sort();
    entities.dedup();
    Some(DocsFrontmatterChange {
        field: "entities".to_string(),
        old_value: page.payload.get("entities").cloned(),
        new_value: Value::Array(entities.into_iter().map(Value::String).collect()),
        reason: format!(
            "API documentation page is linked to `{}` but does not declare it in frontmatter",
            endpoint.stable_key.0
        ),
    })
}

pub(crate) fn upsert_api_doc_managed_section(
    content: &str,
    endpoint: &Entity,
    entities: &[Entity],
    relations: &[Relation],
) -> String {
    let section = api_doc_managed_section(endpoint, entities, relations);
    replace_or_append_generated_section(
        content,
        &managed_start_marker(endpoint),
        MANAGED_END,
        &section,
    )
}

pub(crate) fn upsert_api_docs_coordination_section(
    content: &str,
    endpoint: &Entity,
    documented_pages: &[&Entity],
) -> String {
    let section = coordination_section(endpoint, documented_pages);
    replace_or_append_generated_section(
        content,
        &coordination_start_marker(endpoint),
        COORDINATION_END,
        &section,
    )
}

fn replace_or_append_generated_section(
    content: &str,
    start_marker: &str,
    end_marker: &str,
    section: &str,
) -> String {
    let Some(start) = content.find(start_marker) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(section);
        return updated;
    };
    let Some(relative_end) = content[start..].find(end_marker) else {
        let mut updated = content.trim_end().to_string();
        updated.push_str("\n\n");
        updated.push_str(section);
        return updated;
    };
    let end = start + relative_end + end_marker.len();
    let mut updated = String::new();
    updated.push_str(content[..start].trim_end());
    updated.push_str("\n\n");
    updated.push_str(section);
    updated.push_str(content[end..].trim_start_matches(['\r', '\n']));
    updated
}

fn managed_start_marker(endpoint: &Entity) -> String {
    format!("{MANAGED_START_PREFIX}{} -->", endpoint.stable_key.0)
}

fn coordination_start_marker(endpoint: &Entity) -> String {
    format!("{COORDINATION_START_PREFIX}{} -->", endpoint.stable_key.0)
}

fn api_doc_managed_section(
    endpoint: &Entity,
    entities: &[Entity],
    relations: &[Relation],
) -> String {
    let mut content = String::new();
    content.push_str(&format!(
        "{MANAGED_START_PREFIX}{} -->\n\n",
        endpoint.stable_key.0
    ));
    content.push_str("## Athanor API Contract\n\n");
    push_api_contract_lines(&mut content, endpoint, entities, relations);
    content.push_str(MANAGED_END);
    content.push('\n');
    content
}

fn coordination_section(endpoint: &Entity, documented_pages: &[&Entity]) -> String {
    let mut pages = documented_pages
        .iter()
        .filter_map(|page| page_path(page).map(|path| (*page, path)))
        .collect::<Vec<_>>();
    pages.sort_by(|(left_page, left_path), (right_page, right_path)| {
        (&left_page.stable_key.0, left_path).cmp(&(&right_page.stable_key.0, right_path))
    });

    let mut content = String::new();
    content.push_str(&format!(
        "{COORDINATION_START_PREFIX}{} -->\n\n",
        endpoint.stable_key.0
    ));
    content.push_str("## Athanor API Documentation Map\n\n");
    content.push_str(&format!("- Endpoint: `{}`\n", endpoint.stable_key.0));
    content.push_str("- Related editable API pages:\n");
    for (page, path) in pages {
        content.push_str(&format!("- `{}` at `{path}`\n", page.stable_key.0));
    }
    content.push('\n');
    content.push_str(COORDINATION_END);
    content.push('\n');
    content
}

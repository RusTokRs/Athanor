use std::collections::BTreeMap;

use athanor_domain::{Diagnostic, Entity, Fact, Relation, SourceLocation};

pub(crate) fn canonicalize_entities(entities: Vec<Entity>) -> Vec<Entity> {
    let mut by_id = BTreeMap::new();
    for entity in entities {
        by_id
            .entry(entity.id.0.clone())
            .and_modify(|existing: &mut Entity| merge_entity(existing, &entity))
            .or_insert(entity);
    }
    by_id.into_values().collect()
}

fn merge_entity(existing: &mut Entity, incoming: &Entity) {
    let incoming_wins_source = source_rank(incoming.source.as_ref())
        > source_rank(existing.source.as_ref())
        || existing.source.is_none();
    if incoming_wins_source {
        existing.source = incoming.source.clone();
        if !incoming.ownership.is_empty() {
            existing.ownership = incoming.ownership.clone();
        }
    }
    if incoming.language.is_some() || existing.language.is_none() {
        existing.language = incoming.language.clone();
    }
    if incoming.title.is_some() || existing.title.is_none() {
        existing.title = incoming.title.clone();
    }
    if !incoming.aliases.is_empty() {
        existing.aliases = incoming.aliases.clone();
    }
    if existing.ownership.is_empty() && !incoming.ownership.is_empty() {
        existing.ownership = incoming.ownership.clone();
    }
    existing.payload = incoming.payload.clone();
}

fn source_rank(source: Option<&SourceLocation>) -> u8 {
    let Some(source) = source else {
        return 0;
    };
    if source.path.contains("/contracts/") {
        30
    } else if source.path.contains("/src/") {
        20
    } else {
        10
    }
}

pub(crate) fn canonicalize_facts(facts: Vec<Fact>) -> Vec<Fact> {
    canonicalize_by_id(facts, |fact| fact.id.0.clone())
}
pub(crate) fn canonicalize_relations(relations: Vec<Relation>) -> Vec<Relation> {
    canonicalize_by_id(relations, |relation| relation.id.0.clone())
}
pub(crate) fn canonicalize_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    canonicalize_by_id(diagnostics, |diagnostic| diagnostic.id.0.clone())
}

fn canonicalize_by_id<T>(items: Vec<T>, id: impl Fn(&T) -> String) -> Vec<T> {
    let mut by_id = BTreeMap::new();
    for item in items {
        by_id.insert(id(&item), item);
    }
    by_id.into_values().collect()
}

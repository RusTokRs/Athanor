use async_trait::async_trait;
use athanor_domain::{EntityId, Fact};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CanonicalSnapshotStore, CoreError, CoreResult, SnapshotSelector};

/// Filters facts visible through one committed canonical snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Backend-neutral committed fact query boundary.
#[async_trait]
pub trait FactQueryStore: Send + Sync {
    async fn query_facts(
        &self,
        snapshot: SnapshotSelector,
        query: FactQuery,
    ) -> CoreResult<Vec<Fact>>;
}

/// Every canonical snapshot store exposes the same committed-only fact query semantics.
#[async_trait]
impl<T> FactQueryStore for T
where
    T: CanonicalSnapshotStore + Send + Sync,
{
    async fn query_facts(
        &self,
        snapshot: SnapshotSelector,
        query: FactQuery,
    ) -> CoreResult<Vec<Fact>> {
        let canonical = match snapshot {
            SnapshotSelector::Exact(snapshot) => self
                .load_snapshot(&snapshot)
                .await?
                .ok_or_else(|| CoreError::NotFound(format!("snapshot {}", snapshot.0)))?,
            SnapshotSelector::LatestCommitted => {
                let Some(snapshot) = self.load_latest_snapshot().await? else {
                    return Ok(Vec::new());
                };
                snapshot
            }
        };
        Ok(filter_facts(&canonical.facts, &query))
    }
}

/// Applies the shared in-memory filtering semantics used by canonical-snapshot facades and stores.
pub fn filter_facts<'a>(facts: impl IntoIterator<Item = &'a Fact>, query: &FactQuery) -> Vec<Fact> {
    let mut results = facts
        .into_iter()
        .filter(|fact| {
            query
                .subject
                .as_ref()
                .is_none_or(|subject| &fact.subject == subject)
        })
        .filter(|fact| {
            query
                .object
                .as_ref()
                .is_none_or(|object| fact.object.as_ref() == Some(object))
        })
        .filter(|fact| {
            query
                .kind
                .as_ref()
                .is_none_or(|kind| fact_kind_name(fact).eq_ignore_ascii_case(kind))
        })
        .filter(|fact| {
            query
                .extractor
                .as_ref()
                .is_none_or(|extractor| &fact.extractor == extractor)
        })
        .cloned()
        .collect::<Vec<_>>();
    if let Some(limit) = query.limit {
        results.truncate(limit);
    }
    results
}

fn fact_kind_name(fact: &Fact) -> String {
    match serde_json::to_value(&fact.kind) {
        Ok(Value::String(name)) => name,
        Ok(Value::Object(kind)) if kind.len() == 1 => kind
            .into_iter()
            .map(|(key, _)| key)
            .next()
            .expect("single fact-kind key"),
        _ => format!("{:?}", fact.kind).to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use athanor_domain::{Evidence, FactId, FactKind, SnapshotId};
    use serde_json::json;

    use super::*;

    fn fact(
        id: &str,
        kind: FactKind,
        subject: &str,
        object: Option<&str>,
        extractor: &str,
    ) -> Fact {
        Fact {
            id: FactId(id.to_string()),
            kind,
            subject: EntityId(subject.to_string()),
            object: object.map(|object| EntityId(object.to_string())),
            value: json!({}),
            evidence: Vec::<Evidence>::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            extractor: extractor.to_string(),
            confidence: 1.0,
        }
    }

    #[test]
    fn filters_subject_object_kind_extractor_and_limit() {
        let facts = vec![
            fact(
                "fact_1",
                FactKind::RouteDeclared,
                "entity_a",
                Some("entity_b"),
                "openapi",
            ),
            fact(
                "fact_2",
                FactKind::RouteDeclared,
                "entity_a",
                Some("entity_c"),
                "openapi",
            ),
            fact("fact_3", FactKind::SymbolDefined, "entity_a", None, "rust"),
        ];

        let results = filter_facts(
            &facts,
            &FactQuery {
                subject: Some(EntityId("entity_a".to_string())),
                object: Some(EntityId("entity_b".to_string())),
                kind: Some("route_declared".to_string()),
                extractor: Some("openapi".to_string()),
                limit: Some(1),
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id.0, "fact_1");
    }

    #[test]
    fn kind_matching_uses_serialized_snake_case() {
        let facts = vec![fact(
            "fact_1",
            FactKind::FileDiscovered,
            "entity_a",
            None,
            "source",
        )];

        assert_eq!(
            filter_facts(
                &facts,
                &FactQuery {
                    kind: Some("file_discovered".to_string()),
                    ..FactQuery::default()
                }
            )
            .len(),
            1
        );
    }

    #[test]
    fn data_carrying_kind_matches_its_serialized_variant_name() {
        let facts = vec![fact(
            "fact_1",
            FactKind::Other("custom".to_string()),
            "entity_a",
            None,
            "custom",
        )];

        assert_eq!(
            filter_facts(
                &facts,
                &FactQuery {
                    kind: Some("other".to_string()),
                    ..FactQuery::default()
                }
            )
            .len(),
            1
        );
    }
}

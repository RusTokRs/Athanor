use athanor_core::{CoreError, CoreResult, SnapshotBatch};
use athanor_domain::SnapshotId;

use super::{SurrealKnowledgeStore, insert_record};

impl SurrealKnowledgeStore {
    pub(crate) async fn publish_snapshot_batch_atomic(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        let _guard = self.write_gate.lock().await;
        let record = self.load_snapshot_record(&snapshot).await?;
        if record.committed {
            return Err(CoreError::Conflict(format!(
                "cannot republish committed snapshot {}",
                snapshot.0
            )));
        }

        let entities = batch
            .entities
            .iter()
            .map(|entity| insert_record(&snapshot, &entity.id.0, entity))
            .collect::<CoreResult<Vec<_>>>()?;
        let facts = batch
            .facts
            .iter()
            .map(|fact| insert_record(&snapshot, &fact.id.0, fact))
            .collect::<CoreResult<Vec<_>>>()?;
        let relations = batch
            .relations
            .iter()
            .map(|relation| insert_record(&snapshot, &relation.id.0, relation))
            .collect::<CoreResult<Vec<_>>>()?;
        let diagnostics = batch
            .diagnostics
            .iter()
            .map(|diagnostic| insert_record(&snapshot, &diagnostic.id.0, diagnostic))
            .collect::<CoreResult<Vec<_>>>()?;

        let mut sql = String::from(
            "BEGIN;\n\
             DELETE entity WHERE snapshot = $snapshot;\n\
             DELETE fact WHERE snapshot = $snapshot;\n\
             DELETE relation WHERE snapshot = $snapshot;\n\
             DELETE diagnostic WHERE snapshot = $snapshot;\n",
        );
        if !entities.is_empty() {
            sql.push_str("INSERT INTO entity $entities RETURN NONE;\n");
        }
        if !facts.is_empty() {
            sql.push_str("INSERT INTO fact $facts RETURN NONE;\n");
        }
        if !relations.is_empty() {
            sql.push_str("INSERT INTO relation $relations RETURN NONE;\n");
        }
        if !diagnostics.is_empty() {
            sql.push_str("INSERT INTO diagnostic $diagnostics RETURN NONE;\n");
        }
        sql.push_str(
            "UPDATE ONLY type::thing('snapshot', $snapshot) \
             SET prepared = true, committed = true RETURN NONE;\n\
             COMMIT;",
        );

        self.db
            .query(sql)
            .bind(("snapshot", snapshot.0.clone()))
            .bind(("entities", entities))
            .bind(("facts", facts))
            .bind(("relations", relations))
            .bind(("diagnostics", diagnostics))
            .await
            .map_err(|error| {
                CoreError::Adapter(format!(
                    "failed to execute atomic snapshot publication transaction: {error}"
                ))
            })?
            .check()
            .map_err(|error| {
                CoreError::Adapter(format!(
                    "atomic snapshot publication transaction rolled back: {error}"
                ))
            })?;

        Ok(())
    }
}

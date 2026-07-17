use super::VersionedJsonContract;

pub const REPAIR_INSPECT_SCHEMA_V2: &str = "athanor.repair_inspect.v2";
pub const REPAIR_CLEANUP_SCHEMA_V2: &str = "athanor.repair_cleanup.v2";
pub const REPAIR_REGENERATE_SCHEMA_V1: &str = "athanor.repair_regenerate.v1";
pub const REPAIR_RECOVER_CANONICAL_SCHEMA_V1: &str = "athanor.repair_recover_canonical.v1";
pub const REPAIR_APPLY_SCHEMA_V2: &str = "athanor.repair_apply.v2";
pub const INDEX_GENERATION_CLEANUP_SCHEMA_V1: &str = "athanor.index_generation_cleanup.v1";
pub const REPAIR_RECOVER_INDEX_SCHEMA_V1: &str = "athanor.repair_recover_index.v1";
pub const REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1: &str =
    "athanor.repair_recover_index_cleanup.v1";
pub const REPAIR_CANONICAL_LATEST_SCHEMA_V1: &str = "athanor.repair_canonical_latest.v1";

macro_rules! impl_repair_contract {
    ($type:path, $schema:ident) => {
        impl VersionedJsonContract for $type {
            const SCHEMA: &'static str = $schema;

            fn schema(&self) -> &str {
                &self.schema
            }
        }
    };
}

impl_repair_contract!(crate::repair::RepairInspectReport, REPAIR_INSPECT_SCHEMA_V2);
impl_repair_contract!(crate::repair::RepairCleanupReport, REPAIR_CLEANUP_SCHEMA_V2);
impl_repair_contract!(
    crate::repair::RepairRegenerateReport,
    REPAIR_REGENERATE_SCHEMA_V1
);
impl_repair_contract!(
    crate::repair::RepairRecoverCanonicalReport,
    REPAIR_RECOVER_CANONICAL_SCHEMA_V1
);
impl_repair_contract!(crate::repair::RepairApplyReport, REPAIR_APPLY_SCHEMA_V2);
impl_repair_contract!(
    crate::repair::IndexGenerationCleanupReport,
    INDEX_GENERATION_CLEANUP_SCHEMA_V1
);
impl_repair_contract!(
    crate::repair::RepairRecoverIndexReport,
    REPAIR_RECOVER_INDEX_SCHEMA_V1
);
impl_repair_contract!(
    crate::repair::RepairRecoverIndexCleanupReport,
    REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1
);
impl_repair_contract!(
    crate::repair::RepairCanonicalLatestReport,
    REPAIR_CANONICAL_LATEST_SCHEMA_V1
);

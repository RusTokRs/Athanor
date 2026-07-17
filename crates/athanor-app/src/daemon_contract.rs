use super::VersionedJsonContract;

pub const DAEMON_REQUEST_CONTRACT_SCHEMA_V3: &str = crate::daemon::DAEMON_REQUEST_SCHEMA;
pub const DAEMON_RESPONSE_CONTRACT_SCHEMA_V3: &str = crate::daemon::DAEMON_RESPONSE_SCHEMA;
pub const DAEMON_JOBS_CONTRACT_SCHEMA_V1: &str = crate::daemon::DAEMON_JOBS_SCHEMA;

macro_rules! impl_daemon_contract {
    ($type:path, $schema:ident) => {
        impl VersionedJsonContract for $type {
            const SCHEMA: &'static str = $schema;

            fn schema(&self) -> &str {
                &self.schema
            }
        }
    };
}

impl_daemon_contract!(
    crate::daemon::DaemonRequest,
    DAEMON_REQUEST_CONTRACT_SCHEMA_V3
);
impl_daemon_contract!(
    crate::daemon::DaemonResponse,
    DAEMON_RESPONSE_CONTRACT_SCHEMA_V3
);
impl_daemon_contract!(
    crate::daemon::DaemonJobsReport,
    DAEMON_JOBS_CONTRACT_SCHEMA_V1
);

//! Versioned JSON contracts emitted by repository automation rather than Rust runtime owners.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::boundary_contract::{
    BoundaryLifecycle, JsonBoundaryClass, NonPublicJsonContractDescriptor,
};
use crate::json_contract::{JsonContractDescriptor, validate_schema_id};

pub const VERIFICATION_EVIDENCE_SCHEMA_V1: &str = "athanor.verification_evidence.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomationJsonContractDescriptor {
    pub schema: &'static str,
    pub owner: &'static str,
    pub class: JsonBoundaryClass,
    pub lifecycle: BoundaryLifecycle,
    pub required_fields: &'static [&'static str],
}

pub const AUTOMATION_JSON_CONTRACTS: &[AutomationJsonContractDescriptor] =
    &[AutomationJsonContractDescriptor {
        schema: VERIFICATION_EVIDENCE_SCHEMA_V1,
        owner: ".github/workflows/verification-evidence.yml",
        class: JsonBoundaryClass::Persisted,
        lifecycle: BoundaryLifecycle::Current,
        required_fields: &[
            "schema",
            "workflow",
            "head_sha",
            "run_id",
            "run_url",
            "conclusion",
            "completed_at",
            "matrix",
        ],
    }];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationContractError(pub String);

impl fmt::Display for AutomationContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for AutomationContractError {}

pub fn validate_automation_contract_inventory(
    public_contracts: &[JsonContractDescriptor],
    general_contracts: &[NonPublicJsonContractDescriptor],
    adapter_contracts: &[NonPublicJsonContractDescriptor],
) -> Result<(), AutomationContractError> {
    let occupied = public_contracts
        .iter()
        .map(|contract| contract.schema)
        .chain(general_contracts.iter().map(|contract| contract.schema))
        .chain(adapter_contracts.iter().map(|contract| contract.schema))
        .collect::<BTreeSet<_>>();
    let mut automation = BTreeSet::new();

    for contract in AUTOMATION_JSON_CONTRACTS {
        validate_schema_id(contract.schema)
            .map_err(|error| AutomationContractError(error.to_string()))?;
        if occupied.contains(contract.schema) {
            return Err(AutomationContractError(format!(
                "automation schema {} is already owned by another registry",
                contract.schema
            )));
        }
        if !automation.insert(contract.schema) {
            return Err(AutomationContractError(format!(
                "duplicate automation schema {}",
                contract.schema
            )));
        }
        if contract.owner.is_empty() || contract.required_fields.is_empty() {
            return Err(AutomationContractError(format!(
                "automation schema {} has incomplete ownership metadata",
                contract.schema
            )));
        }
    }

    Ok(())
}

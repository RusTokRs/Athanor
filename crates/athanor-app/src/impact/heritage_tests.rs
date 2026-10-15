use athanor_core::CanonicalSnapshot;
use athanor_domain::{EntityKind, RelationKind, SnapshotId};

use super::tests::{entity, relation};
use super::{FlowDirection, impact_snapshot};

#[test]
fn base_type_changes_propagate_to_subtypes_and_trait_implementers() {
    let base = entity("base", "symbol://ts:Base", EntityKind::Class);
    let derived = entity("derived", "symbol://ts:Derived", EntityKind::Class);
    let contract = entity("contract", "symbol://rust:Contract", EntityKind::Symbol);
    let derived_contract = entity(
        "derived_contract",
        "symbol://rust:DerivedContract",
        EntityKind::Symbol,
    );
    let implementation = entity(
        "implementation",
        "symbol://rust:Implementation",
        EntityKind::Symbol,
    );
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![
            base.clone(),
            derived.clone(),
            contract.clone(),
            derived_contract.clone(),
            implementation.clone(),
        ],
        facts: Vec::new(),
        relations: vec![
            relation("extends", RelationKind::Extends, &derived, &base),
            relation(
                "trait_extends",
                RelationKind::Extends,
                &derived_contract,
                &contract,
            ),
            relation(
                "implements",
                RelationKind::Implements,
                &implementation,
                &derived_contract,
            ),
        ],
        diagnostics: Vec::new(),
    };

    let base_impact = impact_snapshot(&snapshot, vec![base], 2);
    let derived_impact = base_impact
        .impacted_entities
        .iter()
        .find(|impact| impact.entity.id == derived.id)
        .expect("base changes should reach derived classes");
    assert_eq!(derived_impact.path_steps[0].relation_kind, "extends");
    assert_eq!(
        derived_impact.path_steps[0].direction,
        FlowDirection::Backward
    );

    let trait_impact = impact_snapshot(&snapshot, vec![contract], 2);
    assert!(
        trait_impact
            .impacted_entities
            .iter()
            .any(|impact| { impact.entity.id == derived_contract.id })
    );
    assert!(
        trait_impact
            .impacted_entities
            .iter()
            .any(|impact| { impact.entity.id == implementation.id })
    );
}

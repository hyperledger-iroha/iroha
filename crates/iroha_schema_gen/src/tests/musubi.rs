//! Publication recovery exports the complete native comparison and provenance schema.

use super::{IntoSchema, find_missing_schema_references};
use iroha_data_model::{
    isi::musubi::{AdvanceMusubiPinOutboxV1, CheckMusubiPinOutboxV1},
    musubi::{
        MusubiPinOutboxCheckExpectationV1, MusubiPinOutboxCheckFloorV1, MusubiPinOutboxHighWaterV1,
    },
};

#[test]
fn pin_outbox_export_contains_exact_check_and_high_water_schema_closure() {
    let mut expected = CheckMusubiPinOutboxV1::schema();
    AdvanceMusubiPinOutboxV1::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted native pin outbox descriptor: {}",
            descriptor.type_name,
        );
    }
    assert!(expected.contains_key::<MusubiPinOutboxCheckExpectationV1>());
    assert!(expected.contains_key::<MusubiPinOutboxCheckFloorV1>());
    assert!(expected.contains_key::<MusubiPinOutboxHighWaterV1>());
    assert!(find_missing_schema_references(&expected).is_empty());
}

#[test]
fn reserve_policy_proof_exports_canonical_policy_and_history_schema_closure() {
    use iroha_data_model::sorafs::reserve::{
        history::{ReserveEventJournalHeadV1, ReserveStateV1},
        proof::ReservePolicyProofV1,
    };
    let mut expected = ReservePolicyProofV1::schema();
    ReserveStateV1::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing reserve descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(expected.contains_key::<ReserveEventJournalHeadV1>());
    assert!(find_missing_schema_references(&expected).is_empty());
}

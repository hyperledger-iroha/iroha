//! Physical retained allocation, wire projection and exact current/undo restoration.

use super::*;
use iroha_data_model::runtime::definition::DefinitionId;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;

fn key() -> CarrierStorageKeyV1 {
    CarrierStorageKeyV1 {
        storage: DefinitionId([7; 32]),
        key: vec![1, 2, 3],
    }
}

#[test]
fn immutable_key_and_bytes_retain_original_pool_through_last_shared_copy() {
    let budget = AllocationBudget::new(1 << 20);
    let foreign = AllocationBudget::new(1 << 20);
    let raw = key();
    let retained = RetainedCarrierKey::admit(&raw, &budget).unwrap();
    let bytes = RetainedCarrierBytes::admit(&[4; 103], &budget).unwrap();
    assert_eq!(retained.encode(), raw.encode());
    assert_eq!(bytes.encode(), vec![4_u8; 103].encode());
    assert!(retained.belongs_to(&budget) && bytes.belongs_to(&budget));
    assert!(!retained.belongs_to(&foreign) && !bytes.belongs_to(&foreign));
    let paid = budget.reserved_bytes();
    budget.set_limit_bytes(paid);
    let copies = (retained.clone(), bytes.clone());
    assert_eq!(budget.reserved_bytes(), paid);
    drop((retained, bytes));
    assert_eq!(budget.reserved_bytes(), paid);
    drop(copies);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn preparation_retains_original_refusal_and_checks_record_bounds() {
    let original = AllocationBudget::new(0);
    assert!(matches!(
        prepare(&key(), Some(&[1]), &original),
        Err(Error::Admission(_))
    ));
    assert_eq!(original.reserved_bytes(), 0);
    let oversized = vec![0; MAX_CARRIER_FRAME_BYTES + 1];
    assert!(matches!(
        prepare(&key(), Some(&oversized), &original),
        Err(Error::Invalid(_))
    ));
    let mut oversized_key = key();
    oversized_key.key.resize(MAX_CARRIER_KEY_BYTES + 1, 0);
    assert!(matches!(
        RetainedCarrierKey::admit(&oversized_key, &original),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn exact_snapshot_restores_funded_current_and_undo_without_retaining_scratch() {
    let budget = AllocationBudget::new(1 << 20);
    let mut storage = CarrierStorage::try_new_admitted(budget.clone()).unwrap();
    storage.insert(key(), vec![11; 100]);
    storage.insert(key(), vec![22; 101]);
    let mut raw = String::new();
    snapshot_json::serialize(&storage, &mut raw);
    struct JsonStore<'a>(&'a CarrierStorage);
    impl norito::json::JsonSerialize for JsonStore<'_> {
        fn json_serialize(&self, out: &mut String) {
            snapshot_json::serialize(self.0, out);
        }
        fn json_serialize_to(
            &self,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            snapshot_json::serialize_bounded(self.0, out)
        }
    }
    assert_eq!(
        norito::json::to_json_bounded(&JsonStore(&storage), raw.len()).unwrap(),
        raw
    );
    assert!(norito::json::to_json_bounded(&JsonStore(&storage), raw.len() - 1).is_err());
    let other = AllocationBudget::new(1 << 20);
    let restored = crate::state::snapshot_storage::carrier::restore_carriers(&raw, &other).unwrap();
    let mut copied = String::new();
    snapshot_json::serialize(&restored, &mut copied);
    assert_eq!(raw, copied);
    let snapshot = restored.snapshot();
    let (k, v) = snapshot.current().iter().next().unwrap();
    assert!(k.belongs_to(&other) && v.belongs_to(&other));
    assert_eq!(v.as_slice(), &[22; 101]);
    let old = snapshot
        .revert_map()
        .iter()
        .next()
        .unwrap()
        .1
        .as_ref()
        .unwrap();
    assert_eq!(old.as_slice(), &[11; 100]);
    assert!(old.belongs_to(&other));
    drop(snapshot);
    drop(restored);
    assert_eq!(other.reserved_bytes(), 0);
    let refusal =
        crate::state::snapshot_storage::carrier::restore_carriers(&raw, &AllocationBudget::new(0))
            .err()
            .unwrap();
    assert!(matches!(
        refusal,
        crate::state::deserialize::StateRestoreError::Admission(_)
            | crate::state::deserialize::StateRestoreError::ExecutionDeferred(_)
    ));
    for bad in [
        format!("{raw} "),
        raw.replacen("\"revert\":", "\"revert\": ", 1),
        raw.replacen("encoded_hex", "encoded_heX", 1),
    ] {
        assert!(crate::state::snapshot_storage::carrier::restore_carriers(&bad, &other).is_err());
        assert_eq!(other.reserved_bytes(), 0);
    }
}

#[test]
fn artifact_payload_uses_separate_native_key_and_exact_funded_envelope() {
    use iroha_data_model::smart_contract::ContractArtifactId;
    let budget = AllocationBudget::new(1 << 20);
    let id = ContractArtifactId::new(
        iroha_model_base::topology::DataSpaceId::new(0),
        iroha_crypto::Hash::new(b"code"),
    );
    let mut storage = ArtifactStorage::try_new_admitted(budget.clone()).unwrap();
    storage.insert(id, b"code".to_vec());
    let mut raw = String::new();
    artifact_snapshot_json::serialize(&storage, &mut raw);
    let other = AllocationBudget::new(1 << 20);
    let restored =
        crate::state::snapshot_storage::carrier::restore_artifacts(&raw, &other).unwrap();
    assert_eq!(restored.view().get(&id).unwrap().as_slice(), b"code");
    drop(restored);
    assert_eq!(other.reserved_bytes(), 0);
}

#[test]
fn prepared_payload_must_belong_to_the_actual_persistent_state_pool() {
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, StateReadOnly, StateStorageAdmissionError, World},
    };
    use iroha_data_model::block::BlockHeader;
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(
        2.try_into().unwrap(),
        None,
        None,
        1_000,
        0,
    ));
    let mut transaction = block.transaction();
    let original = transaction.execution_budget();
    let prepared = prepare(&key(), Some(&[9; 32]), &original).unwrap();
    assert!(
        prepared
            .key
            .belongs_to(transaction.world.runtime_generic_values.allocation_budget())
    );
    assert!(
        prepared
            .value
            .as_ref()
            .unwrap()
            .belongs_to(transaction.world.runtime_generic_values.allocation_budget())
    );
    prepared.apply(&mut transaction).unwrap();
    assert!(
        transaction
            .world
            .runtime_generic_values
            .get(&key())
            .unwrap()
            .belongs_to(&original)
    );
    let foreign = AllocationBudget::new(1 << 20);
    let foreign_write = prepare(&key(), Some(&[8; 32]), &foreign).unwrap();
    assert!(foreign_write.apply(&mut transaction).is_err());
    assert!(matches!(
        transaction.require_storage_admission(),
        Err(StateStorageAdmissionError::World(
            mv::storage::AdmittedStorageError::PolicyIdentity
        ))
    ));
    assert_eq!(
        transaction
            .world
            .runtime_generic_values
            .get(&key())
            .unwrap()
            .as_slice(),
        &[9; 32]
    );
    drop(transaction);
    drop(block);
    assert!(
        state
            .world
            .runtime_generic_values
            .view()
            .get(&key())
            .is_none()
    );
    assert_eq!(foreign.reserved_bytes(), 0);
}

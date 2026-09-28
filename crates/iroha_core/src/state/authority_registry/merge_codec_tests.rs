//! Exact retained merge-admission codec, independent of publication authority.

use crate::state::{MergeAdmissionState, MergeExecutionFrontier};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, consensus::LaneBlockCommitment},
    merge::{
        MergeLaneAuthorityCatalogV1, MergeLaneSnapshot, MergeLedgerEntry, MergeQuorumCertificate,
    },
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::numeric::Quantity;
use norito::NoritoSchema;

fn test_hash(label: &'static [u8]) -> Hash {
    Hash::new(label)
}

fn test_merge_entry() -> MergeLedgerEntry {
    let validator_set = Vec::<PeerId>::new();
    MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: 7,
        lane_catalog_hash: test_hash(b"catalog"),
        active_lanes: Vec::new(),
        lane_authority_catalog: MergeLaneAuthorityCatalogV1::default(),
        incarnation_root: test_hash(b"incarnations"),
        activation_root: test_hash(b"activations"),
        lane_snapshots: Vec::new(),
        global_state_root: test_hash(b"root"),
        merge_qc: MergeQuorumCertificate::new(
            2,
            7,
            9,
            HashOf::from_untyped_unchecked(test_hash(b"parent")),
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(test_hash(b"genesis"))),
            1,
            HashOf::new(&validator_set),
            validator_set,
            vec![1],
            Vec::new(),
            vec![2],
            test_hash(b"message"),
        ),
        execution_batch: None,
        lane_drain_certificates: Vec::new(),
    }
}

fn test_lane_snapshot() -> MergeLaneSnapshot {
    let settlement_commitment = LaneBlockCommitment {
        block_height: 3,
        lane_id: LaneId::new(4),
        lane_incarnation: test_hash(b"incarnation"),
        dataspace_id: DataSpaceId::new(5),
        tx_count: 0,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    MergeLaneSnapshot {
        lane_id: LaneId::new(4),
        lane_incarnation: test_hash(b"incarnation"),
        incarnation_activation_height: 2,
        proposal_height: 9,
        dataspace_id: DataSpaceId::new(5),
        lane_block_height: 3,
        tip_hash: HashOf::<BlockHeader>::from_untyped_unchecked(test_hash(b"tip")),
        merge_hint_root: test_hash(b"merge-hint"),
        settlement_hash: HashOf::new(&settlement_commitment),
        settlement_commitment,
        relay_envelope: None,
    }
}

#[test]
fn canonical_merge_admission_codec_binds_every_retained_owner() {
    let mut record = MergeAdmissionState::default();
    let key = (
        LaneId::new(4),
        DataSpaceId::new(5),
        test_hash(b"incarnation"),
    );
    record.binding_history.latest_entry = Some(test_merge_entry());
    record
        .binding_history
        .historical_incarnations
        .insert(test_hash(b"historical"));
    record
        .binding_history
        .latest_activation_by_lane
        .insert(LaneId::new(4), 2);
    record
        .latest_lane_snapshots
        .insert(key, test_lane_snapshot());
    record.latest_execution_frontiers.insert(
        key,
        MergeExecutionFrontier {
            height: 3,
            descriptor_hash: test_hash(b"descriptor"),
        },
    );
    assert_eq!(
        MergeAdmissionState::nominal_name(),
        "iroha_core::state::MergeAdmissionState"
    );
    let frame = norito::encode_canonical(&record).expect("canonical merge admission");
    assert_eq!(norito::canonical_frame_len(&record).unwrap(), frame.len());
    assert_eq!(
        norito::decode_canonical::<MergeAdmissionState>(&frame).unwrap(),
        record
    );
    {
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(norito::encode_canonical(&record).unwrap(), frame);
    }
    let mut wrong_schema = frame.clone();
    wrong_schema[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<MergeAdmissionState>(&wrong_schema),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<MergeAdmissionState>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<MergeAdmissionState>(&trailing).is_err());

    let mut changed = record.clone();
    changed
        .binding_history
        .latest_entry
        .as_mut()
        .unwrap()
        .epoch_id += 1;
    assert_ne!(norito::encode_canonical(&changed).unwrap(), frame);
    let mut changed = record.clone();
    changed
        .binding_history
        .historical_incarnations
        .insert(test_hash(b"other"));
    assert_ne!(norito::encode_canonical(&changed).unwrap(), frame);
    let mut changed = record.clone();
    changed
        .binding_history
        .latest_activation_by_lane
        .insert(LaneId::new(4), 3);
    assert_ne!(norito::encode_canonical(&changed).unwrap(), frame);
    let mut changed = record.clone();
    changed
        .latest_lane_snapshots
        .get_mut(&key)
        .unwrap()
        .lane_block_height += 1;
    assert_ne!(norito::encode_canonical(&changed).unwrap(), frame);
    let mut changed = record;
    changed
        .latest_execution_frontiers
        .get_mut(&key)
        .unwrap()
        .descriptor_hash = test_hash(b"different descriptor");
    assert_ne!(norito::encode_canonical(&changed).unwrap(), frame);
}

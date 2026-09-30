//! Boundary and canonical identity tests for original lane custody obligations.
use super::*;
use crate::nexus::PublicLaneValidatorStatus;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_model_base::{metadata::Metadata, peer::PeerId};

fn fixture() -> (PublicLaneValidatorRecord, AssetId) {
    let key = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
    let account = AccountId::new(key.public_key().clone());
    let record = PublicLaneValidatorRecord {
        lane_id: LaneId::SINGLE,
        validator: account.clone(),
        peer_id: PeerId::new(key.public_key().clone()),
        stake_account: account.clone(),
        total_stake: 100_u32.into(),
        self_stake: 100_u32.into(),
        metadata: Metadata::default(),
        status: PublicLaneValidatorStatus::Active,
        activation_height: 2,
        election_exit_height: None,
        deactivation_height: None,
        last_reward_epoch: None,
    };
    let definition = crate::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("custody", "test").unwrap(),
        "xor".parse().unwrap(),
    );
    (record, AssetId::new(definition, account))
}
fn obligation() -> SumeragiLaneCustody {
    let (record, asset) = fixture();
    let signers = vec![SumeragiLaneSignerCustody {
        signer: 0,
        binding: SumeragiLaneStakeBinding::from_record(&record, &asset).unwrap(),
    }]
    .try_into()
    .unwrap();
    SumeragiLaneCustody {
        lane: LaneId::new(1),
        incarnation: [1; 32],
        instance: [2; 32],
        created_at: 10,
        signer_count: 4,
        signers,
        evidence_horizon: 7,
        slashing_delay: 3,
        retired_at: None,
    }
}
#[test]
fn lane_custody_keeps_native_height_out_of_global_fences() {
    let mut value = obligation();
    value.validate().unwrap();
    assert!(!value.admits_at(10).unwrap());
    assert!(value.admits_at(u64::MAX).unwrap());
    assert!(value.retains_at(u64::MAX).unwrap());
    value.retired_at = Some(20);
    assert_eq!(value.admission_deadline().unwrap(), Some(27));
    assert_eq!(value.release_height().unwrap(), Some(30));
    assert!(value.admits_at(27).unwrap());
    assert!(!value.admits_at(28).unwrap());
    assert!(value.retains_at(29).unwrap());
    assert!(!value.retains_at(30).unwrap());
}
#[test]
fn lane_custody_rejects_overflow_bad_geometry_and_unused_signer_slots() {
    let mut value = obligation();
    value.retired_at = Some(u64::MAX - 7);
    assert!(value.validate().is_err());
    value.retired_at = Some(u64::MAX);
    assert!(value.admission_deadline().is_err());
    value.retired_at = None;
    value.signer_count = 0;
    assert!(value.validate().is_err());
    value.signer_count = 4;
    let original = value.signers.as_slice()[0];
    value.signers = vec![SumeragiLaneSignerCustody {
        signer: 4,
        ..original
    }]
    .try_into()
    .unwrap();
    assert!(value.validate().is_err());
    value.signers = vec![original].try_into().unwrap();
    value.slashing_delay = 0;
    assert!(value.validate().is_err());
}
#[test]
fn lane_custody_roundtrip_includes_forensic_only_signers_and_every_fence() {
    let mut value = obligation();
    value.retired_at = Some(20);
    let bytes = norito::encode_canonical(&value).unwrap();
    assert_eq!(
        norito::decode_canonical::<SumeragiLaneCustody>(&bytes).unwrap(),
        value
    );
    let json = norito::json::to_vec(&value).unwrap();
    assert_eq!(
        norito::json::from_slice::<SumeragiLaneCustody>(&json).unwrap(),
        value
    );
    assert_eq!(value.signers.as_slice().len(), 1);
}
#[test]
fn original_tenure_commitment_excludes_balances_but_rejects_reused_keys_and_assets() {
    let (mut record, asset) = fixture();
    let original = SumeragiLaneStakeBinding::from_record(&record, &asset).unwrap();
    assert!(
        original
            .names_account(record.lane_id, &record.validator)
            .unwrap()
    );
    assert!(
        !original
            .names_account(LaneId::new(7), &record.validator)
            .unwrap()
    );
    record.total_stake = 2_u32.into();
    record.status = PublicLaneValidatorStatus::Exited;
    record.deactivation_height = Some(11);
    assert_eq!(
        SumeragiLaneStakeBinding::from_record(&record, &asset).unwrap(),
        original
    );
    record.activation_height = 7;
    assert_ne!(
        SumeragiLaneStakeBinding::from_record(&record, &asset).unwrap(),
        original
    );
    record.activation_height = 2;
    let definition = crate::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("custody", "test").unwrap(),
        "other".parse().unwrap(),
    );
    let other = AssetId::new(definition, record.validator.clone());
    assert_ne!(
        SumeragiLaneStakeBinding::from_record(&record, &other).unwrap(),
        original
    );
    record.deactivation_height = Some(1);
    assert!(SumeragiLaneStakeBinding::from_record(&record, &asset).is_err());
}

#[test]
fn sparse_custody_preserves_native_committee_geometry_and_exact_index() {
    let mut value = obligation();
    let binding = value.signers.as_slice()[0].binding;
    for count in [1_u32, 2, 4, 7, 32, 1024] {
        value.signer_count = count;
        value.signers = vec![SumeragiLaneSignerCustody {
            signer: count - 1,
            binding,
        }]
        .try_into()
        .unwrap();
        value.validate().unwrap();
        let bytes = norito::encode_canonical(&value).unwrap();
        assert_eq!(
            norito::decode_canonical::<SumeragiLaneCustody>(&bytes).unwrap(),
            value
        );
    }
    value.signer_count = 1025;
    assert!(value.validate().is_err());
    value.signer_count = 1024;
    value.signers = SumeragiLaneCustodySigners::default();
    value.validate().unwrap();
}

#[test]
fn sparse_custody_rejects_duplicate_reordered_and_outside_native_signers() {
    let binding = obligation().signers.as_slice()[0].binding;
    for indices in [vec![1, 1], vec![2, 1], vec![1024]] {
        let rows = indices
            .into_iter()
            .map(|signer| SumeragiLaneSignerCustody { signer, binding })
            .collect::<Vec<_>>();
        assert!(SumeragiLaneCustodySigners::try_from(rows).is_err());
    }
    let json = norito::json::to_json(&vec![
        SumeragiLaneSignerCustody { signer: 0, binding };
        MAX_LANE_CUSTODY_SIGNERS + 1
    ])
    .unwrap();
    assert!(norito::json::from_str::<SumeragiLaneCustodySigners>(&json).is_err());
    let mut value = obligation();
    let future = SumeragiLaneStakeBinding {
        activation_height: value.created_at + 1,
        ..binding
    };
    value.signers = vec![SumeragiLaneSignerCustody {
        signer: 0,
        binding: future,
    }]
    .try_into()
    .unwrap();
    assert!(value.validate().is_err());
}

#[test]
fn sparse_custody_binary_count_is_rejected_before_decoding_rows() {
    use norito::core::{self as ncore, DecodeFromSlice};
    // Explicit layout for this bare-payload preflight test; there are deliberately no rows.
    // The hostile count alone must be refused, without trying to build its backing vector.
    let _flags = ncore::DecodeFlagsGuard::enter(0);
    let mut field = Vec::new();
    ncore::write_seq_len(
        &mut field,
        u64::try_from(MAX_LANE_CUSTODY_SIGNERS).unwrap() + 1,
    )
    .unwrap();
    let mut bytes = Vec::new();
    ncore::write_len_with_flags(&mut bytes, u64::try_from(field.len()).unwrap(), 0).unwrap();
    bytes.extend(field);
    assert!(matches!(
        SumeragiLaneCustodySigners::decode_from_slice(&bytes),
        Err(norito::Error::LengthMismatch)
    ));
}

#[test]
fn custody_json_requires_the_complete_current_state_layout() {
    let value = obligation();
    let mut json = norito::json::to_value(&value).unwrap();
    json.as_object_mut().unwrap().remove("signers");
    assert!(norito::json::from_value::<SumeragiLaneCustody>(json).is_err());
    let mut json = norito::json::to_value(&super::super::SumeragiLaneState::default()).unwrap();
    json.as_object_mut().unwrap().remove("custody");
    assert!(norito::json::from_value::<super::super::SumeragiLaneState>(json).is_err());
}

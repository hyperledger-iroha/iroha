//! Canonical source recipe, exact envelope reconstruction and finite lifetime regressions.

use super::*;
use crate::{
    account::AccountId,
    block::BlockHeader,
    sorafs::{
        capacity::ProviderId,
        reputation::{
            ReputationJournalAuthorityPolicyV1, StreamTokenExcludedKindV1,
            StreamTokenValidationBindingV1, StreamTokenValidationOutcomeV1,
            StreamTokenValidationStatusV1,
        },
        stream_token_gateway::StreamTokenGatewayAdmissionQualificationV1,
    },
};
use iroha_crypto::{HashOf, KeyPair};

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
fn account(seed: u8) -> AccountId {
    AccountId::new(key(seed).public_key().clone())
}
fn intent() -> Intent {
    let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"native delivery model"),
    ));
    let qualification = StreamTokenGatewayAdmissionQualificationV1 {
        gateway_id: [9; 32],
        revision: 1,
        policy_digest: [8; 32],
        max_pending: 64,
        max_tracked_tokens: 64,
        lease_ttl_ms: 120_000,
    };
    let record = Record {
        serving_attempt_id: [7; 32],
        admitted_under: qualification,
        provider_id: ProviderId::new([6; 32]),
        outcome: StreamTokenValidationOutcomeV1 {
            binding: StreamTokenValidationBindingV1 {
                gateway_id: qualification.gateway_id,
                gateway_sequence: 1,
                request_context_digest: [5; 32],
            },
            token_body_digest: Some([4; 32]),
            token_key_version: Some(1),
            validated_at_unix_ms: 1_000,
            status: StreamTokenValidationStatusV1::Accepted,
        },
        retry_after_secs: None,
        lease_id: Some([3; 32]),
        lease_expires_at_unix_ms: Some(61_000),
        lease_token_expires_at_epoch: Some(61),
    };
    let policy = ReputationJournalAuthorityPolicyV1 {
        version: 1,
        revision: 1,
        predecessor_policy_digest: None,
        por_recorder_authority: account(1),
        dispute_recorder_authority: account(2),
        token_recorder_authority: account(3),
        stream_token_delivery: StreamTokenReputationDeliveryTemplateV1 {
            allowed_gateways: vec![qualification.gateway_id],
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            time_to_live_ms: 10_000,
            height_ttl: 16,
        },
        max_source_age_ms: 60_000,
    };
    let policy = ReputationJournalAuthorityPolicyRecordV1::try_new(
        policy,
        account(1),
        500,
        crate::sorafs::reputation::ReputationJournalPolicyOriginV1::Network(Execution {
            height: 2,
            transaction_hash: [15; 32],
            entry_index: 0,
            instruction_index: 0,
            recorded_at_unix_ms: 500,
            authority: account(1),
        }),
    )
    .unwrap();
    let execution = Execution {
        height: 10,
        transaction_hash: [2; 32],
        entry_index: 1,
        instruction_index: 0,
        recorded_at_unix_ms: 2_000,
        authority: account(4),
    };
    Intent::derive(network, record, [1; 32], execution, policy, 20_000).unwrap()
}
use StreamTokenReputationDeliveryIntentV1 as Intent;

#[test]
fn native_delivery_recipe_reconstructs_identical_signed_envelope_without_local_wal() {
    let original = intent();
    let first = TransactionBuilder::from_payload(original.payload.clone())
        .unwrap()
        .try_sign(key(3).private_key())
        .unwrap();
    let retained = norito::encode_canonical(&original).unwrap();
    let recovered: Intent = norito::decode_from_bytes(&retained).unwrap();
    recovered.validate().unwrap();
    let builder = TransactionBuilder::decode_payload(
        &TransactionBuilder::from_payload(recovered.payload.clone())
            .unwrap()
            .encode_payload(),
    )
    .unwrap();
    let expected_id = builder.hash_as_entrypoint();
    let second = builder.try_sign(key(3).private_key()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.hash_as_entrypoint(), expected_id);
    assert_eq!(
        norito::encode_canonical(&first).unwrap(),
        norito::encode_canonical(&second).unwrap()
    );
    assert_eq!(original.digest().unwrap(), recovered.digest().unwrap());
}

#[test]
fn native_delivery_recipe_rejects_every_unsigned_envelope_substitution() {
    let original = intent();
    let mut variants = Vec::new();
    let mut value = original.clone();
    value.payload.creation_time_ms += 1;
    variants.push(value);
    let mut value = original.clone();
    value.payload.time_to_live_ms = NonZeroU64::new(10_001);
    variants.push(value);
    let mut value = original.clone();
    value.payload.nonce = None;
    variants.push(value);
    let mut value = original.clone();
    value.payload.authority = account(4);
    variants.push(value);
    let mut value = original.clone();
    value.payload.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(1));
    variants.push(value);
    let mut value = original.clone();
    value.payload.metadata = Metadata::default();
    variants.push(value);
    let mut value = original.clone();
    value.payload.domain = TransactionDomain::Genesis;
    variants.push(value);
    let mut value = original.clone();
    value.payload.instructions =
        Executable::Instructions(Vec::<crate::isi::InstructionBox>::new().into());
    variants.push(value);
    for value in variants {
        assert_eq!(value.validate(), Err(Error::BindingMismatch));
    }
    let mut rebound = original.clone();
    rebound.record.serving_attempt_id = [88; 32];
    assert_eq!(rebound.validate(), Err(Error::BindingMismatch));
}

#[test]
fn native_delivery_expiry_is_inclusive_original_time_and_height() {
    let value = intent();
    assert!(!value.expired_at(26, 12_000).unwrap());
    assert!(value.expired_at(27, 12_000).unwrap());
    assert!(value.expired_at(26, 12_001).unwrap());
    assert!(value.expired_at(9, 2_000).is_err());
    assert!(value.expired_at(10, 1_999).is_err());
    let bounded = Intent::derive(
        value.network_id,
        value.record,
        value.request_digest,
        value.source_execution.clone(),
        value.recorder_policy.clone(),
        250,
    )
    .unwrap();
    assert_eq!(bounded.payload.time_to_live_ms.unwrap().get(), 250);
    assert!(bounded.expired_at(10, 2_251).unwrap());
    let mut source = value.source_execution.clone();
    source.recorded_at_unix_ms = 60_500;
    let bounded = Intent::derive(
        value.network_id,
        value.record,
        value.request_digest,
        source,
        value.recorder_policy.clone(),
        20_000,
    )
    .unwrap();
    assert_eq!(bounded.payload.time_to_live_ms.unwrap().get(), 500);
}

#[test]
fn native_delivery_counted_intent_never_accepts_an_excluded_source() {
    let value = intent();
    let mut excluded = value.record;
    excluded.outcome.status =
        StreamTokenValidationStatusV1::Excluded(StreamTokenExcludedKindV1::InvalidSignature);
    excluded.lease_id = None;
    excluded.lease_expires_at_unix_ms = None;
    excluded.lease_token_expires_at_epoch = None;
    assert!(
        Intent::derive(
            value.network_id,
            excluded,
            value.request_digest,
            value.source_execution,
            value.recorder_policy,
            20_000
        )
        .is_err()
    );
}

#[test]
fn native_delivery_template_is_closed_bounded_and_strictly_ordered() {
    let value = intent();
    let closed = StreamTokenReputationDeliveryTemplateV1::default();
    closed.validate().unwrap();
    let mut policy = value.recorder_policy.policy.clone();
    policy.stream_token_delivery = closed;
    let record = ReputationJournalAuthorityPolicyRecordV1::try_new(
        policy,
        account(1),
        500,
        crate::sorafs::reputation::ReputationJournalPolicyOriginV1::Network(Execution {
            height: 2,
            transaction_hash: [15; 32],
            entry_index: 0,
            instruction_index: 0,
            recorded_at_unix_ms: 500,
            authority: account(1),
        }),
    )
    .unwrap();
    assert!(
        Intent::derive(
            value.network_id,
            value.record,
            value.request_digest,
            value.source_execution,
            record,
            20_000
        )
        .is_err()
    );
    let template = value.recorder_policy.policy.stream_token_delivery;
    for gateways in [
        vec![[0; 32]],
        vec![[2; 32], [1; 32]],
        vec![[1; 32], [1; 32]],
        (1..=17).map(|n| [n; 32]).collect(),
    ] {
        let mut invalid = template.clone();
        invalid.allowed_gateways = gateways;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = template.clone();
    invalid.time_to_live_ms = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = template;
    invalid.height_ttl = STREAM_TOKEN_REPUTATION_MAX_HEIGHT_TTL_V1 + 1;
    assert!(invalid.validate().is_err());
}

#[test]
fn native_delivery_canonical_binary_json_roundtrip_and_unknown_fields() {
    let value = intent();
    let bytes = norito::encode_canonical(&value).unwrap();
    assert_eq!(norito::decode_from_bytes::<Intent>(&bytes).unwrap(), value);
    let json = norito::json::to_json(&value).unwrap();
    assert_eq!(norito::json::from_str::<Intent>(&json).unwrap(), value);
    let unknown = format!("{{\"unknown\":true,{}", &json[1..]);
    assert!(norito::json::from_str::<Intent>(&unknown).is_err());
    let dispositions = [
        Disposition::Pending,
        Disposition::Excluded,
        Disposition::Delivered {
            journal_sequence: 1,
            event_id: super::super::ReputationJournalEventIdV1([1; 32]),
            execution: value.source_execution.clone(),
        },
        Disposition::Expired {
            execution: value.source_execution.clone(),
        },
        Disposition::GovernanceCancelled {
            recorder_policy_digest: value.recorder_policy.policy_digest,
            gateway_qualification: value.record.admitted_under,
            reason: StreamTokenReputationCancellationReasonV1::CredentialUnavailable,
            execution: value.source_execution.clone(),
        },
    ];
    for disposition in dispositions {
        let json = norito::json::to_json(&disposition).unwrap();
        assert_eq!(
            norito::json::from_str::<Disposition>(&json).unwrap(),
            disposition
        );
        let bytes = norito::encode_canonical(&disposition).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<Disposition>(&bytes).unwrap(),
            disposition
        );
    }
}
use StreamTokenReputationDeliveryDispositionV1 as Disposition;

#[test]
fn native_delivery_rejects_inert_network_and_substituted_source_or_policy_origin() {
    let original = intent();
    let zero_network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0; 32]),
    ));
    assert!(
        Intent::derive(
            zero_network,
            original.record,
            original.request_digest,
            original.source_execution.clone(),
            original.recorder_policy.clone(),
            original.transaction_ttl_limit_ms
        )
        .is_err()
    );
    for change in 0..2 {
        let mut source = original.source_execution.clone();
        match change {
            0 => source.height = 1,
            1 => source.transaction_hash = [0; 32],
            _ => unreachable!(),
        }
        assert!(
            Intent::derive(
                original.network_id,
                original.record,
                original.request_digest,
                source,
                original.recorder_policy.clone(),
                original.transaction_ttl_limit_ms
            )
            .is_err()
        );
    }
    let mut ordinal_source = original.source_execution.clone();
    ordinal_source.instruction_index = 3;
    let ordinal = Intent::derive(
        original.network_id,
        original.record,
        original.request_digest,
        ordinal_source,
        original.recorder_policy.clone(),
        original.transaction_ttl_limit_ms,
    )
    .unwrap();
    assert_eq!(ordinal.source_execution.instruction_index, 3);
    assert_ne!(
        ordinal.payload, original.payload,
        "the recipe binds the original signed gateway ordinal"
    );
    let mut policy = original.recorder_policy.clone();
    let crate::sorafs::reputation::ReputationJournalPolicyOriginV1::Network(ref mut execution) =
        policy.origin
    else {
        unreachable!()
    };
    execution.authority = account(9);
    assert!(policy.validate().is_err());
    let mut policy = original.recorder_policy;
    let crate::sorafs::reputation::ReputationJournalPolicyOriginV1::Network(ref mut execution) =
        policy.origin
    else {
        unreachable!()
    };
    execution.height = 1;
    assert!(policy.validate().is_err());
}

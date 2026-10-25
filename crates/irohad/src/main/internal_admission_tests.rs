//! Internal daemon admission retains committed policy and original decoder refusals.

use super::*;
use iroha_core::tx::{AcceptTransactionFail, SignatureRejectionCode};
use iroha_data_model::{
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    parameter::system::{ConsensusFingerprint, SumeragiConsensusMode, TransactionParameter},
    transaction::FeePaymentIntent,
};
use std::{num::NonZeroU64, time::Duration};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"internal daemon admission fixture"),
    ))
}

fn transaction() -> SignedTransaction {
    let key = iroha_crypto::KeyPair::try_from_seed(vec![0x45; 32], Algorithm::Ed25519).unwrap();
    let mut builder = TransactionBuilder::new(
        network(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "internal fixture".to_owned())]);
    // A fixed full-range lifetime makes these policy probes independent of the wall clock.
    builder.set_creation_time(Duration::ZERO);
    builder.set_ttl(Duration::from_millis(u64::MAX));
    builder.try_sign(key.private_key()).unwrap()
}

fn publish(world: &World, algorithm: Algorithm, max_bytes: u64) {
    let mut block = world.block();
    block
        .set_parameter_for_testing(
            CryptoAdmissionPolicyV1::new(vec![algorithm])
                .unwrap()
                .into_parameter()
                .unwrap(),
        )
        .unwrap();
    block
        .set_parameter_for_testing(Parameter::Transaction(TransactionParameter::MaxTxBytes(
            NonZeroU64::new(max_bytes).unwrap(),
        )))
        .unwrap();
    block
        .set_parameter_for_testing(Parameter::Transaction(
            TransactionParameter::MaxTimeToLiveMs(NonZeroU64::MAX),
        ))
        .unwrap();
    block.commit();
}

#[test]
fn internal_admission_requires_an_original_committed_crypto_policy() {
    let world = World::new();
    let view = world.view();
    let error = accept_internal_transaction(
        transaction(),
        &network(),
        view.transaction_admission_parameters(),
    )
    .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<iroha_core::execution_attempt::ExecutionDeferred>()
            .expect("missing policy is an unfinished attempt")
            .reason(),
        ivm::error::ExecutionDeferral::CryptoAdmissionPolicyUnavailable
    );
    assert!(
        view.transaction_admission_parameters()
            .crypto_policy()
            .is_none()
    );
}

#[test]
fn internal_admission_keeps_policy_and_limits_from_the_same_retained_view() {
    let world = World::new();
    publish(&world, Algorithm::Ed25519, 131_072);
    let original = world.view();
    let signed = transaction();
    accept_internal_transaction(
        signed.clone(),
        &network(),
        original.transaction_admission_parameters(),
    )
    .unwrap();

    // Keep signing permitted so this observation reaches its committed byte limit.
    publish(&world, Algorithm::Ed25519, 1);
    let bounded = world.view();
    let error = accept_internal_transaction(
        signed.clone(),
        &network(),
        bounded.transaction_admission_parameters(),
    )
    .unwrap_err();
    let Some(AcceptTransactionFail::TransactionLimit(failure)) =
        error.downcast_ref::<AcceptTransactionFail>()
    else {
        panic!("the retained byte limit must reject this transaction: {error:?}");
    };
    assert!(
        failure.reason.contains("Transaction size")
            && failure.reason.contains("exceeds limit 1 bytes"),
        "expected the committed one-byte limit, got {failure:?}"
    );
    accept_internal_transaction(
        signed.clone(),
        &network(),
        original.transaction_admission_parameters(),
    )
    .unwrap();

    publish(&world, Algorithm::Secp256k1, 262_144);
    let restricted = world.view();
    let error = accept_internal_transaction(
        signed.clone(),
        &network(),
        restricted.transaction_admission_parameters(),
    )
    .unwrap_err();
    let Some(AcceptTransactionFail::SignatureVerification(failure)) =
        error.downcast_ref::<AcceptTransactionFail>()
    else {
        panic!("the original committed policy must reject the transaction algorithm: {error:?}");
    };
    assert_eq!(
        failure.code(),
        SignatureRejectionCode::AlgorithmNotPermitted
    );
    accept_internal_transaction(
        signed,
        &network(),
        original.transaction_admission_parameters(),
    )
    .unwrap();
    assert_eq!(
        bounded
            .transaction_admission_parameters()
            .transaction()
            .max_tx_bytes
            .get(),
        1
    );
}

fn handshake() -> ConsensusHandshakeMeta {
    ConsensusHandshakeMeta {
        mode: SumeragiConsensusMode::Permissioned,
        block_cadence_ms: NonZeroU64::new(1_000).unwrap(),
        wire_protocol_version: u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION),
        consensus_fingerprint: ConsensusFingerprint::new([0x56; 32]),
        sumeragi_context:
            iroha_data_model::block::consensus::SumeragiGenesisContextParameters::recommended(),
    }
}

#[test]
fn handshake_parser_keeps_intrinsic_validation_failures() {
    let malformed = parse_handshake_meta_str("{").unwrap_err();
    assert!(
        matches!(&malformed, norito::Error::Json(_)),
        "ordinary malformed JSON must retain its JSON category: {malformed:?}"
    );
    let original = handshake();
    assert_eq!(
        parse_handshake_meta_str(&norito::json::to_json(&original).unwrap()).unwrap(),
        original
    );
    for invalid_context in [false, true] {
        let mut invalid = original;
        if invalid_context {
            invalid.sumeragi_context.da_layout.parity_shards = 0;
        } else {
            invalid.wire_protocol_version = 2;
        }
        let expected = invalid.validate().unwrap_err().to_string();
        let error =
            parse_handshake_meta_str(&norito::json::to_json(&invalid).unwrap()).unwrap_err();
        assert!(matches!(error, norito::Error::Message(message) if message == expected));
    }
}

#[test]
fn handshake_parser_preserves_enclosing_decoder_refusal_and_retry() {
    let original = handshake();
    let source = norito::json::to_json(&original).unwrap();
    let mut observed = None;
    let error = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || {
            norito::core::classify_decode_attempt(|| {
                let parsed = parse_handshake_meta_str(&source);
                if let Err(norito::Error::ScopedDecodeResource(original)) = &parsed {
                    observed = Some(original.clone());
                }
                parsed
            })
        },
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit,
        "the actual parser refusal must retain its enclosing owner: {error:?}"
    );
    let observed = observed.expect("the parser returns the original scoped refusal");
    assert!(
        matches!(
            error.error(),
            norito::Error::ScopedDecodeResource(original) if original == &observed
        ),
        "classification must preserve the exact original scoped refusal: {error:?}"
    );
    assert_eq!(parse_handshake_meta_str(&source).unwrap(), original);
}

#[test]
fn handshake_payload_preserves_enclosing_decoder_refusal_and_retry() {
    let original = handshake();
    let payload = Json::new(original);
    let original_payload = payload.get().to_owned();
    assert_eq!(decode_consensus_handshake_meta(&payload).unwrap(), original);

    let mut observed = None;
    let error = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        || {
            norito::core::classify_decode_attempt(|| {
                let decoded = decode_consensus_handshake_meta(&payload);
                if let Err(norito::Error::ScopedDecodeResource(original)) = &decoded {
                    observed = Some(original.clone());
                }
                decoded
            })
        },
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    let observed = observed.expect("the payload boundary returns the original scoped refusal");
    assert!(matches!(
        error.error(),
        norito::Error::ScopedDecodeResource(original) if original == &observed
    ));
    assert!(matches!(
        error.error().decode_resource_error(),
        Some(norito::core::DecodeResourceError::TotalAllocationExceeded {
            attempted,
            limit: 0,
        }) if attempted > 0
    ));
    assert_eq!(payload.get(), original_payload.as_str());
    assert_eq!(decode_consensus_handshake_meta(&payload).unwrap(), original);
    assert_eq!(payload.get(), original_payload.as_str());
}

#[test]
fn handshake_payload_keeps_completed_semantic_rejection() {
    let original = handshake();
    for invalid_context in [false, true] {
        let mut invalid = original;
        if invalid_context {
            invalid.sumeragi_context.da_layout.parity_shards = 0;
        } else {
            invalid.wire_protocol_version = 2;
        }
        let expected = invalid.validate().unwrap_err().to_string();
        let payload = Json::new(invalid);
        let original_payload = payload.get().to_owned();
        let error =
            norito::core::classify_decode_attempt(|| decode_consensus_handshake_meta(&payload))
                .unwrap_err();
        assert_eq!(error.kind(), norito::core::DecodeAttemptErrorKind::Invalid);
        assert!(error.local_refusal().is_none());
        assert!(matches!(
            error.error(),
            norito::Error::Message(message) if message == &expected
        ));
        assert_eq!(payload.get(), original_payload.as_str());
    }
}

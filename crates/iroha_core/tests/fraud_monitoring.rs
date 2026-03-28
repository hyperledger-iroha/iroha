//! Fraud monitoring admission tests ensure configuration knobs gate transaction acceptance.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{borrow::Cow, str::FromStr, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_config::parameters::actual::{FraudAttester, FraudMonitoring, FraudRiskBand};
use iroha_core::{
    block::BlockBuilder,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, World},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    ValidationFail,
    asset::AssetDefinition,
    block::{BlockHeader, SignedBlock},
    fraud::types::{AssessmentDecision, FraudAssessment, FraudAssessmentParts},
    metadata::Metadata,
    name::Name,
    prelude::*,
    transaction::error::TransactionRejectionReason,
};
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;
use norito::codec::Encode;

fn build_state() -> (State, ChainId, AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let key_pair = KeyPair::random();
    let (public_key, _) = key_pair.clone().into_parts();
    let domain_id: DomainId = "wonderland".parse().expect("static domain id");
    let account_id = AccountId::of(public_key);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new_in_domain(account_id.clone(), domain_id).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());

    let mut state = State::new_for_testing(world, kura, query_handle);
    let chain_id = ChainId::from("fraud-monitor-chain");
    state.chain_id = chain_id.clone();

    (state, chain_id, account_id, key_pair)
}

fn build_header() -> BlockHeader {
    BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0)
}

fn make_transaction(
    chain_id: &ChainId,
    authority: &AccountId,
    key_pair: &KeyPair,
    metadata: Metadata,
) -> AcceptedTransaction<'static> {
    let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, "noop".to_string())])
        .with_metadata(metadata)
        .sign(key_pair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(tx))
}

fn metadata_key(key: &str) -> Name {
    Name::from_str(key).expect("static metadata key")
}

fn insert_base_metadata(
    metadata: &mut Metadata,
    band: &str,
    score_bps: u64,
    tenant: &str,
    latency_ms: u64,
) {
    metadata.insert(metadata_key("fraud_assessment_band"), Json::new(band));
    metadata.insert(
        metadata_key("fraud_assessment_score_bps"),
        Json::new(score_bps),
    );
    metadata.insert(metadata_key("fraud_assessment_tenant"), Json::new(tenant));
    metadata.insert(
        metadata_key("fraud_assessment_latency_ms"),
        Json::new(latency_ms),
    );
}

fn build_attested_metadata(
    band: &str,
    score_bps: u16,
    tenant: &str,
    latency_ms: u64,
    engine_id: &str,
    attester: &KeyPair,
) -> Metadata {
    let mut metadata = Metadata::default();
    insert_base_metadata(
        &mut metadata,
        band,
        u64::from(score_bps),
        tenant,
        latency_ms,
    );

    let parts = FraudAssessmentParts {
        query_id: [0xAB; 32],
        engine_id: engine_id.to_string(),
        risk_score_bps: score_bps,
        confidence_bps: 9_000,
        decision: AssessmentDecision::Review,
        generated_at_ms: 1_700_000_000_000,
        signature: None,
    };
    let unsigned = FraudAssessment::new(Vec::new(), parts);
    let signature = SignatureOf::new(attester.private_key(), &unsigned);
    let raw_signature: Signature = signature.clone().into();
    let signature_bytes = raw_signature.payload().to_vec();
    let mut signed_assessment = unsigned.clone();
    signed_assessment.signature = Some(signature_bytes);

    let envelope_bytes = signed_assessment.encode();
    let envelope_b64 = BASE64_STANDARD.encode(envelope_bytes);
    metadata.insert(
        metadata_key("fraud_assessment_envelope"),
        Json::new(envelope_b64),
    );

    let unsigned_bytes = unsigned.encode();
    let digest_bytes: [u8; 32] = Hash::new(&unsigned_bytes).into();
    let digest_hex = hex::encode_upper(digest_bytes);
    metadata.insert(
        metadata_key("fraud_assessment_digest"),
        Json::new(digest_hex),
    );

    metadata
}

#[test]
fn admission_allows_when_fraud_disabled() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let cfg = FraudMonitoring {
        enabled: false,
        required_minimum_band: Some(FraudRiskBand::Medium),
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let tx = make_transaction(&chain_id, &authority, &key_pair, Metadata::default());
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    assert!(
        result.is_ok(),
        "disabled fraud monitoring should permit tx: {result:?}"
    );
}

#[test]
fn admission_rejects_missing_assessment_when_required() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::High),
        missing_assessment_grace: Duration::from_secs(0),
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let tx = make_transaction(&chain_id, &authority, &key_pair, Metadata::default());
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    let err = result.expect_err("missing assessment must reject");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
            assert!(
                msg.contains("fraud monitoring requires an attached assessment"),
                "unexpected rejection message: {msg}"
            );
        }
        other => panic!("unexpected rejection reason: {other:?}"),
    }
}

#[test]
fn block_pipeline_rejects_missing_assessment() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::High),
        missing_assessment_grace: Duration::from_secs(0),
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let tx = make_transaction(&chain_id, &authority, &key_pair, Metadata::default());
    let genesis_block = SignedBlock::genesis(
        vec![tx.as_ref().clone()],
        key_pair.private_key(),
        None,
        None,
    );
    let block_builder = BlockBuilder::new(vec![tx]);
    let new_block = block_builder
        .chain(0, Some(&genesis_block))
        .sign(key_pair.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(new_block.header());
    let valid_block = new_block
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let signed_block: SignedBlock = valid_block.into();
    let err = signed_block
        .error(0)
        .expect("fraud monitoring should reject missing assessment");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
            assert!(
                msg.contains("fraud monitoring requires an attached assessment"),
                "unexpected rejection: {msg}"
            );
        }
        other => panic!("unexpected rejection reason: {other:?}"),
    }
}

#[test]
fn admission_rejects_when_band_insufficient() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::High),
        missing_assessment_grace: Duration::from_secs(0),
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let mut metadata = Metadata::default();
    insert_base_metadata(&mut metadata, "medium", 450, "tenant-eu", 85);

    let tx = make_transaction(&chain_id, &authority, &key_pair, metadata);
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    let err = result.expect_err("insufficient band must reject");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
            assert!(
                msg.contains("below required minimum"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("unexpected rejection reason: {other:?}"),
    }
}

#[test]
fn admission_allows_when_band_sufficient() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::Medium),
        missing_assessment_grace: Duration::from_secs(0),
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let mut metadata = Metadata::default();
    insert_base_metadata(&mut metadata, "high", 6_800, "tenant-eu", 92);

    let tx = make_transaction(&chain_id, &authority, &key_pair, metadata);
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    assert!(
        result.is_ok(),
        "sufficient band should permit tx: {result:?}"
    );
}

#[test]
fn admission_rejects_missing_attestation_when_required() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let attester = KeyPair::random();
    let (public_key, _) = attester.clone().into_parts();
    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::Medium),
        missing_assessment_grace: Duration::from_secs(0),
        attesters: vec![FraudAttester {
            engine_id: "risk-engine-eu1".to_string(),
            public_key,
        }],
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let mut metadata = Metadata::default();
    insert_base_metadata(&mut metadata, "high", 6_800, "tenant-eu", 64);

    let tx = make_transaction(&chain_id, &authority, &key_pair, metadata);
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    let err = result.expect_err("attestation metadata must be required");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
            assert!(
                msg.contains("fraud_assessment_envelope"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("unexpected rejection reason: {other:?}"),
    }
}

#[test]
fn admission_rejects_attestation_signature_mismatch() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let attester = KeyPair::random();
    let (public_key, _) = attester.clone().into_parts();
    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::Medium),
        missing_assessment_grace: Duration::from_secs(0),
        attesters: vec![FraudAttester {
            engine_id: "risk-engine-eu1".to_string(),
            public_key,
        }],
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let mut metadata = build_attested_metadata(
        "high",
        6_600,
        "tenant-eu",
        120,
        "risk-engine-eu1",
        &attester,
    );

    let envelope_key = metadata_key("fraud_assessment_envelope");
    let envelope_b64 = metadata
        .get(envelope_key.as_ref())
        .and_then(|value| value.try_into_any_norito::<String>().ok())
        .expect("envelope present");
    let envelope_bytes = BASE64_STANDARD
        .decode(envelope_b64.as_bytes())
        .expect("valid base64");
    let mut cursor = envelope_bytes.as_slice();
    let mut decoded: FraudAssessment =
        norito::codec::Decode::decode(&mut cursor).expect("decode envelope");
    let signature = decoded
        .signature
        .as_mut()
        .expect("signature present in envelope");
    signature[0] ^= 0xFF;
    let tampered_bytes = decoded.encode();
    let tampered_b64 = BASE64_STANDARD.encode(tampered_bytes);
    metadata.insert(envelope_key, Json::new(tampered_b64));

    let tx = make_transaction(&chain_id, &authority, &key_pair, metadata);
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    let err = result.expect_err("tampered signature must reject");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
            assert!(
                msg.contains("signature failed verification"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("unexpected rejection reason: {other:?}"),
    }
}

#[test]
fn admission_allows_with_valid_attestation() {
    let (mut state, chain_id, authority, key_pair) = build_state();

    let attester = KeyPair::random();
    let (public_key, _) = attester.clone().into_parts();
    let cfg = FraudMonitoring {
        enabled: true,
        required_minimum_band: Some(FraudRiskBand::Medium),
        missing_assessment_grace: Duration::from_secs(0),
        attesters: vec![FraudAttester {
            engine_id: "risk-engine-eu1".to_string(),
            public_key,
        }],
        ..Default::default()
    };
    state.set_fraud_monitoring(cfg);

    let metadata = build_attested_metadata(
        "critical",
        9_100,
        "tenant-eu",
        75,
        "risk-engine-eu1",
        &attester,
    );

    let tx = make_transaction(&chain_id, &authority, &key_pair, metadata);
    let header = build_header();
    let mut block = state.block(header);
    let mut cache = IvmCache::new();

    let (_, result) = block.validate_transaction(tx, &mut cache);
    assert!(result.is_ok(), "valid attestation must be accepted");
}

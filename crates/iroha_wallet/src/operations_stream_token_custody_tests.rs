//! Request, canonical-frame and original-journal custody controls using public synthetic fixtures.
//!
//! These tests exercise the existing wallet, Manifest and native record owners. Fixture signatures
//! are generated offline from deterministic public seeds; no node, signer service or live wallet
//! is contacted, and a structurally checked caller record is never treated as finality evidence.

use super::*;
use crate::operations::tests::fixture_config;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::transaction::{FeeChargeKind, FeeChargeLimit, TransactionBuilder};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyActiveHeadV1,
        SignerCustodyAuthorityV1, SignerCustodyStatementV1,
    },
    protocol::SignerKeyAlgorithmV1,
};
use std::time::Instant;

const VALIDATED_AT: u64 = 1_500;
const ORIGINAL_DEADLINE: u64 = 1_900;

struct Fixture {
    config: Config,
    provider: ProviderId,
    policy: SignerCustodyPolicyV1,
    attester: KeyPair,
}

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic public fixture key")
}

fn fixture() -> Fixture {
    let config = fixture_config();
    let provider = ProviderId::new([0x31; 32]);
    let attester = key(0x43);
    let policy = SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: config.chain.to_string(),
            network_id: *config.network_id.as_bytes(),
            runtime_handle: "software://sorafs/stream/primary".into(),
            key_handle: "software://production/stream/key-1".into(),
            service_id: "stream-service".into(),
            administrator_id: "stream-admin".into(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: *provider.as_bytes(),
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: key(0x42).public_key().clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x51; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-service".into(),
            administrator_id: "custody-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x52; 32],
        },
        attester_public_key: attester.public_key().clone(),
        active_from_unix_ms: 900,
        active_until_unix_ms: 3_000,
        max_validity_ms: 1_000,
        max_anchor_age_ms: 100,
    };
    policy
        .validate()
        .expect("independent shared custody policy");
    Fixture {
        config,
        provider,
        policy,
        attester,
    }
}

fn options() -> BoundedTransactionOptions {
    BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::from([(
            XOR_ASSET_DEFINITION.parse().expect("canonical fixture XOR"),
            Quantity::from(10_u32),
        )]),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}

fn configure_request(fixture: &Fixture) -> StreamTokenCustodyConfigureRequest {
    StreamTokenCustodyConfigureRequest {
        selection: StreamTokenCustodySelection {
            provider_id: fixture.provider,
            binding: fixture.policy.binding.clone(),
            expected_revision: 0,
            expected_digest: [0; 32],
            current: None,
        },
        policy: fixture.policy.clone(),
        deadline_unix_ms: ORIGINAL_DEADLINE,
        options: options(),
    }
}

fn native_record(
    fixture: &Fixture,
    state: &SignerCustodyControlStateV1,
) -> StreamTokenCustodyControlRecordV1 {
    let configure = MutateSorafsStreamTokenCustody {
        provider_id: fixture.provider,
        expected_revision: 0,
        expected_digest: [0; 32],
        action: SorafsStreamTokenCustodyActionV1::Configure(
            encode_bounded(&fixture.policy, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)
                .expect("exact policy request frame"),
        ),
    };
    StreamTokenCustodyControlRecordV1 {
        provider_id: fixture.provider,
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest:
            iroha_data_model::sorafs::stream_token_custody::stream_token_custody_request_digest_v1(
                &configure,
                &fixture.config.account,
            )
            .expect("actual authority-bound native request digest"),
        execution_height: 10,
        ordinal: 0,
        recorded_at_unix_ms: 900,
        authority: fixture.config.account.clone(),
        control_state: encode_bounded(state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)
            .expect("shared control frame"),
        active_enrollment: None,
    }
}

fn configured_selection(fixture: &Fixture) -> StreamTokenCustodySelection {
    let state = configure_signer_custody_policy_v1(None, fixture.policy.clone())
        .expect("actual initial shared transition");
    let record = native_record(fixture, &state);
    StreamTokenCustodySelection {
        provider_id: fixture.provider,
        binding: fixture.policy.binding.clone(),
        expected_revision: record.revision,
        expected_digest: record
            .canonical_digest()
            .expect("native predecessor digest"),
        current: Some(record),
    }
}

fn refresh_digest(selection: &mut StreamTokenCustodySelection) {
    selection.expected_digest = selection
        .current
        .as_ref()
        .expect("selected predecessor")
        .canonical_digest()
        .expect("bounded native predecessor digest");
}

fn attest(statement: SignerCustodyStatementV1, attester: &KeyPair) -> Vec<u8> {
    let payload = statement
        .signing_payload()
        .expect("shared canonical signing payload");
    let signature = Signature::try_new(attester.private_key(), &payload)
        .expect("offline synthetic attestation");
    encode_bounded(
        &SignerCustodyRecordV1 {
            statement,
            attestation: signature
                .payload()
                .try_into()
                .expect("Ed25519 signature size"),
        },
        SIGNER_CUSTODY_MAX_BYTES_V1,
    )
    .expect("canonical enrollment")
}

fn enroll_request(fixture: &Fixture) -> StreamTokenCustodyEnrollRequest {
    let selection = configured_selection(fixture);
    let anchor = SignerCustodyAnchorV1 {
        height: 12,
        block_hash: [0x62; 32],
        state_digest: selection.expected_digest,
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: fixture.policy.binding.clone(),
        authority: fixture.policy.attester_authority.clone(),
        anchor,
        sequence: 1,
        predecessor_digest: [0; 32],
        issued_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
        evidence_digest: [0x63; 32],
        revoked: false,
    };
    StreamTokenCustodyEnrollRequest {
        selection,
        anchor,
        anchor_observed_at_unix_ms: 1_450,
        issued_at_unix_ms: statement.issued_at_unix_ms,
        expires_at_unix_ms: statement.expires_at_unix_ms,
        enrollment: attest(statement, &fixture.attester),
        deadline_unix_ms: ORIGINAL_DEADLINE,
        options: options(),
    }
}

fn journal(
    config: &Config,
    plan: &Plan,
    options: &BoundedTransactionOptions,
) -> TransactionJournal {
    let mut terms = BoundedTerms::new(options).expect("actual bounded fee owner");
    terms.deadline_ms = plan.deadline_unix_ms;
    let bytes = encode_bounded(plan, MAX_PLAN_BYTES).expect("original plan");
    let operation = match &plan.action {
        Action::Configure(_) => NativeOperation::StreamTokenCustodyConfigure { plan: bytes, terms },
        Action::Enroll { .. } => NativeOperation::StreamTokenCustodyEnroll { plan: bytes, terms },
    };
    let mut builder = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        options.fee_payment.clone(),
    )
    .with_instructions(
        operation
            .instructions(config)
            .expect("actual planned instructions"),
    );
    builder.set_creation_time(Duration::from_millis(plan.validated_at_unix_ms));
    builder.set_ttl(Duration::from_millis(
        plan.deadline_unix_ms - plan.validated_at_unix_ms,
    ));
    let signed = builder
        .try_sign(config.key_pair.private_key())
        .expect("offline public wallet fixture signature");
    let quote = FeeQuoteResponse {
        intent: options.fee_payment.clone(),
        observation: iroha_torii_shared::FeeQuoteObservation {
            ledger_time_ms: plan.validated_at_unix_ms,
            next_block_height: 13,
            route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        },
        components: Vec::new(),
        capacities: Vec::new(),
        decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
            debit_source: iroha_data_model::nexus::FeeDebitSource::Account(config.account.clone()),
            program_revision: None,
        },
    };
    let record = TransactionJournal {
        schema: "iroha.wallet.native-transaction.v1".into(),
        torii_url: config.torii_api_url.to_string(),
        chain_id: config.chain.to_string(),
        network_id: config.network_id,
        chain_discriminant: config.account_chain_discriminant,
        account_id: config.account.clone(),
        operation,
        requested_fee: options.fee_payment.clone(),
        quote,
        transaction_hash: signed.hash().to_string(),
        signed_transaction_hex: hex::encode(signed.encode_versioned()),
        deadline_ms: transaction_deadline(&signed).expect("original transaction expiry"),
    };
    record
        .verify(config)
        .expect("complete original signed journal");
    record
}

#[test]
fn first_configuration_emits_exact_native_target_absent_cas_and_shared_policy() {
    let fixture = fixture();
    let request = configure_request(&fixture);
    let plan = CustodyExpectation::Configure(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    let expected: InstructionBox = MutateSorafsStreamTokenCustody {
        provider_id: fixture.provider,
        expected_revision: 0,
        expected_digest: [0; 32],
        action: SorafsStreamTokenCustodyActionV1::Configure(
            encode_bounded(&request.policy, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap(),
        ),
    }
    .into();
    assert_eq!(plan.instruction(&fixture.config).unwrap(), expected);
    let state = configure_signer_custody_policy_v1(None, request.policy).unwrap();
    assert_eq!(state.next_sequence, 1);
    assert_eq!(state.predecessor_digest, [0; 32]);
    assert!(state.active_head.is_none());
}

#[test]
fn configuration_retains_exact_predecessor_cas_and_shared_generation_rules() {
    let fixture = fixture();
    let mut request = configure_request(&fixture);
    request.selection = configured_selection(&fixture);
    let unchanged = CustodyExpectation::Configure(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    assert!(unchanged.instruction(&fixture.config).is_err());
    request.policy.binding.policy_revision += 1;
    request.policy.binding.policy_digest = [0x71; 32];
    request.selection.binding = request.policy.binding.clone();
    let plan = CustodyExpectation::Configure(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    let expected: InstructionBox = MutateSorafsStreamTokenCustody {
        provider_id: fixture.provider,
        expected_revision: request.selection.expected_revision,
        expected_digest: request.selection.expected_digest,
        action: SorafsStreamTokenCustodyActionV1::Configure(
            encode_bounded(&request.policy, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap(),
        ),
    }
    .into();
    assert_eq!(plan.instruction(&fixture.config).unwrap(), expected);
    request.policy.binding.policy_revision = 0;
    request.selection.binding = request.policy.binding.clone();
    assert!(
        CustodyExpectation::Configure(&request)
            .plan(VALIDATED_AT)
            .unwrap()
            .instruction(&fixture.config)
            .is_err()
    );
}

#[test]
fn selection_rejects_network_provider_purpose_revision_and_absent_cas_substitution() {
    let fixture = fixture();
    let original = configure_request(&fixture).selection;
    let mutations: [fn(&mut StreamTokenCustodySelection); 7] = [
        |value| value.binding.chain_id = "another-chain".into(),
        |value| value.binding.network_id = [0x72; 32],
        |value| value.provider_id = ProviderId::new([0x73; 32]),
        |value| {
            value.binding.role = SignerRoleV1::Promotion;
            value.binding.purpose = SignerPurposeBindingV1::NativeOrPromotion;
        },
        |value| value.expected_revision = STREAM_TOKEN_CUSTODY_NORMAL_REVISIONS_V1,
        |value| value.expected_revision = 1,
        |value| value.expected_digest = [0x74; 32],
    ];
    assert!(original.validate(&fixture.config).unwrap().is_none());
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut value = original.clone();
        mutate(&mut value);
        assert!(
            value.validate(&fixture.config).is_err(),
            "target mutation {index}"
        );
    }
}

#[test]
fn predecessor_requires_digest_and_native_provenance_even_after_rehashing() {
    let fixture = fixture();
    let original = configured_selection(&fixture);
    assert!(original.validate(&fixture.config).unwrap().is_some());
    let mut substituted = original.clone();
    substituted.current.as_mut().unwrap().ordinal += 1;
    assert!(
        substituted.validate(&fixture.config).is_err(),
        "exact original digest"
    );
    let mutations: [fn(&mut StreamTokenCustodyControlRecordV1); 8] = [
        |record| record.provider_id = ProviderId::new([0x75; 32]),
        |record| record.revision = 0,
        |record| record.revision = 2,
        |record| record.predecessor_digest = [0x76; 32],
        |record| record.request_digest = [0; 32],
        |record| record.execution_height = 0,
        |record| record.recorded_at_unix_ms = 0,
        |record| record.recorded_at_unix_ms = u64::MAX,
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut value = original.clone();
        mutate(value.current.as_mut().unwrap());
        refresh_digest(&mut value);
        assert!(
            value.validate(&fixture.config).is_err(),
            "provenance mutation {index}"
        );
    }
}

#[test]
fn predecessor_rejects_corrupt_control_and_another_provider_scope() {
    let fixture = fixture();
    let mut corrupt = configured_selection(&fixture);
    corrupt.current.as_mut().unwrap().control_state[0] ^= 1;
    refresh_digest(&mut corrupt);
    assert!(corrupt.validate(&fixture.config).is_err());
    let mut substituted = configured_selection(&fixture);
    let mut state = substituted.validate(&fixture.config).unwrap().unwrap();
    state.policy.binding.purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x77; 32],
    };
    state.validate().unwrap();
    substituted.current.as_mut().unwrap().control_state =
        encode_bounded(&state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap();
    refresh_digest(&mut substituted);
    assert!(substituted.validate(&fixture.config).is_err());
    assert!(current_policy(&configure_request(&fixture).selection).is_err());
}

#[test]
fn signer_attester_and_manager_are_independently_bound() {
    let fixture = fixture();
    let request = configure_request(&fixture);
    let plan = CustodyExpectation::Configure(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    assert!(plan.instruction(&fixture.config).is_ok());
    for manager in [key(0x42), key(0x43)] {
        let mut config = fixture.config.clone();
        config.account = AccountId::new(manager.public_key().clone());
        config.key_pair = manager;
        assert!(
            plan.instruction(&config).is_err(),
            "manager cannot hold either custody key"
        );
    }
    let mut self_attested = request.clone();
    self_attested.policy.attester_public_key = self_attested.policy.binding.public_key.clone();
    assert!(
        CustodyExpectation::Configure(&self_attested)
            .plan(VALIDATED_AT)
            .unwrap()
            .instruction(&fixture.config)
            .is_err()
    );
    let mut shared_identity = request;
    shared_identity.policy.attester_authority.administrator_id =
        shared_identity.policy.binding.service_id.clone();
    assert!(
        CustodyExpectation::Configure(&shared_identity)
            .plan(VALIDATED_AT)
            .unwrap()
            .instruction(&fixture.config)
            .is_err()
    );
}

#[test]
fn enrollment_emits_original_signed_frame_and_exact_configured_cas() {
    let fixture = fixture();
    let request = enroll_request(&fixture);
    let plan = CustodyExpectation::Enroll(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    let expected: InstructionBox = MutateSorafsStreamTokenCustody {
        provider_id: request.selection.provider_id,
        expected_revision: request.selection.expected_revision,
        expected_digest: request.selection.expected_digest,
        action: SorafsStreamTokenCustodyActionV1::Enroll(request.enrollment.clone()),
    }
    .into();
    assert_eq!(plan.instruction(&fixture.config).unwrap(), expected);
    let original: SignerCustodyRecordV1 =
        decode_bounded(&request.enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
    assert_eq!(original.statement.anchor, request.anchor);
    assert_eq!(original.statement.binding, request.selection.binding);
    assert_eq!(
        original.statement.issued_at_unix_ms,
        request.issued_at_unix_ms
    );
    assert_eq!(
        original.statement.expires_at_unix_ms,
        request.expires_at_unix_ms
    );
}

#[test]
fn enrollment_rejects_substituted_original_interval_anchor_and_predecessor() {
    let fixture = fixture();
    let original = enroll_request(&fixture);
    let mutations: [fn(&mut StreamTokenCustodyEnrollRequest); 8] = [
        |request| request.issued_at_unix_ms += 1,
        |request| request.expires_at_unix_ms -= 1,
        |request| request.anchor.block_hash = [0x78; 32],
        |request| request.anchor.state_digest = [0x79; 32],
        |request| request.anchor.height = 9,
        |request| request.anchor_observed_at_unix_ms = 1_399,
        |request| request.deadline_unix_ms = request.expires_at_unix_ms + 1,
        |request| request.selection.current = None,
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut request = original.clone();
        mutate(&mut request);
        assert!(
            CustodyExpectation::Enroll(&request)
                .plan(VALIDATED_AT)
                .unwrap()
                .instruction(&fixture.config)
                .is_err(),
            "enrollment input mutation {index}"
        );
    }
}

#[test]
fn enrollment_uses_independent_signature_binding_sequence_and_revocation_checks() {
    let fixture = fixture();
    let original = enroll_request(&fixture);
    let signed: SignerCustodyRecordV1 =
        decode_bounded(&original.enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
    let mut invalid_signature = original.clone();
    let mut corrupted = signed.clone();
    corrupted.attestation[0] ^= 1;
    invalid_signature.enrollment = encode_bounded(&corrupted, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
    assert!(
        CustodyExpectation::Enroll(&invalid_signature)
            .plan(VALIDATED_AT)
            .unwrap()
            .instruction(&fixture.config)
            .is_err()
    );
    let mut wrong_attester = original.clone();
    wrong_attester.enrollment = attest(signed.statement.clone(), &key(0x44));
    assert!(
        CustodyExpectation::Enroll(&wrong_attester)
            .plan(VALIDATED_AT)
            .unwrap()
            .instruction(&fixture.config)
            .is_err()
    );
    let mutations: [fn(&mut SignerCustodyStatementV1); 2] = [
        |statement: &mut SignerCustodyStatementV1| {
            statement.binding.public_key = key(0x45).public_key().clone()
        },
        |statement: &mut SignerCustodyStatementV1| {
            statement.sequence = 2;
            statement.predecessor_digest = [0x7a; 32];
        },
    ];
    for mutation in mutations {
        let mut request = original.clone();
        let mut statement = signed.statement.clone();
        mutation(&mut statement);
        request.enrollment = attest(statement, &fixture.attester);
        assert!(
            CustodyExpectation::Enroll(&request)
                .plan(VALIDATED_AT)
                .unwrap()
                .instruction(&fixture.config)
                .is_err()
        );
    }
    for signer_revoked in [true, false] {
        let mut request = original.clone();
        let mut state = request
            .selection
            .validate(&fixture.config)
            .unwrap()
            .unwrap();
        state.signer_revoked = signer_revoked;
        state.attester_revoked = !signer_revoked;
        request.selection.current.as_mut().unwrap().control_state =
            encode_bounded(&state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap();
        refresh_digest(&mut request.selection);
        request.anchor.state_digest = request.selection.expected_digest;
        let mut statement = signed.statement.clone();
        statement.anchor = request.anchor;
        request.enrollment = attest(statement, &fixture.attester);
        assert!(
            CustodyExpectation::Enroll(&request)
                .plan(VALIDATED_AT)
                .unwrap()
                .instruction(&fixture.config)
                .is_err()
        );
    }
}

#[test]
fn retained_active_enrollment_must_match_the_shared_control_head() {
    let fixture = fixture();
    let request = enroll_request(&fixture);
    let original: SignerCustodyRecordV1 =
        decode_bounded(&request.enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
    let mut state = request
        .selection
        .validate(&fixture.config)
        .unwrap()
        .unwrap();
    let digest = original.canonical_digest().unwrap();
    state.next_sequence = 2;
    state.predecessor_digest = digest;
    state.active_head = Some(SignerCustodyActiveHeadV1 {
        record_digest: digest,
        sequence: original.statement.sequence,
        approved_anchor: original.statement.anchor,
        key_revision: original.statement.binding.key_revision,
        policy_revision: original.statement.binding.policy_revision,
        policy_digest: original.statement.binding.policy_digest,
    });
    state.validate().unwrap();
    let mut selection = request.selection;
    let record = selection.current.as_mut().unwrap();
    record.revision = 2;
    record.predecessor_digest = selection.expected_digest;
    record.control_state = encode_bounded(&state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap();
    record.active_enrollment = Some(request.enrollment);
    selection.expected_revision = 2;
    refresh_digest(&mut selection);
    selection.validate(&fixture.config).unwrap();
    let mut missing = selection.clone();
    missing.current.as_mut().unwrap().active_enrollment = None;
    refresh_digest(&mut missing);
    assert!(missing.validate(&fixture.config).is_err());
    let mut substituted = selection;
    let mut statement = original.statement;
    statement.evidence_digest = [0x7b; 32];
    substituted.current.as_mut().unwrap().active_enrollment =
        Some(attest(statement, &fixture.attester));
    refresh_digest(&mut substituted);
    assert!(substituted.validate(&fixture.config).is_err());
}

#[test]
fn canonical_frames_admit_exact_bound_and_refuse_empty_corrupt_trailing_or_alternate_bytes() {
    let fixture = fixture();
    let canonical = norito::encode_canonical(&fixture.policy).unwrap();
    assert_eq!(
        encode_bounded(&fixture.policy, canonical.len()).unwrap(),
        canonical
    );
    assert_eq!(
        decode_bounded::<SignerCustodyPolicyV1>(&canonical, canonical.len()).unwrap(),
        fixture.policy
    );
    assert!(encode_bounded(&fixture.policy, canonical.len() - 1).is_err());
    assert!(decode_bounded::<SignerCustodyPolicyV1>(&canonical, canonical.len() - 1).is_err());
    assert!(
        decode_bounded::<SignerCustodyPolicyV1>(&[], SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).is_err()
    );
    let mut corrupt = canonical.clone();
    corrupt[0] ^= 1;
    assert!(
        decode_bounded::<SignerCustodyPolicyV1>(&corrupt, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)
            .is_err()
    );
    let mut trailing = canonical.clone();
    trailing.push(0);
    assert!(
        decode_bounded::<SignerCustodyPolicyV1>(&trailing, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)
            .is_err()
    );
    let alternate = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN,
        );
        assert_eq!(
            encode_bounded(&fixture.policy, canonical.len()).unwrap(),
            canonical
        );
        norito::core::to_bytes(&fixture.policy).unwrap()
    };
    assert_ne!(alternate, canonical);
    assert!(
        decode_bounded::<SignerCustodyPolicyV1>(&alternate, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)
            .is_err()
    );
}

#[test]
fn caller_frames_and_fee_containers_are_bounded_before_plan_cloning() {
    let fixture = fixture();
    let mut enroll = enroll_request(&fixture);
    enroll.enrollment.clear();
    assert!(
        CustodyExpectation::Enroll(&enroll)
            .plan(VALIDATED_AT)
            .is_err()
    );
    enroll.enrollment = vec![0; SIGNER_CUSTODY_MAX_BYTES_V1 + 1];
    assert!(
        CustodyExpectation::Enroll(&enroll)
            .plan(VALIDATED_AT)
            .is_err()
    );
    let mut selection = configured_selection(&fixture);
    selection.current.as_mut().unwrap().control_state =
        vec![0; SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1 + 1];
    assert!(selection.admit().is_err());
    let mut selection = configured_selection(&fixture);
    selection.current.as_mut().unwrap().active_enrollment =
        Some(vec![0; SIGNER_CUSTODY_MAX_BYTES_V1 + 1]);
    assert!(selection.admit().is_err());
    let mut selection = configure_request(&fixture).selection;
    selection.binding.key_handle = "x".repeat(SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1);
    assert!(selection.admit().is_err());
    let mut invalid = options();
    invalid
        .max_total_fees
        .values_mut()
        .for_each(|maximum| *maximum = Quantity::from(0_u32));
    assert!(validate_options(&invalid).is_err());
    let mut invalid = options();
    let limit = FeeChargeLimit::new(
        FeeChargeKind::Nexus,
        XOR_ASSET_DEFINITION.parse().unwrap(),
        Quantity::from(1_u32),
    );
    invalid.fee_payment = FeePaymentIntent::authority(vec![limit; 17], None);
    assert!(validate_options(&invalid).is_err());
}

#[test]
fn instruction_recovery_preserves_original_deadline_and_operation_purpose() {
    let fixture = fixture();
    let request = configure_request(&fixture);
    let mut plan = CustodyExpectation::Configure(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    let bytes = encode_bounded(&plan, MAX_PLAN_BYTES).unwrap();
    let kind = NativeOperationKind::StreamTokenCustodyConfigure;
    assert_eq!(
        instructions(&fixture.config, &bytes, kind, ORIGINAL_DEADLINE).unwrap(),
        vec![plan.instruction(&fixture.config).unwrap()]
    );
    assert!(instructions(&fixture.config, &bytes, kind, ORIGINAL_DEADLINE - 1).is_ok());
    assert!(instructions(&fixture.config, &bytes, kind, ORIGINAL_DEADLINE + 1).is_err());
    assert!(instructions(&fixture.config, &bytes, kind, VALIDATED_AT).is_err());
    assert!(
        instructions(
            &fixture.config,
            &bytes,
            NativeOperationKind::StreamTokenCustodyEnroll,
            ORIGINAL_DEADLINE
        )
        .is_err()
    );
    assert!(
        instructions(
            &fixture.config,
            &bytes,
            NativeOperationKind::Transfer,
            ORIGINAL_DEADLINE
        )
        .is_err()
    );
    plan.validated_at_unix_ms = 0;
    assert!(plan.instruction(&fixture.config).is_err());
    plan.validated_at_unix_ms = ORIGINAL_DEADLINE;
    assert!(plan.instruction(&fixture.config).is_err());
    assert!(
        instructions(
            &fixture.config,
            &vec![0; MAX_PLAN_BYTES + 1],
            kind,
            ORIGINAL_DEADLINE
        )
        .is_err()
    );
}

#[test]
fn original_signed_configure_journal_survives_expiry_and_rejects_request_or_fee_substitution() {
    let fixture = fixture();
    let _profile = ChainDiscriminantGuard::enter(fixture.config.account_chain_discriminant);
    let request = configure_request(&fixture);
    let expected = CustodyExpectation::Configure(&request);
    let plan = expected.plan(VALIDATED_AT).unwrap();
    let record = journal(&fixture.config, &plan, &request.options);
    expected.verify(&record).unwrap();
    assert!(transaction_expired(&record.verify(&fixture.config).unwrap()).unwrap());
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("original-configure");
    let retained = Journal::create_prepared(&path, &record).unwrap();
    let recovered: TransactionJournal = retained.read_operation().unwrap();
    assert_eq!(recovered.deadline_ms, ORIGINAL_DEADLINE);
    assert_eq!(
        recovered.signed_transaction_hex,
        record.signed_transaction_hex
    );
    assert_eq!(recovered.transaction_hash, record.transaction_hash);
    recovered.verify(&fixture.config).unwrap();
    expected.verify(&recovered).unwrap();
    let mut changed = request.clone();
    changed.deadline_unix_ms += 1;
    assert!(
        CustodyExpectation::Configure(&changed)
            .verify(&recovered)
            .is_err()
    );
    let mut changed = request.clone();
    changed.selection.expected_digest = [0x7c; 32];
    assert!(
        CustodyExpectation::Configure(&changed)
            .verify(&recovered)
            .is_err()
    );
    let mut changed = request.clone();
    *changed.options.max_total_fees.values_mut().next().unwrap() = Quantity::from(11_u32);
    assert!(
        CustodyExpectation::Configure(&changed)
            .verify(&recovered)
            .is_err()
    );
    let mut changed = request.clone();
    changed.options.fee_payment =
        FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
    assert!(
        CustodyExpectation::Configure(&changed)
            .verify(&recovered)
            .is_err()
    );
    let mut changed_record = recovered;
    let NativeOperation::StreamTokenCustodyConfigure { plan, terms } = changed_record.operation
    else {
        panic!("configure purpose retained");
    };
    changed_record.operation = NativeOperation::StreamTokenCustodyEnroll { plan, terms };
    assert!(expected.verify(&changed_record).is_err());
    assert!(changed_record.verify(&fixture.config).is_err());
}

#[test]
fn original_signed_enrollment_journal_rejects_renewed_interval_and_substituted_attestation() {
    let fixture = fixture();
    let _profile = ChainDiscriminantGuard::enter(fixture.config.account_chain_discriminant);
    let request = enroll_request(&fixture);
    let expected = CustodyExpectation::Enroll(&request);
    let plan = expected.plan(VALIDATED_AT).unwrap();
    let record = journal(&fixture.config, &plan, &request.options);
    expected.verify(&record).unwrap();
    let bytes = norito::json::to_vec(&record).unwrap();
    let recovered: TransactionJournal = norito::json::from_slice(&bytes).unwrap();
    expected.verify(&recovered).unwrap();
    recovered.verify(&fixture.config).unwrap();
    let mut changed = request.clone();
    changed.expires_at_unix_ms += 1;
    assert!(
        CustodyExpectation::Enroll(&changed)
            .verify(&recovered)
            .is_err()
    );
    let mut changed = request;
    let mut enrollment: SignerCustodyRecordV1 =
        decode_bounded(&changed.enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
    enrollment.statement.evidence_digest = [0x7d; 32];
    changed.enrollment = attest(enrollment.statement, &fixture.attester);
    assert!(
        CustodyExpectation::Enroll(&changed)
            .verify(&recovered)
            .is_err()
    );
    let mut changed_record = recovered;
    let NativeOperation::StreamTokenCustodyEnroll { terms, .. } = &mut changed_record.operation
    else {
        panic!("enrollment purpose retained");
    };
    terms.deadline_ms += 1;
    assert!(changed_record.verify(&fixture.config).is_err());
}

#[test]
fn declared_custody_plan_frame_recovers_original_instruction_and_rejects_other_owners() {
    let fixture = fixture();
    let request = configure_request(&fixture);
    let plan = CustodyExpectation::Configure(&request)
        .plan(VALIDATED_AT)
        .unwrap();
    let frame = encode_bounded(&plan, MAX_PLAN_BYTES).unwrap();
    let header = norito::core::Header::read(frame.as_slice()).unwrap();
    assert_eq!(
        header.schema,
        norito::core::schema_hash_for_name("iroha_wallet::operations::stream_token_custody::Plan")
    );
    let recovered: Plan = decode_bounded(&frame, MAX_PLAN_BYTES).unwrap();
    assert_eq!(encode_bounded(&recovered, MAX_PLAN_BYTES).unwrap(), frame);
    assert_eq!(recovered.validated_at_unix_ms, VALIDATED_AT);
    assert_eq!(recovered.deadline_unix_ms, ORIGINAL_DEADLINE);
    assert_eq!(
        recovered.instruction(&fixture.config).unwrap(),
        plan.instruction(&fixture.config).unwrap()
    );
    let selection_frame = encode_bounded(&plan.selection, MAX_PLAN_BYTES).unwrap();
    let selection_header = norito::core::Header::read(selection_frame.as_slice()).unwrap();
    assert_eq!(
        selection_header.schema,
        norito::core::schema_hash_for_name("iroha_wallet::operations::StreamTokenCustodySelection")
    );
    let action_frame = encode_bounded(&plan.action, MAX_PLAN_BYTES).unwrap();
    let action_header = norito::core::Header::read(action_frame.as_slice()).unwrap();
    assert_eq!(
        action_header.schema,
        norito::core::schema_hash_for_name(
            "iroha_wallet::operations::stream_token_custody::Action"
        )
    );
    assert_ne!(header.schema, selection_header.schema);
    assert_ne!(header.schema, action_header.schema);
    assert!(decode_bounded::<Plan>(&selection_frame, MAX_PLAN_BYTES).is_err());
    assert!(decode_bounded::<Plan>(&action_frame, MAX_PLAN_BYTES).is_err());
    assert!(decode_bounded::<StreamTokenCustodySelection>(&frame, MAX_PLAN_BYTES).is_err());
}

//! Custody planning preserves independent inputs, original signatures, fees and once-only dispatch.

use super::*;
use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    protocol::SignerKeyAlgorithmV1,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

#[derive(Debug, Default)]
struct Transport {
    quotes: AtomicUsize,
    submissions: AtomicUsize,
}
impl HttpTransport for Transport {
    fn send_blocking(&self, request: TransportRequest) -> Result<Response<Vec<u8>>> {
        let (status, body) = match request.url.path() {
            "/v1/node/capabilities" => (
                200,
                norito::json::to_vec(&norito::json!({
                    "data_model_version": (iroha_data_model::DATA_MODEL_VERSION),
                    "signed_transaction_schema_hash_hex": (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>()))
                }))?,
            ),
            "/v1/fees/quote" => {
                self.quotes.fetch_add(1, Ordering::SeqCst);
                let request: iroha_torii_shared::FeeQuoteRequest =
                    norito::json::from_slice(&request.body)?;
                let payload = request.payload;
                (
                    200,
                    norito::json::to_vec(&FeeQuoteResponse {
                        intent: payload.fee_payment_intent().clone(),
                        observation: iroha_torii_shared::FeeQuoteObservation {
                            ledger_time_ms: current_unix_ms()?,
                            next_block_height: 1,
                            route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                        },
                        components: Vec::new(),
                        capacities: Vec::new(),
                        decision: iroha_torii_shared::FeeQuoteDecision::Accepted {
                            debit_source: iroha_data_model::nexus::FeeDebitSource::Account(
                                payload.authority().clone(),
                            ),
                            program_revision: None,
                        },
                    })?,
                )
            }
            "/v1/pipeline/transactions/status" => {
                let hash = request
                    .url
                    .query_pairs()
                    .find(|(key, _)| key == "hash")
                    .unwrap()
                    .1
                    .parse::<iroha_crypto::HashOf<SignedTransaction>>()?;
                let absence = iroha_torii_shared::ErrorEnvelope::new(
                    iroha_torii_shared::PIPELINE_TRANSACTION_STATUS_NOT_FOUND_CODE,
                    "Missing status.",
                )
                .with_details(iroha_torii_shared::ErrorDetails {
                    pipeline_transaction_status_not_found: Some(
                        iroha_torii_shared::PipelineTransactionStatusNotFoundV1::new(
                            &hash, "global",
                        ),
                    ),
                    ..iroha_torii_shared::ErrorDetails::default()
                });
                (404, norito::json::to_vec(&absence)?)
            }
            path if path == iroha_torii_shared::route_catalog::pipeline::TRANSACTION.path() => {
                self.submissions.fetch_add(1, Ordering::SeqCst);
                (503, b"unavailable".to_vec())
            }
            path => panic!("unexpected custody HTTP request {path}"),
        };
        Ok(Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(body)?)
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move { self.send_blocking(request) })
    }
}
fn service() -> (AccountService, Arc<Transport>) {
    let config = super::super::tests::fixture_config();
    let transport = Arc::new(Transport::default());
    let client = Client::with_http_transport(config.clone(), transport.clone()).unwrap();
    (
        AccountService {
            config,
            client,
            deadline: None,
        },
        transport,
    )
}
fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
}
fn configure(config: &Config, now: u64) -> StreamTokenCustodyConfigureRequest {
    let policy = SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: config.chain.to_string(),
            network_id: *config.network_id.as_bytes(),
            runtime_handle: "software://stream/primary".into(),
            key_handle: "software://stream/key".into(),
            service_id: "stream-service".into(),
            administrator_id: "stream-admin".into(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: [3; 32],
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: key(4).public_key().clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [5; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-service".into(),
            administrator_id: "custody-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [6; 32],
        },
        attester_public_key: key(7).public_key().clone(),
        active_from_unix_ms: now - 1000,
        active_until_unix_ms: now + 120_000,
        max_validity_ms: 120_000,
        max_anchor_age_ms: 60_000,
    };
    StreamTokenCustodyConfigureRequest {
        selection: StreamTokenCustodySelection {
            provider_id: ProviderId::new([3; 32]),
            binding: policy.binding.clone(),
            expected_revision: 0,
            expected_digest: [0; 32],
            current: None,
        },
        policy,
        deadline_unix_ms: now + 50_000,
        options: BoundedTransactionOptions {
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            max_total_fees: BTreeMap::new(),
            deadline: Instant::now() + Duration::from_secs(60),
        },
    }
}
fn enroll(config: &Config, now: u64) -> StreamTokenCustodyEnrollRequest {
    let request = configure(config, now);
    let state = configure_signer_custody_policy_v1(None, request.policy.clone()).unwrap();
    let record = StreamTokenCustodyControlRecordV1 {
        provider_id: request.selection.provider_id,
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [8; 32],
        execution_height: 1,
        ordinal: 0,
        recorded_at_unix_ms: now - 10,
        authority: config.account.clone(),
        control_state: norito::encode_canonical(&state).unwrap(),
        active_enrollment: None,
    };
    let digest = record.canonical_digest().unwrap();
    let anchor = SignerCustodyAnchorV1 {
        height: 2,
        block_hash: [9; 32],
        state_digest: digest,
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: request.policy.binding.clone(),
        authority: request.policy.attester_authority.clone(),
        anchor,
        sequence: 1,
        predecessor_digest: [0; 32],
        issued_at_unix_ms: now - 1,
        expires_at_unix_ms: now + 100_000,
        evidence_digest: [10; 32],
        revoked: false,
    };
    let attestation: [u8; 64] =
        Signature::new(key(7).private_key(), &statement.signing_payload().unwrap())
            .payload()
            .try_into()
            .unwrap();
    StreamTokenCustodyEnrollRequest {
        selection: StreamTokenCustodySelection {
            expected_revision: 1,
            expected_digest: digest,
            current: Some(record),
            ..request.selection
        },
        anchor,
        anchor_observed_at_unix_ms: now,
        issued_at_unix_ms: statement.issued_at_unix_ms,
        expires_at_unix_ms: statement.expires_at_unix_ms,
        enrollment: norito::encode_canonical(&SignerCustodyRecordV1 {
            statement,
            attestation,
        })
        .unwrap(),
        deadline_unix_ms: request.deadline_unix_ms,
        options: request.options,
    }
}

#[test]
fn configure_and_enroll_retain_one_canonical_instruction_and_exact_original_wire() {
    let (service, transport) = service();
    let now = current_unix_ms().unwrap();
    let configure = configure(&service.config, now);
    let enroll = enroll(&service.config, now);
    let root = tempfile::tempdir().unwrap();
    let configured = root.path().join("configure");
    let enrolled = root.path().join("enroll");
    service
        .prepare_stream_token_custody_configure(&configure, &configured)
        .unwrap();
    service
        .prepare_stream_token_custody_enroll(&enroll, &enrolled)
        .unwrap();
    let configured_transaction = service
        .verify_stream_token_custody_configure_journal(&configured, &configure)
        .unwrap();
    let enrolled_transaction = service
        .verify_stream_token_custody_enroll_journal(&enrolled, &enroll)
        .unwrap();
    for (path, returned) in [
        (&configured, configured_transaction),
        (&enrolled, enrolled_transaction),
    ] {
        let record: TransactionJournal = Journal::open(path).unwrap().read_operation().unwrap();
        let transaction = record.verify(&service.config).unwrap();
        assert_eq!(returned.encode_versioned(), transaction.encode_versioned());
        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("native operation")
        };
        assert_eq!(instructions.len(), 1);
        assert!(
            instructions[0]
                .as_any()
                .is::<MutateSorafsStreamTokenCustody>()
        );
        assert!(record.deadline_ms <= configure.deadline_unix_ms);
        assert!(transaction.attachments().is_none() && transaction.multisig_signatures().is_none());
        assert!(service.submit(path, record.operation.kind()).is_err());
        assert!(service.resume(path, record.operation.kind()).is_err());
    }
    assert_eq!(transport.quotes.load(Ordering::SeqCst), 2);
    assert_eq!(transport.submissions.load(Ordering::SeqCst), 0);
    assert!(
        service
            .prepare_stream_token_custody_configure(&configure, &configured)
            .is_err()
    );
}

#[test]
fn independently_selected_cas_binding_interval_and_attester_cannot_be_substituted() {
    let (service, transport) = service();
    let now = current_unix_ms().unwrap();
    let request = enroll(&service.config, now);
    let mut variants = Vec::new();
    let mut changed = request.clone();
    changed.selection.expected_revision += 1;
    variants.push(changed);
    let mut changed = request.clone();
    changed.selection.expected_digest[0] ^= 1;
    variants.push(changed);
    let mut changed = request.clone();
    changed.issued_at_unix_ms += 1;
    variants.push(changed);
    let mut changed = request.clone();
    changed.anchor.block_hash[0] ^= 1;
    variants.push(changed);
    let mut changed = request.clone();
    changed.selection.binding.network_id[0] ^= 1;
    variants.push(changed);
    let mut changed = request.clone();
    changed.enrollment.push(0);
    variants.push(changed);
    let mut changed = request.clone();
    let mut signed: SignerCustodyRecordV1 =
        decode_bounded(&changed.enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
    signed.attestation[0] ^= 1;
    changed.enrollment = norito::encode_canonical(&signed).unwrap();
    variants.push(changed);
    let mut changed = request.clone();
    changed.deadline_unix_ms = request.expires_at_unix_ms + 1;
    variants.push(changed);
    let mut changed = request.clone();
    changed.enrollment = vec![0; SIGNER_CUSTODY_MAX_BYTES_V1 + 1];
    variants.push(changed);
    let root = tempfile::tempdir().unwrap();
    for (index, changed) in variants.iter().enumerate() {
        let path = root.path().join(index.to_string());
        assert!(
            service
                .prepare_stream_token_custody_enroll(changed, &path)
                .is_err()
        );
        assert!(!path.exists());
    }
    assert_eq!(transport.quotes.load(Ordering::SeqCst), 0);
    assert_eq!(transport.submissions.load(Ordering::SeqCst), 0);
}

#[test]
fn recovery_keeps_original_request_and_dispatches_each_purpose_at_most_once() {
    let (service, transport) = service();
    let now = current_unix_ms().unwrap();
    let configure = configure(&service.config, now);
    let mut enroll = enroll(&service.config, now);
    let root = tempfile::tempdir().unwrap();
    let configured = root.path().join("configure");
    let enrolled = root.path().join("enroll");
    service
        .prepare_stream_token_custody_configure(&configure, &configured)
        .unwrap();
    service
        .prepare_stream_token_custody_enroll(&enroll, &enrolled)
        .unwrap();
    let before = std::fs::read(enrolled.join("operation.json")).unwrap();
    let mut changed = enroll.clone();
    changed.deadline_unix_ms += 1;
    assert!(
        service
            .submit_stream_token_custody_enroll(&enrolled, &changed)
            .is_err()
    );
    let mut changed = enroll.clone();
    changed
        .options
        .max_total_fees
        .insert(XOR_ASSET_DEFINITION.parse().unwrap(), Quantity::from(1u32));
    assert!(
        service
            .verify_stream_token_custody_enroll_journal(&enrolled, &changed)
            .is_err()
    );
    assert!(
        service
            .submit_stream_token_custody_enroll(&enrolled, &changed)
            .is_err()
    );
    let mut changed = configure.clone();
    changed.options.fee_payment =
        FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(99));
    assert!(
        service
            .verify_stream_token_custody_configure_journal(&configured, &changed)
            .is_err()
    );
    assert!(
        service
            .submit_stream_token_custody_configure(&configured, &changed)
            .is_err()
    );
    assert_eq!(transport.submissions.load(Ordering::SeqCst), 0);
    for _ in 0..2 {
        assert_eq!(
            service
                .submit_stream_token_custody_configure(&configured, &configure)
                .unwrap()
                .status,
            OperationStatus::Pending
        );
        assert_eq!(
            service
                .submit_stream_token_custody_enroll(&enrolled, &enroll)
                .unwrap()
                .status,
            OperationStatus::Pending
        );
    }
    assert_eq!(
        service
            .resume_stream_token_custody_configure(&configured, &configure)
            .unwrap()
            .status,
        OperationStatus::Pending
    );
    assert_eq!(
        service
            .resume_stream_token_custody_enroll(&enrolled, &enroll)
            .unwrap()
            .status,
        OperationStatus::Pending
    );
    assert_eq!(transport.submissions.load(Ordering::SeqCst), 2);
    enroll.options.deadline = Instant::now() + Duration::from_secs(60);
    service
        .verify_stream_token_custody_enroll_journal(&enrolled, &enroll)
        .unwrap();
    assert_eq!(
        service
            .resume_stream_token_custody_enroll(&enrolled, &enroll)
            .unwrap()
            .status,
        OperationStatus::Pending
    );
    assert_eq!(
        std::fs::read(enrolled.join("operation.json")).unwrap(),
        before
    );
}

#[test]
fn bounded_fee_comparison_rejects_changed_assets_or_amounts_and_excessive_cardinality() {
    let (service, transport) = service();
    let mut request = configure(&service.config, current_unix_ms().unwrap());
    let asset: AssetDefinitionId = XOR_ASSET_DEFINITION.parse().unwrap();
    request
        .options
        .max_total_fees
        .insert(asset.clone(), Quantity::from(1u32));
    let terms = BoundedTerms::new(&request.options).unwrap();
    assert!(terms.matches_options(&request.options).unwrap());
    request
        .options
        .max_total_fees
        .insert(asset.clone(), Quantity::from(2u32));
    assert!(!terms.matches_options(&request.options).unwrap());
    request.options.max_total_fees.clear();
    assert!(!terms.matches_options(&request.options).unwrap());
    for value in 1..=17u8 {
        let mut bytes = [value; 16];
        bytes[6] = 0x40;
        bytes[8] = 0x80;
        request.options.max_total_fees.insert(
            AssetDefinitionId::from_uuid_bytes(bytes).unwrap(),
            Quantity::from(1u32),
        );
    }
    assert!(validate_options(&request.options).is_err());
    request.options.max_total_fees.clear();
    request.options.deadline = Instant::now() - Duration::from_secs(1);
    let root = tempfile::tempdir().unwrap();
    assert!(
        service
            .prepare_stream_token_custody_configure(&request, &root.path().join("expired"))
            .is_err()
    );
    assert_eq!(transport.quotes.load(Ordering::SeqCst), 0);
}

#[test]
fn configure_rejects_unbounded_utc_authorization_and_role_key_as_manager() {
    let (service, transport) = service();
    let root = tempfile::tempdir().unwrap();
    let mut request = configure(&service.config, current_unix_ms().unwrap());
    request.deadline_unix_ms = u64::MAX;
    assert!(
        service
            .prepare_stream_token_custody_configure(&request, &root.path().join("unbounded"))
            .is_err()
    );
    let mut request = configure(&service.config, current_unix_ms().unwrap());
    request.policy.binding.public_key = service.config.key_pair.public_key().clone();
    request.selection.binding = request.policy.binding.clone();
    assert!(
        service
            .prepare_stream_token_custody_configure(&request, &root.path().join("manager"))
            .is_err()
    );
    assert_eq!(transport.quotes.load(Ordering::SeqCst), 0);
    assert_eq!(transport.submissions.load(Ordering::SeqCst), 0);
}

#[test]
fn historical_plan_validation_does_not_renew_an_expired_utc_interval() {
    let (service, _) = service();
    let now = current_unix_ms().unwrap();
    let request = enroll(&service.config, now - 200_000);
    let expected = CustodyExpectation::Enroll(&request);
    let plan = expected.plan(now - 200_000).unwrap();
    let bytes = encode_bounded(&plan, MAX_PLAN_BYTES).unwrap();
    assert!(
        instructions(
            &service.config,
            &bytes,
            NativeOperationKind::StreamTokenCustodyEnroll,
            request.deadline_unix_ms
        )
        .is_ok()
    );
    assert!(
        instructions(
            &service.config,
            &bytes,
            NativeOperationKind::StreamTokenCustodyConfigure,
            request.deadline_unix_ms
        )
        .is_err()
    );
    assert!(
        instructions(
            &service.config,
            &bytes,
            NativeOperationKind::StreamTokenCustodyEnroll,
            now + 1
        )
        .is_err()
    );
    let root = tempfile::tempdir().unwrap();
    assert!(
        service
            .prepare_stream_token_custody_enroll(&request, &root.path().join("expired"))
            .is_err()
    );
}

#[test]
fn enrollment_rejects_revoked_roles_and_replaced_attester_even_with_consistent_cas() {
    let (service, transport) = service();
    let now = current_unix_ms().unwrap();
    let root = tempfile::tempdir().unwrap();
    for case in 0..3 {
        let mut request = enroll(&service.config, now);
        let native = request.selection.current.as_mut().unwrap();
        let mut state: SignerCustodyControlStateV1 =
            decode_bounded(&native.control_state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1).unwrap();
        match case {
            0 => state.signer_revoked = true,
            1 => state.attester_revoked = true,
            _ => state.policy.attester_public_key = key(11).public_key().clone(),
        }
        native.control_state = norito::encode_canonical(&state).unwrap();
        request.selection.expected_digest = native.canonical_digest().unwrap();
        request.anchor.state_digest = request.selection.expected_digest;
        let mut signed: SignerCustodyRecordV1 =
            decode_bounded(&request.enrollment, SIGNER_CUSTODY_MAX_BYTES_V1).unwrap();
        signed.statement.anchor = request.anchor;
        signed.attestation = Signature::new(
            key(7).private_key(),
            &signed.statement.signing_payload().unwrap(),
        )
        .payload()
        .try_into()
        .unwrap();
        request.enrollment = norito::encode_canonical(&signed).unwrap();
        assert!(
            service
                .prepare_stream_token_custody_enroll(&request, &root.path().join(case.to_string()))
                .is_err()
        );
    }
    assert_eq!(transport.quotes.load(Ordering::SeqCst), 0);
    assert_eq!(transport.submissions.load(Ordering::SeqCst), 0);
}

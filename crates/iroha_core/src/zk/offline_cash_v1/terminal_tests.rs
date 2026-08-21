use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    block::BlockHeader,
    domain::DomainId,
    offline::{
        KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2,
        OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1, OFFLINE_CASH_HALO2_K_V1,
        OFFLINE_CASH_HANDOFF_P95_MAX_MS_V1, OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1,
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OFFLINE_CASH_MIN_FUZZ_CASES_V1,
        OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1, OFFLINE_CASH_PARAMS_BYTES_V1,
        OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1, OFFLINE_CASH_PROVE_P95_MAX_MS_V1,
        OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1, OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1,
        OFFLINE_CASH_VALIDATOR_COUNT_V1, OFFLINE_CASH_VERIFY_P95_MAX_MS_V1,
        OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
        OfflineCashAcknowledgementV1, OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
        OfflineCashInternalValidationReceiptV1, OfflineCashPairedProofV1,
        OfflineCashPaymentRequestV1, OfflineCashPaymentV1, OfflineCashReleaseApprovalV1,
        OfflineCashReleaseAttestationV1, OfflineCashReleaseAuthorityPolicyV1,
        OfflineCashReleaseManifestV1, OfflineCashTransferStatementV1,
        offline_cash_artifact_set_digest_v1, offline_cash_receiver_key_reference_v1,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::sync::Mutex;

fn signing_key() -> SigningKey {
    SigningKey::from_bytes((&[0x27_u8; 32]).into()).expect("P-256 signing key")
}

fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV2 {
    let signature: Signature = key.sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical signature")
}

fn network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"offline-cash-core-v1",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn artifact_len(role: OfflineCashArtifactRoleV1) -> u64 {
    match role {
        OfflineCashArtifactRoleV1::ParamsEq | OfflineCashArtifactRoleV1::ParamsEp => {
            OFFLINE_CASH_PARAMS_BYTES_V1
        }
        OfflineCashArtifactRoleV1::StatePkEq | OfflineCashArtifactRoleV1::StatePkEp => {
            OFFLINE_CASH_STATE_PROVING_KEY_MAX_BYTES_V1 / 2
        }
        OfflineCashArtifactRoleV1::StateVkEq
        | OfflineCashArtifactRoleV1::StateVkEp
        | OfflineCashArtifactRoleV1::GuardUseVkEq
        | OfflineCashArtifactRoleV1::GuardUseVkEp
        | OfflineCashArtifactRoleV1::PlatformBindVkEq
        | OfflineCashArtifactRoleV1::PlatformBindVkEp
        | OfflineCashArtifactRoleV1::AndroidKeyCertVkEq
        | OfflineCashArtifactRoleV1::AndroidKeyCertVkEp
        | OfflineCashArtifactRoleV1::GuardBundleVkEq
        | OfflineCashArtifactRoleV1::GuardBundleVkEp => OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1 / 2,
        _ => OFFLINE_CASH_HELPER_PROVING_KEY_MAX_BYTES_V1 / 8,
    }
}

pub(super) fn authenticated_release() -> OfflineCashAuthenticatedReleaseV1 {
    let artifacts = OfflineCashArtifactRoleV1::ALL
        .iter()
        .copied()
        .enumerate()
        .map(|(index, role)| OfflineCashArtifactBindingV1 {
            role,
            sha256: [u8::try_from(index + 1).expect("role index"); 32],
            byte_len: artifact_len(role),
        })
        .collect::<Vec<_>>();
    authenticated_release_for_artifacts(artifacts)
}

pub(super) fn authenticated_release_for_artifacts(
    artifacts: Vec<OfflineCashArtifactBindingV1>,
) -> OfflineCashAuthenticatedReleaseV1 {
    let profile_digest = offline_cash_halo2_profile_digest_v1();
    let eq_protocol_digest = offline_cash_halo2_protocol_identity_v1(
        OfflineCashHalo2ParityV1::Eq,
        OfflineCashHalo2CircuitRoleV1::State,
    )
    .digest();
    let ep_protocol_digest = offline_cash_halo2_protocol_identity_v1(
        OfflineCashHalo2ParityV1::Ep,
        OfflineCashHalo2CircuitRoleV1::State,
    )
    .digest();
    authenticated_release_for_artifacts_and_protocol(
        artifacts,
        profile_digest,
        eq_protocol_digest,
        ep_protocol_digest,
    )
}

pub(super) fn authenticated_release_for_artifacts_and_protocol(
    artifacts: Vec<OfflineCashArtifactBindingV1>,
    profile_digest: [u8; 32],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
) -> OfflineCashAuthenticatedReleaseV1 {
    assert!(
        artifacts
            .iter()
            .map(|artifact| artifact.byte_len)
            .sum::<u64>()
            < OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1
    );
    let artifact_set_digest =
        offline_cash_artifact_set_digest_v1(&artifacts).expect("artifact set");
    let receipt = OfflineCashInternalValidationReceiptV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        source_tree_digest: [1; 32],
        cargo_lock_digest: [2; 32],
        profile_digest,
        eq_protocol_digest,
        ep_protocol_digest,
        artifact_set_digest,
        hardware_policy_digest: [6; 32],
        circuit_shape_report_digest: [7; 32],
        security_review_digest: [8; 32],
        kat_report_digest: [9; 32],
        fuzz_report_digest: [10; 32],
        resource_report_digest: [11; 32],
        ios_device_report_digest: [12; 32],
        android_device_report_digest: [13; 32],
        four_peer_report_digest: [14; 32],
        max_proof_pair_bytes: 6_000,
        max_session_bytes: 8_900,
        max_process_rss_bytes: OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1,
        prove_p95_ms: OFFLINE_CASH_PROVE_P95_MAX_MS_V1,
        verify_p95_ms: OFFLINE_CASH_VERIFY_P95_MAX_MS_V1,
        handoff_p95_ms: OFFLINE_CASH_HANDOFF_P95_MAX_MS_V1,
        qualified_handoffs: OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1,
        fuzz_cases: OFFLINE_CASH_MIN_FUZZ_CASES_V1,
        reproducible_builds: OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1,
        validator_count: OFFLINE_CASH_VALIDATOR_COUNT_V1,
    };
    let manifest = OfflineCashReleaseManifestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: [0; 32],
        source_tree_digest: receipt.source_tree_digest,
        cargo_lock_digest: receipt.cargo_lock_digest,
        profile_digest: receipt.profile_digest,
        eq_protocol_digest: receipt.eq_protocol_digest,
        ep_protocol_digest: receipt.ep_protocol_digest,
        hardware_policy_digest: receipt.hardware_policy_digest,
        validation_receipt_digest: receipt.canonical_digest().expect("receipt digest"),
        halo2_k: OFFLINE_CASH_HALO2_K_V1,
        artifacts,
    }
    .seal()
    .expect("seal release");
    let authority = KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519);
    let policy = OfflineCashReleaseAuthorityPolicyV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        authority_set_id: [0x62; 32],
        threshold: 1,
        authorized_signers: vec![authority.public_key().clone()],
    };
    let subject = manifest
        .release_attestation_subject(&receipt, &policy)
        .expect("release attestation subject");
    let attestation = OfflineCashReleaseAttestationV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        subject,
        approvals: vec![OfflineCashReleaseApprovalV1 {
            public_key: authority.public_key().clone(),
            signature: SignatureOf::try_new(authority.private_key(), &subject.approval_payload())
                .expect("release authority signature"),
        }],
    };
    manifest
        .authenticate(&receipt, &policy, &attestation)
        .expect("authenticate release")
}

pub(super) fn request(release: &OfflineCashAuthenticatedReleaseV1) -> OfflineCashPaymentRequestV1 {
    let signing_key = signing_key();
    let encoded = signing_key.verifying_key().to_encoded_point(false);
    let public_key =
        KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded.as_bytes()).expect("public key");
    let placeholder = sign(&signing_key, b"placeholder");
    let mut request = OfflineCashPaymentRequestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: release.release_id(),
        network_id: network_id(),
        asset: asset(),
        scale: 4,
        amount: 9_001,
        recipient: iroha_data_model::account::AccountId::new(
            KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        ),
        receiver_balance_commitment: [0x21; 32],
        recipient_key_reference: offline_cash_receiver_key_reference_v1(&public_key),
        receiver_public_key: public_key,
        request_id: [0x22; 32],
        issued_at_ms: 1_000,
        expires_at_ms: 61_000,
        hardware_policy_id: [6; 32],
        signature: placeholder,
    };
    request.signature = sign(
        &signing_key,
        &request.canonical_signing_bytes().expect("request bytes"),
    );
    request
}

pub(super) fn payment(
    release: &OfflineCashAuthenticatedReleaseV1,
    request: &OfflineCashPaymentRequestV1,
) -> OfflineCashPaymentV1 {
    let request_digest = request.canonical_digest().expect("request digest");
    let statement = OfflineCashTransferStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: release.release_id(),
        network_id: request.network_id.clone(),
        asset: request.asset.clone(),
        scale: request.scale,
        amount: request.amount,
        request_digest,
        sender_before: [0x31; 32],
        sender_after: [0x32; 32],
        receiver_before: request.receiver_balance_commitment,
        credit_commitment: [0x33; 32],
        transition_digest: [0; 32],
    }
    .seal_transition()
    .expect("seal transition");
    let semantic_digest = statement.canonical_digest().expect("statement digest");
    OfflineCashPaymentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        statement,
        proof: OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: release.eq_protocol_digest(),
            ep_protocol_digest: release.ep_protocol_digest(),
            semantic_digest,
            eq_proof: vec![0x41; 128],
            ep_proof: vec![0x42; 128],
            eq_history: vec![0x43; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0x44; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
        },
        encrypted_credit: vec![0x45; 128],
        artifact_manifest_digest: release.manifest_digest(),
    }
}

#[derive(Default)]
struct RecordingBackend {
    calls: Mutex<Vec<OfflineCashVerificationStageV1>>,
    artifact_calls: Mutex<Vec<(OfflineCashArtifactBindingV1, [u8; 32])>>,
    fail: Option<OfflineCashVerificationStageV1>,
}

impl super::paired_verifier_sealed::Sealed for RecordingBackend {}

impl RecordingBackend {
    fn record(
        &self,
        stage: OfflineCashVerificationStageV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(), String> {
        self.calls.lock().expect("calls lock").push(stage);
        self.artifact_calls
            .lock()
            .expect("artifact calls lock")
            .push((verifying_key, protocol_digest));
        if self.fail == Some(stage) {
            Err("injected failure".to_owned())
        } else {
            Ok(())
        }
    }
}

impl OfflineCashPairedProofVerifierV1 for RecordingBackend {
    fn verify_eq_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        _semantic_digest: [u8; 32],
        _proof: &[u8],
        _history: &[u8],
    ) -> Result<(), String> {
        self.record(
            OfflineCashVerificationStageV1::EqCurrent,
            verifying_key,
            protocol_digest,
        )
    }

    fn verify_ep_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        _semantic_digest: [u8; 32],
        _proof: &[u8],
        _history: &[u8],
    ) -> Result<(), String> {
        self.record(
            OfflineCashVerificationStageV1::EpCurrent,
            verifying_key,
            protocol_digest,
        )
    }

    fn decide_eq_history(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        _history: &[u8],
    ) -> Result<(), String> {
        self.record(
            OfflineCashVerificationStageV1::EqHistory,
            verifying_key,
            protocol_digest,
        )
    }

    fn decide_ep_history(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        _history: &[u8],
    ) -> Result<(), String> {
        self.record(
            OfflineCashVerificationStageV1::EpHistory,
            verifying_key,
            protocol_digest,
        )
    }
}

#[test]
fn terminal_boundary_decides_both_current_proofs_and_histories() {
    let release = authenticated_release();
    let request = request(&release);
    let payment = payment(&release, &request);
    let backend = RecordingBackend::default();
    let verifier = OfflineCashTerminalVerifierV1::new(&release, &backend);
    let receipt = verifier
        .verify_payment(&request, &payment, 2_000)
        .expect("verify payment");
    assert_eq!(receipt.amount(), request.amount);
    assert_eq!(
        receipt.recipient_key_reference(),
        request.recipient_key_reference
    );
    assert_eq!(
        receipt.transition_digest(),
        payment.statement.transition_digest
    );
    assert_eq!(
        *backend.calls.lock().expect("calls lock"),
        [
            OfflineCashVerificationStageV1::EqCurrent,
            OfflineCashVerificationStageV1::EpCurrent,
            OfflineCashVerificationStageV1::EqHistory,
            OfflineCashVerificationStageV1::EpHistory,
        ]
    );
    let eq_vk = release.artifact(OfflineCashArtifactRoleV1::StateVkEq);
    let ep_vk = release.artifact(OfflineCashArtifactRoleV1::StateVkEp);
    assert_eq!(
        *backend.artifact_calls.lock().expect("artifact calls lock"),
        [
            (eq_vk, release.eq_protocol_digest()),
            (ep_vk, release.ep_protocol_digest()),
            (eq_vk, release.eq_protocol_digest()),
            (ep_vk, release.ep_protocol_digest()),
        ]
    );
}

#[test]
fn protocol_substitution_fails_before_backend_dispatch() {
    let release = authenticated_release();
    let request = request(&release);
    let mut payment = payment(&release, &request);
    payment.proof.ep_protocol_digest = [0xEE; 32];
    let backend = RecordingBackend::default();
    let verifier = OfflineCashTerminalVerifierV1::new(&release, &backend);
    assert!(matches!(
        verifier.verify_payment(&request, &payment, 2_000),
        Err(OfflineCashVerificationErrorV1::ProtocolMismatch)
    ));
    assert!(backend.calls.lock().expect("calls lock").is_empty());
}

#[test]
fn manifest_substitution_fails_before_artifact_dispatch() {
    let release = authenticated_release();
    let request = request(&release);
    let mut payment = payment(&release, &request);
    payment.artifact_manifest_digest[0] ^= 1;
    let backend = RecordingBackend::default();
    let verifier = OfflineCashTerminalVerifierV1::new(&release, &backend);
    assert!(matches!(
        verifier.verify_payment(&request, &payment, 2_000),
        Err(OfflineCashVerificationErrorV1::ReleaseMismatch)
    ));
    assert!(backend.calls.lock().expect("calls lock").is_empty());
    assert!(
        backend
            .artifact_calls
            .lock()
            .expect("artifact calls lock")
            .is_empty()
    );
}

#[test]
fn failed_history_decision_never_yields_a_receipt() {
    let release = authenticated_release();
    let request = request(&release);
    let payment = payment(&release, &request);
    let backend = RecordingBackend {
        calls: Mutex::default(),
        artifact_calls: Mutex::default(),
        fail: Some(OfflineCashVerificationStageV1::EqHistory),
    };
    let verifier = OfflineCashTerminalVerifierV1::new(&release, &backend);
    assert!(matches!(
        verifier.verify_payment(&request, &payment, 2_000),
        Err(OfflineCashVerificationErrorV1::Cryptographic {
            stage: OfflineCashVerificationStageV1::EqHistory,
            ..
        })
    ));
}

#[test]
fn acknowledgement_is_bound_to_the_verified_credit() {
    let release = authenticated_release();
    let request = request(&release);
    let payment = payment(&release, &request);
    let backend = RecordingBackend::default();
    let verifier = OfflineCashTerminalVerifierV1::new(&release, &backend);
    let receipt = verifier
        .verify_payment(&request, &payment, 2_000)
        .expect("verify payment");
    let key = signing_key();
    let mut acknowledgement = OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: release.release_id(),
        request_digest: receipt.request_digest(),
        payment_digest: receipt.payment_digest(),
        receiver_balance_commitment: [0x61; 32],
        acknowledged_at_ms: 2_001,
        signature: sign(&key, b"placeholder"),
    };
    acknowledgement.signature = sign(
        &key,
        &acknowledgement
            .canonical_signing_bytes()
            .expect("acknowledgement bytes"),
    );
    verifier
        .verify_acknowledgement(&request, &payment, &acknowledgement, &receipt)
        .expect("verify acknowledgement");
}

use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use iroha_data_model::sorafs::moderation::{
    MODERATION_MODEL_WORKING_MEMORY_BYTES_V1, MODERATION_REPRO_MANIFEST_VERSION_V1,
    MODERATION_SIGNED_RESULT_VERSION_V1, MODERATION_TRUST_POLICY_VERSION_V1,
    ModerationFeatureProfileV1, ModerationModelEngineV1, ModerationModelFingerprintV1,
    ModerationModelScoreV1, ModerationReproBodyV1, ModerationReproSignatureV1,
    ModerationSeedMaterialV1, ModerationSignedScreeningBodyV1, ModerationThresholdsV1,
    ModerationTrustPolicyBodyV1, ModerationTrustPolicySignatureV1, ModerationTrustedSignerV1,
    moderation_model_required_operations_v1,
};

use super::*;

const SCREENING_AUTH_NOW: u64 = 1_800_000_000;
const TEST_QUARANTINE_KEY_PROVIDER_HANDLE: &str = "kms://moderation/quarantine/primary";
const TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION: ModerationQuarantineKeyProviderQualificationV1 =
    ModerationQuarantineKeyProviderQualificationV1::new(7, [0xC7; 32]);

fn deterministic_ed25519_key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("deterministic Ed25519 key")
}

fn authenticated_screening_manifest(signing_key: &KeyPair) -> ModerationReproManifestV1 {
    let mut body = ModerationReproBodyV1 {
        schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
        manifest_id: [0xA1; 16],
        manifest_digest: [0; 32],
        runner_hash: [0xA2; 32],
        runtime_version: "sorafs-ai-runner-v1".to_owned(),
        issued_at_unix: SCREENING_AUTH_NOW - 2_000,
        seed_material: ModerationSeedMaterialV1 {
            domain_tag: "sorafs:moderation:v1".to_owned(),
            seed_version: 1,
            run_nonce: [0xA3; 32],
        },
        thresholds: ModerationThresholdsV1 {
            quarantine: 4_000,
            escalate: 8_000,
        },
        models: vec![ModerationModelFingerprintV1 {
            model_id: [0xA4; 16],
            artifact_path: "models/moderation-v1.norito".to_owned(),
            artifact_bytes: 4096,
            artifact_digest: [0xA5; 32],
            weights_digest: [0xA6; 32],
            engine: ModerationModelEngineV1::DeterministicLinearV1,
            feature_profile: ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
            calibration_knot_count: 2,
            max_input_bytes: 1024,
            max_operations: moderation_model_required_operations_v1(1024, 2)
                .expect("model operation budget"),
            working_memory_bytes: MODERATION_MODEL_WORKING_MEMORY_BYTES_V1,
            weight: Some(10_000),
        }],
        notes: None,
    };
    body.refresh_manifest_digest().expect("manifest digest");
    ModerationReproManifestV1 {
        body: body.clone(),
        signatures: vec![ModerationReproSignatureV1 {
            role: "model-governance".to_owned(),
            public_key: signing_key.public_key().clone(),
            signature: SignatureOf::try_new(signing_key.private_key(), &body)
                .expect("sign manifest"),
        }],
    }
}

fn authenticated_screening_policy(
    manifest: &ModerationReproManifestV1,
    governance_key: &KeyPair,
    runner_keys: &[&KeyPair],
    result_quorum: u16,
) -> ModerationTrustPolicyV1 {
    let mut trusted_signers = runner_keys
        .iter()
        .enumerate()
        .map(|(index, key)| ModerationTrustedSignerV1 {
            role: format!("runner-{index}"),
            public_key: key.public_key().clone(),
            valid_from_unix: SCREENING_AUTH_NOW - 1_000,
            valid_until_unix: SCREENING_AUTH_NOW + 1_000,
            revoked_at_unix: None,
        })
        .collect::<Vec<_>>();
    trusted_signers.sort_by(|left, right| left.public_key.cmp(&right.public_key));
    let mut body = ModerationTrustPolicyBodyV1 {
        schema_version: MODERATION_TRUST_POLICY_VERSION_V1,
        policy_id: [0xB1; 16],
        policy_digest: [0; 32],
        manifest_id: manifest.body.manifest_id,
        manifest_digest: manifest.body.manifest_digest,
        runner_hash: manifest.body.runner_hash,
        issued_at_unix: SCREENING_AUTH_NOW - 2_000,
        valid_from_unix: SCREENING_AUTH_NOW - 1_000,
        valid_until_unix: SCREENING_AUTH_NOW + 1_000,
        result_quorum,
        governance_quorum: 1,
        max_result_age_secs: 600,
        max_result_ttl_secs: 300,
        max_clock_skew_secs: 30,
        trusted_signers,
        notes: None,
    };
    body.refresh_policy_digest().expect("policy digest");
    ModerationTrustPolicyV1 {
        body: body.clone(),
        signatures: vec![ModerationTrustPolicySignatureV1 {
            role: "governance".to_owned(),
            public_key: governance_key.public_key().clone(),
            signature: SignatureOf::try_new(governance_key.private_key(), &body)
                .expect("sign policy"),
        }],
    }
}

fn authenticated_screening_result(
    manifest: &ModerationReproManifestV1,
    policy: &ModerationTrustPolicyV1,
    runner_key: &KeyPair,
    score_bps: u16,
    subject: &str,
) -> ModerationSignedScreeningResultV1 {
    let verdict = if score_bps >= manifest.body.thresholds.escalate {
        "escalate"
    } else if score_bps >= manifest.body.thresholds.quarantine {
        "quarantine"
    } else {
        "pass"
    };
    let mut body = ModerationSignedScreeningBodyV1 {
        schema_version: MODERATION_SIGNED_RESULT_VERSION_V1,
        manifest_id: manifest.body.manifest_id,
        manifest_digest: manifest.body.manifest_digest,
        runner_hash: manifest.body.runner_hash,
        trust_policy_id: policy.body.policy_id,
        trust_policy_digest: policy.body.policy_digest,
        subject: subject.to_owned(),
        subject_digest: *blake3::hash(subject.as_bytes()).as_bytes(),
        model_scores: vec![ModerationModelScoreV1 {
            model_id: manifest.body.models[0].model_id,
            artifact_digest: manifest.body.models[0].artifact_digest,
            score_bps,
        }],
        combined_score_bps: score_bps,
        verdict: verdict.to_owned(),
        screened_at_unix: SCREENING_AUTH_NOW - 10,
        expires_at_unix: SCREENING_AUTH_NOW + 100,
        policy_digest: manifest
            .body
            .computed_screening_policy_digest()
            .expect("screening policy digest"),
        evidence_digest: [0; 32],
        notes: None,
    };
    body.refresh_evidence_digest().expect("evidence digest");
    ModerationSignedScreeningResultV1 {
        signer_public_key: runner_key.public_key().clone(),
        signature: SignatureOf::try_new(runner_key.private_key(), &body).expect("sign result"),
        body,
    }
}

#[derive(Debug)]
struct TestQuarantineKeyWrapper {
    provider_handle: String,
    qualification: Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    >,
    key_id: String,
    wrapping_key: [u8; 32],
}

impl ModerationQuarantineKeyWrapper for TestQuarantineKeyWrapper {
    fn provider_handle(&self) -> &str {
        &self.provider_handle
    }

    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        self.qualification
    }

    fn active_key_id(&self) -> &str {
        &self.key_id
    }

    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        let mut nonce_hasher = blake3::Hasher::new_keyed(&self.wrapping_key);
        nonce_hasher.update(b"sorafs.moderation.test-key-wrapper.nonce.v1");
        nonce_hasher.update(self.key_id.as_bytes());
        nonce_hasher.update(&context_digest);
        let digest = nonce_hasher.finalize();
        let nonce = &digest.as_bytes()[..12];
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(self.wrapping_key)
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })?
            .encrypt(nonce, context_digest.as_slice(), dek.as_slice())
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })
    }

    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        if key_id != self.key_id {
            return Err(ModerationQuarantineKeyOperationErrorV1::StaleOrRevoked);
        }
        let mut nonce_hasher = blake3::Hasher::new_keyed(&self.wrapping_key);
        nonce_hasher.update(b"sorafs.moderation.test-key-wrapper.nonce.v1");
        nonce_hasher.update(self.key_id.as_bytes());
        nonce_hasher.update(&context_digest);
        let digest = nonce_hasher.finalize();
        let nonce = &digest.as_bytes()[..12];
        let plaintext = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(self.wrapping_key)
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })?
            .decrypt(nonce, context_digest.as_slice(), wrapped_dek)
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })?;
        plaintext
            .try_into()
            .map_err(|_| ModerationQuarantineKeyOperationErrorV1::Rejected)
    }
}

fn test_key_wrapper(seed: u8, key_id: &str) -> TestQuarantineKeyWrapper {
    test_key_wrapper_for_provider(
        seed,
        key_id,
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE,
        Ok(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION),
    )
}

fn test_key_wrapper_for_provider(
    seed: u8,
    key_id: &str,
    provider_handle: &str,
    qualification: Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    >,
) -> TestQuarantineKeyWrapper {
    TestQuarantineKeyWrapper {
        provider_handle: provider_handle.to_owned(),
        qualification,
        key_id: key_id.to_owned(),
        wrapping_key: [seed; 32],
    }
}

fn test_key_provider_binding() -> ModerationQuarantineKeyProviderBindingV1 {
    ModerationQuarantineKeyProviderBindingV1::try_new(
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE.to_owned(),
        TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
    )
    .expect("valid test quarantine-key provider binding")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QualificationDriftTrigger {
    Wrap,
    Unwrap,
}

#[derive(Debug)]
struct DriftingQuarantineKeyWrapper {
    inner: TestQuarantineKeyWrapper,
    trigger: QualificationDriftTrigger,
    qualification: std::sync::Mutex<ModerationQuarantineKeyProviderQualificationV1>,
}

impl DriftingQuarantineKeyWrapper {
    fn new(inner: TestQuarantineKeyWrapper, trigger: QualificationDriftTrigger) -> Self {
        let qualification = inner
            .qualification
            .expect("drift wrapper requires an initially qualified provider");
        Self {
            inner,
            trigger,
            qualification: std::sync::Mutex::new(qualification),
        }
    }

    fn drift_qualification(&self) {
        let mut qualification = self.qualification.lock().expect("test qualification lock");
        *qualification = ModerationQuarantineKeyProviderQualificationV1::new(
            qualification.revision() + 1,
            qualification.policy_digest(),
        );
    }
}

impl ModerationQuarantineKeyWrapper for DriftingQuarantineKeyWrapper {
    fn provider_handle(&self) -> &str {
        self.inner.provider_handle()
    }

    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        self.qualification
            .lock()
            .map(|qualification| *qualification)
            .map_err(|_| ModerationQuarantineKeyProviderReadinessErrorV1::Rejected)
    }

    fn active_key_id(&self) -> &str {
        self.inner.active_key_id()
    }

    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        let output = self.inner.wrap_dek(context_digest, dek);
        if self.trigger == QualificationDriftTrigger::Wrap {
            self.drift_qualification();
        }
        output
    }

    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        let output = self.inner.unwrap_dek(key_id, context_digest, wrapped_dek);
        if self.trigger == QualificationDriftTrigger::Unwrap {
            self.drift_qualification();
        }
        output
    }
}

#[derive(Debug)]
struct ActiveKeyIdDriftQuarantineKeyWrapper {
    original: TestQuarantineKeyWrapper,
    replacement: TestQuarantineKeyWrapper,
    use_replacement: std::sync::atomic::AtomicBool,
}

impl ActiveKeyIdDriftQuarantineKeyWrapper {
    fn new(original: TestQuarantineKeyWrapper, replacement: TestQuarantineKeyWrapper) -> Self {
        assert_eq!(original.provider_handle, replacement.provider_handle);
        assert_eq!(original.qualification, replacement.qualification);
        Self {
            original,
            replacement,
            use_replacement: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl ModerationQuarantineKeyWrapper for ActiveKeyIdDriftQuarantineKeyWrapper {
    fn provider_handle(&self) -> &str {
        self.original.provider_handle()
    }

    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        self.original.qualification()
    }

    fn active_key_id(&self) -> &str {
        if self
            .use_replacement
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            self.replacement.active_key_id()
        } else {
            self.original.active_key_id()
        }
    }

    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        self.use_replacement
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.replacement.wrap_dek(context_digest, dek)
    }

    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        if key_id == self.original.active_key_id() {
            self.original
                .unwrap_dek(key_id, context_digest, wrapped_dek)
        } else {
            self.replacement
                .unwrap_dek(key_id, context_digest, wrapped_dek)
        }
    }
}

const SECRET_PROVIDER_ERROR_SENTINEL: &str = "SECRET-PKCS11-PIN-DO-NOT-EMIT";

#[derive(Debug)]
struct FailingQuarantineKeyWrapper;

impl ModerationQuarantineKeyWrapper for FailingQuarantineKeyWrapper {
    fn provider_handle(&self) -> &str {
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE
    }

    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        Ok(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION)
    }

    fn active_key_id(&self) -> &str {
        "pkcs11:test/redacted-provider-error"
    }

    fn wrap_dek(
        &self,
        _context_digest: [u8; 32],
        _dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        Err(ModerationQuarantineKeyOperationErrorV1::Ambiguous
            .after_scrubbing_provider_diagnostic(SECRET_PROVIDER_ERROR_SENTINEL.to_owned()))
    }

    fn unwrap_dek(
        &self,
        _key_id: &str,
        _context_digest: [u8; 32],
        _wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        Err(ModerationQuarantineKeyOperationErrorV1::Rejected
            .after_scrubbing_provider_diagnostic(SECRET_PROVIDER_ERROR_SENTINEL.to_owned()))
    }
}

#[derive(Debug)]
struct FailingOperationThenStaleWrapper {
    qualification_calls: std::sync::atomic::AtomicU64,
    failure: ModerationQuarantineKeyOperationErrorV1,
}

impl FailingOperationThenStaleWrapper {
    fn new(failure: ModerationQuarantineKeyOperationErrorV1) -> Self {
        Self {
            qualification_calls: std::sync::atomic::AtomicU64::new(0),
            failure,
        }
    }
}

impl ModerationQuarantineKeyWrapper for FailingOperationThenStaleWrapper {
    fn provider_handle(&self) -> &str {
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE
    }

    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        if self
            .qualification_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            == 0
        {
            Ok(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION)
        } else {
            Err(ModerationQuarantineKeyProviderReadinessErrorV1::Rejected)
        }
    }

    fn active_key_id(&self) -> &str {
        "pkcs11:test/failure-before-requalification"
    }

    fn wrap_dek(
        &self,
        _context_digest: [u8; 32],
        _dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        Err(self.failure)
    }

    fn unwrap_dek(
        &self,
        _key_id: &str,
        _context_digest: [u8; 32],
        _wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        Err(self.failure)
    }
}

fn screening_input(subject: &str, verdict: ModerationScreeningVerdict) -> ModerationScreeningInput {
    ModerationScreeningInput {
        subject: subject.to_owned(),
        subject_digest: *blake3::hash(subject.as_bytes()).as_bytes(),
        manifest_id: [0x12; 16],
        runner_hash: [0x34; 32],
        combined_score_bps: if verdict.requires_quarantine_record() {
            7_000
        } else {
            1_000
        },
        verdict,
        screened_at_unix: 1_800_000_050,
        evidence_digest: Some([0xE1; 32]),
        policy_digest: Some([0xC1; 32]),
        notes: None,
    }
}

fn quarantine_object_record(seed: u8) -> ModerationQuarantineObjectRecord {
    let wrapper = test_key_wrapper(0x7B, "pkcs11:test/quarantine");
    let binding = test_key_provider_binding();
    seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [seed; 16],
            payload: vec![seed; 32],
            captured_at_unix: 1_800_000_100 + u64::from(seed),
            content_type: None,
            notes: None,
        },
        &binding,
        &wrapper,
    )
    .expect("seal quarantine object")
    .0
}

fn evidence_session_input(
    quarantine_id: [u8; 16],
    nonce: u8,
) -> ModerationEvidenceViewerSessionInput {
    ModerationEvidenceViewerSessionInput {
        quarantine_id,
        requested_by: "operator@moderation".to_owned(),
        viewer_account: "juror@moderation".to_owned(),
        viewer_role: "juror".to_owned(),
        purpose: "appeal evidence review".to_owned(),
        attestation_digest: [0xA7; 32],
        watermark_metadata_digest: [0xB7; 32],
        session_nonce_digest: [nonce; 32],
        issued_at_unix_ms: 1_800_000_100_000,
        expires_at_unix_ms: 1_800_000_200_000,
        legal_hold_id: None,
        notes: None,
        raw_evidence_included: false,
        signed_url_included: false,
        session_token_included: false,
        watermark_secret_included: false,
    }
}

fn evidence_access_input(session_id: [u8; 16]) -> ModerationEvidenceViewerAccessInput {
    ModerationEvidenceViewerAccessInput {
        session_id,
        kind: ModerationEvidenceViewerAccessKind::Viewed,
        actor_account: "juror@moderation".to_owned(),
        event_at_unix_ms: 1_800_000_100_001,
        request_digest: [0xD7; 32],
        event_metadata_digest: None,
        notes: None,
        raw_evidence_included: false,
        signed_url_included: false,
        session_token_included: false,
        response_body_included: false,
    }
}

#[test]
fn authenticated_screening_gate_accepts_signed_result_only_for_single_signer_policy() {
    let governance_key = deterministic_ed25519_key(0x31);
    let manifest_key = deterministic_ed25519_key(0x32);
    let runner = deterministic_ed25519_key(0x33);
    let manifest = authenticated_screening_manifest(&manifest_key);
    let policy = authenticated_screening_policy(&manifest, &governance_key, &[&runner], 1);
    let anchors = BTreeSet::from([governance_key.public_key().clone()]);
    let signed = authenticated_screening_result(&manifest, &policy, &runner, 6_000, "cid:screened");
    let expected_digest = signed.body.evidence_digest;

    let verified = verify_authenticated_moderation_screening_v1(
        ModerationAuthenticatedScreeningRequestV1 {
            idempotency_key: [0x91; 32],
            evidence: ModerationAuthenticatedScreeningEvidenceV1::Signed(signed.clone()),
        },
        &manifest,
        &policy,
        &anchors,
        1,
        SCREENING_AUTH_NOW,
    )
    .expect("authenticate signed result");
    assert_eq!(verified.authority_digest, expected_digest);
    assert_eq!(verified.authority_kind, "signed_result");
    assert_eq!(
        verified.screening.verdict,
        ModerationScreeningVerdict::Quarantine
    );

    let mut tampered = signed;
    tampered.body.subject_digest[0] ^= 1;
    assert!(matches!(
        verify_authenticated_moderation_screening_v1(
            ModerationAuthenticatedScreeningRequestV1 {
                idempotency_key: [0x92; 32],
                evidence: ModerationAuthenticatedScreeningEvidenceV1::Signed(tampered),
            },
            &manifest,
            &policy,
            &anchors,
            1,
            SCREENING_AUTH_NOW,
        ),
        Err(ModerationScreeningAuthenticationError::InvalidSignedResult { .. })
    ));
    assert!(matches!(
        verify_authenticated_moderation_screening_v1(
            ModerationAuthenticatedScreeningRequestV1 {
                idempotency_key: [0; 32],
                evidence: ModerationAuthenticatedScreeningEvidenceV1::Signed(
                    authenticated_screening_result(
                        &manifest,
                        &policy,
                        &runner,
                        6_000,
                        "cid:screened",
                    ),
                ),
            },
            &manifest,
            &policy,
            &anchors,
            1,
            SCREENING_AUTH_NOW,
        ),
        Err(ModerationScreeningAuthenticationError::MissingIdempotencyKey)
    ));
}

#[test]
fn authenticated_screening_gate_reconstructs_committee_and_rejects_duplicates() {
    let governance_key = deterministic_ed25519_key(0x41);
    let manifest_key = deterministic_ed25519_key(0x42);
    let runner_a = deterministic_ed25519_key(0x43);
    let runner_b = deterministic_ed25519_key(0x44);
    let manifest = authenticated_screening_manifest(&manifest_key);
    let policy =
        authenticated_screening_policy(&manifest, &governance_key, &[&runner_a, &runner_b], 2);
    let anchors = BTreeSet::from([governance_key.public_key().clone()]);
    let result_a =
        authenticated_screening_result(&manifest, &policy, &runner_a, 8_500, "cid:committee");
    let result_b =
        authenticated_screening_result(&manifest, &policy, &runner_b, 8_700, "cid:committee");

    assert!(matches!(
        verify_authenticated_moderation_screening_v1(
            ModerationAuthenticatedScreeningRequestV1 {
                idempotency_key: [0x93; 32],
                evidence: ModerationAuthenticatedScreeningEvidenceV1::Signed(result_a.clone()),
            },
            &manifest,
            &policy,
            &anchors,
            1,
            SCREENING_AUTH_NOW,
        ),
        Err(ModerationScreeningAuthenticationError::CommitteeRequired { required: 2 })
    ));

    let aggregate = ModerationCommitteeAggregateV1::aggregate_authenticated(
        &manifest,
        &policy,
        &anchors,
        1,
        &[result_a.clone(), result_b.clone()],
        SCREENING_AUTH_NOW,
    )
    .expect("build authenticated aggregate");
    let verified = verify_authenticated_moderation_screening_v1(
        ModerationAuthenticatedScreeningRequestV1 {
            idempotency_key: [0x94; 32],
            evidence: ModerationAuthenticatedScreeningEvidenceV1::Committee {
                aggregate: aggregate.clone(),
                signed_results: vec![result_a.clone(), result_b],
            },
        },
        &manifest,
        &policy,
        &anchors,
        1,
        SCREENING_AUTH_NOW,
    )
    .expect("authenticate exact committee aggregate");
    assert_eq!(verified.authority_digest, aggregate.aggregate_digest);
    assert_eq!(verified.authority_kind, "committee_aggregate");
    assert_eq!(
        verified.screening.verdict,
        ModerationScreeningVerdict::Escalate
    );

    assert!(matches!(
        verify_authenticated_moderation_screening_v1(
            ModerationAuthenticatedScreeningRequestV1 {
                idempotency_key: [0x95; 32],
                evidence: ModerationAuthenticatedScreeningEvidenceV1::Committee {
                    aggregate,
                    signed_results: vec![result_a.clone(), result_a],
                },
            },
            &manifest,
            &policy,
            &anchors,
            1,
            SCREENING_AUTH_NOW,
        ),
        Err(ModerationScreeningAuthenticationError::InvalidCommittee { .. })
    ));
}

#[test]
fn authenticated_screening_runtime_persists_idempotency_and_replay_bindings() {
    let governance_key = deterministic_ed25519_key(0x51);
    let manifest_key = deterministic_ed25519_key(0x52);
    let runner = deterministic_ed25519_key(0x53);
    let manifest = authenticated_screening_manifest(&manifest_key);
    let policy = authenticated_screening_policy(&manifest, &governance_key, &[&runner], 1);
    let anchors = BTreeSet::from([governance_key.public_key().clone()]);
    let signed = authenticated_screening_result(&manifest, &policy, &runner, 6_700, "cid:durable");
    let verified = verify_authenticated_moderation_screening_v1(
        ModerationAuthenticatedScreeningRequestV1 {
            idempotency_key: [0xA1; 32],
            evidence: ModerationAuthenticatedScreeningEvidenceV1::Signed(signed),
        },
        &manifest,
        &policy,
        &anchors,
        1,
        SCREENING_AUTH_NOW,
    )
    .expect("authenticate signed result");

    let mut runtime = ModerationScreeningRuntime::with_entry_limit(4);
    let admitted = runtime
        .record_authenticated_screening(verified.clone())
        .expect("record authenticated result");
    assert_eq!(
        admitted.admission.authority_digest,
        verified.authority_digest
    );
    assert_eq!(
        runtime
            .record_authenticated_screening(verified.clone())
            .expect("idempotent authenticated retry"),
        admitted
    );

    let mut conflicting_key = verified.clone();
    conflicting_key.authority_digest[0] ^= 1;
    assert!(matches!(
        runtime
            .record_authenticated_screening(conflicting_key)
            .expect_err("idempotency conflict rejected"),
        ModerationScreeningError::ConflictingIdempotencyKey { .. }
    ));
    let mut replayed_authority = verified.clone();
    replayed_authority.idempotency_key = [0xA2; 32];
    assert!(matches!(
        runtime
            .record_authenticated_screening(replayed_authority)
            .expect_err("authority replay rejected"),
        ModerationScreeningError::ReplayedAuthority { .. }
    ));

    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.authenticated_admissions.len(), 1);
    let mut restored = ModerationScreeningRuntime::with_entry_limit(4);
    restored
        .restore_snapshot(snapshot.clone())
        .expect("restore authenticated replay receipt");
    assert_eq!(
        restored
            .record_authenticated_screening(verified)
            .expect("idempotent retry survives restore"),
        admitted
    );

    let mut tampered = snapshot;
    tampered.authenticated_admissions[0].receipt_digest[0] ^= 1;
    assert!(matches!(
        restored
            .restore_snapshot(tampered)
            .expect_err("tampered replay receipt rejected"),
        ModerationScreeningError::InvalidSnapshot { .. }
    ));
}

#[test]
fn moderation_quarantine_key_provider_rejects_unavailable_and_stale_adapters() {
    let binding = test_key_provider_binding();
    let ready = test_key_wrapper(0x81, "kms:test/ready");
    assert_eq!(
        validate_moderation_quarantine_key_wrapper(&binding, &ready),
        Ok(())
    );
    let unavailable = test_key_wrapper_for_provider(
        0x81,
        "kms:test/unavailable",
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE,
        Err(ModerationQuarantineKeyProviderReadinessErrorV1::Unavailable),
    );
    assert_eq!(
        binding.qualify(&unavailable),
        Err(ModerationQuarantineKeyProviderQualificationErrorV1::UnavailableOrStale)
    );
    assert_eq!(
        validate_moderation_quarantine_key_wrapper(&binding, &unavailable),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );

    let rejected_as_stale = test_key_wrapper_for_provider(
        0x81,
        "kms:test/stale",
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE,
        Err(ModerationQuarantineKeyProviderReadinessErrorV1::Rejected),
    );
    assert_eq!(
        binding.qualify(&rejected_as_stale),
        Err(ModerationQuarantineKeyProviderQualificationErrorV1::UnavailableOrStale)
    );

    let old_revision = test_key_wrapper_for_provider(
        0x81,
        "kms:test/old-revision",
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE,
        Ok(ModerationQuarantineKeyProviderQualificationV1::new(
            TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION.revision() - 1,
            TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION.policy_digest(),
        )),
    );
    assert_eq!(
        binding.qualify(&old_revision),
        Err(ModerationQuarantineKeyProviderQualificationErrorV1::QualificationMismatch)
    );
}

#[test]
fn moderation_quarantine_key_provider_rejects_substitution() {
    let binding = test_key_provider_binding();
    let substituted = test_key_wrapper_for_provider(
        0x82,
        "kms:test/substituted",
        "kms://moderation/quarantine/secondary",
        Ok(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION),
    );
    assert_eq!(
        binding.qualify(&substituted),
        Err(ModerationQuarantineKeyProviderQualificationErrorV1::SubstitutedProvider)
    );
    assert_eq!(
        validate_moderation_quarantine_key_wrapper(&binding, &substituted),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );
}

#[test]
fn moderation_quarantine_provider_handles_use_canonical_production_grammar() {
    for handle in [
        "kms://sorafs/moderation/quarantine-primary",
        "hsm://sorafs/moderation/quarantine-primary",
    ] {
        assert_eq!(
            validate_moderation_quarantine_key_provider_handle(handle, true),
            Ok(())
        );
    }
    for handle in [
        "kms://sorafs/moderation/operator@quarantine",
        "kms://sorafs/moderation/quarantine?token",
        "kms://sorafs/moderation/quarantine#fragment",
        "kms://sorafs/moderation/%71uarantine",
        "kms://sorafs/moderation/quarantine\\primary",
    ] {
        assert_eq!(
            validate_moderation_quarantine_key_provider_handle(handle, true),
            Err(ModerationQuarantineKeyProviderQualificationErrorV1::InvalidConfiguredHandle)
        );
        assert_eq!(
            validate_moderation_quarantine_key_provider_handle(handle, false),
            Err(ModerationQuarantineKeyProviderQualificationErrorV1::InvalidProviderHandle)
        );
    }
}

#[test]
fn moderation_quarantine_key_provider_rejects_test_markers_and_zero_qualification() {
    assert_eq!(
        ModerationQuarantineKeyProviderBindingV1::try_new(
            "kms://moderation/dummy/primary".to_owned(),
            TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
        ),
        Err(ModerationQuarantineKeyProviderQualificationErrorV1::TestMarkedConfiguredHandle)
    );

    let binding = test_key_provider_binding();
    let test_marked = test_key_wrapper_for_provider(
        0x83,
        "kms:test/marked-provider",
        "kms://moderation/dummy/primary",
        Ok(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION),
    );
    assert_eq!(
        binding.qualify(&test_marked),
        Err(ModerationQuarantineKeyProviderQualificationErrorV1::TestMarkedProviderHandle)
    );

    for invalid in [
        ModerationQuarantineKeyProviderQualificationV1::new(0, [0xC7; 32]),
        ModerationQuarantineKeyProviderQualificationV1::new(7, [0; 32]),
    ] {
        assert_eq!(
            ModerationQuarantineKeyProviderBindingV1::try_new(
                TEST_QUARANTINE_KEY_PROVIDER_HANDLE.to_owned(),
                invalid,
            ),
            Err(
                ModerationQuarantineKeyProviderQualificationErrorV1::InvalidConfiguredQualification
            )
        );
        let invalid_provider = test_key_wrapper_for_provider(
            0x83,
            "kms:test/invalid-qualification",
            TEST_QUARANTINE_KEY_PROVIDER_HANDLE,
            Ok(invalid),
        );
        assert_eq!(
            binding.qualify(&invalid_provider),
            Err(ModerationQuarantineKeyProviderQualificationErrorV1::InvalidQualification)
        );
    }
}

#[test]
fn moderation_quarantine_wrap_and_unwrap_discard_outputs_on_provider_drift() {
    let binding = test_key_provider_binding();
    let wrapping = DriftingQuarantineKeyWrapper::new(
        test_key_wrapper(0x84, "kms:test/drifting-wrap"),
        QualificationDriftTrigger::Wrap,
    );
    assert_eq!(
        seal_moderation_quarantine_object(
            ModerationQuarantineObjectInput {
                quarantine_id: [0x84; 16],
                payload: b"provider output must remain held".to_vec(),
                captured_at_unix: 1_800_000_484,
                content_type: None,
                notes: None,
            },
            &binding,
            &wrapping,
        ),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );

    let stable = test_key_wrapper(0x85, "kms:test/drifting-unwrap");
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x85; 16],
            payload: b"recovered plaintext must never escape".to_vec(),
            captured_at_unix: 1_800_000_485,
            content_type: None,
            notes: None,
        },
        &binding,
        &stable,
    )
    .expect("seal drift fixture with stable provider");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode drift fixture");
    let unwrapping = DriftingQuarantineKeyWrapper::new(
        test_key_wrapper(0x85, "kms:test/drifting-unwrap"),
        QualificationDriftTrigger::Unwrap,
    );
    assert_eq!(
        open_moderation_quarantine_object(&envelope, &record, &binding, &unwrapping),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );
}

#[test]
fn moderation_quarantine_discards_wrap_output_when_active_key_changes() {
    let binding = test_key_provider_binding();
    let seal_wrapper = ActiveKeyIdDriftQuarantineKeyWrapper::new(
        test_key_wrapper(0x88, "kms:test/active-key-v1"),
        test_key_wrapper(0x89, "kms:test/active-key-v2"),
    );
    assert_eq!(
        seal_moderation_quarantine_object(
            ModerationQuarantineObjectInput {
                quarantine_id: [0x88; 16],
                payload: b"active key must remain stable across wrapping".to_vec(),
                captured_at_unix: 1_800_000_488,
                content_type: None,
                notes: None,
            },
            &binding,
            &seal_wrapper,
        ),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );

    let current_wrapper = test_key_wrapper(0x8A, "kms:test/rewrap-active-current");
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x8A; 16],
            payload: b"replacement active key must remain stable".to_vec(),
            captured_at_unix: 1_800_000_490,
            content_type: None,
            notes: None,
        },
        &binding,
        &current_wrapper,
    )
    .expect("seal active-key rewrap fixture");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode active-key rewrap fixture");
    let replacement_wrapper = ActiveKeyIdDriftQuarantineKeyWrapper::new(
        test_key_wrapper(0x8B, "kms:test/rewrap-active-v1"),
        test_key_wrapper(0x8C, "kms:test/rewrap-active-v2"),
    );
    assert_eq!(
        rewrap_moderation_quarantine_object(
            &envelope,
            &record,
            &binding,
            &current_wrapper,
            &binding,
            &replacement_wrapper,
        ),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );
}

#[test]
fn moderation_quarantine_rewrap_checks_current_and_replacement_provider_drift_independently() {
    let current_binding = test_key_provider_binding();
    let current_wrapper = test_key_wrapper(0x86, "kms:test/rewrap-current");
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x86; 16],
            payload: b"independent rewrap provider bindings".to_vec(),
            captured_at_unix: 1_800_000_486,
            content_type: None,
            notes: None,
        },
        &current_binding,
        &current_wrapper,
    )
    .expect("seal independent rewrap fixture");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode independent rewrap fixture");

    let replacement_qualification =
        ModerationQuarantineKeyProviderQualificationV1::new(9, [0xD9; 32]);
    let replacement_binding = ModerationQuarantineKeyProviderBindingV1::try_new(
        "kms://moderation/quarantine/secondary".to_owned(),
        replacement_qualification,
    )
    .expect("valid replacement provider binding");
    let replacement_wrapper = test_key_wrapper_for_provider(
        0x87,
        "kms:test/rewrap-replacement",
        replacement_binding.provider_handle(),
        Ok(replacement_qualification),
    );

    let drifting_current = DriftingQuarantineKeyWrapper::new(
        test_key_wrapper(0x86, "kms:test/rewrap-current"),
        QualificationDriftTrigger::Unwrap,
    );
    assert_eq!(
        rewrap_moderation_quarantine_object(
            &envelope,
            &record,
            &current_binding,
            &drifting_current,
            &replacement_binding,
            &replacement_wrapper,
        ),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );

    let drifting_replacement = DriftingQuarantineKeyWrapper::new(
        test_key_wrapper_for_provider(
            0x87,
            "kms:test/rewrap-replacement",
            replacement_binding.provider_handle(),
            Ok(replacement_qualification),
        ),
        QualificationDriftTrigger::Wrap,
    );
    assert_eq!(
        rewrap_moderation_quarantine_object(
            &envelope,
            &record,
            &current_binding,
            &current_wrapper,
            &replacement_binding,
            &drifting_replacement,
        ),
        Err(ModerationQuarantineObjectError::KeyWrapperUnqualified)
    );
}

#[test]
fn moderation_quarantine_object_seal_open_preserves_object_id() {
    let wrapper = test_key_wrapper(0x7B, "pkcs11:test/quarantine");
    let binding = test_key_provider_binding();
    let payload = b"quarantine payload bytes".to_vec();
    let input = ModerationQuarantineObjectInput {
        quarantine_id: [0x42; 16],
        payload: payload.clone(),
        captured_at_unix: 1_700_000_001,
        content_type: Some("application/octet-stream".to_owned()),
        notes: None,
    };

    let (record, envelope_bytes) =
        seal_moderation_quarantine_object(input, &binding, &wrapper).expect("seal object");
    let envelope =
        norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&envelope_bytes)
            .expect("decode envelope");
    let expected_object_id = moderation_quarantine_object_id(
        &quarantine_immutable_metadata_from_envelope(&envelope)
            .expect("rebuild immutable metadata"),
    )
    .expect("derive object id");

    assert_eq!(record.object_id, expected_object_id);
    assert_eq!(envelope.object_id, expected_object_id);
    let opened = open_moderation_quarantine_object(&envelope, &record, &binding, &wrapper)
        .expect("open object");
    assert_eq!(opened, payload);
}

#[test]
fn moderation_quarantine_key_operation_errors_are_stable_and_payload_free() {
    let cases = [
        (
            ModerationQuarantineKeyOperationErrorV1::Unavailable,
            "moderation quarantine key operation unavailable",
        ),
        (
            ModerationQuarantineKeyOperationErrorV1::Rejected,
            "moderation quarantine key operation rejected",
        ),
        (
            ModerationQuarantineKeyOperationErrorV1::StaleOrRevoked,
            "moderation quarantine key or policy is stale or revoked",
        ),
        (
            ModerationQuarantineKeyOperationErrorV1::Ambiguous,
            "moderation quarantine key wrap outcome is ambiguous",
        ),
    ];

    for (failure, expected_display) in cases {
        let failure =
            failure.after_scrubbing_provider_diagnostic(SECRET_PROVIDER_ERROR_SENTINEL.to_owned());
        assert_eq!(failure.to_string(), expected_display);
        for rendered in [failure.to_string(), format!("{failure:?}")] {
            assert!(!rendered.contains(SECRET_PROVIDER_ERROR_SENTINEL));
            assert!(!rendered.contains("PIN"));
        }
        assert!(matches!(
            ModerationQuarantineObjectError::key_operation_failure(
                "kms:production/moderation/quarantine".to_owned(),
                failure,
            ),
            ModerationQuarantineObjectError::KeyWrapping {
                failure: mapped,
                ..
            } if mapped == failure
        ));
    }
}

#[test]
fn key_operation_failure_is_not_masked_by_post_call_requalification() {
    let binding = test_key_provider_binding();
    let wrap_failure =
        FailingOperationThenStaleWrapper::new(ModerationQuarantineKeyOperationErrorV1::Ambiguous);
    let error = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x4F; 16],
            payload: b"operation-error-ordering".to_vec(),
            captured_at_unix: 1_800_000_499,
            content_type: None,
            notes: None,
        },
        &binding,
        &wrap_failure,
    )
    .expect_err("ambiguous wrap must fail before a second qualification");
    assert!(matches!(
        error,
        ModerationQuarantineObjectError::KeyWrapping {
            failure: ModerationQuarantineKeyOperationErrorV1::Ambiguous,
            ..
        }
    ));
    assert_eq!(
        wrap_failure
            .qualification_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    let working_wrapper = test_key_wrapper(0x6F, "kms:test/error-ordering-source");
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x4E; 16],
            payload: b"operation-error-ordering".to_vec(),
            captured_at_unix: 1_800_000_498,
            content_type: None,
            notes: None,
        },
        &binding,
        &working_wrapper,
    )
    .expect("seal ordering fixture");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode ordering fixture");
    let unwrap_failure =
        FailingOperationThenStaleWrapper::new(ModerationQuarantineKeyOperationErrorV1::Unavailable);
    let error = open_moderation_quarantine_object(&envelope, &record, &binding, &unwrap_failure)
        .expect_err("unavailable unwrap must fail before a second qualification");
    assert!(matches!(
        error,
        ModerationQuarantineObjectError::KeyWrapping {
            failure: ModerationQuarantineKeyOperationErrorV1::Unavailable,
            ..
        }
    ));
    assert_eq!(
        unwrap_failure
            .qualification_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[test]
fn moderation_quarantine_plaintext_and_provider_errors_are_redacted() {
    let binding = test_key_provider_binding();
    let secret_payload = b"SECRET-QUARANTINE-PLAINTEXT-DO-NOT-EMIT";
    let secret_content_type = "SECRET-CONTENT-TYPE-DO-NOT-EMIT";
    let secret_notes = "SECRET-PII-NOTES-DO-NOT-EMIT";
    let input = ModerationQuarantineObjectInput {
        quarantine_id: [0x50; 16],
        payload: secret_payload.to_vec(),
        captured_at_unix: 1_800_000_500,
        content_type: Some(secret_content_type.to_owned()),
        notes: Some(secret_notes.to_owned()),
    };
    let debug = format!("{input:?}");
    assert!(!debug.contains(std::str::from_utf8(secret_payload).expect("ASCII sentinel")));
    assert!(!debug.contains(secret_content_type));
    assert!(!debug.contains(secret_notes));
    assert!(debug.contains("<redacted>"));
    assert!(debug.contains(&format!("payload_len: {}", secret_payload.len())));
    assert!(debug.contains(&format!(
        "content_type_len: Some({})",
        secret_content_type.len()
    )));
    assert!(debug.contains(&format!("notes_len: Some({})", secret_notes.len())));

    let error = normalize_moderation_quarantine_object_input(input)
        .expect_err("plaintext notes must fail closed");
    assert!(!error.to_string().contains(secret_notes));

    let error = normalize_moderation_quarantine_object_input(ModerationQuarantineObjectInput {
        quarantine_id: [0x50; 16],
        payload: secret_payload.to_vec(),
        captured_at_unix: 1_800_000_500,
        content_type: Some(secret_content_type.to_owned()),
        notes: None,
    })
    .expect_err("non-canonical or private content type must fail closed");
    assert!(!error.to_string().contains(secret_content_type));

    let error = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x50; 16],
            payload: secret_payload.to_vec(),
            captured_at_unix: 1_800_000_500,
            content_type: Some("application/octet-stream".to_owned()),
            notes: None,
        },
        &binding,
        &FailingQuarantineKeyWrapper,
    )
    .expect_err("provider failure must fail closed");
    assert!(matches!(
        &error,
        ModerationQuarantineObjectError::KeyWrapping {
            failure: ModerationQuarantineKeyOperationErrorV1::Ambiguous,
            ..
        }
    ));
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(!rendered.contains(SECRET_PROVIDER_ERROR_SENTINEL));
        assert!(!rendered.contains("PIN"));
    }

    let working_wrapper = test_key_wrapper(0x70, "pkcs11:test/redaction-source");
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x51; 16],
            payload: secret_payload.to_vec(),
            captured_at_unix: 1_800_000_500,
            content_type: None,
            notes: None,
        },
        &binding,
        &working_wrapper,
    )
    .expect("seal provider-redaction fixture");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode provider-redaction fixture");
    let error = open_moderation_quarantine_object(
        &envelope,
        &record,
        &binding,
        &FailingQuarantineKeyWrapper,
    )
    .expect_err("unwrap provider failure must fail closed");
    assert!(matches!(
        &error,
        ModerationQuarantineObjectError::KeyWrapping {
            failure: ModerationQuarantineKeyOperationErrorV1::Rejected,
            ..
        }
    ));
    for rendered in [error.to_string(), format!("{error:?}")] {
        assert!(!rendered.contains(SECRET_PROVIDER_ERROR_SENTINEL));
        assert!(!rendered.contains("PIN"));
    }
}

#[test]
fn moderation_quarantine_object_authenticates_ranges_and_chunk_order() {
    let wrapper = test_key_wrapper(0x71, "kms:test/active");
    let binding = test_key_provider_binding();
    let payload_len =
        usize::try_from(MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1).expect("chunk size") * 2 + 137;
    let payload = (0..payload_len)
        .map(|index| u8::try_from(index % 251).expect("modulo fits u8"))
        .collect::<Vec<_>>();
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x52; 16],
            payload: payload.clone(),
            captured_at_unix: 1_800_000_501,
            content_type: Some("application/octet-stream".to_owned()),
            notes: None,
        },
        &binding,
        &wrapper,
    )
    .expect("seal chunked object");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode chunked envelope");
    assert_eq!(envelope.chunks.len(), 3);
    let nonces = envelope
        .chunks
        .iter()
        .map(|chunk| moderation_quarantine_chunk_nonce(envelope.nonce_prefix, chunk.index))
        .collect::<BTreeSet<_>>();
    assert_eq!(nonces.len(), envelope.chunks.len());

    let start = u64::from(MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1) - 23;
    let end = start + 100;
    let opened =
        open_moderation_quarantine_object_range(&envelope, &record, &binding, &wrapper, start..end)
            .expect("open authenticated cross-chunk range");
    assert_eq!(
        opened,
        payload[usize::try_from(start).unwrap()..usize::try_from(end).unwrap()]
    );

    let mut reordered = envelope.clone();
    reordered.chunks.swap(0, 1);
    assert!(matches!(
        open_moderation_quarantine_object(&reordered, &record, &binding, &wrapper),
        Err(ModerationQuarantineObjectError::InvalidSnapshot { .. })
    ));

    let mut late_failure = envelope.clone();
    let last = late_failure.chunks[1].ciphertext.len() - 1;
    late_failure.chunks[1].ciphertext[last] ^= 0x40;
    late_failure.ciphertext_digest = moderation_quarantine_ciphertext_digest(&late_failure.chunks);
    let late_failure_record = moderation_quarantine_object_record_from_envelope(
        &late_failure,
        record.envelope_path.clone(),
    )
    .expect("rebuild late-failure record");
    assert!(matches!(
        open_moderation_quarantine_object_range(
            &late_failure,
            &late_failure_record,
            &binding,
            &wrapper,
            0..u64::from(MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1) + 32,
        ),
        Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
    ));
}

#[test]
fn moderation_quarantine_object_rejects_wrong_key_tag_aad_and_wrapped_key_replay() {
    let wrapper = test_key_wrapper(0x72, "kms:test/active");
    let wrong_wrapper = test_key_wrapper(0x73, "kms:test/active");
    let binding = test_key_provider_binding();
    let input = |quarantine_id| ModerationQuarantineObjectInput {
        quarantine_id,
        payload: vec![0xA5; 96],
        captured_at_unix: 1_800_000_502,
        content_type: Some("application/octet-stream".to_owned()),
        notes: None,
    };
    let (record, bytes) = seal_moderation_quarantine_object(input([0x61; 16]), &binding, &wrapper)
        .expect("seal object");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode envelope");

    assert!(matches!(
        open_moderation_quarantine_object(&envelope, &record, &binding, &wrong_wrapper),
        Err(ModerationQuarantineObjectError::KeyWrapping { .. })
            | Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
    ));

    let mut bad_tag = envelope.clone();
    let last = bad_tag.chunks[0].ciphertext.len() - 1;
    bad_tag.chunks[0].ciphertext[last] ^= 0x80;
    bad_tag.ciphertext_digest = moderation_quarantine_ciphertext_digest(&bad_tag.chunks);
    let bad_tag_record =
        moderation_quarantine_object_record_from_envelope(&bad_tag, record.envelope_path.clone())
            .expect("rebuild record around tampered ciphertext");
    assert!(matches!(
        open_moderation_quarantine_object(&bad_tag, &bad_tag_record, &binding, &wrapper),
        Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
    ));

    let mut bad_aad = envelope.clone();
    bad_aad.captured_at_unix += 1;
    let bad_aad_metadata = quarantine_immutable_metadata_from_envelope(&bad_aad).expect("metadata");
    bad_aad.object_id = moderation_quarantine_object_id(&bad_aad_metadata).expect("object id");
    let bad_aad_record = moderation_quarantine_object_record_from_envelope(
        &bad_aad,
        moderation_quarantine_object_relative_path(bad_aad.quarantine_id, bad_aad.object_id),
    )
    .expect("rebuild AAD record");
    assert!(matches!(
        open_moderation_quarantine_object(&bad_aad, &bad_aad_record, &binding, &wrapper),
        Err(ModerationQuarantineObjectError::KeyWrapping { .. })
            | Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
    ));

    let (second_record, second_bytes) =
        seal_moderation_quarantine_object(input([0x62; 16]), &binding, &wrapper)
            .expect("seal second object");
    let mut second: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&second_bytes).expect("decode second envelope");
    second.wrapped_dek = envelope.wrapped_dek;
    assert!(matches!(
        open_moderation_quarantine_object(&second, &second_record, &binding, &wrapper),
        Err(ModerationQuarantineObjectError::KeyWrapping { .. })
            | Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
    ));
}

#[test]
fn moderation_quarantine_object_rewrap_keeps_ciphertext_and_identity_stable() {
    let original_wrapper = test_key_wrapper(0x74, "pkcs11:test/key-v1");
    let replacement_wrapper = test_key_wrapper(0x75, "pkcs11:test/key-v2");
    let binding = test_key_provider_binding();
    let payload = vec![0xC3; 70_000];
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x63; 16],
            payload: payload.clone(),
            captured_at_unix: 1_800_000_503,
            content_type: None,
            notes: None,
        },
        &binding,
        &original_wrapper,
    )
    .expect("seal object");
    let envelope: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode envelope");
    let (replacement_record, replacement_bytes) = rewrap_moderation_quarantine_object(
        &envelope,
        &record,
        &binding,
        &original_wrapper,
        &binding,
        &replacement_wrapper,
    )
    .expect("rewrap object DEK");
    let replacement: ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&replacement_bytes).expect("decode replacement");

    assert_eq!(replacement.object_id, envelope.object_id);
    assert_eq!(replacement.ciphertext_digest, envelope.ciphertext_digest);
    assert_eq!(replacement.chunks, envelope.chunks);
    assert_ne!(replacement.wrapped_dek, envelope.wrapped_dek);
    assert_eq!(replacement_record.object_id, record.object_id);
    assert_eq!(
        open_moderation_quarantine_object(
            &replacement,
            &replacement_record,
            &binding,
            &replacement_wrapper,
        )
        .expect("open rewrapped object"),
        payload
    );
    assert!(matches!(
        open_moderation_quarantine_object(
            &replacement,
            &replacement_record,
            &binding,
            &original_wrapper,
        ),
        Err(ModerationQuarantineObjectError::KeyWrapping { .. })
    ));

    let mut corrupt = envelope.clone();
    let last = corrupt.chunks[1].ciphertext.len() - 1;
    corrupt.chunks[1].ciphertext[last] ^= 0x20;
    corrupt.ciphertext_digest = moderation_quarantine_ciphertext_digest(&corrupt.chunks);
    let corrupt_record =
        moderation_quarantine_object_record_from_envelope(&corrupt, record.envelope_path.clone())
            .expect("rebuild record around corrupt ciphertext");
    assert!(matches!(
        rewrap_moderation_quarantine_object(
            &corrupt,
            &corrupt_record,
            &binding,
            &original_wrapper,
            &binding,
            &replacement_wrapper,
        ),
        Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
    ));
}

#[test]
fn authoritative_moderation_collections_refuse_over_limit_without_replacement() {
    let repro = ModerationReproRegistryRecord {
        manifest_id: [1; 16],
        manifest_digest: [2; 32],
        runner_hash: [3; 32],
        runtime_version: "runner-1".to_owned(),
        issued_at_unix: 1,
        model_count: 1,
        signer_count: 1,
    };
    let mut registry = ModerationModelRegistry::with_entry_limit(1);
    registry
        .restore_snapshot(ModerationModelRegistrySnapshot {
            reproducibility_manifests: vec![repro.clone()],
            adversarial_corpora: Vec::new(),
        })
        .expect("restore registry at boundary");
    let registry_before = registry.snapshot();
    let mut second_repro = repro.clone();
    second_repro.manifest_id = [4; 16];
    assert!(matches!(
        registry
            .restore_snapshot(ModerationModelRegistrySnapshot {
                reproducibility_manifests: vec![repro, second_repro],
                adversarial_corpora: Vec::new(),
            })
            .expect_err("over-limit registry snapshot must fail"),
        ModerationModelRegistryError::ResourceExhausted { .. }
    ));
    assert_eq!(registry.snapshot(), registry_before);

    let first_object = quarantine_object_record(1);
    let second_object = quarantine_object_record(2);
    let mut objects = ModerationQuarantineObjectRuntime::with_entry_limit(1);
    objects
        .insert(first_object.clone())
        .expect("insert object at boundary");
    assert_eq!(
        objects
            .insert(first_object.clone())
            .expect("replay object at capacity"),
        first_object
    );
    assert!(matches!(
        objects
            .insert(second_object.clone())
            .expect_err("new object above capacity must fail"),
        ModerationQuarantineObjectError::ResourceExhausted { .. }
    ));
    let objects_before = objects.snapshot();
    assert!(matches!(
        objects
            .restore_snapshot(ModerationQuarantineObjectSnapshot {
                objects: vec![first_object.clone(), second_object],
            })
            .expect_err("over-limit object snapshot must fail"),
        ModerationQuarantineObjectError::ResourceExhausted { .. }
    ));
    assert_eq!(objects.snapshot(), objects_before);

    let mut viewer = ModerationEvidenceViewerRuntime::with_entry_limit(1);
    let first_input = evidence_session_input(first_object.quarantine_id, 1);
    let session = viewer
        .create_session(first_input.clone(), &first_object)
        .expect("create session at boundary");
    assert_eq!(
        viewer
            .create_session(first_input, &first_object)
            .expect("replay session at capacity"),
        session
    );
    assert!(matches!(
        viewer
            .create_session(
                evidence_session_input(first_object.quarantine_id, 2),
                &first_object
            )
            .expect_err("new session above capacity must fail"),
        ModerationEvidenceViewerError::ResourceExhausted { .. }
    ));
    viewer
        .record_access(evidence_access_input(session.session_id))
        .expect("record access at boundary");
    assert!(matches!(
        viewer
            .record_access(evidence_access_input(session.session_id))
            .expect_err("new access above capacity must fail"),
        ModerationEvidenceViewerError::ResourceExhausted { .. }
    ));
    let viewer_before = viewer.snapshot();
    let mut extra_session = viewer_before.sessions[0].clone();
    extra_session.session_id = [9; 16];
    let mut over_limit_viewer = viewer_before.clone();
    over_limit_viewer.sessions.push(extra_session);
    assert!(matches!(
        viewer
            .restore_snapshot(over_limit_viewer)
            .expect_err("over-limit viewer snapshot must fail"),
        ModerationEvidenceViewerError::ResourceExhausted { .. }
    ));
    assert_eq!(viewer.snapshot(), viewer_before);

    let mut screening = ModerationScreeningRuntime::with_entry_limit(1);
    let input = screening_input("first", ModerationScreeningVerdict::Quarantine);
    let accepted = screening
        .record_screening(input.clone())
        .expect("record screening at boundary");
    assert_eq!(
        screening
            .record_screening(input)
            .expect("replay screening at capacity")
            .record,
        accepted.record
    );
    assert!(matches!(
        screening
            .record_screening(screening_input("second", ModerationScreeningVerdict::Pass))
            .expect_err("new screening above capacity must fail"),
        ModerationScreeningError::ResourceExhausted { .. }
    ));
    let screening_before = screening.snapshot();
    let mut extra_screening = screening_before.screening_records[0].clone();
    extra_screening.record_id = [8; 16];
    let mut over_limit_screening = screening_before.clone();
    over_limit_screening.screening_records.push(extra_screening);
    assert!(matches!(
        screening
            .restore_snapshot(over_limit_screening)
            .expect_err("over-limit screening snapshot must fail"),
        ModerationScreeningError::ResourceExhausted { .. }
    ));
    assert_eq!(screening.snapshot(), screening_before);
}

#[test]
fn moderation_read_views_bound_clones_before_response_materialization() {
    let retained = MODERATION_READ_VIEW_MAX_RECORDS_V1 + 1;
    let mut registry = ModerationModelRegistry::with_entry_limit(retained);
    for index in 0..retained {
        let mut manifest_id = [0_u8; 16];
        manifest_id[..8].copy_from_slice(
            &u64::try_from(index)
                .expect("fixture index fits u64")
                .to_be_bytes(),
        );
        registry.repro_manifests.insert(
            manifest_id,
            ModerationReproRegistryRecord {
                manifest_id,
                manifest_digest: [2; 32],
                runner_hash: [3; 32],
                runtime_version: "runner-1".to_owned(),
                issued_at_unix: 1,
                model_count: 1,
                signer_count: 1,
            },
        );
    }
    for index in 0..2_u8 {
        let corpus_digest = [index; 32];
        registry.corpora.insert(
            corpus_digest,
            ModerationCorpusRegistryRecord {
                corpus_digest,
                issued_at_unix: 1,
                cohort_label: None,
                family_count: 1,
                variant_count: 1,
            },
        );
    }

    let registry_view = registry.read_view(usize::MAX);
    assert_eq!(registry_view.reproducibility_manifest_count, retained);
    assert_eq!(registry_view.adversarial_corpus_count, 2);
    assert_eq!(
        registry_view.reproducibility_manifests.len(),
        MODERATION_READ_VIEW_MAX_RECORDS_V1
    );
    assert_eq!(registry_view.adversarial_corpora.len(), 2);

    let mut screening = ModerationScreeningRuntime::with_entry_limit(retained);
    let mut first_quarantine_id = None;
    for index in 0..retained {
        let outcome = screening
            .record_screening(screening_input(
                &format!("bounded-read-{index}"),
                ModerationScreeningVerdict::Quarantine,
            ))
            .expect("record screening read-view fixture");
        first_quarantine_id.get_or_insert(
            outcome
                .quarantine
                .expect("quarantine verdict creates queue record")
                .quarantine_id,
        );
    }

    let screening_view = screening.read_view(usize::MAX);
    assert_eq!(screening_view.screening_count, retained);
    assert_eq!(screening_view.quarantine_count, retained);
    assert_eq!(
        screening_view.screening_records.len(),
        MODERATION_READ_VIEW_MAX_RECORDS_V1
    );
    assert_eq!(
        screening_view.quarantine_records.len(),
        MODERATION_READ_VIEW_MAX_RECORDS_V1
    );
    let quarantine_view = screening.quarantine_read_view(1);
    assert_eq!(quarantine_view.quarantine_count, retained);
    assert_eq!(quarantine_view.quarantine_records.len(), 1);
    assert!(
        screening
            .quarantine_record(&first_quarantine_id.expect("first quarantine id"))
            .is_some()
    );
}

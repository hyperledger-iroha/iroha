//! Worker DATA and genuine test-scalar issuer signatures exercise durable publication ordering.
//! No mock platform original establishes device attestation or authenticated worker custody.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_core_zk::kagemusha_wallet_enrollment_v1::{
    IssuerEvidenceV1, PlatformEvidenceV1, PreKeyDispatchV1, RequestV1, ResultV1,
    issuer_worker::{OutcomeV1, VerifierRequestV1},
};
use iroha_data_model::kagemusha::*;
use norito::json::Value;
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

const CONFIGURATION: [u8; 32] = [31; 32];
const EXCHANGE: [u8; 32] = [32; 32];

struct Fixture {
    temp: tempfile::TempDir,
    _parent: PrivateDirectory,
    journal: EnrollmentJournalV1,
    dispatch: PreKeyDispatchV1,
    attempt: EnrollmentAttemptV1,
    signer: SigningKey,
    request: RequestV1,
}
impl Fixture {
    fn new() -> Self {
        let (temp, parent, mut journal) = tests::initialized();
        let (dispatch, mut attempt, signer) = permit_tests::fixture(&mut journal);
        let permit =
            permit_tests::signed(&dispatch, permit_tests::body(&dispatch, &attempt), &signer);
        journal.retain_permit(&attempt, &dispatch, permit).unwrap();
        let request = permit_tests::account_request(&dispatch, &attempt);
        journal
            .select_verification(&mut attempt, request.clone(), CONFIGURATION, 2_000)
            .unwrap();
        Self {
            temp,
            _parent: parent,
            journal,
            dispatch,
            attempt,
            signer,
            request,
        }
    }
    fn worker(&self) -> VerifierRequestV1 {
        self.journal
            .retained_worker_request(&self.attempt, CONFIGURATION)
            .unwrap()
    }
    fn projection(&self) -> Value {
        let body = &self.request.body;
        let PlatformEvidenceV1::Apple {
            key_id,
            attestation,
            key_binding_assertion,
        } = PlatformEvidenceV1::decode(&body.evidence, &body.policy).unwrap()
        else {
            panic!("Apple fixture")
        };
        let kind = KagemushaWalletEvidenceKindV1::AppleAppAttest;
        let originals = IssuerEvidenceV1::Apple {
            attestation: attestation.clone(),
            key_binding_assertion: key_binding_assertion.clone(),
        };
        norito::json!({
            "config_sha256": (hex::encode(CONFIGURATION)),
            "challenge_digest": (hex::encode(body.challenge.challenge_digest())),
            "key_binding": (hex::encode(kagemusha_wallet_enrollment_key_binding_v1(&body.challenge.challenge_digest(), &body.marker.payment_key))),
            "payment_key_base64": (STANDARD.encode(body.marker.payment_key.as_sec1_bytes())),
            "kind_tag": (kind.tag()), "time_ms": (2_000_u64),
            "facts": (kind.required_enrollment_facts() | KAGEMUSHA_WALLET_FACT_APP_ATTEST_PRODUCTION_V1),
            "os_patch_level": (0_u32), "vendor_patch_level": (0_u32), "boot_patch_level": (0_u32),
            "evidence_digest": (hex::encode(originals.digest(body, kind).unwrap())),
            "original_items_base64": (vec![STANDARD.encode(&attestation), STANDARD.encode(&key_binding_assertion)]),
            "app_attest_key_id": (hex::encode(key_id)), "app_attest_counter": (1_u32),
        })
    }
    fn frame(&self, outcome: &str, projection: Option<Value>) -> Vec<u8> {
        self.frame_for(EXCHANGE, outcome, projection)
    }
    fn frame_for(&self, exchange: [u8; 32], outcome: &str, projection: Option<Value>) -> Vec<u8> {
        let evidence = projection
            .map(|v| Value::from(STANDARD.encode(norito::json::to_vec(&v).unwrap())))
            .unwrap_or(Value::Null);
        let json = norito::json!({
            "schema": ("iroha.kagemusha.wallet-e1-verifier.v1"), "version": (1_u16),
            "exchange_id": (hex::encode(exchange)),
            "original_sha256": (hex::encode(Sha256::digest(self.worker().original()))),
            "outcome": (outcome), "evidence_base64": (evidence),
        });
        let bytes = norito::json::to_vec(&json).unwrap();
        let mut framed = (bytes.len() as u32).to_le_bytes().to_vec();
        framed.extend(bytes);
        framed
    }
    fn evidence(&mut self) {
        let frame = self.frame("evidence", Some(self.projection()));
        assert!(matches!(
            self.journal
                .retain_worker_response(&mut self.attempt, CONFIGURATION, EXCHANGE, &frame),
            Ok(OutcomeV1::Evidence(_))
        ));
    }
    fn signed_result(&self, body: KagemushaWalletCredentialBodyV1) -> Vec<u8> {
        let signature: Signature = self.signer.sign(&body.signing_message());
        let credential = KagemushaWalletCredentialV1::sign(
            body,
            &self.dispatch.enrollment_certificate,
            KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().as_slice().try_into().unwrap()),
        )
        .unwrap();
        let evidence = self
            .journal
            .retained_worker_evidence(&self.attempt, CONFIGURATION)
            .unwrap();
        ResultV1 {
            version: 1,
            credential: credential.to_canonical_bytes().unwrap(),
            certificates: norito::encode_canonical(&KagemushaWalletCertificateSetV1 {
                certificates: vec![self.dispatch.enrollment_certificate],
            })
            .unwrap(),
            evidence: evidence.originals,
        }
        .encode()
        .unwrap()
    }
}

#[test]
fn unknown_unavailable_and_foreign_worker_responses_never_restore_verify() {
    let mut f = Fixture::new();
    for outcome in ["outcome_unknown", "unavailable"] {
        let frame = f.frame(outcome, None);
        assert!(
            f.journal
                .retain_worker_response(&mut f.attempt, CONFIGURATION, EXCHANGE, &frame)
                .is_ok()
        );
        assert_eq!(f.attempt.phase(), EnrollmentJournalPhaseV1::Verifying);
        assert!(f.attempt.worker_result().is_none());
        assert!(matches!(
            f.journal
                .select_verification(&mut f.attempt, f.request.clone(), CONFIGURATION, 3_000),
            Err(Conflict)
        ));
    }
    let frame = f.frame("evidence", Some(f.projection()));
    assert!(matches!(
        f.journal
            .retain_worker_response(&mut f.attempt, CONFIGURATION, [99; 32], &frame),
        Err(Invalid)
    ));
    assert!(matches!(
        f.journal
            .retain_worker_response(&mut f.attempt, [99; 32], EXCHANGE, &frame),
        Err(Conflict)
    ));
    let mut foreign = f.projection();
    foreign
        .as_object_mut()
        .unwrap()
        .insert("config_sha256".into(), Value::from(hex::encode([99; 32])));
    let frame = f.frame("evidence", Some(foreign));
    assert!(matches!(
        f.journal
            .retain_worker_response(&mut f.attempt, CONFIGURATION, EXCHANGE, &frame),
        Err(Invalid)
    ));
    assert_eq!(f.attempt.phase(), EnrollmentJournalPhaseV1::Verifying);
    assert!(matches!(
        f.journal
            .retained_worker_evidence(&f.attempt, CONFIGURATION),
        Err(Conflict)
    ));
}

#[test]
fn rejection_is_durable_and_cannot_select_a_credential() {
    let mut f = Fixture::new();
    let frame = f.frame("rejected", None);
    assert!(matches!(
        f.journal
            .retain_worker_response(&mut f.attempt, CONFIGURATION, EXCHANGE, &frame),
        Ok(OutcomeV1::Rejected)
    ));
    assert_eq!(f.attempt.worker_result(), Some(frame.as_slice()));
    let recovery_exchange = [33; 32];
    let recovery = f.frame_for(recovery_exchange, "rejected", None);
    assert_ne!(recovery, frame);
    assert!(matches!(
        f.journal.retain_worker_response(
            &mut f.attempt,
            CONFIGURATION,
            recovery_exchange,
            &recovery,
        ),
        Ok(OutcomeV1::Rejected)
    ));
    assert_eq!(f.attempt.worker_result(), Some(frame.as_slice()));
    assert!(matches!(
        f.journal
            .select_credential_body(&mut f.attempt, &f.dispatch, CONFIGURATION, 3_000),
        Err(Conflict)
    ));
    let evidence = f.frame("evidence", Some(f.projection()));
    assert!(matches!(
        f.journal
            .retain_worker_response(&mut f.attempt, CONFIGURATION, EXCHANGE, &evidence),
        Err(Conflict)
    ));
    drop(f.journal);
    let journal =
        EnrollmentJournalV1::open(&f.temp.path().join("issuer"), b"approved scope DATA").unwrap();
    assert_eq!(
        journal
            .read(&f.attempt.selection().key)
            .unwrap()
            .unwrap()
            .phase(),
        EnrollmentJournalPhaseV1::Rejected
    );
}

#[test]
fn signing_selection_and_exact_e6_survive_restart_without_refreshing_time() {
    let mut f = Fixture::new();
    assert!(matches!(
        f.journal
            .retain_credential(&mut f.attempt, &f.dispatch, CONFIGURATION, vec![1]),
        Err(Conflict)
    ));
    f.evidence();
    let evidence = f
        .journal
        .retained_worker_evidence(&f.attempt, CONFIGURATION)
        .unwrap();
    assert_eq!(evidence.evidence.time_ms, 2_000);
    assert!(matches!(
        f.journal
            .select_credential_body(&mut f.attempt, &f.dispatch, CONFIGURATION, 1_999),
        Err(Invalid)
    ));
    let body = f
        .journal
        .select_credential_body(&mut f.attempt, &f.dispatch, CONFIGURATION, 3_000)
        .unwrap();
    assert_eq!(f.attempt.phase(), EnrollmentJournalPhaseV1::Signing);
    drop(f.journal);
    f.journal =
        EnrollmentJournalV1::open(&f.temp.path().join("issuer"), b"approved scope DATA").unwrap();
    f.attempt = f.journal.read(&f.attempt.selection().key).unwrap().unwrap();
    assert_eq!(
        f.journal
            .select_credential_body(&mut f.attempt, &f.dispatch, CONFIGURATION, 999_999)
            .unwrap(),
        body
    );
    let mut changed = body;
    changed.issued_at_ms += 1;
    let foreign = f.signed_result(changed);
    assert!(matches!(
        f.journal
            .retain_credential(&mut f.attempt, &f.dispatch, CONFIGURATION, foreign),
        Err(Conflict)
    ));
    let original = f.signed_result(body);
    f.journal
        .retain_credential(&mut f.attempt, &f.dispatch, CONFIGURATION, original.clone())
        .unwrap();
    assert_eq!(f.attempt.phase(), EnrollmentJournalPhaseV1::Issued);
    assert_eq!(f.attempt.issued(), Some(original.as_slice()));
    drop(f.journal);
    f.journal =
        EnrollmentJournalV1::open(&f.temp.path().join("issuer"), b"approved scope DATA").unwrap();
    f.attempt = f.journal.read(&f.attempt.selection().key).unwrap().unwrap();
    f.journal
        .retain_credential(&mut f.attempt, &f.dispatch, CONFIGURATION, original.clone())
        .unwrap();
    assert_eq!(f.attempt.issued(), Some(original.as_slice()));
}

#[test]
fn altered_retained_worker_originals_cannot_reach_signing() {
    let mut f = Fixture::new();
    f.evidence();
    let frame = f.frame("evidence", Some(f.projection()));
    f.journal
        .retain_worker_response(&mut f.attempt, CONFIGURATION, EXCHANGE, &frame)
        .unwrap();
    let mut changed = f.attempt.record.clone();
    let mut value: Value = norito::json::from_slice(&changed.worker_result).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("time_ms".into(), Value::from(2_001_u64));
    changed.worker_result = norito::json::to_vec(&value).unwrap();
    f.journal
        .directory
        .write_atomic(
            filename(&f.attempt.selection().key).unwrap(),
            &encode(&changed).unwrap(),
            PublishMode::Replace,
        )
        .unwrap();
    f.attempt = f.journal.read(&f.attempt.selection().key).unwrap().unwrap();
    assert!(matches!(
        f.journal
            .retained_worker_evidence(&f.attempt, CONFIGURATION),
        Err(Invalid)
    ));
    assert!(matches!(
        f.journal
            .select_credential_body(&mut f.attempt, &f.dispatch, CONFIGURATION, 3_000),
        Err(Invalid)
    ));
    assert_eq!(f.attempt.phase(), EnrollmentJournalPhaseV1::Evidence);
}

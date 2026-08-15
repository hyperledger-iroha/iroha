//! Deterministic provider-admission fixture and lifecycle contract tests.
use ed25519_dalek::{Signer, SigningKey};
use norito::json::Value;
use sorafs_manifest::{
    AdmissionRecord, CouncilSignature, ProviderAdmissionCouncilPolicy,
    ProviderAdmissionEnvelopeError, ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1,
    ProviderAdmissionRenewalError, ProviderAdmissionRenewalV1, ProviderAdmissionRevocationError,
    ProviderAdmissionRevocationV1, ProviderAdmissionSignatureError, ProviderAdvertV1,
    compute_advert_body_digest, compute_envelope_authorization_digest, compute_envelope_digest,
    compute_proposal_digest, validate_provider_admission_renewal_bytes,
    validate_provider_admission_revocation_bytes, verify_advert_against_record,
    verify_revocation_signatures,
};
use std::{fs, path::PathBuf};
const COUNCIL_KEY_BYTES: [u8; 32] = [0x45; 32];
fn committed_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/sorafs_manifest/provider_admission")
}
fn read_fixture(name: &str) -> Vec<u8> {
    fs::read(committed_fixture_dir().join(name))
        .unwrap_or_else(|error| panic!("read committed fixture {name}: {error}"))
}
fn fixture_policy() -> ProviderAdmissionCouncilPolicy {
    let key = SigningKey::from_bytes(&COUNCIL_KEY_BYTES);
    ProviderAdmissionCouncilPolicy::new([key.verifying_key().to_bytes()], 1)
        .expect("fixture council policy")
}
fn resign_envelope(envelope: &mut ProviderAdmissionEnvelopeV1, key: &SigningKey) {
    envelope.council_signatures.clear();
    let digest = compute_envelope_authorization_digest(envelope)
        .expect("compute envelope authorization digest");
    envelope.council_signatures.push(CouncilSignature {
        signer: key.verifying_key().to_bytes(),
        signature: key.sign(&digest).to_bytes().to_vec(),
    });
}
fn resign_revocation(revocation: &mut ProviderAdmissionRevocationV1, key: &SigningKey) {
    revocation.council_signatures.clear();
    let digest = revocation.digest().expect("compute revocation digest");
    revocation.council_signatures.push(CouncilSignature {
        signer: key.verifying_key().to_bytes(),
        signature: key.sign(&digest).to_bytes().to_vec(),
    });
}
macro_rules! decode_canonical_fixture {
    ($type:ty, $name:literal) => {{
        let bytes = read_fixture($name);
        let value: $type = norito::decode_from_bytes(&bytes)
            .unwrap_or_else(|error| panic!("decode {}: {error}", $name));
        let canonical =
            norito::to_bytes(&value).unwrap_or_else(|error| panic!("re-encode {}: {error}", $name));
        assert_eq!(canonical, bytes, "{} is not canonical Norito", $name);
        value
    }};
}
#[test]
fn committed_provider_admission_fixtures_are_canonical_and_linked() {
    let proposal = decode_canonical_fixture!(ProviderAdmissionProposalV1, "proposal_v1.to");
    let advert = decode_canonical_fixture!(ProviderAdvertV1, "advert_v1.to");
    let envelope = decode_canonical_fixture!(ProviderAdmissionEnvelopeV1, "envelope_v1.to");
    let renewed_proposal =
        decode_canonical_fixture!(ProviderAdmissionProposalV1, "proposal_renewed_v1.to");
    let renewed_advert = decode_canonical_fixture!(ProviderAdvertV1, "advert_renewed_v1.to");
    let renewed_envelope =
        decode_canonical_fixture!(ProviderAdmissionEnvelopeV1, "envelope_renewed_v1.to");
    let renewal = decode_canonical_fixture!(ProviderAdmissionRenewalV1, "renewal_v1.to");
    let revocation = decode_canonical_fixture!(ProviderAdmissionRevocationV1, "revocation_v1.to");
    assert_eq!(envelope.proposal, proposal);
    assert_eq!(envelope.advert_body, advert.body);
    assert_eq!(
        envelope.proposal_digest,
        compute_proposal_digest(&proposal).expect("proposal digest")
    );
    assert_eq!(
        envelope.advert_body_digest,
        compute_advert_body_digest(&advert.body).expect("advert body digest")
    );
    assert_eq!(renewed_envelope.proposal, renewed_proposal);
    assert_eq!(renewed_envelope.advert_body, renewed_advert.body);
    assert_eq!(renewal.envelope, renewed_envelope);
    let policy = fixture_policy();
    let base_record = AdmissionRecord::new(envelope.clone(), &policy).expect("base admission");
    assert!(base_record.is_council_verified());
    assert_eq!(renewal.provider_id, *base_record.provider_id());
    assert_eq!(
        renewal.previous_envelope_digest,
        *base_record.envelope_digest()
    );
    assert_eq!(
        renewal.envelope_digest,
        compute_envelope_digest(&renewed_envelope).expect("renewed envelope digest")
    );
    verify_advert_against_record(&advert, &base_record).expect("base advert linkage");
    let renewed_record = base_record
        .apply_renewal(&renewal, &policy)
        .expect("apply governed renewal");
    assert!(renewed_record.is_council_verified());
    assert_eq!(renewed_record.envelope(), &renewed_envelope);
    verify_advert_against_record(&renewed_advert, &renewed_record).expect("renewed advert linkage");
    assert_eq!(revocation.provider_id, *base_record.provider_id());
    assert_eq!(revocation.envelope_digest, *base_record.envelope_digest());
    let revocation_digest =
        verify_revocation_signatures(&revocation, &policy).expect("revocation council signature");
    assert_eq!(
        revocation_digest,
        revocation.digest().expect("revocation digest")
    );
    base_record
        .verify_revocation(&revocation, &policy)
        .expect("base revocation linkage");
    let metadata_text =
        fs::read_to_string(committed_fixture_dir().join("metadata.json")).expect("metadata.json");
    let metadata: Value = norito::json::from_str(&metadata_text).expect("parse metadata.json");
    assert_eq!(
        metadata.get("proposal_digest_hex").and_then(Value::as_str),
        Some(hex::encode(envelope.proposal_digest).as_str())
    );
    assert_eq!(
        metadata.get("envelope_digest_hex").and_then(Value::as_str),
        Some(hex::encode(base_record.envelope_digest()).as_str())
    );
    assert_eq!(
        metadata
            .get("renewal_envelope_digest_hex")
            .and_then(Value::as_str),
        Some(hex::encode(renewal.envelope_digest).as_str())
    );
    assert_eq!(
        metadata
            .get("revocation_digest_hex")
            .and_then(Value::as_str),
        Some(hex::encode(revocation_digest).as_str())
    );
    let mut truncated = read_fixture("envelope_v1.to");
    truncated.pop();
    assert!(
        norito::decode_from_bytes::<ProviderAdmissionEnvelopeV1>(&truncated).is_err(),
        "truncated canonical fixture must fail closed"
    );
    let mut trailing = read_fixture("envelope_v1.to");
    trailing.push(0);
    assert!(
        norito::decode_from_bytes::<ProviderAdmissionEnvelopeV1>(&trailing).is_err(),
        "canonical fixture with trailing bytes must fail closed"
    );
}
#[test]
fn provider_admission_fixture_lifecycle_rejects_adversarial_mutations() {
    let envelope = decode_canonical_fixture!(ProviderAdmissionEnvelopeV1, "envelope_v1.to");
    let renewal = decode_canonical_fixture!(ProviderAdmissionRenewalV1, "renewal_v1.to");
    let revocation = decode_canonical_fixture!(ProviderAdmissionRevocationV1, "revocation_v1.to");
    let council_key = SigningKey::from_bytes(&COUNCIL_KEY_BYTES);
    let policy = fixture_policy();
    let base_record = AdmissionRecord::new(envelope.clone(), &policy).expect("base admission");
    let renewed_record = base_record
        .apply_renewal(&renewal, &policy)
        .expect("valid renewal");
    let untrusted_record =
        AdmissionRecord::new_untrusted_signers(envelope.clone()).expect("integrity-only record");
    assert!(!untrusted_record.is_council_verified());
    assert!(matches!(
        untrusted_record.apply_renewal(&renewal, &policy),
        Err(ProviderAdmissionRenewalError::UntrustedBaseRecord)
    ));
    let second_council_key = SigningKey::from_bytes(&[0x47; 32]);
    let quorum_policy = ProviderAdmissionCouncilPolicy::new(
        [
            council_key.verifying_key().to_bytes(),
            second_council_key.verifying_key().to_bytes(),
        ],
        2,
    )
    .expect("two-member fixture policy");
    assert!(matches!(
        AdmissionRecord::new(envelope.clone(), &quorum_policy),
        Err(ProviderAdmissionEnvelopeError::Signature(
            ProviderAdmissionSignatureError::ThresholdNotMet {
                required: 2,
                verified: 1,
            }
        ))
    ));
    let mut duplicate_signer = envelope.clone();
    let repeated_signature = duplicate_signer.council_signatures[0].clone();
    duplicate_signer.council_signatures.push(repeated_signature);
    let duplicate_policy = ProviderAdmissionCouncilPolicy::new(
        [
            council_key.verifying_key().to_bytes(),
            second_council_key.verifying_key().to_bytes(),
        ],
        1,
    )
    .expect("two-member fixture policy");
    assert!(matches!(
        AdmissionRecord::new(duplicate_signer, &duplicate_policy),
        Err(ProviderAdmissionEnvelopeError::Signature(
            ProviderAdmissionSignatureError::DuplicateSigner { .. }
        ))
    ));
    let attacker_key = SigningKey::from_bytes(&[0x46; 32]);
    let attacker_policy =
        ProviderAdmissionCouncilPolicy::new([attacker_key.verifying_key().to_bytes()], 1)
            .expect("attacker policy");
    assert!(matches!(
        AdmissionRecord::new(envelope.clone(), &attacker_policy),
        Err(ProviderAdmissionEnvelopeError::Signature(
            ProviderAdmissionSignatureError::UntrustedSigner { .. }
        ))
    ));
    let mut bad_envelope_signature = envelope.clone();
    bad_envelope_signature.council_signatures[0].signature[0] ^= 0x80;
    assert!(matches!(
        AdmissionRecord::new(bad_envelope_signature, &policy),
        Err(ProviderAdmissionEnvelopeError::Signature(
            ProviderAdmissionSignatureError::Verification { .. }
        ))
    ));
    let mut wrong_previous = renewal.clone();
    wrong_previous.previous_envelope_digest[0] ^= 0x01;
    assert!(matches!(
        base_record.apply_renewal(&wrong_previous, &policy),
        Err(ProviderAdmissionRenewalError::PreviousDigestMismatch { .. })
    ));
    assert!(matches!(
        renewed_record.apply_renewal(&renewal, &policy),
        Err(ProviderAdmissionRenewalError::PreviousDigestMismatch { .. })
    ));
    let mut wrong_renewal_digest = renewal.clone();
    wrong_renewal_digest.envelope_digest[0] ^= 0x01;
    assert!(matches!(
        base_record.apply_renewal(&wrong_renewal_digest, &policy),
        Err(ProviderAdmissionRenewalError::EnvelopeDigestMismatch { .. })
    ));
    let mut unsupported_renewal = renewal.clone();
    unsupported_renewal.version = 0;
    assert!(matches!(
        base_record.apply_renewal(&unsupported_renewal, &policy),
        Err(ProviderAdmissionRenewalError::UnsupportedVersion { found: 0 })
    ));
    let mut wrong_provider = renewal.clone();
    wrong_provider.provider_id[0] ^= 0x01;
    assert!(matches!(
        base_record.apply_renewal(&wrong_provider, &policy),
        Err(ProviderAdmissionRenewalError::ProviderMismatch { .. })
    ));
    let mut bad_renewal_signature = renewal.clone();
    bad_renewal_signature.envelope.council_signatures[0].signature[0] ^= 0x01;
    bad_renewal_signature.envelope_digest =
        compute_envelope_digest(&bad_renewal_signature.envelope).expect("mutated envelope digest");
    assert!(matches!(
        base_record.apply_renewal(&bad_renewal_signature, &policy),
        Err(ProviderAdmissionRenewalError::Envelope(
            ProviderAdmissionEnvelopeError::Signature(
                ProviderAdmissionSignatureError::Verification { .. }
            )
        ))
    ));
    let mut retention_rollback = renewal.clone();
    retention_rollback.envelope.retention_epoch = envelope.retention_epoch - 1;
    resign_envelope(&mut retention_rollback.envelope, &council_key);
    retention_rollback.envelope_digest =
        compute_envelope_digest(&retention_rollback.envelope).expect("rollback envelope digest");
    assert!(matches!(
        base_record.apply_renewal(&retention_rollback, &policy),
        Err(ProviderAdmissionRenewalError::RetentionNotExtended { .. })
    ));
    let mut issued_at_regression = renewal.clone();
    issued_at_regression.envelope.issued_at = envelope.issued_at - 1;
    resign_envelope(&mut issued_at_regression.envelope, &council_key);
    issued_at_regression.envelope_digest =
        compute_envelope_digest(&issued_at_regression.envelope).expect("regressed envelope digest");
    assert!(matches!(
        base_record.apply_renewal(&issued_at_regression, &policy),
        Err(ProviderAdmissionRenewalError::IssuedAtRegression { .. })
    ));
    let mut bad_revocation_signature = revocation.clone();
    bad_revocation_signature.council_signatures[0].signature[0] ^= 0x01;
    assert!(matches!(
        base_record.verify_revocation(&bad_revocation_signature, &policy),
        Err(ProviderAdmissionRevocationError::Signature(
            ProviderAdmissionSignatureError::Verification { .. }
        ))
    ));
    let mut wrong_revocation_target = revocation.clone();
    wrong_revocation_target.envelope_digest[0] ^= 0x01;
    resign_revocation(&mut wrong_revocation_target, &council_key);
    assert!(matches!(
        base_record.verify_revocation(&wrong_revocation_target, &policy),
        Err(ProviderAdmissionRevocationError::EnvelopeDigestMismatch { .. })
    ));
    assert!(matches!(
        renewed_record.verify_revocation(&revocation, &policy),
        Err(ProviderAdmissionRevocationError::EnvelopeDigestMismatch { .. })
    ));
    let mut wrong_revocation_provider = revocation.clone();
    wrong_revocation_provider.provider_id[0] ^= 0x01;
    resign_revocation(&mut wrong_revocation_provider, &council_key);
    assert!(matches!(
        base_record.verify_revocation(&wrong_revocation_provider, &policy),
        Err(ProviderAdmissionRevocationError::ProviderMismatch { .. })
    ));
    let mut empty_reason = revocation.clone();
    empty_reason.reason.clear();
    resign_revocation(&mut empty_reason, &council_key);
    assert!(matches!(
        base_record.verify_revocation(&empty_reason, &policy),
        Err(ProviderAdmissionRevocationError::ReasonEmpty)
    ));
    let mut whitespace_reason = revocation.clone();
    whitespace_reason.reason = " \t\n".into();
    resign_revocation(&mut whitespace_reason, &council_key);
    assert!(matches!(
        base_record.verify_revocation(&whitespace_reason, &policy),
        Err(ProviderAdmissionRevocationError::ReasonEmpty)
    ));
    let mut unsigned_revocation = revocation.clone();
    unsigned_revocation.council_signatures.clear();
    assert!(matches!(
        base_record.verify_revocation(&unsigned_revocation, &policy),
        Err(ProviderAdmissionRevocationError::MissingSignatures)
    ));
    let mut unsupported_revocation = revocation.clone();
    unsupported_revocation.version = 0;
    resign_revocation(&mut unsupported_revocation, &council_key);
    assert!(matches!(
        base_record.verify_revocation(&unsupported_revocation, &policy),
        Err(ProviderAdmissionRevocationError::UnsupportedVersion { found: 0 })
    ));
    let envelope_bytes = norito::to_bytes(&envelope).expect("encode base envelope");
    let wrong_previous_bytes = norito::to_bytes(&wrong_previous).expect("encode wrong renewal");
    let renewal_outcome = validate_provider_admission_renewal_bytes(
        &envelope_bytes,
        &wrong_previous_bytes,
        "envelope_v1.to",
        "wrong_previous.to",
        1,
    );
    assert!(!renewal_outcome.is_ok());
    let wrong_target_bytes =
        norito::to_bytes(&wrong_revocation_target).expect("encode wrong revocation");
    let revocation_outcome = validate_provider_admission_revocation_bytes(
        &envelope_bytes,
        &wrong_target_bytes,
        "envelope_v1.to",
        "wrong_target.to",
        1,
    );
    assert!(!revocation_outcome.is_ok());
}

//! Deterministic provider-admission fixture and lifecycle contract tests.
use ed25519_dalek::{Signer, SigningKey};
use norito::json::Value;
use sorafs_manifest::{
    AdmissionRecord, AdvertSignature, AdvertValidationError, CouncilSignature,
    ProviderAdmissionAdvertError, ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeError,
    ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdmissionRenewalError,
    ProviderAdmissionRenewalV1, ProviderAdmissionRevocationError, ProviderAdmissionRevocationV1,
    ProviderAdmissionSignatureError, ProviderAdmissionValidationError, ProviderAdvertBodyV1,
    ProviderAdvertV1, compute_advert_body_digest, compute_envelope_authorization_digest,
    compute_envelope_digest, compute_proposal_digest, validate_provider_admission_renewal_bytes,
    validate_provider_admission_revocation_bytes, verify_advert_against_record, verify_envelope,
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
fn resign_advert(advert: &mut ProviderAdvertV1, key: &SigningKey) {
    let payload = advert
        .signature_payload_bytes()
        .expect("canonical network-bound advert signing payload");
    advert.signature.signature = key.sign(&payload).to_bytes().to_vec();
}

#[derive(norito::NoritoSchema, norito::NoritoSerialize)]
#[norito_schema(
    name = "tests::RetiredAdmissionEnvelopeWithoutNetwork",
    frame = "sorafs_manifest::provider_admission::ProviderAdmissionEnvelopeV1"
)]
struct RetiredAdmissionEnvelopeWithoutNetwork {
    version: u8,
    proposal: ProviderAdmissionProposalV1,
    proposal_digest: [u8; 32],
    advert_body: ProviderAdvertBodyV1,
    advert_body_digest: [u8; 32],
    issued_at: u64,
    retention_epoch: u64,
    council_signatures: Vec<CouncilSignature>,
    #[norito(default)]
    notes: Option<String>,
}

#[derive(norito::NoritoSchema, norito::NoritoSerialize)]
#[norito_schema(
    name = "tests::RetiredAdmissionEnvelopeWithoutLineage",
    frame = "sorafs_manifest::provider_admission::ProviderAdmissionEnvelopeV1"
)]
struct RetiredAdmissionEnvelopeWithoutLineage {
    version: u8,
    network_id: [u8; 32],
    proposal: ProviderAdmissionProposalV1,
    proposal_digest: [u8; 32],
    advert_body: ProviderAdvertBodyV1,
    advert_body_digest: [u8; 32],
    issued_at: u64,
    retention_epoch: u64,
    council_signatures: Vec<CouncilSignature>,
    #[norito(default)]
    notes: Option<String>,
}

#[derive(norito::NoritoSchema, norito::NoritoSerialize)]
#[norito_schema(
    name = "tests::RetiredProviderAdvertWithoutNetwork",
    frame = "sorafs_manifest::provider_advert::ProviderAdvertV1"
)]
struct RetiredProviderAdvertWithoutNetwork {
    version: u8,
    issued_at: u64,
    expires_at: u64,
    body: ProviderAdvertBodyV1,
    signature: AdvertSignature,
    signature_strict: bool,
    #[norito(default)]
    allow_unknown_capabilities: bool,
}

#[derive(norito::NoritoSchema, norito::NoritoSerialize)]
#[norito_schema(
    name = "tests::RetiredAdmissionRevocationWithoutNetwork",
    frame = "sorafs_manifest::provider_admission::ProviderAdmissionRevocationV1"
)]
struct RetiredAdmissionRevocationWithoutNetwork {
    version: u8,
    provider_id: [u8; 32],
    envelope_digest: [u8; 32],
    revoked_at: u64,
    reason: String,
    council_signatures: Vec<CouncilSignature>,
    #[norito(default)]
    notes: Option<String>,
}

#[derive(norito::NoritoSchema, norito::NoritoSerialize)]
#[norito_schema(
    name = "tests::RetiredAdmissionRevocationWithoutLineage",
    frame = "sorafs_manifest::provider_admission::ProviderAdmissionRevocationV1"
)]
struct RetiredAdmissionRevocationWithoutLineage {
    version: u8,
    network_id: [u8; 32],
    provider_id: [u8; 32],
    envelope_digest: [u8; 32],
    revoked_at: u64,
    reason: String,
    council_signatures: Vec<CouncilSignature>,
    #[norito(default)]
    notes: Option<String>,
}

#[test]
fn retired_pre_lineage_v1_frames_fail_canonical_decode() {
    let envelope: ProviderAdmissionEnvelopeV1 =
        norito::decode_from_bytes(&read_fixture("envelope_v1.to")).expect("current envelope");
    let revocation: ProviderAdmissionRevocationV1 =
        norito::decode_from_bytes(&read_fixture("revocation_v1.to")).expect("current revocation");
    let old_envelope = RetiredAdmissionEnvelopeWithoutLineage {
        version: envelope.version,
        network_id: envelope.network_id,
        proposal: envelope.proposal,
        proposal_digest: envelope.proposal_digest,
        advert_body: envelope.advert_body,
        advert_body_digest: envelope.advert_body_digest,
        issued_at: envelope.issued_at,
        retention_epoch: envelope.retention_epoch,
        council_signatures: envelope.council_signatures,
        notes: envelope.notes,
    };
    let old_revocation = RetiredAdmissionRevocationWithoutLineage {
        version: revocation.version,
        network_id: revocation.network_id,
        provider_id: revocation.provider_id,
        envelope_digest: revocation.envelope_digest,
        revoked_at: revocation.revoked_at,
        reason: revocation.reason,
        council_signatures: revocation.council_signatures,
        notes: revocation.notes,
    };
    let old_envelope_bytes = norito::encode_canonical(&old_envelope).expect("retired envelope");
    let old_revocation_bytes =
        norito::encode_canonical(&old_revocation).expect("retired revocation");
    assert!(norito::decode_canonical::<ProviderAdmissionEnvelopeV1>(&old_envelope_bytes).is_err());
    assert!(
        norito::decode_canonical::<ProviderAdmissionRevocationV1>(&old_revocation_bytes).is_err()
    );
}

#[test]
fn retired_networkless_v1_frames_fail_canonical_decode() {
    let envelope: ProviderAdmissionEnvelopeV1 =
        norito::decode_from_bytes(&read_fixture("envelope_v1.to")).expect("current envelope");
    let advert: ProviderAdvertV1 =
        norito::decode_from_bytes(&read_fixture("advert_v1.to")).expect("current advert");
    let revocation: ProviderAdmissionRevocationV1 =
        norito::decode_from_bytes(&read_fixture("revocation_v1.to")).expect("current revocation");
    let old_envelope = RetiredAdmissionEnvelopeWithoutNetwork {
        version: envelope.version,
        proposal: envelope.proposal,
        proposal_digest: envelope.proposal_digest,
        advert_body: envelope.advert_body,
        advert_body_digest: envelope.advert_body_digest,
        issued_at: envelope.issued_at,
        retention_epoch: envelope.retention_epoch,
        council_signatures: envelope.council_signatures,
        notes: envelope.notes,
    };
    let old_advert = RetiredProviderAdvertWithoutNetwork {
        version: advert.version,
        issued_at: advert.issued_at,
        expires_at: advert.expires_at,
        body: advert.body,
        signature: advert.signature,
        signature_strict: advert.signature_strict,
        allow_unknown_capabilities: advert.allow_unknown_capabilities,
    };
    let old_revocation = RetiredAdmissionRevocationWithoutNetwork {
        version: revocation.version,
        provider_id: revocation.provider_id,
        envelope_digest: revocation.envelope_digest,
        revoked_at: revocation.revoked_at,
        reason: revocation.reason,
        council_signatures: revocation.council_signatures,
        notes: revocation.notes,
    };
    let old_envelope_frame =
        norito::encode_canonical(&old_envelope).expect("retired envelope frame");
    let old_advert_frame = norito::encode_canonical(&old_advert).expect("retired advert frame");
    let old_revocation_frame =
        norito::encode_canonical(&old_revocation).expect("retired revocation frame");
    assert_eq!(
        norito::core::from_bytes_view(&old_envelope_frame)
            .expect("retired envelope archive")
            .schema(),
        norito::schema::identity::frame_hash::<ProviderAdmissionEnvelopeV1>()
    );
    assert_eq!(
        norito::core::from_bytes_view(&old_advert_frame)
            .expect("retired advert archive")
            .schema(),
        norito::schema::identity::frame_hash::<ProviderAdvertV1>()
    );
    assert_eq!(
        norito::core::from_bytes_view(&old_revocation_frame)
            .expect("retired revocation archive")
            .schema(),
        norito::schema::identity::frame_hash::<ProviderAdmissionRevocationV1>()
    );
    for (label, rejected) in [
        (
            "envelope",
            norito::decode_canonical::<ProviderAdmissionEnvelopeV1>(&old_envelope_frame).is_err(),
        ),
        (
            "advert",
            norito::decode_canonical::<ProviderAdvertV1>(&old_advert_frame).is_err(),
        ),
        (
            "revocation",
            norito::decode_canonical::<ProviderAdmissionRevocationV1>(&old_revocation_frame)
                .is_err(),
        ),
    ] {
        assert!(
            rejected,
            "retired networkless {label} V1 frame must fail closed"
        );
    }
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
fn network_identity_is_signed_and_linked_across_admission_lifecycle() {
    let envelope = decode_canonical_fixture!(ProviderAdmissionEnvelopeV1, "envelope_v1.to");
    let advert = decode_canonical_fixture!(ProviderAdvertV1, "advert_v1.to");
    let renewal = decode_canonical_fixture!(ProviderAdmissionRenewalV1, "renewal_v1.to");
    let revocation = decode_canonical_fixture!(ProviderAdmissionRevocationV1, "revocation_v1.to");
    let council_key = SigningKey::from_bytes(&COUNCIL_KEY_BYTES);
    let provider_key = SigningKey::from_bytes(&[0x21; 32]);
    let policy = fixture_policy();
    let record = AdmissionRecord::new(envelope.clone(), &policy).expect("fixture admission");
    assert_eq!(envelope.network_id, [0xA1; 32]);
    assert_eq!(advert.network_id, envelope.network_id);
    assert_eq!(renewal.envelope.network_id, envelope.network_id);
    assert_eq!(revocation.network_id, envelope.network_id);

    let mut altered_envelope = envelope.clone();
    altered_envelope.network_id[0] ^= 1;
    assert!(matches!(
        AdmissionRecord::new(altered_envelope.clone(), &policy),
        Err(ProviderAdmissionEnvelopeError::Signature(
            ProviderAdmissionSignatureError::Verification { .. }
        ))
    ));
    resign_envelope(&mut altered_envelope, &council_key);
    verify_envelope(&altered_envelope, &policy).expect("independently signed foreign envelope");

    let mut altered_advert = advert.clone();
    altered_advert.network_id[0] ^= 1;
    assert!(altered_advert.verify_signature().is_err());
    resign_advert(&mut altered_advert, &provider_key);
    altered_advert
        .verify_signature()
        .expect("valid foreign-network provider signature");
    assert!(matches!(
        sorafs_manifest::verify_advert_against_record(&altered_advert, &record),
        Err(ProviderAdmissionAdvertError::NetworkMismatch { .. })
    ));

    let mut altered_renewal = renewal.clone();
    altered_renewal.envelope.network_id[0] ^= 1;
    resign_envelope(&mut altered_renewal.envelope, &council_key);
    altered_renewal.envelope_digest =
        compute_envelope_digest(&altered_renewal.envelope).expect("foreign renewal digest");
    verify_envelope(&altered_renewal.envelope, &policy)
        .expect("independently signed foreign renewal envelope");
    assert!(matches!(
        record.apply_renewal(&altered_renewal, &policy),
        Err(ProviderAdmissionRenewalError::NetworkMismatch { .. })
    ));

    let mut altered_revocation = revocation.clone();
    altered_revocation.network_id[0] ^= 1;
    assert!(matches!(
        verify_revocation_signatures(&altered_revocation, &policy),
        Err(ProviderAdmissionRevocationError::Signature(
            ProviderAdmissionSignatureError::Verification { .. }
        ))
    ));
    resign_revocation(&mut altered_revocation, &council_key);
    verify_revocation_signatures(&altered_revocation, &policy)
        .expect("independently signed foreign revocation");
    assert!(matches!(
        record.verify_revocation(&altered_revocation, &policy),
        Err(ProviderAdmissionRevocationError::NetworkMismatch { .. })
    ));

    let mut zero_envelope = envelope;
    zero_envelope.network_id = [0; 32];
    resign_envelope(&mut zero_envelope, &council_key);
    assert!(matches!(
        AdmissionRecord::new(zero_envelope, &policy),
        Err(ProviderAdmissionEnvelopeError::Validation(
            ProviderAdmissionValidationError::InvalidNetworkId
        ))
    ));
    let mut zero_advert = advert.clone();
    zero_advert.network_id = [0; 32];
    resign_advert(&mut zero_advert, &provider_key);
    assert!(matches!(
        zero_advert.validate(zero_advert.issued_at),
        Err(AdvertValidationError::InvalidNetworkId)
    ));
    let mut zero_revocation = revocation;
    zero_revocation.network_id = [0; 32];
    resign_revocation(&mut zero_revocation, &council_key);
    assert!(matches!(
        verify_revocation_signatures(&zero_revocation, &policy),
        Err(ProviderAdmissionRevocationError::InvalidNetworkId)
    ));
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

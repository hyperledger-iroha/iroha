//! Council-policy canonical shape, lineage, and signed-claim regressions.

use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use sorafs_manifest::{
    CouncilSignature, provider_admission::compute_envelope_authorization_digest,
};

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn raw_public_key(key: &KeyPair) -> [u8; 32] {
    key.public_key()
        .try_to_bytes()
        .expect("Ed25519 public key")
        .1
        .try_into()
        .expect("fixed Ed25519 key")
}

fn policy() -> ProviderAdmissionCouncilPolicyV1 {
    ProviderAdmissionCouncilPolicyV1 {
        network_id: [0xA1; 32],
        policy_id: [0xC1; 32],
        version: PROVIDER_ADMISSION_COUNCIL_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        trusted_signers: vec![raw_public_key(&key(31))],
        signature_threshold: 1,
        paused: false,
    }
}

fn signed_fixture_envelope(
    policy: &ProviderAdmissionCouncilPolicyV1,
    signer: &KeyPair,
) -> ProviderAdmissionEnvelopeV1 {
    let mut envelope: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(include_bytes!(
        "../../../../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
    ))
    .expect("canonical fixture admission envelope");
    envelope.network_id = policy.network_id;
    envelope.policy_id = policy.policy_id;
    envelope.policy_revision = policy.revision;
    envelope.policy_digest = policy.canonical_digest().expect("canonical policy digest");
    envelope.council_signatures.clear();
    let authorization =
        compute_envelope_authorization_digest(&envelope).expect("canonical authorization");
    envelope.council_signatures.push(CouncilSignature {
        signer: raw_public_key(signer),
        signature: Signature::new(signer.private_key(), &authorization)
            .payload()
            .to_vec(),
    });
    envelope
}

#[test]
fn council_policy_validates_bounded_strong_canonical_keys_and_norito() {
    let policy = policy();
    policy.validate().expect("valid policy");
    let encoded = norito::to_bytes(&policy).expect("encode policy");
    let decoded: ProviderAdmissionCouncilPolicyV1 =
        norito::decode_from_bytes(&encoded).expect("decode policy");
    assert_eq!(decoded, policy);
    assert_eq!(decoded.canonical_digest(), policy.canonical_digest());
    let json = norito::json::to_json(&policy).expect("encode policy JSON");
    let decoded_json: ProviderAdmissionCouncilPolicyV1 =
        norito::json::from_str(&json).expect("decode policy JSON");
    assert_eq!(decoded_json, policy);

    let mut invalid = policy.clone();
    invalid.trusted_signers.push(invalid.trusted_signers[0]);
    assert_eq!(
        invalid.validate(),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::SignerOrder)
    );
    invalid.trusted_signers = vec![[0; 32]];
    assert_eq!(
        invalid.validate(),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::InvalidSigner)
    );
    invalid.trusted_signers = vec![raw_public_key(&key(31)); 33];
    assert_eq!(
        invalid.validate(),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::SignerCount)
    );
    invalid = policy.clone();
    invalid.signature_threshold = 2;
    assert_eq!(
        invalid.validate(),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::Threshold)
    );
}

#[test]
fn council_policy_successor_requires_exact_immediate_predecessor() {
    let previous = policy();
    let mut next = previous.clone();
    next.revision = 2;
    next.predecessor_policy_digest = Some(previous.canonical_digest().expect("previous digest"));
    next.trusted_signers = vec![raw_public_key(&key(32))];
    next.validate_successor(&previous)
        .expect("overlapping or rotating keys use one exact lineage");

    let mut changed = next.clone();
    changed.predecessor_policy_digest = Some([0xAB; 32]);
    assert_eq!(
        changed.validate_successor(&previous),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::PredecessorMismatch)
    );
    changed = next.clone();
    changed.revision = 3;
    assert_eq!(
        changed.validate_successor(&previous),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::PredecessorMismatch)
    );
    changed = next;
    changed.network_id[0] ^= 1;
    assert_eq!(
        changed.validate_successor(&previous),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::IdentityChanged)
    );
}

#[test]
fn council_policy_claim_check_rejects_substitution_pause_and_expiry() {
    let signer = key(31);
    let policy = policy();
    let envelope = signed_fixture_envelope(&policy, &signer);
    policy
        .verify_envelope_policy_claim(&envelope, envelope.issued_at)
        .expect("matching signed policy claim");

    for coordinate in 0..4 {
        let mut changed = envelope.clone();
        match coordinate {
            0 => changed.network_id[0] ^= 1,
            1 => changed.policy_id[0] ^= 1,
            2 => changed.policy_revision += 1,
            3 => changed.policy_digest[0] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            policy.verify_envelope_policy_claim(&changed, envelope.issued_at),
            Err(ProviderAdmissionCouncilPolicyValidationErrorV1::ClaimMismatch),
            "substituted council claim coordinate {coordinate}",
        );
    }
    let mut changed = envelope.clone();
    changed.council_signatures[0].signature[0] ^= 1;
    assert_eq!(
        policy.verify_envelope_policy_claim(&changed, envelope.issued_at),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::InvalidEnvelope)
    );
    assert_eq!(
        policy.verify_envelope_policy_claim(&envelope, envelope.retention_epoch),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::Expired)
    );
    let mut paused = policy;
    paused.paused = true;
    assert_eq!(
        paused.verify_envelope_policy_claim(&envelope, envelope.issued_at),
        Err(ProviderAdmissionCouncilPolicyValidationErrorV1::Paused)
    );
}

#[test]
fn council_policy_bounded_decoder_preserves_every_advertised_wire_layout() {
    let policy = policy();
    let projection = ProviderAdmissionCouncilPolicyDecodedV1 {
        network_id: policy.network_id,
        policy_id: policy.policy_id,
        version: policy.version,
        revision: policy.revision,
        predecessor_policy_digest: policy.predecessor_policy_digest,
        trusted_signers: bounded_council_signers::Vec(policy.trusted_signers.clone()),
        signature_threshold: policy.signature_threshold,
        paused: policy.paused,
    };
    for flags in (0..=ncore::supported_header_flags())
        .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
    {
        let _requested = ncore::DecodeFlagsGuard::enter(flags);
        let (policy_bytes, policy_flags) = norito::codec::encode_with_header_flags(&policy);
        let (projection_bytes, projection_flags) =
            norito::codec::encode_with_header_flags(&projection);
        assert_eq!(projection_flags, policy_flags);
        assert_eq!(projection_bytes, policy_bytes);
        let _actual = ncore::DecodeFlagsGuard::enter(policy_flags);
        let (decoded, used) = ProviderAdmissionCouncilPolicyV1::decode_from_slice(&policy_bytes)
            .expect("decode bounded policy");
        assert_eq!(used, policy_bytes.len());
        assert_eq!(decoded, policy);
    }
}

#[test]
fn council_policy_decoder_rejects_oversized_signer_count_before_elements() {
    let mut payload = policy().encode();
    let mut offset = 0usize;
    for _ in 0..5 {
        let (field_len, prefix) =
            ncore::read_len_dyn_slice(&payload[offset..]).expect("canonical policy field frame");
        offset += prefix + field_len;
    }
    let (_, prefix) =
        ncore::read_len_dyn_slice(&payload[offset..]).expect("canonical signer field frame");
    let count_start = offset + prefix;
    payload[count_start..count_start + 8].copy_from_slice(&33_u64.to_le_bytes());
    let error = ProviderAdmissionCouncilPolicyV1::decode_from_slice(&payload)
        .expect_err("oversized signer count must fail before Vec decoding");
    assert!(matches!(
        error,
        ncore::Error::Message(ref message)
            if message == "provider admission council signer count exceeds 32"
    ));
    assert!(norito::codec::decode_adaptive::<ProviderAdmissionCouncilPolicyV1>(&payload).is_err());
    let frame = ncore::frame_bare_with_header_flags::<ProviderAdmissionCouncilPolicyV1>(
        &payload,
        ncore::default_encode_flags(),
    )
    .expect("forged policy frame");
    assert!(norito::decode_from_bytes::<ProviderAdmissionCouncilPolicyV1>(&frame).is_err());
}

#[test]
fn council_policy_json_decoder_rejects_the_33rd_signer_before_parsing_it() {
    let signers = vec![vec![7_u8; 32]; PROVIDER_ADMISSION_COUNCIL_MAX_SIGNERS_V1];
    let json = norito::json::to_json(&signers).expect("encode signer JSON");
    let oversized = format!(
        "{},null]",
        json.strip_suffix(']').expect("signer JSON array")
    );
    let mut parser = norito::json::Parser::new(&oversized);
    let error = bounded_council_signers_json::deserialize(&mut parser)
        .expect_err("reject entry 33 before parsing its invalid body");
    assert!(matches!(
        error,
        norito::json::Error::Message(ref message)
            if message == "provider admission council signer count exceeds 32"
    ));
}

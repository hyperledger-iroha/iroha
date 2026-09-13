//! Public control schema and existing structural trust contract regressions.
use super::*;
use crate::signer::{custody::SignerCustodyAnchorV1, protocol::SignerKeyAlgorithmV1};
use iroha_crypto::{Algorithm, KeyPair};
fn policy() -> StreamTokenCustodyPolicyV1 {
    let key = |seed| {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked fixture key")
    };
    StreamTokenCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: "custody-control".into(),
            network_id: [1; 32],
            runtime_handle: "hsm://stream/primary".into(),
            key_handle: "pkcs11:stream/key-1".into(),
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
        active_from_unix_ms: 100,
        active_until_unix_ms: 5_000,
        max_validity_ms: 2_000,
        max_anchor_age_ms: 1_000,
    }
}
#[test]
fn policy_uses_existing_independent_trust_and_canonical_frame() {
    let policy = policy();
    policy
        .validate()
        .expect("valid independently governed policy");
    let frame = norito::encode_canonical(&policy).expect("encode policy");
    assert_eq!(
        norito::decode_canonical::<StreamTokenCustodyPolicyV1>(&frame).expect("decode policy"),
        policy
    );
    let trust = policy.custody_trust();
    assert_eq!(trust.authority, policy.attester_authority);
    assert_eq!(trust.public_key, policy.attester_public_key);
    assert_eq!(trust.active_from_unix_ms, policy.active_from_unix_ms);
    assert_eq!(trust.active_until_unix_ms, policy.active_until_unix_ms);
    assert_eq!(trust.max_validity_ms, policy.max_validity_ms);
    assert_eq!(trust.max_anchor_age_ms, policy.max_anchor_age_ms);
    assert!(frame.len() < STREAM_TOKEN_CUSTODY_CONTROL_MAX_BYTES_V1);
}
#[test]
fn policy_rejects_wrong_role_and_self_attesting_or_unbounded_trust() {
    let mut value = policy();
    value.binding.role = SignerRoleV1::Promotion;
    value.binding.purpose = SignerPurposeBindingV1::NativeOrPromotion;
    assert!(value.validate().is_err());
    let mut value = policy();
    value.attester_public_key = value.binding.public_key.clone();
    assert_eq!(value.validate(), Err(SignerCustodyErrorV1::SelfAttestation));
    let mut value = policy();
    value.attester_authority.administrator_id = value.binding.service_id.clone();
    assert_eq!(value.validate(), Err(SignerCustodyErrorV1::SelfAttestation));
    let mut value = policy();
    value.max_anchor_age_ms = u64::MAX;
    assert_eq!(
        value.validate(),
        Err(SignerCustodyErrorV1::UntrustedAuthority)
    );
}
#[test]
fn control_requires_exact_enrollment_sequence_predecessor_and_generation() {
    let mut state = StreamTokenCustodyControlStateV1 {
        policy: policy(),
        next_sequence: 1,
        predecessor_digest: [0; 32],
        active_head: None,
        signer_revoked: false,
        attester_revoked: false,
    };
    state
        .validate()
        .expect("unenrolled policy is an approval state");
    state.next_sequence = 2;
    assert!(state.validate().is_err());
    state.predecessor_digest = [8; 32];
    state.active_head = Some(SignerCustodyActiveHeadV1 {
        record_digest: [8; 32],
        sequence: 1,
        approved_anchor: SignerCustodyAnchorV1 {
            height: 1,
            block_hash: [9; 32],
            state_digest: [10; 32],
        },
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [5; 32],
    });
    state.validate().expect("exact enrolled state");
    let frame = norito::encode_canonical(&state).expect("control frame");
    assert_eq!(
        norito::decode_canonical::<StreamTokenCustodyControlStateV1>(&frame)
            .expect("control replay"),
        state
    );
    state.active_head.as_mut().expect("head").record_digest = [11; 32];
    assert!(state.validate().is_err());
    state.active_head.as_mut().expect("head").record_digest = [8; 32];
    state.policy.binding.key_revision = 2;
    assert!(state.validate().is_err());
}

//! Public control schema and existing structural trust contract regressions.
use super::*;
use crate::signer::{
    custody::SignerCustodyAnchorV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use iroha_crypto::{Algorithm, KeyPair};
fn policy() -> SignerCustodyPolicyV1 {
    let key = |seed| {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked fixture key")
    };
    SignerCustodyPolicyV1 {
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
        norito::decode_canonical::<SignerCustodyPolicyV1>(&frame).expect("decode policy"),
        policy
    );
    let trust = policy.custody_trust();
    assert_eq!(trust.authority, policy.attester_authority);
    assert_eq!(trust.public_key, policy.attester_public_key);
    assert_eq!(trust.active_from_unix_ms, policy.active_from_unix_ms);
    assert_eq!(trust.active_until_unix_ms, policy.active_until_unix_ms);
    assert_eq!(trust.max_validity_ms, policy.max_validity_ms);
    assert_eq!(trust.max_anchor_age_ms, policy.max_anchor_age_ms);
    assert!(frame.len() < SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1);
}
#[test]
fn shared_policy_accepts_valid_application_bindings_without_changing_the_control_schema() {
    for (role, purpose) in [
        (
            SignerRoleV1::FinalPromotionAccountTransaction,
            SignerPurposeBindingV1::FinalPromotionAccountTransaction {
                deployment_id: "production-primary".into(),
            },
        ),
        (
            SignerRoleV1::FinalPromotionProvenance,
            SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "production-primary".into(),
            },
        ),
        (
            SignerRoleV1::ReleaseManifest,
            SignerPurposeBindingV1::ReleaseManifest {
                deployment_id: "production-primary".into(),
            },
        ),
        (
            SignerRoleV1::Promotion,
            SignerPurposeBindingV1::NativeOrPromotion,
        ),
    ] {
        let mut policy = policy();
        policy.binding.role = role;
        policy.binding.purpose = purpose;
        policy
            .validate()
            .expect("common binding and independent trust");
        let frame = norito::encode_canonical(&policy).expect("shared policy frame");
        assert_eq!(
            norito::decode_canonical::<SignerCustodyPolicyV1>(&frame).expect("shared policy"),
            policy
        );
        let state = SignerCustodyControlStateV1 {
            policy,
            next_sequence: 1,
            predecessor_digest: [0; 32],
            active_head: None,
            signer_revoked: false,
            attester_revoked: false,
        };
        state.validate().expect("shared unenrolled approval state");
        let frame = norito::encode_canonical(&state).expect("shared control frame");
        assert_eq!(
            norito::decode_canonical::<SignerCustodyControlStateV1>(&frame)
                .expect("shared control"),
            state
        );
    }
}
#[test]
fn policy_rejects_role_purpose_mismatch_and_self_attesting_or_unbounded_trust() {
    let mut value = policy();
    value.binding.role = SignerRoleV1::Promotion;
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
    let mut state = SignerCustodyControlStateV1 {
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
        norito::decode_canonical::<SignerCustodyControlStateV1>(&frame).expect("control replay"),
        state
    );
    state.active_head.as_mut().expect("head").record_digest = [11; 32];
    assert!(state.validate().is_err());
    state.active_head.as_mut().expect("head").record_digest = [8; 32];
    state.policy.binding.key_revision = 2;
    assert!(state.validate().is_err());
}

fn transition_state(final_promotion: bool) -> SignerCustodyControlStateV1 {
    let mut policy = policy();
    if final_promotion {
        policy.binding.role = SignerRoleV1::FinalPromotionProvenance;
        policy.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
            deployment_id: "production-primary".into(),
        };
    }
    policy.binding.key_revision = 7;
    policy.binding.policy_revision = 9;
    policy.attester_authority.key_revision = 3;
    policy.attester_authority.policy_revision = 5;
    let state = SignerCustodyControlStateV1 {
        active_head: Some(SignerCustodyActiveHeadV1 {
            record_digest: [8; 32],
            sequence: 41,
            approved_anchor: SignerCustodyAnchorV1 {
                height: 12,
                block_hash: [9; 32],
                state_digest: [10; 32],
            },
            key_revision: policy.binding.key_revision,
            policy_revision: policy.binding.policy_revision,
            policy_digest: policy.binding.policy_digest,
        }),
        policy,
        next_sequence: 42,
        predecessor_digest: [8; 32],
        signer_revoked: true,
        attester_revoked: true,
    };
    state.validate().expect("exact governed retained state");
    state
}
fn transition_key(seed: u8) -> PublicKey {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("new independent fixture key")
        .public_key()
        .clone()
}

#[test]
fn configuration_starts_an_unenrolled_sequence_for_each_native_role() {
    for final_promotion in [false, true] {
        let policy = transition_state(final_promotion).policy;
        let configured = configure_signer_custody_policy_v1(None, policy.clone())
            .expect("first governed configuration");
        assert_eq!(configured.policy, policy);
        assert_eq!(configured.next_sequence, 1);
        assert_eq!(configured.predecessor_digest, [0; 32]);
        assert_eq!(configured.active_head, None);
        assert!(!configured.signer_revoked);
        assert!(!configured.attester_revoked);
        configured.validate().expect("valid initial control");
    }
}

#[test]
fn configuration_rejects_invalid_current_candidate_and_unchanged_policy() {
    use SignerCustodyPolicyTransitionErrorV1 as Error;
    let current = transition_state(true);
    assert_eq!(
        configure_signer_custody_policy_v1(Some(&current), current.policy.clone()),
        Err(Error::Unchanged)
    );
    let mut invalid = current.policy.clone();
    invalid.binding.service_id.clear();
    assert_eq!(
        configure_signer_custody_policy_v1(None, invalid),
        Err(Error::Invalid)
    );
    let mut malformed = current.clone();
    malformed.next_sequence += 1;
    let mut next = current.policy.clone();
    next.binding.policy_revision += 1;
    assert_eq!(
        configure_signer_custody_policy_v1(Some(&malformed), next),
        Err(Error::Invalid)
    );
    assert_eq!(
        current,
        transition_state(true),
        "rejections never mutate predecessor state"
    );
    for (error, message) in [
        (Error::Invalid, "invalid signer custody policy transition"),
        (Error::Unchanged, "signer custody policy is unchanged"),
        (
            Error::BindingMismatch,
            "signer custody policy scope mismatch",
        ),
        (
            Error::Generation,
            "signer custody policy generation mismatch",
        ),
    ] {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn independent_key_rotations_clear_only_their_revocation_and_preserve_lineage() {
    for final_promotion in [false, true] {
        for rotate in 0..4 {
            let current = transition_state(final_promotion);
            let mut policy = current.policy.clone();
            if rotate == 0 {
                policy.binding.policy_revision += 1;
                policy.binding.policy_digest[0] ^= 1;
            }
            if rotate & 1 != 0 {
                policy.binding.key_revision += 1;
                policy.binding.public_key = transition_key(21);
                policy.binding.key_handle = "pkcs11:stream/key-8".into();
            }
            if rotate & 2 != 0 {
                policy.attester_authority.key_revision += 1;
                policy.attester_public_key = transition_key(22);
            }
            let configured = configure_signer_custody_policy_v1(Some(&current), policy.clone())
                .expect("independent governed generation advance");
            assert_eq!(configured.policy, policy);
            assert_eq!(configured.active_head, None);
            assert_eq!(configured.next_sequence, current.next_sequence);
            assert_eq!(configured.predecessor_digest, current.predecessor_digest);
            assert_eq!(configured.signer_revoked, rotate & 1 == 0);
            assert_eq!(configured.attester_revoked, rotate & 2 == 0);
            configured
                .validate()
                .expect("configured control retains valid lineage");
        }
    }
}

#[test]
fn key_and_policy_generation_substitutions_and_rollbacks_are_rejected() {
    let changes: &[fn(&mut SignerCustodyPolicyV1)] = &[
        |policy| policy.binding.key_revision -= 1,
        |policy| policy.binding.key_revision += 1,
        |policy| policy.binding.public_key = transition_key(21),
        |policy| policy.attester_authority.key_revision -= 1,
        |policy| policy.attester_authority.key_revision += 1,
        |policy| policy.attester_public_key = transition_key(22),
        |policy| policy.binding.policy_revision -= 1,
        |policy| policy.binding.policy_digest[0] ^= 1,
        |policy| policy.attester_authority.policy_revision -= 1,
        |policy| policy.attester_authority.policy_digest[0] ^= 1,
    ];
    for final_promotion in [false, true] {
        let current = transition_state(final_promotion);
        for change in changes {
            let mut policy = current.policy.clone();
            change(&mut policy);
            policy
                .validate()
                .expect("structurally valid isolated generation mutation");
            assert_eq!(
                configure_signer_custody_policy_v1(Some(&current), policy),
                Err(SignerCustodyPolicyTransitionErrorV1::Generation)
            );
        }
    }
}

#[test]
fn each_changed_signing_identity_requires_its_own_policy_revision() {
    let changes: &[fn(&mut SignerCustodyPolicyV1)] = &[
        |policy| policy.binding.runtime_handle = "hsm://stream/secondary".into(),
        |policy| policy.binding.key_handle = "pkcs11:stream/same-key-new-handle".into(),
        |policy| policy.binding.service_id = "stream-service-secondary".into(),
        |policy| policy.binding.administrator_id = "stream-admin-secondary".into(),
    ];
    let current = transition_state(false);
    for change in changes {
        let mut next = current.policy.clone();
        change(&mut next);
        next.attester_authority.policy_revision += 1;
        assert_eq!(
            configure_signer_custody_policy_v1(Some(&current), next.clone()),
            Err(SignerCustodyPolicyTransitionErrorV1::Generation),
            "attester advancement cannot version a signing-policy change"
        );
        next.binding.policy_revision += 1;
        let configured = configure_signer_custody_policy_v1(Some(&current), next.clone())
            .expect("versioned signing identity change");
        assert_eq!(configured.policy, next);
        assert!(configured.signer_revoked && configured.attester_revoked);
    }
}

#[test]
fn each_changed_attestation_identity_or_limit_requires_its_own_policy_revision() {
    let changes: &[fn(&mut SignerCustodyPolicyV1)] = &[
        |policy| policy.attester_authority.service_id = "custody-service-secondary".into(),
        |policy| policy.attester_authority.administrator_id = "custody-admin-secondary".into(),
        |policy| policy.active_from_unix_ms += 1,
        |policy| policy.active_until_unix_ms += 1,
        |policy| policy.max_validity_ms += 1,
        |policy| policy.max_anchor_age_ms += 1,
    ];
    let current = transition_state(true);
    for change in changes {
        let mut next = current.policy.clone();
        change(&mut next);
        next.binding.policy_revision += 1;
        assert_eq!(
            configure_signer_custody_policy_v1(Some(&current), next.clone()),
            Err(SignerCustodyPolicyTransitionErrorV1::Generation),
            "signer advancement cannot version an attestation-policy change"
        );
        next.attester_authority.policy_revision += 1;
        let configured = configure_signer_custody_policy_v1(Some(&current), next.clone())
            .expect("versioned independent attestation policy change");
        assert_eq!(configured.policy, next);
        assert!(configured.signer_revoked && configured.attester_revoked);
    }
}

#[test]
fn immutable_scope_cannot_change_even_with_a_new_policy_revision() {
    let changes: &[fn(&mut SignerCustodyPolicyV1)] = &[
        |policy| policy.binding.chain_id = "foreign-chain".into(),
        |policy| policy.binding.network_id[0] ^= 1,
        |policy| {
            policy.binding.purpose = SignerPurposeBindingV1::StreamToken {
                provider_id: [4; 32],
            }
        },
        |policy| {
            policy.binding.role = SignerRoleV1::FinalPromotionProvenance;
            policy.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "production-primary".into(),
            };
        },
    ];
    let current = transition_state(false);
    for change in changes {
        let mut next = current.policy.clone();
        change(&mut next);
        next.binding.policy_revision += 1;
        next.binding.policy_digest[0] ^= 1;
        next.validate().expect("valid isolated scope substitution");
        assert_eq!(
            configure_signer_custody_policy_v1(Some(&current), next),
            Err(SignerCustodyPolicyTransitionErrorV1::BindingMismatch)
        );
    }
    let current = transition_state(true);
    let mut next = current.policy.clone();
    next.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
        deployment_id: "production-substituted".into(),
    };
    next.binding.policy_revision += 1;
    assert_eq!(
        configure_signer_custody_policy_v1(Some(&current), next),
        Err(SignerCustodyPolicyTransitionErrorV1::BindingMismatch)
    );
}

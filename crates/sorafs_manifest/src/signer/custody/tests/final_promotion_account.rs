//! Shared account custody verification; fixture signatures do not qualify deployment custody.

use super::*;
use crate::signer::custody_control::{
    SignerCustodyControlStateV1, SignerCustodyPolicyTransitionErrorV1, SignerCustodyPolicyV1,
    configure_signer_custody_policy_v1,
};

fn account_fixture() -> Fixture {
    let mut f = custody_fixture();
    f.statement.binding.role = SignerRoleV1::FinalPromotionAccountTransaction;
    f.statement.binding.purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
        deployment_id: "production-primary".into(),
    };
    f.statement.binding.runtime_handle =
        "software://sorafs/final-promotion-account-transaction/primary".into();
    f.statement.binding.key_handle =
        "software://sorafs/final-promotion-account-transaction/key-7".into();
    f.statement.binding.service_id = "account-signing-primary".into();
    f.statement.binding.administrator_id = "account-security-primary".into();
    f
}

fn policy(f: &Fixture) -> SignerCustodyPolicyV1 {
    SignerCustodyPolicyV1 {
        binding: f.statement.binding.clone(),
        attester_authority: f.trust.authority.clone(),
        attester_public_key: f.trust.public_key.clone(),
        active_from_unix_ms: f.trust.active_from_unix_ms,
        active_until_unix_ms: f.trust.active_until_unix_ms,
        max_validity_ms: f.trust.max_validity_ms,
        max_anchor_age_ms: f.trust.max_anchor_age_ms,
    }
}

fn current_use(f: &Fixture, bytes: &[u8]) -> SignerCustodyUseContextV1 {
    let enrolled = verify(bytes, f).expect("exact account enrollment with fixture authority");
    SignerCustodyUseContextV1 {
        now_unix_ms: f.context.now_unix_ms,
        anchor_observed_at_unix_ms: f.context.anchor_observed_at_unix_ms,
        current_anchor: SignerCustodyAnchorV1 {
            height: f.context.current_anchor.height + 1,
            block_hash: [0x81; 32],
            state_digest: [0x83; 32],
        },
        active_head: SignerCustodyActiveHeadV1 {
            record_digest: enrolled.record_digest(),
            sequence: f.statement.sequence,
            approved_anchor: f.statement.anchor,
            key_revision: f.statement.binding.key_revision,
            policy_revision: f.statement.binding.policy_revision,
            policy_digest: f.statement.binding.policy_digest,
        },
        signer_revoked: false,
        attester_revoked: false,
    }
}

#[test]
fn account_custody_reuses_canonical_enrollment_and_exact_current_use() {
    let f = account_fixture();
    policy(&f)
        .validate()
        .expect("shared independent account trust");
    let bytes = attest_unchecked(f.statement.clone(), &f.attester);
    let record: SignerCustodyRecordV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(record.statement, f.statement);
    assert_eq!(norito::encode_canonical(&record).unwrap(), bytes);
    let current = current_use(&f, &bytes);
    let verified = verify_signer_custody_use_v1(&bytes, &f.statement.binding, &f.trust, &current)
        .expect("shared verifier authorizes exact enrolled account binding");
    assert_eq!(verified.statement(), &f.statement);
    assert_eq!(verified.record_digest(), current.active_head.record_digest);
    for signer_revoked in [true, false] {
        let revoked = SignerCustodyUseContextV1 {
            signer_revoked,
            attester_revoked: !signer_revoked,
            ..current
        };
        assert_eq!(
            verify_signer_custody_use_v1(&bytes, &f.statement.binding, &f.trust, &revoked)
                .expect_err("either independent key revocation fences use"),
            SignerCustodyErrorV1::Revoked
        );
    }
    let stale = SignerCustodyUseContextV1 {
        now_unix_ms: f.statement.expires_at_unix_ms,
        ..current
    };
    assert_eq!(
        verify_signer_custody_use_v1(&bytes, &f.statement.binding, &f.trust, &stale)
            .expect_err("exclusive account custody expiry"),
        SignerCustodyErrorV1::Freshness
    );
}

#[test]
fn account_custody_cannot_cross_receipt_role_deployment_network_or_account_key() {
    let f = account_fixture();
    let bytes = attest_unchecked(f.statement.clone(), &f.attester);
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |b| {
            b.role = SignerRoleV1::FinalPromotionProvenance;
            b.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "production-primary".into(),
            };
        },
        |b| {
            b.purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
                deployment_id: "production-secondary".into(),
            }
        },
        |b| b.network_id[0] ^= 1,
        |b| b.public_key = key(0x97).public_key().clone(),
    ];
    for mutate in mutations {
        let mut expected = f.statement.binding.clone();
        mutate(&mut expected);
        assert_ne!(expected, f.statement.binding);
        validate_binding(&expected).expect("substituted expectation is independently well formed");
        assert_eq!(
            verify_signer_custody_enrollment_v1(&bytes, &expected, &f.trust, &f.context)
                .expect_err("account custody cannot authorize a different binding"),
            SignerCustodyErrorV1::BindingMismatch
        );
        let mut changed = f.statement.clone();
        changed.binding = expected;
        let changed_bytes = attest_unchecked(changed, &f.attester);
        assert_ne!(changed_bytes, bytes);
        assert_error(&changed_bytes, &f, SignerCustodyErrorV1::BindingMismatch);
    }
}

#[test]
fn account_custody_rejects_invalid_handles_wrong_algorithm_and_self_attestation() {
    let binding_mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |b| b.runtime_handle = "file:private-key".into(),
        |b| b.key_handle = "not-a-handle".into(),
        |b| b.algorithm = SignerKeyAlgorithmV1::MlDsa,
        |b| b.purpose = SignerPurposeBindingV1::NativeOrPromotion,
    ];
    for mutate in binding_mutations {
        let mut f = account_fixture();
        mutate(&mut f.statement.binding);
        assert_eq!(
            policy(&f).validate(),
            Err(SignerCustodyErrorV1::InvalidRecord)
        );
        let bytes = attest_unchecked(f.statement.clone(), &f.attester);
        assert_error(&bytes, &f, SignerCustodyErrorV1::InvalidRecord);
    }
    let mut f = account_fixture();
    let mldsa = KeyPair::try_from_seed(vec![0x29; 32], Algorithm::MlDsa)
        .expect("well-formed fixture ML-DSA-65 key");
    f.statement.binding.algorithm = SignerKeyAlgorithmV1::MlDsa;
    f.statement.binding.public_key = mldsa.public_key().clone();
    assert_eq!(
        f.statement.binding.public_key.try_algorithm().unwrap(),
        Algorithm::MlDsa
    );
    assert_eq!(
        policy(&f).validate(),
        Err(SignerCustodyErrorV1::InvalidRecord)
    );
    let bytes = attest_unchecked(f.statement.clone(), &f.attester);
    assert_error(&bytes, &f, SignerCustodyErrorV1::InvalidRecord);
    let mut f = account_fixture();
    f.trust.public_key = f.signer.public_key().clone();
    let bytes = attest_unchecked(f.statement.clone(), &f.signer);
    assert_error(&bytes, &f, SignerCustodyErrorV1::SelfAttestation);
    assert_eq!(
        policy(&f).validate(),
        Err(SignerCustodyErrorV1::SelfAttestation)
    );
}

#[test]
fn account_control_preserves_enrollment_history_and_refuses_cross_role_scope_rotation() {
    let f = account_fixture();
    let policy = policy(&f);
    let bytes = attest_unchecked(f.statement.clone(), &f.attester);
    let current = current_use(&f, &bytes);
    let mut control = configure_signer_custody_policy_v1(None, policy.clone()).unwrap();
    control.next_sequence = f.statement.sequence + 1;
    control.predecessor_digest = current.active_head.record_digest;
    control.active_head = Some(current.active_head);
    control.signer_revoked = true;
    control
        .validate()
        .expect("retained revoked account enrollment");
    let frame = norito::encode_canonical(&control).unwrap();
    assert_eq!(
        norito::decode_canonical::<SignerCustodyControlStateV1>(&frame).unwrap(),
        control
    );
    let mut successor = policy.clone();
    successor.binding.key_revision += 1;
    successor.binding.public_key = key(0x99).public_key().clone();
    successor.binding.key_handle =
        "software://sorafs/final-promotion-account-transaction/key-8".into();
    let rotated = configure_signer_custody_policy_v1(Some(&control), successor).unwrap();
    assert_eq!(rotated.next_sequence, control.next_sequence);
    assert_eq!(rotated.predecessor_digest, control.predecessor_digest);
    assert!(rotated.active_head.is_none());
    assert!(!rotated.signer_revoked);
    assert_eq!(rotated.policy.binding.role, policy.binding.role);
    assert_eq!(rotated.policy.binding.purpose, policy.binding.purpose);
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |b| {
            b.role = SignerRoleV1::FinalPromotionProvenance;
            b.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "production-primary".into(),
            };
        },
        |b| b.network_id[0] ^= 1,
        |b| b.chain_id = "sorafs-secondary".into(),
        |b| {
            b.purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
                deployment_id: "production-secondary".into(),
            }
        },
    ];
    for mutate in mutations {
        let mut changed = policy.clone();
        mutate(&mut changed.binding);
        assert_ne!(changed, policy);
        changed
            .validate()
            .expect("separately valid proposed policy");
        assert_eq!(
            configure_signer_custody_policy_v1(Some(&control), changed),
            Err(SignerCustodyPolicyTransitionErrorV1::BindingMismatch)
        );
    }
    assert_eq!(norito::encode_canonical(&control).unwrap(), frame);
}

#[test]
fn account_binding_public_validation_checks_exact_shape_without_authenticating_custody() {
    let f = account_fixture();
    let binding = &f.statement.binding;
    assert_eq!(binding.validate(), Ok(()));
    let mutations: &[fn(&mut SignerCustodyBindingV1)] = &[
        |b| b.network_id = [0; 32],
        |b| b.runtime_handle = "file:private-key".into(),
        |b| b.key_handle = "hsm://sorafs/account/../key".into(),
        |b| b.runtime_handle = "hsm://user:secret@provider/key".into(),
        |b| b.key_revision = 0,
        |b| b.policy_revision = 0,
        |b| b.policy_digest = [0; 32],
        |b| b.administrator_id = b.service_id.clone(),
        |b| b.service_id = "invalid/service".into(),
        |b| {
            b.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: "production-primary".into(),
            }
        },
        |b| {
            b.public_key =
                iroha_crypto::KeyPair::from_seed(vec![0x86; 32], iroha_crypto::Algorithm::Secp256k1)
                    .public_key()
                    .clone()
        },
        |b| b.algorithm = SignerKeyAlgorithmV1::MlDsa,
    ];
    for mutate in mutations {
        let mut changed = binding.clone();
        mutate(&mut changed);
        assert_ne!(&changed, binding);
        assert_eq!(changed.validate(), Err(SignerCustodyErrorV1::InvalidRecord));
    }
    // A different well-formed network/key is grammar-valid; authenticating the exact
    // independently expected binding remains the existing enrollment/use verifier's job.
    let mut substituted = binding.clone();
    substituted.network_id[0] ^= 1;
    substituted.public_key = key(0x87).public_key().clone();
    assert_ne!(&substituted, binding);
    assert_eq!(substituted.validate(), Ok(()));
    let bytes = attest_unchecked(f.statement.clone(), &f.attester);
    assert_eq!(
        verify_signer_custody_enrollment_v1(&bytes, &substituted, &f.trust, &f.context)
            .unwrap_err(),
        SignerCustodyErrorV1::BindingMismatch
    );
}

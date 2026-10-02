//! Purpose1 full-subject scope regression fixtures; no Native owner or money grant.
use super::*;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, kagemusha::KagemushaHardwareTransitionSelectionV1};
fn challenge(operation: KagemushaOperationKindV1) -> KagemushaAppOperationApprovalChallengeV1 {
    let mut c = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: [1; 32],
        nonce: [2; 32],
        account_binding: [3; 32],
        authority_policy_digest: [4; 32],
        attested_key_id: [5; 32],
        enrollment_digest: [6; 32],
        subject_signing_digest: [7; 32],
        normalized_guard_digest: [8; 32],
        issued_at_ms: 100,
        expires_at_ms: 200,
        subject: KagemushaHardwareTransitionSelectionV1 {
            version: 1,
            release_id: [9; 32],
            provider_policy_root: [10; 32],
            app_policy_digest: [11; 32],
            credential_id: [6; 32],
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"ordinary terminal challenge network",
            ))),
            lane_commitment: [13; 32],
            hardware_profile_id: [14; 32],
            policy_epoch: 15,
            hardware_epoch_id: [16; 32],
            hardware_epoch_generation: 17,
            operation_kind: operation,
            transition_statement_digest: [18; 32],
            candidate_envelope_digest: [19; 32],
            terminal_body_commitment: [20; 32],
            secure_index_before: (1_u128 << 100) + 9,
            secure_index_after: (1_u128 << 100) + 10,
        },
    };
    c.subject_signing_digest = Sha256::digest(c.canonical_subject_signing_bytes().unwrap()).into();
    c
}

fn check(
    c: &KagemushaAppOperationApprovalChallengeV1,
    op: KagemushaOperationKindV1,
) -> Result<DigestV1> {
    require_terminal_challenge(
        c,
        [6; 32],
        [8; 32],
        [18; 32],
        [19; 32],
        [20; 32],
        (1_u128 << 100) + 9,
        (1_u128 << 100) + 10,
        op,
    )
}
#[test]
fn terminal_guard_scope_requires_purpose1_actual_candidate_body_and_u128_secure_indexes() {
    for op in [
        KagemushaOperationKindV1::SendSplit,
        KagemushaOperationKindV1::RedeemSplit,
    ] {
        let c = challenge(op);
        assert_eq!(check(&c, op).unwrap(), c.subject_signing_digest);
        let changes: [fn(&mut KagemushaAppOperationApprovalChallengeV1); 11] = [
            |c| c.purpose = KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
            |c| c.enrollment_digest[0] ^= 1,
            |c| c.subject.credential_id[0] ^= 1,
            |c| c.normalized_guard_digest[0] ^= 1,
            |c| c.subject.transition_statement_digest[0] ^= 1,
            |c| c.subject.candidate_envelope_digest[0] ^= 1,
            |c| c.subject.terminal_body_commitment[0] ^= 1,
            |c| c.subject.secure_index_before = 9,
            |c| c.subject.secure_index_after = 10,
            |c| c.subject.candidate_envelope_digest = [0; 32],
            |c| c.subject.terminal_body_commitment = [0; 32],
        ];
        for change in changes {
            let mut changed = c;
            change(&mut changed);
            if let Ok(bytes) = changed.canonical_subject_signing_bytes() {
                changed.subject_signing_digest = Sha256::digest(bytes).into();
            }
            assert!(check(&changed, op).is_err());
        }
        let other = if op == KagemushaOperationKindV1::SendSplit {
            KagemushaOperationKindV1::RedeemSplit
        } else {
            KagemushaOperationKindV1::SendSplit
        };
        assert!(check(&c, other).is_err());
    }
}
#[test]
fn terminal_guard_subject_sha_is_complete_original_and_never_a_preparation_upgrade() {
    let c = challenge(KagemushaOperationKindV1::SendSplit);
    let mut altered = c;
    altered.subject.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::new(b"substituted ordinary terminal challenge network"),
    ));
    assert!(check(&altered, KagemushaOperationKindV1::SendSplit).is_err());
    let mut preparation = c;
    preparation.purpose = KagemushaAppOperationApprovalPurposeV1::PrepareTransition;
    preparation.subject.candidate_envelope_digest = [0; 32];
    preparation.subject.terminal_body_commitment = [0; 32];
    preparation.subject_signing_digest =
        Sha256::digest(preparation.canonical_subject_signing_bytes().unwrap()).into();
    assert!(check(&preparation, KagemushaOperationKindV1::SendSplit).is_err());
    for inactive in [
        KagemushaOperationKindV1::Bootstrap,
        KagemushaOperationKindV1::MintFold,
        KagemushaOperationKindV1::ReceiveFold,
        KagemushaOperationKindV1::Rotate,
    ] {
        assert!(
            require_terminal_challenge(
                &c,
                [6; 32],
                [8; 32],
                [18; 32],
                [19; 32],
                [20; 32],
                (1_u128 << 100) + 9,
                (1_u128 << 100) + 10,
                inactive
            )
            .is_err()
        );
    }
}

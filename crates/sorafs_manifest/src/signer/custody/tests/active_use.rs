//! Enrollment and ordinary-use separation with authoritative-state drift simulations.

use super::*;

fn activate(fixture: &Fixture, bytes: &[u8]) -> SignerCustodyUseContextV1 {
    let enrollment = verify(bytes, fixture).expect("test authority admits exact enrollment slot");
    // This test models the authoritative CAS result, not candidate-provided runtime trust.
    SignerCustodyUseContextV1 {
        now_unix_ms: fixture.context.now_unix_ms,
        anchor_observed_at_unix_ms: fixture.context.anchor_observed_at_unix_ms,
        current_anchor: SignerCustodyAnchorV1 {
            height: fixture.context.current_anchor.height + 1,
            block_hash: [0x81; 32],
            state_digest: [0x83; 32],
        },
        active_head: SignerCustodyActiveHeadV1 {
            record_digest: enrollment.record_digest(),
            sequence: fixture.statement.sequence,
            approved_anchor: fixture.statement.anchor,
            key_revision: fixture.statement.binding.key_revision,
            policy_revision: fixture.statement.binding.policy_revision,
            policy_digest: fixture.statement.binding.policy_digest,
        },
        signer_revoked: false,
        attester_revoked: false,
    }
}

fn use_current(
    bytes: &[u8],
    fixture: &Fixture,
    context: &SignerCustodyUseContextV1,
) -> Result<VerifiedSignerCustodyV1, SignerCustodyErrorV1> {
    verify_signer_custody_use_v1(bytes, &fixture.statement.binding, &fixture.trust, context)
}

#[test]
fn enroll_once_use_many_and_advance_finalized_height_without_reattestation() {
    let mut fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let mut current = activate(&fixture, &bytes);
    fixture.context.next_sequence = 2;
    fixture.context.predecessor_digest = current.active_head.record_digest;
    assert_error(&bytes, &fixture, SignerCustodyErrorV1::ReplayOrRollback);
    let before = use_current(&bytes, &fixture, &current).expect("enrolled key eligible for use");
    current.current_anchor.height += 1;
    current.current_anchor.block_hash = [0x85; 32];
    current.now_unix_ms += 1;
    let after = use_current(&bytes, &fixture, &current).expect("same key at later finalized block");
    assert!(after.continues_active_state(&before));
    assert!(!before.continues_active_state(&after));
    assert_eq!(after.statement(), &fixture.statement);
    assert_eq!(after.record_digest(), current.active_head.record_digest);
    assert_eq!(after.verified_at_unix_ms(), current.now_unix_ms);
    assert_eq!(after.current_anchor(), current.current_anchor);
    assert_ne!(after.current_anchor(), fixture.statement.anchor);
    assert!(!format!("{after:?}").contains(&fixture.statement.binding.key_handle));
}

#[test]
fn active_record_generation_policy_and_original_approval_are_independently_pinned() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let current = activate(&fixture, &bytes);
    let mutations: &[fn(&mut SignerCustodyActiveHeadV1)] = &[
        |head| head.record_digest[0] ^= 1,
        |head| head.record_digest = [0; 32],
        |head| head.sequence += 1,
        |head| head.sequence = 0,
        |head| head.key_revision += 1,
        |head| head.policy_revision += 1,
        |head| head.policy_digest[0] ^= 1,
    ];
    for mutate in mutations {
        let mut substituted = current;
        mutate(&mut substituted.active_head);
        assert_eq!(
            use_current(&bytes, &fixture, &substituted).expect_err("inactive or substituted head"),
            SignerCustodyErrorV1::ReplayOrRollback
        );
    }
    let mutations: &[fn(&mut SignerCustodyUseContextV1)] = &[
        |context| context.active_head.approved_anchor.block_hash[0] ^= 1,
        |context| context.active_head.approved_anchor.state_digest[0] ^= 1,
        |context| context.current_anchor.height = context.active_head.approved_anchor.height - 1,
        |context| context.current_anchor.height = context.active_head.approved_anchor.height,
        |context| context.current_anchor.state_digest = [0; 32],
    ];
    for mutate in mutations {
        let mut substituted = current;
        mutate(&mut substituted);
        assert_eq!(
            use_current(&bytes, &fixture, &substituted).expect_err("unapproved anchor or fork"),
            SignerCustodyErrorV1::AnchorMismatch
        );
    }
}

#[test]
fn active_use_rejects_stale_state_expiry_revocation_and_wrong_attestation_authority() {
    let mut fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let current = activate(&fixture, &bytes);
    let mutations: &[fn(&mut SignerCustodyUseContextV1)] = &[
        |context| context.now_unix_ms = 2_000,
        |context| context.anchor_observed_at_unix_ms = context.now_unix_ms - 101,
        |context| context.anchor_observed_at_unix_ms = context.now_unix_ms + 1,
        |context| context.now_unix_ms = 0,
    ];
    for mutate in mutations {
        let mut stale = current;
        mutate(&mut stale);
        assert_eq!(
            use_current(&bytes, &fixture, &stale).expect_err("ineligible use time"),
            SignerCustodyErrorV1::Freshness
        );
    }
    for signer_revoked in [true, false] {
        let revoked = SignerCustodyUseContextV1 {
            signer_revoked,
            attester_revoked: !signer_revoked,
            ..current
        };
        assert_eq!(
            use_current(&bytes, &fixture, &revoked).expect_err("revocation blocks key use"),
            SignerCustodyErrorV1::Revoked
        );
    }
    fixture.trust.public_key = key(0x91).public_key().clone();
    assert_eq!(
        use_current(&bytes, &fixture, &current).expect_err("wrong independent authority key"),
        SignerCustodyErrorV1::InvalidAttestation
    );
}

#[test]
fn rotation_during_provider_io_fences_the_old_record_and_requires_exact_new_generation() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let current = activate(&fixture, &bytes);
    let before = use_current(&bytes, &fixture, &current).expect("pre-I/O observation");
    let mut successor = custody_fixture();
    successor.signer = key(0x95);
    successor.statement.binding.public_key = successor.signer.public_key().clone();
    successor.statement.binding.key_handle = "hsm:production/key-8".into();
    successor.statement.binding.key_revision += 1;
    successor.statement.binding.policy_revision += 1;
    successor.statement.binding.policy_digest = [0x97; 32];
    successor.statement.sequence = 2;
    successor.statement.predecessor_digest = current.active_head.record_digest;
    successor.statement.anchor = current.current_anchor;
    successor.context.current_anchor = current.current_anchor;
    successor.context.next_sequence = 2;
    successor.context.predecessor_digest = current.active_head.record_digest;
    let next_bytes = attest_unchecked(successor.statement.clone(), &successor.attester);
    let mut rotated = activate(&successor, &next_bytes);
    rotated.current_anchor.block_hash = [0x99; 32];
    rotated.current_anchor.state_digest = [0x9B; 32];
    assert_eq!(
        use_current(&bytes, &fixture, &rotated).expect_err("old key after authoritative rotation"),
        SignerCustodyErrorV1::AnchorMismatch
    );
    assert_eq!(
        use_current(&bytes, &successor, &rotated).expect_err("old record under new reviewed key"),
        SignerCustodyErrorV1::BindingMismatch
    );
    let after = use_current(&next_bytes, &successor, &rotated).expect("new enrolled generation");
    assert!(!after.continues_active_state(&before));
    // Even if the generation is unchanged, drift in the authenticated custody state must fence
    // signature release. The future operation owner must enforce this comparison around I/O.
    let mut drifted = current;
    drifted.current_anchor.height += 1;
    drifted.current_anchor.block_hash = [0x9D; 32];
    drifted.current_anchor.state_digest[0] ^= 1;
    let after_drift =
        use_current(&bytes, &fixture, &drifted).expect("fresh changed-state observation");
    assert!(!after_drift.continues_active_state(&before));
}

#[test]
fn post_io_comparison_rejects_backwards_time_or_finalized_height_and_same_height_forks() {
    let fixture = custody_fixture();
    let bytes = attest_unchecked(fixture.statement.clone(), &fixture.attester);
    let mut current = activate(&fixture, &bytes);
    current.current_anchor.height += 1;
    let before = use_current(&bytes, &fixture, &current).expect("pre-I/O observation");
    let mutations: &[fn(&mut SignerCustodyUseContextV1)] = &[
        |context| context.now_unix_ms -= 1,
        |context| context.current_anchor.height -= 1,
        |context| context.current_anchor.block_hash[0] ^= 1,
    ];
    for mutate in mutations {
        let mut stale = current;
        mutate(&mut stale);
        let after = use_current(&bytes, &fixture, &stale).expect("individually valid observation");
        assert!(!after.continues_active_state(&before));
    }
}

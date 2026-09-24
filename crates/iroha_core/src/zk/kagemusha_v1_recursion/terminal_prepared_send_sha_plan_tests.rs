//! Paired fixed-shape planning for the complete outgoing opening.

use super::*;
use crate::zk::kagemusha_v1_recursion::{
    KagemushaPastaParityV1, mint_hash_claim_fold::KagemushaMintHashClaimPlanV1,
    mint_hash_shard::KagemushaMintHashPlanV1, terminal_body_commitment,
    terminal_durable_commitments as durable,
};
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use sha2::{Digest as _, Sha256};

fn blocks(length: usize) -> usize {
    (length + 9).div_ceil(BLOCK_BYTE_SIZE)
}

fn bounded(logical_message: Vec<u8>, capacity: usize) -> PastaSha256PlanMessageV1 {
    PastaSha256PlanMessageV1::Bounded {
        selected_block: blocks(logical_message.len()) - 1,
        max_blocks: blocks(capacity),
        logical_message,
        capacity,
    }
}

fn message_mut(job: &mut PastaSha256PlanMessageV1) -> &mut Vec<u8> {
    match job {
        PastaSha256PlanMessageV1::Ordinary(message) => message,
        PastaSha256PlanMessageV1::Bounded {
            logical_message, ..
        } => logical_message,
    }
}

fn plan_case() -> (
    TerminalSemanticShaPlanV1,
    Vec<PastaSha256PlanMessageV1>,
    Vec<PastaSha256PlanMessageV1>,
) {
    let mut semantic = TerminalSemanticShaPlanV1 {
        eq_messages: (0_u8..26).map(|index| vec![index]).collect(),
        ep_messages: (0_u8..26).map(|index| vec![index + 26]).collect(),
        job_block_counts: vec![1; 26],
        prepared_candidate_bindings: TerminalPreparedCandidateBindingsV1 {
            preparation_id: [0; 32],
            sealed_stream_digests: [[0; 32]; 2],
        },
    };
    let transition = [
        durable::SEALED_TRANSITION_DIGEST_DOMAIN_V1,
        &[0],
        &3_u64.to_le_bytes(),
        &[0x11, 0x22, 0x33],
    ]
    .concat();
    let seeds = [
        durable::SEALED_RECOVERY_DIGEST_DOMAIN_V1,
        &[0],
        &2_u64.to_le_bytes(),
        &[0x44, 0x55],
    ]
    .concat();
    let mut transcript = vec![0_u8; 475];
    let transcript_prefix = [durable::PREPARATION_ID_DOMAIN_V1, &[0]].concat();
    transcript[..transcript_prefix.len()].copy_from_slice(&transcript_prefix);
    transcript[transcript_prefix.len()..transcript_prefix.len() + 2]
        .copy_from_slice(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
    transcript[transcript_prefix.len() + 2] = 2;
    let transcript_lengths_start = transcript_prefix.len() + 2 + 1 + 11 * 32;
    transcript[transcript_lengths_start..transcript_lengths_start + 8]
        .copy_from_slice(&3_u64.to_le_bytes());
    transcript[transcript_lengths_start + 8..transcript_lengths_start + 8 + 32]
        .copy_from_slice(&Sha256::digest(&transition));
    transcript[transcript_lengths_start + 8 + 32..transcript_lengths_start + 8 + 32 + 8]
        .copy_from_slice(&2_u64.to_le_bytes());
    transcript[transcript_lengths_start + 8 + 32 + 8..transcript_lengths_start + 8 + 32 + 8 + 32]
        .copy_from_slice(&Sha256::digest(&seeds));
    semantic.prepared_candidate_bindings = TerminalPreparedCandidateBindingsV1 {
        preparation_id: Sha256::digest(&transcript).into(),
        sealed_stream_digests: [
            Sha256::digest(&transition).into(),
            Sha256::digest(&seeds).into(),
        ],
    };
    let journal_frame = crate::zk::kagemusha_v1_state::terminal_journal_canonical_layout_v1()
        .expect("canonical journal frame")
        .0
        .into_iter()
        .map(|fixed| fixed.unwrap_or_default())
        .collect::<Vec<_>>();
    let journal = [
        (durable::TERMINAL_JOURNAL_DOMAIN_V1.len() as u64)
            .to_be_bytes()
            .as_slice(),
        durable::TERMINAL_JOURNAL_DOMAIN_V1,
        (journal_frame.len() as u64).to_be_bytes().as_slice(),
        journal_frame.as_slice(),
    ]
    .concat();
    let mut recovery_frame =
        crate::zk::kagemusha_v1_state::terminal_recovery_canonical_frame_prefix_v1()
            .expect("canonical recovery prefix")
            .into_iter()
            .map(|fixed| fixed.unwrap_or_default())
            .collect::<Vec<_>>();
    recovery_frame.resize(120, 0);
    let recovery = [
        (durable::TERMINAL_RECOVERY_DOMAIN_V1.len() as u64)
            .to_be_bytes()
            .as_slice(),
        durable::TERMINAL_RECOVERY_DOMAIN_V1,
        (recovery_frame.len() as u64).to_be_bytes().as_slice(),
        recovery_frame.as_slice(),
    ]
    .concat();
    let body_bytes = KagemushaHardwareTerminalBodyCommitmentLayoutV1::BODY_BYTES;
    let mut body = [
        terminal_body_commitment::TERMINAL_BODY_DOMAIN_V1,
        &[0],
        (body_bytes as u64).to_le_bytes().as_slice(),
        vec![0_u8; body_bytes].as_slice(),
    ]
    .concat();
    let body_version_offset = terminal_body_commitment::TERMINAL_BODY_DOMAIN_V1.len() + 1 + 8;
    body[body_version_offset..body_version_offset + 2]
        .copy_from_slice(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
    let recovery_prefix =
        crate::zk::kagemusha_v1_state::terminal_recovery_canonical_frame_prefix_v1()
            .expect("canonical recovery prefix");
    let recovery_capacity = 16
        + durable::TERMINAL_RECOVERY_DOMAIN_V1.len()
        + recovery_prefix.len()
        + 1
        + 32
        + 1
        + 32
        + 2
        + 8
        + KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize
        + 2
        + 8
        + KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize;
    let tail = vec![
        bounded(
            transition,
            durable::SEALED_TRANSITION_DIGEST_DOMAIN_V1.len()
                + 1
                + 8
                + KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize,
        ),
        bounded(
            seeds,
            durable::SEALED_RECOVERY_DIGEST_DOMAIN_V1.len()
                + 1
                + 8
                + KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize,
        ),
        bounded(transcript, 475),
        PastaSha256PlanMessageV1::Ordinary(journal),
        bounded(recovery, recovery_capacity),
        PastaSha256PlanMessageV1::Ordinary(body),
    ];
    let eq = semantic
        .eq_messages
        .iter()
        .cloned()
        .map(PastaSha256PlanMessageV1::Ordinary)
        .chain(tail.iter().cloned())
        .collect();
    let ep = semantic
        .ep_messages
        .iter()
        .cloned()
        .map(PastaSha256PlanMessageV1::Ordinary)
        .chain(tail)
        .collect();
    (semantic, eq, ep)
}

#[test]
fn complete_send_sha_plan_preserves_parity_and_fixed_capacity() {
    let (semantic, eq, ep) = plan_case();
    let plan = plan_terminal_prepared_outgoing_sha_v1(
        &semantic,
        &eq,
        &ep,
        KagemushaOperationV1::SendSplit,
    )
    .expect("complete paired send SHA plan");
    assert_eq!(plan.eq_messages.len(), 32);
    assert_eq!(plan.ep_messages.len(), 32);
    assert_eq!(&plan.active_job_block_counts[..26], &[1; 26]);
    assert_eq!(&plan.capacity_job_block_counts[26..], &[33, 9, 8, 5, 44, 6]);
    assert_eq!(
        plan.capacity_job_block_counts[26..].iter().sum::<u32>(),
        105
    );
    assert_eq!(&plan.active_job_block_counts[26..], &[1, 1, 8, 5, 3, 6]);
    assert_eq!(&plan.eq_messages[..26], &semantic.eq_messages);
    assert_eq!(&plan.ep_messages[..26], &semantic.ep_messages);
    assert_eq!(&plan.eq_messages[26..], &plan.ep_messages[26..]);
}

#[test]
fn send_opening_cannot_be_relabelled_as_redemption() {
    let (semantic, eq, ep) = plan_case();
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &eq,
            &ep,
            KagemushaOperationV1::RedeemSplit,
        )
        .is_err()
    );
}

#[test]
fn complete_send_active_messages_feed_the_existing_typed_claim_plan() {
    let (semantic, eq, ep) = plan_case();
    let plan = plan_terminal_prepared_outgoing_sha_v1(
        &semantic,
        &eq,
        &ep,
        KagemushaOperationV1::SendSplit,
    )
    .expect("paired prepared send plan");
    let release = [0x5a; 32];
    let expected_stages: u64 = plan
        .active_job_block_counts
        .iter()
        .map(|count| u64::from(*count))
        .sum();
    for (parity, messages) in [
        (KagemushaPastaParityV1::Eq, plan.eq_messages),
        (KagemushaPastaParityV1::Ep, plan.ep_messages),
    ] {
        let leaves = KagemushaMintHashPlanV1::from_messages(release, parity, [1; 32], messages)
            .expect("active prepared messages have canonical one-block shard leaves");
        let (jobs, stages) = match parity {
            KagemushaPastaParityV1::Eq => {
                let claim =
                    KagemushaMintHashClaimPlanV1::from_leaves::<Fp>(release, leaves.leaves())
                        .expect("Eq typed claim plan");
                (claim.total_jobs, claim.total_stages)
            }
            KagemushaPastaParityV1::Ep => {
                let claim =
                    KagemushaMintHashClaimPlanV1::from_leaves::<Fq>(release, leaves.leaves())
                        .expect("Ep typed claim plan");
                (claim.total_jobs, claim.total_stages)
            }
        };
        assert_eq!(jobs, 32);
        assert_eq!(stages, expected_stages);
    }
}

#[test]
fn complete_send_sha_plan_accepts_active_block_growth_with_fixed_capacity() {
    let (mut semantic, mut eq, mut ep) = plan_case();
    let content_start = durable::SEALED_TRANSITION_DIGEST_DOMAIN_V1.len() + 1 + 8;
    for jobs in [&mut eq, &mut ep] {
        let PastaSha256PlanMessageV1::Bounded {
            logical_message,
            selected_block,
            ..
        } = &mut jobs[26]
        else {
            panic!("transition stream must be bounded");
        };
        logical_message.resize(content_start + 100, 0x7a);
        logical_message[content_start - 8..content_start].copy_from_slice(&100_u64.to_le_bytes());
        *selected_block = blocks(logical_message.len()) - 1;
        let digest = Sha256::digest(logical_message.as_slice());
        let transcript = message_mut(&mut jobs[28]);
        let lengths_start = durable::PREPARATION_ID_DOMAIN_V1.len() + 1 + 2 + 1 + 11 * 32;
        transcript[lengths_start..lengths_start + 8].copy_from_slice(&100_u64.to_le_bytes());
        transcript[lengths_start + 8..lengths_start + 8 + 32].copy_from_slice(&digest);
    }
    semantic.prepared_candidate_bindings.sealed_stream_digests[0] =
        Sha256::digest(message_mut(&mut eq[26]).as_slice()).into();
    semantic.prepared_candidate_bindings.preparation_id =
        Sha256::digest(message_mut(&mut eq[28]).as_slice()).into();
    let plan = plan_terminal_prepared_outgoing_sha_v1(
        &semantic,
        &eq,
        &ep,
        KagemushaOperationV1::SendSplit,
    )
    .expect("active stream length may grow within its fixed capacity");
    assert_eq!(plan.active_job_block_counts[26], 3);
    assert_eq!(plan.capacity_job_block_counts[26], 33);
}

#[test]
fn complete_send_sha_plan_rejects_missing_or_altered_paired_jobs() {
    let (semantic, eq, ep) = plan_case();
    let mut changed_semantic = TerminalSemanticShaPlanV1 {
        eq_messages: semantic.eq_messages.clone(),
        ep_messages: semantic.ep_messages.clone(),
        job_block_counts: semantic.job_block_counts.clone(),
        prepared_candidate_bindings: semantic.prepared_candidate_bindings,
    };
    changed_semantic.job_block_counts[0] += 1;
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &changed_semantic,
            &eq,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    changed.pop();
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    message_mut(&mut changed[0])[0] ^= 1;
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    if let PastaSha256PlanMessageV1::Bounded { selected_block, .. } = &mut changed[26] {
        *selected_block += 1;
    }
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    if let PastaSha256PlanMessageV1::Bounded { capacity, .. } = &mut changed[27] {
        *capacity -= 1;
    }
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    if let PastaSha256PlanMessageV1::Bounded {
        logical_message,
        capacity,
        ..
    } = &mut changed[26]
    {
        logical_message.resize(*capacity + 1, 0);
    }
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    if let PastaSha256PlanMessageV1::Bounded { max_blocks, .. } = &mut changed[30] {
        *max_blocks -= 1;
    }
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = eq.clone();
    changed[28] = PastaSha256PlanMessageV1::Ordinary(vec![0; 475]);
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed,
            &ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = ep.clone();
    message_mut(&mut changed[31])[20] ^= 1;
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &eq,
            &changed,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );

    let mut changed = ep.clone();
    if let PastaSha256PlanMessageV1::Bounded {
        logical_message,
        selected_block,
        ..
    } = &mut changed[26]
    {
        let content_start = durable::SEALED_TRANSITION_DIGEST_DOMAIN_V1.len() + 1 + 8;
        logical_message.resize(content_start + 100, 0x7a);
        logical_message[content_start - 8..content_start].copy_from_slice(&100_u64.to_le_bytes());
        *selected_block = blocks(logical_message.len()) - 1;
    }
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &eq,
            &changed,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );
}

#[test]
fn complete_send_sha_plan_rejects_changed_state_candidate_carriers() {
    let (semantic, eq, ep) = plan_case();
    for role in 0..3 {
        let mut changed = TerminalSemanticShaPlanV1 {
            eq_messages: semantic.eq_messages.clone(),
            ep_messages: semantic.ep_messages.clone(),
            job_block_counts: semantic.job_block_counts.clone(),
            prepared_candidate_bindings: semantic.prepared_candidate_bindings,
        };
        if role == 0 {
            changed.prepared_candidate_bindings.preparation_id[0] ^= 1;
        } else {
            changed.prepared_candidate_bindings.sealed_stream_digests[role - 1][0] ^= 1;
        }
        assert!(
            plan_terminal_prepared_outgoing_sha_v1(
                &changed,
                &eq,
                &ep,
                KagemushaOperationV1::SendSplit
            )
            .is_err(),
            "accepted opening detached from State candidate carrier {role}"
        );
    }
}

#[test]
fn prepared_candidate_bindings_decode_the_same_fixed_column_in_both_parities() {
    fn check<F: KagemushaPoseidonFieldV1>() {
        let mut column = vec![
            F::from(0);
            state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT
                + accumulator_limb_count()
        ];
        let expected = TerminalPreparedCandidateBindingsV1 {
            preparation_id: [0x19; 32],
            sealed_stream_digests: [[0x27; 32], [0x35; 32]],
        };
        for (offset, digest) in [
            (
                state_relation::public_instance::PREPARATION_ID_LO,
                expected.preparation_id,
            ),
            (
                state_relation::public_instance::SEALED_TRANSITION_INPUTS_LO,
                expected.sealed_stream_digests[0],
            ),
            (
                state_relation::public_instance::SEALED_RECOVERY_SEEDS_LO,
                expected.sealed_stream_digests[1],
            ),
        ] {
            column[offset..offset + 2]
                .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest));
        }
        assert_eq!(
            terminal_prepared_candidate_bindings_v1(&column).unwrap(),
            expected
        );
        column[state_relation::public_instance::PREPARATION_ID_LO] =
            crate::zk::kagemusha_v1_poseidon::from_u128::<F>(u128::MAX) + F::from(1);
        assert!(terminal_prepared_candidate_bindings_v1(&column).is_err());
        column.pop();
        assert!(terminal_prepared_candidate_bindings_v1(&column).is_err());
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn prepared_candidate_bindings_require_identical_eq_ep_carriers() {
    let eq = TerminalPreparedCandidateBindingsV1 {
        preparation_id: [0x19; 32],
        sealed_stream_digests: [[0x27; 32], [0x35; 32]],
    };
    assert_eq!(
        require_paired_prepared_candidate_bindings_v1(eq, eq),
        Ok(eq)
    );

    for role in 0..3 {
        let mut ep = eq;
        if role == 0 {
            ep.preparation_id[0] ^= 1;
        } else {
            ep.sealed_stream_digests[role - 1][0] ^= 1;
        }
        assert_eq!(
            require_paired_prepared_candidate_bindings_v1(eq, ep),
            Err("terminal paired State candidates carry different prepared intents".to_owned()),
            "accepted Eq/Ep prepared-intent mismatch at carrier {role}"
        );
    }
}

#[test]
fn complete_send_sha_plan_rejects_changed_opening_preimages() {
    let (semantic, eq, ep) = plan_case();
    for (job, byte) in [(26, 0), (27, 0), (28, 0), (29, 8), (30, 8), (31, 0)] {
        let mut changed_eq = eq.clone();
        let mut changed_ep = ep.clone();
        message_mut(&mut changed_eq[job])[byte] ^= 1;
        message_mut(&mut changed_ep[job])[byte] ^= 1;
        assert!(
            plan_terminal_prepared_outgoing_sha_v1(
                &semantic,
                &changed_eq,
                &changed_ep,
                KagemushaOperationV1::SendSplit
            )
            .is_err(),
            "accepted mutated opening job {job}"
        );
    }
    for (job, length_offset) in [
        (26, durable::SEALED_TRANSITION_DIGEST_DOMAIN_V1.len() + 1),
        (27, durable::SEALED_RECOVERY_DIGEST_DOMAIN_V1.len() + 1),
        (29, 8 + durable::TERMINAL_JOURNAL_DOMAIN_V1.len()),
        (30, 8 + durable::TERMINAL_RECOVERY_DOMAIN_V1.len()),
        (
            31,
            terminal_body_commitment::TERMINAL_BODY_DOMAIN_V1.len() + 1,
        ),
    ] {
        let mut changed_eq = eq.clone();
        let mut changed_ep = ep.clone();
        message_mut(&mut changed_eq[job])[length_offset] ^= 1;
        message_mut(&mut changed_ep[job])[length_offset] ^= 1;
        assert!(
            plan_terminal_prepared_outgoing_sha_v1(
                &semantic,
                &changed_eq,
                &changed_ep,
                KagemushaOperationV1::SendSplit
            )
            .is_err(),
            "accepted mutated opening length in job {job}"
        );
    }
    let mut changed_eq = eq.clone();
    let mut changed_ep = ep.clone();
    let version_offset = terminal_body_commitment::TERMINAL_BODY_DOMAIN_V1.len() + 1 + 8;
    message_mut(&mut changed_eq[31])[version_offset] ^= 1;
    message_mut(&mut changed_ep[31])[version_offset] ^= 1;
    assert!(
        plan_terminal_prepared_outgoing_sha_v1(
            &semantic,
            &changed_eq,
            &changed_ep,
            KagemushaOperationV1::SendSplit
        )
        .is_err()
    );
}

#[test]
fn complete_send_sha_plan_rejects_transcript_stream_length_mismatch() {
    let (semantic, eq, ep) = plan_case();
    let lengths_start = durable::PREPARATION_ID_DOMAIN_V1.len() + 1 + 2 + 1 + 11 * 32;
    for offset in [lengths_start, lengths_start + 8 + 32] {
        let mut changed_eq = eq.clone();
        let mut changed_ep = ep.clone();
        message_mut(&mut changed_eq[28])[offset] ^= 1;
        message_mut(&mut changed_ep[28])[offset] ^= 1;
        assert!(
            plan_terminal_prepared_outgoing_sha_v1(
                &semantic,
                &changed_eq,
                &changed_ep,
                KagemushaOperationV1::SendSplit
            )
            .is_err(),
            "accepted preparation transcript length unrelated to its sealed stream"
        );
    }
}

#[test]
fn complete_send_sha_plan_rejects_transcript_stream_digest_mismatch() {
    let (semantic, eq, ep) = plan_case();
    let lengths_start = durable::PREPARATION_ID_DOMAIN_V1.len() + 1 + 2 + 1 + 11 * 32;
    for offset in [lengths_start + 8, lengths_start + 8 + 32 + 8] {
        let mut changed_eq = eq.clone();
        let mut changed_ep = ep.clone();
        message_mut(&mut changed_eq[28])[offset] ^= 1;
        message_mut(&mut changed_ep[28])[offset] ^= 1;
        assert!(
            plan_terminal_prepared_outgoing_sha_v1(
                &semantic,
                &changed_eq,
                &changed_ep,
                KagemushaOperationV1::SendSplit
            )
            .is_err(),
            "accepted preparation transcript digest unrelated to its sealed stream"
        );
    }
}

#[test]
fn complete_send_sha_plan_rejects_noncanonical_durable_frame_headers_in_both_parities() {
    let (semantic, eq, ep) = plan_case();
    for (job, domain) in [
        (29, durable::TERMINAL_JOURNAL_DOMAIN_V1),
        (30, durable::TERMINAL_RECOVERY_DOMAIN_V1),
    ] {
        // The outer domain and frame byte count still agree. Only one encoder-owned
        // fixed header byte changes, which the circuit cannot treat as witness data.
        let frame_start = 8 + domain.len() + 8;
        let mut changed_eq = eq.clone();
        let mut changed_ep = ep.clone();
        message_mut(&mut changed_eq[job])[frame_start] ^= 1;
        message_mut(&mut changed_ep[job])[frame_start] ^= 1;
        assert!(
            plan_terminal_prepared_outgoing_sha_v1(
                &semantic,
                &changed_eq,
                &changed_ep,
                KagemushaOperationV1::SendSplit
            )
            .is_err(),
            "accepted altered canonical header in opening job {job}"
        );
    }
}

//! Exact assigned SHA queue to complete-claim constraints, without proof or wallet authority.

use super::super::{assign_bytes, hash_terminal_prepared_transfer_v1};
use super::*;
use crate::zk::{
    kagemusha_v1_poseidon::encode,
    kagemusha_v1_recursion::{
        DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaPastaParityV1,
        mint_hash_claim_fold::{
            KagemushaMintHashClaimMetadataV1, KagemushaMintHashClaimPairStateV1,
            KagemushaMintHashClaimPlanV1, KagemushaMintHashClaimStateV1, claim_public_values_v1,
            constrain_complete_claim_against_sha_jobs_v1, public_instance as claim_cell,
        },
        mint_hash_shard::KagemushaMintHashPlanV1,
    },
};
use halo2_base::{AssignedValue, gates::circuit::builder::BaseCircuitBuilder};
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};
use sha2::{Digest as _, Sha256};

const TEST_K: usize = 12;
const RELEASE: DigestV1 = [7; 32];

fn builder<F: KagemushaPoseidonFieldV1>() -> BaseCircuitBuilder<F> {
    BaseCircuitBuilder::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(1)
}

fn parity<F: KagemushaPoseidonFieldV1>() -> KagemushaPastaParityV1 {
    if F::IS_EQ_PARITY {
        KagemushaPastaParityV1::Eq
    } else {
        KagemushaPastaParityV1::Ep
    }
}

fn queue<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    messages: &[Vec<u8>],
    changed_output: bool,
) -> PastaSha256JobsV1<F> {
    let mut jobs = PastaSha256JobsV1::default();
    if changed_output {
        // Apply before job construction so every later range/Poseidon witness is regenerated
        // from the changed actual output word; the retained claim remains unchanged.
        jobs = jobs.with_output_word_xor(0, 0, 1);
    }
    let range = builder.range_chip();
    for message in messages {
        let bytes = assign_bytes(builder.main(0), &range, message);
        jobs.digest_constrained(builder.main(0), &bytes)
            .expect("ordinary queued job");
    }
    jobs
}

fn complete_state<F: KagemushaPoseidonFieldV1>(
    messages: &[Vec<u8>],
) -> KagemushaMintHashClaimStateV1 {
    let provisional =
        KagemushaMintHashPlanV1::from_messages(RELEASE, parity::<F>(), [1; 32], messages.to_vec())
            .expect("provisional ordered leaves");
    let plan = KagemushaMintHashClaimPlanV1::from_leaves::<F>(RELEASE, provisional.leaves())
        .expect("exact queue plan");
    let leaves = KagemushaMintHashPlanV1::from_messages(
        RELEASE,
        parity::<F>(),
        plan.plan_binding,
        messages.to_vec(),
    )
    .expect("plan-bound ordered leaves");
    assert_eq!(
        KagemushaMintHashClaimPlanV1::from_leaves::<F>(RELEASE, leaves.leaves()).unwrap(),
        plan
    );
    let mut state = None;
    for leaf in leaves.leaves() {
        state = Some(KagemushaMintHashClaimStateV1::apply::<F>(plan, state, leaf).unwrap());
    }
    let state = state.expect("nonempty queue");
    assert!(state.complete);
    state
}

fn metadata() -> KagemushaMintHashClaimMetadataV1 {
    KagemushaMintHashClaimMetadataV1 {
        eq_claim_protocol: encode(Fp::from(11)),
        ep_claim_protocol: encode(Fq::from(12)),
        eq_shard_protocol: encode(Fp::from(13)),
        ep_shard_protocol: encode(Fq::from(14)),
        eq_deferred_audit: encode(Fp::from(15)),
        ep_deferred_audit: encode(Fq::from(16)),
        eq_proof_chain_root: encode(Fp::from(17)),
        ep_proof_chain_root: encode(Fq::from(18)),
    }
}

fn complete_column<F: KagemushaPoseidonFieldV1>(messages: &[Vec<u8>]) -> Vec<F> {
    let state = KagemushaMintHashClaimPairStateV1 {
        eq: complete_state::<Fp>(messages),
        ep: complete_state::<Fq>(messages),
    };
    // This is only a claim projection for testing the byte/word consumer. The deliberately
    // unverified history and metadata below are never passed to a proof verifier or decider.
    claim_public_values_v1::<F>(
        parity::<F>(),
        &state,
        metadata(),
        &[0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    )
    .expect("complete projection shape")
}

fn digest_cells<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    std::array::from_fn(|index| {
        let value = u128::from_le_bytes(digest[index * 16..index * 16 + 16].try_into().unwrap());
        builder
            .main(0)
            .load_constant(halo2_base::utils::biguint_to_fe(&value.into()))
    })
}

fn constrain_column<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &PastaSha256JobsV1<F>,
    column: &[F],
) -> Result<(), String> {
    let release = digest_cells(builder, RELEASE);
    let metadata = metadata();
    let protocols = [
        metadata.eq_claim_protocol,
        metadata.ep_claim_protocol,
        metadata.eq_shard_protocol,
        metadata.ep_shard_protocol,
    ]
    .map(|digest| digest_cells(builder, digest));
    let assigned = column
        .iter()
        .map(|value| builder.main(0).load_witness(*value))
        .collect::<Vec<_>>();
    let range = builder.range_chip();
    constrain_complete_claim_against_sha_jobs_v1(
        builder.main(0),
        &range,
        jobs,
        &assigned,
        parity::<F>(),
        release,
        protocols[0],
        protocols[1],
        protocols[2],
        protocols[3],
    )?;
    builder.assigned_instances = vec![assigned];
    Ok(())
}

fn check<F: KagemushaPoseidonFieldV1>(
    messages: &[Vec<u8>],
    column: &[F],
    changed_output: bool,
) -> bool {
    let mut builder = builder::<F>();
    let jobs = queue(&mut builder, messages, changed_output);
    let inventory = TerminalShaInventoryV1::from_queue(&jobs).expect("ordinary inventory");
    assert_eq!(inventory.messages, messages);
    constrain_column(&mut builder, inventory.queue, column).expect("consumer graph");
    builder.calculate_params(Some(9));
    MockProver::run(TEST_K as u32, &builder, vec![column.to_vec()])
        .expect("Base-only claim consumer synthesis")
        .verify()
        .is_ok()
}

#[test]
fn terminal_sha_inventory_rejects_empty_and_bounded_queues_in_both_fields() {
    fn run<F: KagemushaPoseidonFieldV1>() {
        let mut builder = builder::<F>();
        let empty = PastaSha256JobsV1::<F>::default();
        assert!(TerminalShaInventoryV1::from_queue(&empty).is_err());
        let range = builder.range_chip();
        let bytes = assign_bytes(builder.main(0), &range, &[5; 56]);
        let length = builder.main(0).load_witness(F::from(56));
        let mut bounded = PastaSha256JobsV1::default();
        bounded
            .digest_bounded_constrained(builder.main(0), &range, &bytes, length)
            .unwrap();
        let error = match TerminalShaInventoryV1::from_queue(&bounded) {
            Ok(_) => panic!("bounded capacity messages cannot enter the ordinary inventory"),
            Err(error) => error,
        };
        assert!(error.contains("bounded padding"));
        let empty_message = queue(&mut builder, &[vec![]], false);
        let inventory = TerminalShaInventoryV1::from_queue(&empty_message).unwrap();
        assert_eq!(inventory.compression_blocks, 1);
        assert_eq!(inventory.jobs[0].message_bytes, 0);
    }
    run::<Fp>();
    run::<Fq>();
}

#[test]
fn terminal_sha_inventory_preserves_actual_prepared_transfer_and_claim_binding() {
    fn run<F: KagemushaPoseidonFieldV1>() {
        let fixture = crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(
            0x41, 9, 7, 11, 128, 128,
        );
        let output = &fixture.payment.output;
        let digests = [
            fixture.request.canonical_digest().unwrap(),
            output.sender_before_commitment,
            output.sender_after_commitment,
            output.transition_nullifier,
            fixture.request.recipient_encryption_key,
            output.ciphertext_commitment,
        ];
        let expected = iroha_data_model::kagemusha::kagemusha_prepared_transfer_digest_v1(
            &fixture.request,
            digests[1],
            digests[2],
            digests[3],
            digests[5],
        )
        .unwrap();
        let mut builder = builder::<F>();
        let range = builder.range_chip();
        let assigned = digests.map(|digest| assign_bytes(builder.main(0), &range, &digest));
        let amount = assign_bytes(
            builder.main(0),
            &range,
            &fixture.request.amount.to_le_bytes(),
        );
        let mut jobs = PastaSha256JobsV1::default();
        hash_terminal_prepared_transfer_v1(
            builder.main(0),
            &mut jobs,
            assigned.each_ref().map(Vec::as_slice),
            &amount,
        )
        .unwrap();
        let inventory = TerminalShaInventoryV1::from_queue(&jobs).unwrap();
        assert_eq!(inventory.jobs.len(), 1);
        let digest = inventory.jobs[0]
            .output_words
            .iter()
            .flat_map(|word| word.to_be_bytes())
            .collect::<Vec<_>>();
        assert_eq!(digest, expected);
        assert_eq!(digest, Sha256::digest(&inventory.messages[0]).as_slice());
        let column = complete_column::<F>(&inventory.messages);
        constrain_column(&mut builder, inventory.queue, &column).unwrap();
        builder.calculate_params(Some(9));
        MockProver::run(TEST_K as u32, &builder, vec![column.clone()])
            .unwrap()
            .assert_satisfied();

        let mut changed_builder = self::builder::<F>();
        let range = changed_builder.range_chip();
        let assigned = digests.map(|digest| assign_bytes(changed_builder.main(0), &range, &digest));
        let amount = assign_bytes(
            changed_builder.main(0),
            &range,
            &(fixture.request.amount + 1).to_le_bytes(),
        );
        let mut changed = PastaSha256JobsV1::default();
        hash_terminal_prepared_transfer_v1(
            changed_builder.main(0),
            &mut changed,
            assigned.each_ref().map(Vec::as_slice),
            &amount,
        )
        .unwrap();
        constrain_column(&mut changed_builder, &changed, &column).unwrap();
        changed_builder.calculate_params(Some(9));
        assert!(
            MockProver::run(TEST_K as u32, &changed_builder, vec![column])
                .unwrap()
                .verify()
                .is_err()
        );
    }
    run::<Fp>();
    run::<Fq>();
}

#[test]
fn terminal_sha_complete_claim_rejects_changed_queue_and_output_in_both_fields() {
    fn run<F: KagemushaPoseidonFieldV1>() {
        // Distinct equal-size jobs exercise ordering without a geometry change; the third
        // crosses the 55/56-byte padding boundary from one block to two blocks.
        let messages = vec![vec![0x31; 55], vec![0x72; 55], vec![0xa4; 56]];
        let mut inventory_builder = builder::<F>();
        let inventory_jobs = queue(&mut inventory_builder, &messages, false);
        let inventory = TerminalShaInventoryV1::from_queue(&inventory_jobs).unwrap();
        assert_eq!(inventory.compression_blocks, 4);
        assert_eq!(
            inventory
                .jobs
                .iter()
                .map(|job| job.compression_blocks)
                .collect::<Vec<_>>(),
            [1, 1, 2],
        );
        let column = complete_column::<F>(&messages);
        assert!(check(&messages, &column, false));
        let mut changed = messages.clone();
        changed[0][17] ^= 1;
        assert!(
            !check(&changed, &column, false),
            "changed byte with regenerated output"
        );
        let mut reordered = messages.clone();
        reordered.swap(0, 1);
        assert!(!check(&reordered, &column, false), "same-shape job swap");
        let mut duplicated = messages.clone();
        duplicated[1] = duplicated[0].clone();
        assert!(!check(&duplicated, &column, false), "same-shape duplicate");
        assert!(!check(&messages[..2], &column, false), "missing final job");
        let mut appended = messages.clone();
        appended.push(messages[0].clone());
        assert!(!check(&appended, &column, false), "extra job");
        let mut boundary = messages.clone();
        boundary[2].pop();
        assert!(
            !check(&boundary, &column, false),
            "changed canonical padding boundary"
        );
        assert!(
            !check(&messages, &column, true),
            "changed output with regenerated Poseidon witness"
        );
    }
    run::<Fp>();
    run::<Fq>();
}

#[test]
fn terminal_sha_complete_claim_pins_identity_and_complete_cursor_in_both_fields() {
    fn run<F: KagemushaPoseidonFieldV1>() {
        let messages = vec![vec![0x31; 3]];
        let original = complete_column::<F>(&messages);
        assert!(check(&messages, &original, false));
        for offset in [
            claim_cell::VERSION,
            claim_cell::PARITY,
            claim_cell::COMPLETE,
            claim_cell::RELEASE_LO,
            claim_cell::RELEASE_LO + 1,
            claim_cell::EQ_CLAIM_PROTOCOL_LO,
            claim_cell::EP_CLAIM_PROTOCOL_LO,
            claim_cell::EQ_SHARD_PROTOCOL_LO,
            claim_cell::EP_SHARD_PROTOCOL_LO,
            claim_cell::TOTAL_STAGES,
            claim_cell::TOTAL_JOBS,
            claim_cell::NEXT_STAGE,
            claim_cell::NEXT_JOB,
            claim_cell::NEXT_BLOCK,
            claim_cell::ACTIVE_JOB_BLOCKS,
            claim_cell::EQ_CHAINING_STATE,
            claim_cell::EP_CHAINING_STATE,
        ] {
            let mut altered = original.clone();
            altered[offset] += F::ONE;
            assert!(!check(&messages, &altered, false), "claim cell {offset}");
        }
    }
    run::<Fp>();
    run::<Fq>();
}

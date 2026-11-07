//! A frame whose slot the witness chooses (`ChainSide::choices`): a Q step that
//! may fold the A instance before it into either of two kept accumulators
//! (as a step that follows several kinds does). The chain `Q1 A2 Q3 A4 Q5 A6`
//! folds A2 into slot 0 and A4 into slot 1; both accumulators decide. A witness
//! that names one slot and opens the other slot's accumulator is unsatisfiable.

use ff::Field;
use iroha_fold::{
    CommitKey, FoldedRelation,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    decide::{decide, decide_fresh},
    fold::fresh_instance,
    ivc::{ChainSide, Fresh, FrameWitness, Kept, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, synthesize},
    pcs::ipa::PinnedParams,
};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

mod common;

use common::step::{BASE, DIGESTS, DOMAIN_A, DOMAIN_Q, IVC_Q, K, Side, Step, business, business_digests, start_passthrough};

/// Q's digest list here: two accumulators of A instances, then its claim slot.
const Q_DIGESTS: usize = 3;

fn q_chain(slot: usize) -> ChainSide {
    ChainSide { ivc_domain: IVC_Q, fold_domain: DOMAIN_A, first_step: 1, slot, digests: Q_DIGESTS, choices: 0b11 }
}

/// The toy Q business (double and add one) on a frame that chooses its slot.
#[derive(Clone)]
struct SelectStep {
    shape: FoldShape,
    witness: Option<FrameWitness<Eq>>,
}

#[derive(Clone, Debug)]
struct SelectConfig {
    fold: FoldVerifierConfig<Eq>,
    instance: Column<InstanceColumn>,
}

impl Circuit<Fq> for SelectStep {
    type Config = SelectConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { shape: self.shape.clone(), witness: None }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> SelectConfig {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        SelectConfig { fold, instance }
    }

    fn synthesize(&self, config: SelectConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let chain = q_chain(0);
        let outputs = layouter.assign_region(
            || "select step",
            |mut region| {
                let frame = fold_frame(
                    &mut chips.lanes(),
                    &mut region,
                    &chain,
                    &self.shape,
                    Fq::from(BASE),
                    start_passthrough(),
                    1,
                    self.witness.as_ref(),
                )?;
                let later = chips.glue.not(&mut region, &frame.first)?;
                assert_equal_unless(&mut chips.lanes(), &mut region, &frame.state[0], &frame.base, Some(&later))?;
                let next = chips.glue.linear(&mut region, &[(Fq::from(2), &frame.state[0])], Fq::ONE)?;
                frame_outputs(&mut chips.lanes(), &mut region, &chain, &frame, &[next], &frame.digests, &[frame.step.clone()])
            },
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, row)?;
        }
        Ok(())
    }
}

struct Relations {
    q: FoldedRelation<Fq>,
    key_q: CommitKey<Ep>,
    a: FoldedRelation<Fp>,
    key_a: CommitKey<Eq>,
}

fn relations() -> Relations {
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![3],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let a_relation = |shape: FoldShape| {
        let layout = synthesize(&Step::<Ep> { side: Side::A, shape, witness: None }, K, None).expect("A layout");
        FoldedRelation::<Fp>::from_synthesized(&layout).expect("A relation")
    };
    let first = a_relation(placeholder).shape();
    let layout = synthesize(&SelectStep { shape: first.clone(), witness: None }, K, None).expect("Q layout");
    let q = FoldedRelation::<Fq>::from_synthesized(&layout).expect("Q relation");
    let a = a_relation(q.shape());
    assert_eq!(a.shape(), first);
    Relations {
        key_q: CommitKey::lagrange(PinnedParams::<Ep>::derive(K).expect("params").params()),
        key_a: CommitKey::lagrange(PinnedParams::<Eq>::derive(K).expect("params").params()),
        q,
        a,
    }
}

fn satisfied<F: iroha_pasta::PastaField>(circuit: &impl Circuit<F, Params = ()>, public: &[Vec<F>]) -> bool {
    check_circuit(circuit, K, public, CheckMode::Strict).expect("synthesis").is_satisfied()
}

#[test]
#[ignore = "k16 IVC steps; run optimized"]
fn a_selected_slot_folds_into_the_accumulator_the_witness_names() {
    let rel = relations();
    let mut rng = ChaCha20Rng::seed_from_u64(57);
    let (mut q_state, mut a_state) = (Fq::from(BASE), Fp::from(BASE));
    let mut q_digests = vec![Fq::ZERO; Q_DIGESTS];
    let mut a_digests = vec![Fp::ZERO; DIGESTS];
    let mut q_kept: [Option<Kept<Eq>>; 2] = [None, None];
    let mut a_kept: Option<Kept<Ep>> = None;
    let mut last_a: Option<Fresh<Eq>> = None;
    let mut last_q: Option<Fresh<Ep>> = None;
    let a_chain = Side::A.chain();
    // The slot each Q step folds into (step 1 has no predecessor).
    let slots = [0, 0, 0, 0, 1, 1, 0];
    for j in 1..=7u64 {
        if j % 2 == 1 {
            let slot = slots[j as usize - 1];
            let chain = q_chain(slot);
            let prepared = prepare_step(
                &chain,
                j,
                &rel.a,
                q_kept[slot].as_ref(),
                last_a.as_ref(),
                vec![q_state],
                q_digests.clone(),
                start_passthrough(),
            )
            .expect("prepare");
            let next = business(Side::Q, q_state, prepared.bridge[0]);
            let public = vec![step_public(&chain, j, Fq::from(BASE), prepared.passthrough, &[next], &prepared.digests, &[Fq::from(j)])];
            let circuit = SelectStep { shape: rel.a.shape(), witness: Some(prepared.witness.clone()) };
            assert!(satisfied(&circuit, &public), "Q step {j} into slot {slot}");
            if j == 7 {
                // Naming slot 0 while opening slot 1's accumulator: the opened
                // digest is not the one the list holds in the named slot.
                let mut wrong = prepared.witness.clone();
                wrong.acc = q_kept[1].as_ref().expect("slot 1 kept").0.clone();
                let circuit = SelectStep { shape: rel.a.shape(), witness: Some(wrong) };
                assert!(!satisfied(&circuit, &public), "a witness opened another slot's accumulator");
                break;
            }
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.q.stage_witness(&synthesized).expect("stage");
            last_q = Some(fresh_instance(&rel.q, &rel.key_q, &DOMAIN_Q, advice, public, &mut rng).expect("fresh"));
            if last_a.is_some() {
                q_kept[slot] = prepared.kept;
            }
            q_digests = prepared.digests;
            q_state = next;
        } else {
            let prepared = prepare_step(
                &a_chain,
                j,
                &rel.q,
                a_kept.as_ref(),
                last_q.as_ref(),
                vec![a_state],
                a_digests.clone(),
                Fp::ZERO,
            )
            .expect("prepare");
            let next = business(Side::A, a_state, prepared.bridge[0]);
            let digests = business_digests(Side::A, j == 2, prepared.digests.clone());
            let public = vec![step_public(&a_chain, j, Fp::from(BASE), prepared.passthrough, &[next], &digests, &[Fp::from(j)])];
            let circuit = Step::<Ep> { side: Side::A, shape: rel.q.shape(), witness: Some(prepared.witness) };
            assert!(satisfied(&circuit, &public), "A step {j}");
            let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
            let (advice, public) = rel.a.stage_witness(&synthesized).expect("stage");
            last_a = Some(fresh_instance(&rel.a, &rel.key_a, &DOMAIN_A, advice, public, &mut rng).expect("fresh"));
            a_kept = prepared.kept;
            a_digests = digests;
            a_state = next;
        }
    }
    // Slot 0 holds A2, slot 1 holds A4; each decides, and so does A's
    // accumulator of Q instances and the last A instance.
    for (slot, kept) in q_kept.iter().enumerate() {
        let (acc, witness) = kept.as_ref().expect("both slots kept");
        assert_eq!(q_digests[slot], acc.digest(&DOMAIN_A));
        decide(&rel.a, &rel.key_a, acc, witness).expect("a selected slot's accumulator decides");
    }
    let (acc, witness) = a_kept.as_ref().expect("A keeps Q instances");
    decide(&rel.q, &rel.key_q, acc, witness).expect("Q instances decide");
    let (last, witness) = last_a.as_ref().expect("last A");
    decide_fresh(&rel.a, &rel.key_a, &DOMAIN_A, last, witness).expect("the last A decides fresh");
    // A witness choosing a slot outside the circuit's choices does not lay out.
    let prepared =
        prepare_step(&q_chain(2).folding_into(0), 3, &rel.a, None, None, vec![q_state], vec![Fq::ZERO; Q_DIGESTS], start_passthrough())
            .expect("prepare");
    let mut outside = prepared.witness;
    outside.slot = 2;
    assert!(synthesize(&SelectStep { shape: rel.a.shape(), witness: Some(outside) }, K, None).is_err());
}

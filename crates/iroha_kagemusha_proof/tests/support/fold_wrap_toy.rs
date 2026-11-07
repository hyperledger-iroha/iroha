//! A two-kind toy chain for the compression wraps, the wraps' sizes for any
//! spec ([`sizes`]), and an Omega over the toy chain for other tests
//! ([`omega_fixture`]).
//!
//! Each side has two relation kinds, so R, P and Omega decide batches. The
//! schedule repeats `Q_a, A_a, Q_b, A_b` (`Q_a` folds `A_b`, `Q_b` folds
//! `A_a`, `A_a` folds `Q_a`, `A_b` folds `Q_b`), and each side keeps its claim
//! accumulator from iroha_fold's fixtures.
//!
//! The A state has today's 20 words: the 18 lineage fields, the step context
//! ([`A_CONTEXT`]) and the program counter ([`A_PC`]). The chain starts from
//! the lineage `[BASE, 0, ..]` with both zero. `A_a` keeps the lineage, adds
//! the square of Q's bridge to the context and counts the program counter.
//! `A_b` closes the operation: it writes the successor lineage (a free
//! witness in the toy) and zeros the context and the program counter, so a
//! chain that ends with `A_b` ends at an operation boundary.
//!
//! Like the real A kinds, every A kind has a second public column that only
//! one kind fills: `A_a` exports its new context and program counter there,
//! `A_b` zeros.
//!
//! Include it with `#[path = "support/fold_wrap_toy.rs"] mod fold_wrap_toy;`
//! (it includes iroha_fold's test fixtures itself). The relations and keys
//! are made once per test process ([`toy`]); heavy tests hold [`serial`] so
//! they neither run together nor mix the verifier usage diagnostics.

#![allow(dead_code)]

#[path = "../../../iroha_fold/tests/common/mod.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
mod fold_common;

use std::{
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
    time::Instant,
};

use ff::Field;
use fold_common::step::{CLAIM_A, CLAIM_Q, claim_a, claim_q};
use iroha_fold::{
    CommitKey, FoldedRelation,
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    fold::fresh_instance,
    ivc::{ChainSide, FrameWitness, Fresh, Kept, embed, ivc_hash, prepare_step, step_public},
    relation::FoldShape,
};
use iroha_kagemusha_proof::{
    a_relation::LINEAGE_FIELDS,
    fold_wrap::{
        Domains, WrapSpec,
        native::{self, FinalChain, WrapKeys, WrapTapes},
        omega::{OmegaCircuit, OmegaPlan},
        p::{PCircuit, PPlan},
        r::{RCircuit, RPlan},
    },
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, PastaField};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::{Column, ConstraintSystem, Instance as InstanceColumn, InstanceType},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value, synthesize},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::Word;
use iroha_plonk_recursion::{
    ACCUMULATOR_BYTES, FoldConfig, FoldTape,
    verifier::{VerifierUsage, diagnostic_take_usage},
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

/// k of every toy relation and of the wraps.
pub const K: u32 = 16;
const BASE: u64 = 0x5eed;
const FOLD_Q: [u8; 32] = *b"fold wrap toy: Q kinds' domain..";
const FOLD_A: [u8; 32] = *b"fold wrap toy: A kinds' domain..";
const IVC_Q: [u8; 32] = *b"fold wrap toy: Q side IVC domain";
const IVC_A: [u8; 32] = *b"fold wrap toy: A side IVC domain";
/// Kinds per side; a side's digest list is its kept accumulators, then its
/// claim accumulator.
const KINDS: usize = 2;
const DIGESTS: usize = KINDS + 1;
const CLAIM: usize = KINDS;
/// Words of the A state: the lineage fields, the step context, the program
/// counter.
pub const A_STATE: usize = LINEAGE_FIELDS + 2;
/// The step context's word in the A state.
pub const A_CONTEXT: usize = LINEAGE_FIELDS;
/// The program counter's word in the A state.
pub const A_PC: usize = LINEAGE_FIELDS + 1;
/// `A_a`'s index among the A kinds.
pub const A_A: usize = 0;
/// `A_b`'s index among the A kinds: the kind every operation ends with.
pub const A_B: usize = 1;
/// The frame column's words: passthrough, `h_out`, one bridge value.
const FRAME: usize = 3;
/// Words of the A kinds' second public column.
const A_EXTRA: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Q,
    A,
}

/// One relation kind: its side and its index in that side's kind list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kind {
    side: Side,
    index: usize,
}

const QA: Kind = Kind {
    side: Side::Q,
    index: 0,
};
const QB: Kind = Kind {
    side: Side::Q,
    index: 1,
};
const AA: Kind = Kind {
    side: Side::A,
    index: A_A,
};
const AB: Kind = Kind {
    side: Side::A,
    index: A_B,
};
const SCHEDULE: [Kind; 4] = [QA, AA, QB, AB];

impl Kind {
    /// The other side's kind this kind folds (its slot in this side's list).
    const fn folds(self) -> usize {
        match (self.side, self.index) {
            (Side::Q, 0) => 1,
            (Side::Q, _) => 0,
            (Side::A, index) => index,
        }
    }

    const fn chain(self) -> ChainSide {
        let slot = self.folds();
        let (ivc_domain, fold_domain, first_step) = match self.side {
            Side::Q => (IVC_Q, FOLD_A, 1),
            Side::A => (IVC_A, FOLD_Q, 2),
        };
        ChainSide {
            ivc_domain,
            fold_domain,
            first_step,
            slot,
            digests: DIGESTS,
            choices: 1 << slot,
        }
    }

    const fn state_len(self) -> usize {
        match self.side {
            Side::Q => 0,
            Side::A => A_STATE,
        }
    }

    /// The public columns: the frame column, then A's second column.
    fn instance_lengths(self) -> Vec<usize> {
        match self.side {
            Side::Q => vec![FRAME],
            Side::A => vec![FRAME, A_EXTRA],
        }
    }
}

/// The chain's first A state: the lineage `[BASE, 0, ..]`, a zero context and
/// program counter.
fn a_base() -> Vec<Fp> {
    let mut base = vec![Fp::ZERO; A_STATE];
    base[0] = Fp::from(BASE);
    base
}

/// The passthrough of the chain's first instance: A's base hash.
fn start_passthrough<F: PastaField>() -> F {
    embed(&ivc_hash::<Fp>(
        &IVC_A,
        0,
        Fp::from(BASE),
        &a_base(),
        &[Fp::ZERO; DIGESTS],
    ))
}

/// A side's claim-accumulator digest over the side's field.
fn claim_digest<F: PastaField>(side: Side) -> F {
    match side {
        Side::Q => embed(&claim_q().0.digest(&CLAIM_Q)),
        Side::A => embed(&claim_a().0.digest(&CLAIM_A)),
    }
}

/// A toy step of one kind: the fold frame, the claim accumulator started on
/// the side's first instance, the toy business of the module docs, and the
/// frame outputs.
struct KindStep<C: PastaCurve> {
    kind: Kind,
    shape: FoldShape,
    witness: Option<FrameWitness<C>>,
    /// The successor lineage `A_b` writes.
    successor: Option<[C::Base; LINEAGE_FIELDS]>,
}

impl<C: PastaCurve> Circuit<C::Base> for KindStep<C> {
    type Config = (FoldVerifierConfig<C>, Vec<Column<InstanceColumn>>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = Vec<usize>;

    fn params(&self) -> Vec<usize> {
        self.kind.instance_lengths()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            kind: self.kind,
            shape: self.shape.clone(),
            witness: None,
            successor: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self::Config {
        Self::configure_with_params(meta, vec![FRAME])
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<C::Base>,
        lengths: Vec<usize>,
    ) -> Self::Config {
        let fold = FoldVerifierConfig::configure(meta);
        let columns = lengths
            .into_iter()
            .map(|length| {
                let column = meta.instance_column(length);
                meta.enable_equality(column);
                column
            })
            .collect();
        (fold, columns)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.0);
        chips.load_tables(&mut layouter)?;
        let columns =
            layouter.assign_region(|| "step", |mut region| self.layout(&mut chips, &mut region))?;
        if columns.len() != config.1.len() {
            return Err(Error::Synthesis);
        }
        for (words, column) in columns.iter().zip(&config.1) {
            for (row, word) in words.iter().enumerate() {
                layouter.constrain_instance(word.cell(), *column, row)?;
            }
        }
        Ok(())
    }
}

impl<C: PastaCurve> KindStep<C> {
    /// The step's public columns: the frame outputs, then A's second column.
    fn layout(
        &self,
        chips: &mut FoldVerifierChips<C>,
        region: &mut Region<'_, C::Base>,
    ) -> Result<Vec<Vec<Word<C::Base>>>, Error> {
        let chain = self.kind.chain();
        let frame = fold_frame(
            &mut chips.lanes(),
            region,
            &chain,
            &self.shape,
            C::Base::from(BASE),
            start_passthrough(),
            self.kind.state_len(),
            self.witness.as_ref(),
        )?;
        let started = chips.glue.constant(region, claim_digest(self.kind.side))?;
        let mut digests = frame.digests.clone();
        digests[CLAIM] =
            chips
                .glue
                .select(region, &frame.first, &started, &frame.digests[CLAIM])?;
        let (next, extra) = match self.kind.side {
            Side::Q => (Vec::new(), None),
            Side::A => {
                // The side's first instance starts from a_base().
                let later = chips.glue.not(region, &frame.first)?;
                let zero = chips.glue.constant(region, <C::Base as Field>::ZERO)?;
                assert_equal_unless(
                    &mut chips.lanes(),
                    region,
                    &frame.state[0],
                    &frame.base,
                    Some(&later),
                )?;
                for word in &frame.state[1..] {
                    assert_equal_unless(&mut chips.lanes(), region, word, &zero, Some(&later))?;
                }
                if self.kind.index == A_A {
                    let square = chips.glue.mul(region, &frame.bridge[0], &frame.bridge[0])?;
                    let context = chips.glue.add(region, &frame.state[A_CONTEXT], &square)?;
                    let one = chips.glue.constant(region, <C::Base as Field>::ONE)?;
                    let pc = chips.glue.add(region, &frame.state[A_PC], &one)?;
                    let mut next = frame.state[..LINEAGE_FIELDS].to_vec();
                    next.extend([context.clone(), pc.clone()]);
                    (next, Some(vec![context, pc]))
                } else {
                    let values: Vec<_> = (0..LINEAGE_FIELDS)
                        .map(|i| {
                            self.successor
                                .map_or_else(Value::unknown, |s| Value::known(s[i]))
                        })
                        .collect();
                    let mut next = chips.glue.witnesses(region, &values)?;
                    next.extend([zero.clone(), zero.clone()]);
                    (next, Some(vec![zero; A_EXTRA]))
                }
            }
        };
        let outputs = frame_outputs(
            &mut chips.lanes(),
            region,
            &chain,
            &frame,
            &next,
            &digests,
            &[frame.step.clone()],
        )?;
        Ok(core::iter::once(outputs).chain(extra).collect())
    }
}

/// The toy relations of both sides, under the monomial generators (the
/// multilinear decider's basis).
pub struct Relations {
    /// `Q_a`, `Q_b`.
    pub q: Vec<FoldedRelation<Fq>>,
    /// `A_a`, `A_b`.
    pub a: Vec<FoldedRelation<Fp>>,
    /// Q's commitment key.
    pub key_q: CommitKey<Ep>,
    /// A's commitment key.
    pub key_a: CommitKey<Eq>,
    /// k16 Pallas parameters.
    pub params_q: PinnedParams<Ep>,
    /// k16 Vesta parameters.
    pub params_a: PinnedParams<Eq>,
}

fn relation_q(kind: Kind, shape: FoldShape) -> FoldedRelation<Fq> {
    let step = KindStep::<Eq> {
        kind,
        shape,
        witness: None,
        successor: None,
    };
    FoldedRelation::from_synthesized(&synthesize(&step, K, None).expect("Q layout"))
        .expect("Q relation")
}

fn relation_a(kind: Kind, shape: FoldShape) -> FoldedRelation<Fp> {
    let step = KindStep::<Ep> {
        kind,
        shape,
        witness: None,
        successor: None,
    };
    FoldedRelation::from_synthesized(&synthesize(&step, K, None).expect("A layout"))
        .expect("A relation")
}

/// Each kind's circuit embeds the shape of the kind it folds; the shapes
/// form a cycle, so iterate until they repeat.
pub fn relations() -> Relations {
    let placeholder = FoldShape {
        columns: 8,
        advice_columns: 8,
        groups: Vec::new(),
        instance_lengths: vec![3],
        theta_len: 0,
        index_bits: 18,
        degree: 5,
    };
    let mut q_shapes = vec![placeholder.clone(); KINDS];
    let mut a_shapes = vec![placeholder; KINDS];
    let mut found = None;
    for _ in 0..5 {
        let q: Vec<_> = [QA, QB]
            .iter()
            .map(|k| relation_q(*k, a_shapes[k.folds()].clone()))
            .collect();
        let a: Vec<_> = [AA, AB]
            .iter()
            .map(|k| relation_a(*k, q_shapes[k.folds()].clone()))
            .collect();
        let (next_q, next_a): (Vec<_>, Vec<_>) = (
            q.iter().map(FoldedRelation::shape).collect(),
            a.iter().map(FoldedRelation::shape).collect(),
        );
        let stable = next_q == q_shapes && next_a == a_shapes;
        q_shapes = next_q;
        a_shapes = next_a;
        if stable {
            found = Some((q, a));
            break;
        }
    }
    let (q, a) = found.expect("the kinds' shapes reach a fixed point");
    let params_q = PinnedParams::<Ep>::derive(K).expect("params");
    let params_a = PinnedParams::<Eq>::derive(K).expect("params");
    Relations {
        key_q: CommitKey::monomial(params_q.params()),
        key_a: CommitKey::monomial(params_a.params()),
        q,
        a,
        params_q,
        params_a,
    }
}

/// The wraps' spec for the toy chain: `A_n` is an `A_b`, `D_A` hashes the
/// lineage fields, and the context and program counter are zero at an
/// operation boundary.
#[must_use]
pub fn spec(rel: &Relations) -> Arc<WrapSpec> {
    Arc::new(WrapSpec {
        q: rel.q.clone(),
        a: rel.a.clone(),
        last: A_B,
        key_q: rel.key_q.clone(),
        key_a: rel.key_a.clone(),
        params_q: rel.params_q.clone(),
        params_a: rel.params_a.clone(),
        domains: Domains {
            q_fold: FOLD_Q,
            a_fold: FOLD_A,
            a_ivc: IVC_A,
            q_ivc: IVC_Q,
            pallas_claims: CLAIM_A,
            vesta_claims: CLAIM_Q,
        },
        base_a: Fp::from(BASE),
        base_q: Fq::from(BASE),
        a_state: A_STATE,
        q_state: 0,
        lineage: core::array::from_fn(|i| i),
        boundary: vec![A_CONTEXT, A_PC],
    })
}

/// The toy relations and the wraps' keys over them.
pub struct Toy {
    /// The relations.
    pub relations: Relations,
    /// R's, P's and Omega's plans and keys.
    pub keys: WrapKeys,
}

/// The toy relations and keys, made once per test process.
pub fn toy() -> &'static Toy {
    static TOY: OnceLock<Toy> = OnceLock::new();
    TOY.get_or_init(|| {
        let started = Instant::now();
        let relations = relations();
        println!("toy relations: {:.1?}", started.elapsed());
        let now = Instant::now();
        let keys = native::keygen(spec(&relations)).expect("toy wrap keys");
        println!(
            "keygen R, P and Omega: {:.1?}; R has {} public rows, P {}",
            now.elapsed(),
            keys.r_plan.layout.rows(),
            keys.p_plan.layout.rows()
        );
        Toy { relations, keys }
    })
}

/// Holds off other heavy tests of the process (memory, and the verifier
/// usage diagnostics are process-wide).
pub fn serial() -> MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `steps` half-steps of the schedule (ending with `A_b`); every `A_b`
/// writes `successor` as the lineage.
#[must_use]
pub fn chain(rel: &Relations, steps: u64, successor: &[Fp; LINEAGE_FIELDS]) -> FinalChain {
    let mut rng = ChaCha20Rng::seed_from_u64(91);
    let mut q_kept: Vec<Option<Kept<Eq>>> = vec![None; KINDS];
    let mut a_kept: Vec<Option<Kept<Ep>>> = vec![None; KINDS];
    let (mut q_digests, mut a_digests) = (vec![Fq::ZERO; DIGESTS], vec![Fp::ZERO; DIGESTS]);
    let mut a_state = a_base();
    let (mut last_a, mut last_q): (Option<Fresh<Eq>>, Option<Fresh<Ep>>) = (None, None);
    for j in 1..=steps {
        let kind = SCHEDULE[usize::try_from((j - 1) % 4).expect("small")];
        let chain = kind.chain();
        let slot = kind.folds();
        match kind.side {
            Side::Q => {
                let prepared = prepare_step(
                    &chain,
                    j,
                    &rel.a[slot],
                    q_kept[slot].as_ref(),
                    last_a.as_ref(),
                    Vec::new(),
                    q_digests.clone(),
                    start_passthrough(),
                )
                .expect("prepare");
                q_digests.clone_from(&prepared.digests);
                if j == chain.first_step {
                    q_digests[CLAIM] = claim_digest(Side::Q);
                }
                let public = step_public(
                    &chain,
                    j,
                    Fq::from(BASE),
                    prepared.passthrough,
                    &[],
                    &q_digests,
                    &[Fq::from(j)],
                );
                let circuit = KindStep::<Eq> {
                    kind,
                    shape: rel.a[slot].shape(),
                    witness: Some(prepared.witness),
                    successor: None,
                };
                let synthesized = synthesize(&circuit, K, Some(&[public])).expect("witness");
                let relation = &rel.q[kind.index];
                let (advice, public) = relation.stage_witness(&synthesized).expect("stage");
                last_q = Some(
                    fresh_instance(relation, &rel.key_q, &FOLD_Q, advice, public, &mut rng)
                        .expect("fresh"),
                );
                q_kept[slot] = prepared.kept;
            }
            Side::A => {
                let prepared = prepare_step(
                    &chain,
                    j,
                    &rel.q[slot],
                    a_kept[slot].as_ref(),
                    last_q.as_ref(),
                    a_state.clone(),
                    a_digests.clone(),
                    Fp::ZERO,
                )
                .expect("prepare");
                a_digests.clone_from(&prepared.digests);
                if j == chain.first_step {
                    a_digests[CLAIM] = claim_digest(Side::A);
                }
                let (next, extra) = if kind.index == A_A {
                    let mut next = a_state.clone();
                    next[A_CONTEXT] += prepared.bridge[0].square();
                    next[A_PC] += Fp::ONE;
                    let extra = vec![next[A_CONTEXT], next[A_PC]];
                    (next, extra)
                } else {
                    let mut next = successor.to_vec();
                    next.extend([Fp::ZERO, Fp::ZERO]);
                    (next, vec![Fp::ZERO; A_EXTRA])
                };
                let public = vec![
                    step_public(
                        &chain,
                        j,
                        Fp::from(BASE),
                        prepared.passthrough,
                        &next,
                        &a_digests,
                        &[Fp::from(j)],
                    ),
                    extra,
                ];
                let circuit = KindStep::<Ep> {
                    kind,
                    shape: rel.q[slot].shape(),
                    witness: Some(prepared.witness),
                    successor: Some(*successor),
                };
                let synthesized = synthesize(&circuit, K, Some(&public)).expect("witness");
                let relation = &rel.a[kind.index];
                let (advice, public) = relation.stage_witness(&synthesized).expect("stage");
                last_a = Some(
                    fresh_instance(relation, &rel.key_a, &FOLD_A, advice, public, &mut rng)
                        .expect("fresh"),
                );
                a_kept[slot] = prepared.kept;
                a_state = next;
            }
        }
    }
    FinalChain {
        step: steps,
        a_state,
        q_state: Vec::new(),
        u_q: a_kept
            .into_iter()
            .map(|kept| kept.expect("A keeps every Q kind"))
            .collect(),
        u_a: q_kept,
        a_n: last_a.expect("A_n"),
        pallas_claims: claim_a().clone(),
        vesta_claims: claim_q().clone(),
    }
}

/// A deterministic test tape (SplitMix64 words; production samples OS coins).
fn tape<C: PastaCurve>(seed: u64) -> FoldTape<C> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    FoldTape::<C>::try_fill::<core::convert::Infallible>(|coin| {
        *coin = C::ScalarExt::from_raw_reduced([next(), next(), next(), next() >> 2]);
        Ok(())
    })
    .expect("infallible")
}

/// Deterministic restructuring and fold tapes for the wraps.
#[must_use]
pub fn tapes(seed: u64) -> WrapTapes {
    WrapTapes {
        pallas_restructure: tape(seed),
        pallas_fold: tape(seed + 1),
        vesta_restructure: tape(seed + 2),
        vesta_fold: tape(seed + 3),
    }
}

/// Prints the verifier lanes' rows recorded since the last call and returns
/// the last record (the circuit synthesized last).
pub fn take_rows(label: &str) -> Option<VerifierUsage> {
    let entries = diagnostic_take_usage();
    for entry in &entries {
        println!(
            "{label} rows: sponge {}, glue {}, ECC {}, range {}",
            entry.sponge, entry.arithmetic, entry.curve, entry.range
        );
    }
    entries.last().copied()
}

/// The rows one wrap uses, from a synthesis without witnesses.
#[derive(Clone, Copy, Debug)]
pub struct Size {
    /// R, P or Omega.
    pub wrap: &'static str,
    /// Whether the wrap fits k16; otherwise its rows are counted at k17.
    pub fits: bool,
    /// Its verifier lanes' rows.
    pub usage: VerifierUsage,
}

/// The rows R, P and Omega use for `spec`, without a chain or proofs: each
/// wrap is synthesized without witnesses at k16, or at k17 when it overflows
/// k16. The next wrap verifies the previous one's k16 key, so sizing stops
/// after a wrap that overflows.
///
/// # Panics
/// When `spec` admits no plan or a wrap overflows k17.
#[must_use]
pub fn sizes(spec: &Arc<WrapSpec>) -> Vec<Size> {
    let field = || KeygenConfigV2::pipa_r(vec![InstanceType::Field]);
    let mut sizes = Vec::new();
    let r_plan = Arc::new(RPlan::new(Arc::clone(spec)).expect("R plan"));
    let r = RCircuit {
        plan: Arc::clone(&r_plan),
        witness: None,
    };
    sizes.push(size("R", &r));
    if !sizes[0].fits {
        return sizes;
    }
    let r_key = keygen_pk_v2(&spec.params_q, &r, &field()).expect("R key");
    let p_plan =
        Arc::new(PPlan::new(r_plan, r_key.binding().clone(), r_key.vk().clone()).expect("P plan"));
    let p = PCircuit {
        plan: Arc::clone(&p_plan),
        witness: None,
    };
    sizes.push(size("P", &p));
    if !sizes[1].fits {
        return sizes;
    }
    let p_key = keygen_pk_v2(&spec.params_a, &p, &field()).expect("P key");
    let omega_plan =
        OmegaPlan::new(p_plan, p_key.binding().clone(), p_key.vk().clone()).expect("Omega plan");
    let omega = OmegaCircuit {
        plan: Arc::new(omega_plan),
        witness: None,
    };
    sizes.push(size("Omega", &omega));
    sizes
}

fn size<F: PastaField, C: Circuit<F>>(wrap: &'static str, circuit: &C) -> Size {
    let _ = diagnostic_take_usage();
    let fits = synthesize(circuit, K, None).is_ok();
    if !fits {
        let _ = diagnostic_take_usage();
        synthesize(circuit, K + 1, None).expect("the wrap fits k17");
    }
    let usage = diagnostic_take_usage()
        .pop()
        .expect("the wrap's verifier lanes report their rows");
    Size { wrap, fits, usage }
}

/// An Omega over the toy chain of eight half-steps whose last `A_b` writes
/// the chosen lineage, so `D_A` hashes it. Omega is a k16 Pallas proof
/// (parameters `PinnedParams::<Ep>::derive(16)`).
pub struct OmegaFixture {
    /// Omega's descriptor binding.
    pub binding: DescriptorBinding,
    /// Omega's verifying key.
    pub vk: VerifyingKey<Ep>,
    /// Omega's proof.
    pub proof: Vec<u8>,
    /// Omega's public columns `[D_A]`, `acc_V.G` and `acc_V.u`: 1 + 2 + 16
    /// values.
    pub instances: Vec<Vec<Fq>>,
    /// `acc_P`'s canonical bytes (bound in `D_A`).
    pub acc_p: [u8; ACCUMULATOR_BYTES],
    /// `acc_V`'s canonical bytes.
    pub acc_v: [u8; ACCUMULATOR_BYTES],
    /// The final A state: the lineage, then a zero context and program
    /// counter.
    pub z_a: Vec<Fp>,
    /// `D_A`.
    pub d_a: Fp,
}

/// Proves an Omega over the toy chain whose final lineage is `lineage`
/// (about a minute once [`toy`]'s relations and keys exist).
#[must_use]
pub fn omega_fixture(lineage: [Fp; LINEAGE_FIELDS]) -> OmegaFixture {
    let toy = toy();
    let chain = chain(&toy.relations, 8, &lineage);
    let z_a = chain.a_state.clone();
    let output = native::prove(&toy.keys, &chain, tapes(41), &FoldConfig::default())
        .expect("the toy chain's Omega");
    OmegaFixture {
        binding: toy.keys.omega_key.binding().clone(),
        vk: toy.keys.omega_key.vk().clone(),
        proof: output.proof,
        instances: output.instances,
        acc_p: output.acc_p.to_bytes(),
        acc_v: output.acc_v.to_bytes(),
        z_a,
        d_a: output.d_a,
    }
}

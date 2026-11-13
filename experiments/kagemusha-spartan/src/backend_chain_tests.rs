//! Full-program compiler and a real two-wallet chain. Component proofs are
//! used only to size the first recursive generation; none enter the final
//! Bootstrap -> Load -> Send -> Receive chain.
use super::*;
use crate::{
    bootstrap_layout::{self, Requirements},
    chain_fixture::{self, Chain},
    deferral::{self, Kind, MatPoly},
    ipa::Gens,
    payment_wire,
    r1cs::{Instance, Witness},
    uniform::{Layout, UniformCircuit},
};
use group::{Curve, Group};
use iroha_pasta::{Ep, Eq};
use std::sync::Arc;

const P_COLUMNS: usize = 16;
const S_COLUMNS: usize = 15;
const DIMENSIONS: ((usize, usize), (usize, usize)) = ((28, 26), (27, 25));
type Built<F> = (UniformCircuit<F>, Instance<F>, Witness<F>);

struct Generation {
    /// Selector order is Receive, Send, Bootstrap, Load.
    kinds: [UniformPair; 4],
    receiver_boot: UniformPair,
}

impl Generation {
    fn generators(&self) -> PairGens {
        next_gens_with_load_at(
            &self.kinds[RECEIVE],
            Some(&self.kinds[SEND]),
            Some(&self.kinds[BOOTSTRAP]),
            Some(&self.kinds[LOAD]),
            DIMENSIONS,
        )
    }

    fn program_ids(&self) -> [[[u8; 32]; 2]; 4] {
        std::array::from_fn(|k| {
            [
                self.kinds[k].cp.program_digest(),
                self.kinds[k].cs.program_digest(),
            ]
        })
    }
}

#[derive(Clone, Copy)]
enum Step {
    SenderBoot,
    ReceiverBoot,
    Load,
    Send,
    Receive,
}

impl Step {
    fn kind(self) -> usize {
        match self {
            Self::SenderBoot | Self::ReceiverBoot => BOOTSTRAP,
            Self::Load => LOAD,
            Self::Send => SEND,
            Self::Receive => RECEIVE,
        }
    }
}

fn build<F: Scalar>(w: Wiring<F>, primary: bool, target: Option<Layout>) -> Built<F> {
    let (arrays, pi, counts, tables, copies) = w.finish();
    UniformCircuit::build_with_copies_at(
        if primary { "P-uniform" } else { "S-uniform" },
        arrays,
        pi,
        counts,
        tables,
        copies,
        if primary { P_COLUMNS } else { S_COLUMNS },
        if primary { 64 } else { 62 },
        if primary {
            b"full backend P"
        } else {
            b"full backend S"
        },
        target,
    )
}

fn empty_generators() -> PairGens {
    fn empty<F: Scalar>() -> Kind<F> {
        Kind {
            sx: 0,
            sz: 0,
            deg: (0, 0),
            eval: Box::new(|_, _, _| (F::ZERO, F::ZERO, F::ZERO)),
            eval_many: None,
        }
    }
    PairGens {
        p: Gens::<Eq>::new(1 << P_COLUMNS),
        s: Gens::<Ep>::new(1 << S_COLUMNS),
        defer_p: MatPoly::with_min(
            std::array::from_fn::<_, 4, _>(|_| empty()),
            DIMENSIONS.0.0,
            DIMENSIONS.0.1,
        ),
        defer_s: MatPoly::with_min(
            std::array::from_fn::<_, 4, _>(|_| empty()),
            DIMENSIONS.1.0,
            DIMENSIONS.1.1,
        ),
        keys_p: None,
        keys_s: None,
    }
}

/// Seed inputs are constrained constants, never unbound fixture authority.
fn pin_fixture_arrays(w: &mut Wiring<Fp>, start: usize) {
    for array in &mut w.arrays[start..] {
        let n = 1 << array.rho;
        for (cell, values) in array.values.iter().enumerate() {
            array.rows.push([
                plus(
                    &own(cell, n),
                    &constant(values.iter().map(|v| -*v).collect()),
                ),
                constant(vec![Fp::ONE; n]),
                Vec::new(),
            ]);
        }
    }
}

fn seed_secondary(gens: &PairGens, kind: usize) -> Wiring<Fq> {
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    let (_, pin) = kind_pin(&mut w, f, kind);
    let bits = w.push(Array {
        name: "seed bit table",
        family: f,
        rho: 1,
        r_count: 2,
        fixed: true,
        lookups: vec![],
        widths: vec![1],
        rows: vec![],
        values: vec![vec![Fq::ZERO, Fq::ONE]],
    });
    w.tables.push((bits, BIT_TABLE));
    w.arrays[pin].lookups.push((BIT_TABLE, vec![own(0, 2)]));
    let values = vec![kind as u128];
    let mu = Option::<Fq>::from(Fq::from_repr(nonnative::fq_bytes(&binding_mu(&values)))).unwrap();
    native_binding(&mut w, f, mu, &values);
    deferral::expose_constant(&mut w, f, &gens.defer_s.empty_claim(), LATE_IO);
    generator_integration::expose_empty(&mut w, f, &gens.s);
    w
}

fn seed_primary(chain: &Chain, gens: &PairGens, step: Step) -> Wiring<Fp> {
    match step {
        Step::SenderBoot => {
            return p_boot_from_inputs(gens, &chain.sender_context, &chain.sender_bootstrap)
                .unwrap()
                .0;
        }
        Step::ReceiverBoot => {
            return p_boot_from_inputs(gens, &chain.receiver_context, &chain.receiver_bootstrap)
                .unwrap()
                .0;
        }
        _ => {}
    }
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    let prior = match step {
        Step::Load => {
            crate::bootstrap::public_profile(&chain.sender_context, &chain.sender_bootstrap)
        }
        Step::Send => chain.after_load_public,
        Step::Receive => {
            crate::bootstrap::public_profile(&chain.receiver_context, &chain.receiver_bootstrap)
        }
        _ => unreachable!(),
    };
    let previous = VerifiedPredecessor::fixture(&mut w, f, prior);
    let incoming = matches!(step, Step::Receive).then(|| {
        IncomingProofFacts::fixture(
            &mut w,
            f,
            chain.after_send_public,
            Fp::ONE,
            crate::payment::proof_digest(b"component sizing seed only").unwrap(),
        )
    });
    pin_fixture_arrays(&mut w, 0);
    let complete = match step {
        Step::Load => {
            CompleteOperation::Load(crate::load::append(&mut w, f, &chain.load, &previous).unwrap())
        }
        Step::Send => {
            CompleteOperation::Send(crate::send::append(&mut w, f, &chain.send, &previous).unwrap())
        }
        Step::Receive => {
            let witness = crate::receive_fixture::from_send(
                &chain.receiver_bootstrap,
                &chain.receiver_key,
                &chain.send,
                b"component sizing seed only",
                prior,
            )
            .unwrap();
            CompleteOperation::Receive(
                crate::receive::append(&mut w, f, &witness, &previous, incoming.as_ref().unwrap())
                    .unwrap(),
            )
        }
        _ => unreachable!(),
    };
    kind_pin(&mut w, f, step.kind());
    let values = vec![step.kind() as u128];
    p_binding(&mut w, f, &binding_mu(&values), &values);
    expose_operation(&mut w, f, &complete);
    deferral::expose_constant(&mut w, f, &gens.defer_p.empty_claim(), LATE_IO);
    generator_integration::expose_empty(&mut w, f, &gens.p);
    let mut seen = std::collections::HashSet::new();
    w.tables.retain(|(_, t)| seen.insert(*t));
    w
}

fn recursive_pairs(previous: &Generation, gens: &PairGens, step: Step) -> Vec<TracedPair> {
    let selected: Vec<&UniformPair> = match step {
        Step::Load => vec![&previous.kinds[BOOTSTRAP]],
        Step::Send => vec![&previous.kinds[LOAD]],
        Step::Receive => vec![&previous.receiver_boot, &previous.kinds[SEND]],
        _ => vec![],
    };
    selected
        .into_iter()
        .map(|p| trace_uniform(p.clone(), gens))
        .collect()
}

struct OperationPlan {
    pairs: Vec<TracedPair>,
    folds: Option<GeneratorFolds>,
}
impl OperationPlan {
    fn new(gens: &PairGens, previous: &Generation, step: Step) -> Self {
        let pairs = recursive_pairs(previous, gens, step);
        let folds = (!pairs.is_empty()).then(|| {
            GeneratorFolds::prepare(
                gens,
                &pairs,
                &crate::fs::ProofEntropy::fresh().expect("OS entropy for private generator fold"),
            )
            .expect("valid predecessor generator claims")
        });
        Self { pairs, folds }
    }
}

fn recursive_primary(
    chain: &Chain,
    gens: &PairGens,
    plan: &OperationPlan,
    step: Step,
) -> Wiring<Fp> {
    match step {
        Step::SenderBoot => {
            return p_boot_from_inputs(gens, &chain.sender_context, &chain.sender_bootstrap)
                .unwrap()
                .0;
        }
        Step::ReceiverBoot => {
            return p_boot_from_inputs(gens, &chain.receiver_context, &chain.receiver_bootstrap)
                .unwrap()
                .0;
        }
        _ => {}
    }
    let pairs = &plan.pairs;
    let receive;
    let operation = match step {
        Step::Load => OperationWitness::Load(&chain.load),
        Step::Send => OperationWitness::Send(&chain.send),
        Step::Receive => {
            let raw = incoming_encoding(&pairs[1]);
            let valid = incoming_verdict_values(gens, &pairs[1])
                .into_iter()
                .all(|b| b == 1);
            receive = crate::receive_fixture::from_send_with_validity(
                &chain.receiver_bootstrap,
                &chain.receiver_key,
                &chain.send,
                &raw.bytes,
                pairs[0]
                    .inst_p
                    .io
                    .clone()
                    .try_into()
                    .expect("64 predecessor fields"),
                valid,
            )
            .unwrap();
            OperationWitness::Receive(&receive)
        }
        _ => unreachable!(),
    };
    p_operation(operation, gens, pairs, plan.folds.as_ref().unwrap())
        .unwrap()
        .0
}

fn recursive_secondary(gens: &PairGens, plan: &OperationPlan, step: Step) -> Wiring<Fq> {
    if step.kind() == BOOTSTRAP {
        return s_boot(gens).0;
    }
    s_operation(step.kind(), gens, &plan.pairs, plan.folds.as_ref().unwrap()).0
}

fn primary(chain: &Chain, gens: &PairGens, plan: Option<&OperationPlan>, step: Step) -> Wiring<Fp> {
    match plan {
        Some(p) => recursive_primary(chain, gens, p, step),
        None => seed_primary(chain, gens, step),
    }
}

fn secondary(gens: &PairGens, plan: Option<&OperationPlan>, step: Step) -> Wiring<Fq> {
    match plan {
        Some(p) => recursive_secondary(gens, p, step),
        None => seed_secondary(gens, step.kind()),
    }
}

fn layouts(chain: &Chain, gens: &PairGens, previous: Option<&Generation>) -> (Layout, Layout) {
    let mut p = Vec::new();
    let mut s = Vec::new();
    // Each natural witness is released before constructing the next kind.
    for step in [Step::Receive, Step::Send, Step::SenderBoot, Step::Load] {
        let plan = previous.map(|p| OperationPlan::new(gens, p, step));
        let (c, _, _) = build(primary(chain, gens, plan.as_ref(), step), true, None);
        p.push(Requirements::of(&c));
        drop(c);
        let (c, _, _) = build(secondary(gens, plan.as_ref(), step), false, None);
        s.push(Requirements::of(&c));
    }
    (
        bootstrap_layout::common_requirements(&p),
        bootstrap_layout::common_requirements(&s),
    )
}

fn prove_step(
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
    step: Step,
    layout: (Layout, Layout),
    generation: usize,
) -> UniformPair {
    let plan = previous.map(|p| OperationPlan::new(gens, p, step));
    let (cp, ip, wp) = build(
        primary(chain, gens, plan.as_ref(), step),
        true,
        Some(layout.0),
    );
    let (cs, is, ws) = build(secondary(gens, plan.as_ref(), step), false, Some(layout.1));
    assert_eq!((cp.layout(), cs.layout()), layout);
    let pair = crate::prove_linked_pair(
        "full backend chain",
        generation,
        &cp,
        &ip,
        &wp,
        &cs,
        &is,
        &ws,
    );
    assert!(pair.ok, "both linked native proofs");
    crate::linked_uniform(&Arc::new(cp), ip, &Arc::new(cs), is, pair, step.kind())
}

fn generation(
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
    generation: usize,
) -> Generation {
    let layout = layouts(chain, gens, previous);
    eprintln!(
        "full backend generation {generation}: P {:#?}, S {:#?}",
        layout.0, layout.1
    );
    let kinds = [Step::Receive, Step::Send, Step::SenderBoot, Step::Load]
        .map(|step| prove_step(chain, gens, previous, step, layout, generation));
    let mut receiver_boot = prove_step(
        chain,
        gens,
        previous,
        Step::ReceiverBoot,
        layout,
        generation,
    );
    assert_eq!(
        receiver_boot.cp.program_digest(),
        kinds[BOOTSTRAP].cp.program_digest(),
        "wallet-independent Bootstrap program"
    );
    assert_eq!(
        receiver_boot.cs.program_digest(),
        kinds[BOOTSTRAP].cs.program_digest()
    );
    receiver_boot.cp = kinds[BOOTSTRAP].cp.clone();
    receiver_boot.cs = kinds[BOOTSTRAP].cs.clone();
    Generation {
        kinds,
        receiver_boot,
    }
}

fn matrix_checks(pair: &UniformPair, gens: &PairGens) -> (bool, bool) {
    let p = deferral::claim_of(&pair.pp.late[..gens.defer_p.n() + 1]);
    let s = deferral::claim_of(&pair.ps.late[..gens.defer_s.n() + 1]);
    (
        gens.defer_p.eval(&p.point) == p.value,
        gens.defer_s.eval(&s.point) == s.value,
    )
}

/// Fresh final witnesses and proofs consume only final-program predecessors.
fn final_chain(chain: &Chain, gens: &PairGens, mut fixed: Generation) -> Generation {
    let expected = fixed.program_ids();
    let layout = (fixed.kinds[0].cp.layout(), fixed.kinds[0].cs.layout());
    for step in [
        Step::SenderBoot,
        Step::ReceiverBoot,
        Step::Load,
        Step::Send,
        Step::Receive,
    ] {
        let pair = prove_step(chain, gens, Some(&fixed), step, layout, 200 + step.kind());
        assert_eq!(
            [pair.cp.program_digest(), pair.cs.program_digest()],
            expected[step.kind()],
            "final operation uses the fixed authenticated program"
        );
        assert_eq!(
            matrix_checks(&pair, gens),
            (true, true),
            "both final matrix accumulators"
        );
        let traced = trace_uniform(pair.clone(), gens);
        assert!(
            crate::generator_claim::decide(
                &gens.p,
                &inherited_generator_claim(&traced.tr_p, &gens.p).unwrap()
            ),
            "primary inherited generator accumulator"
        );
        assert!(
            crate::generator_claim::decide(
                &gens.s,
                &inherited_generator_claim(&traced.tr_s, &gens.s).unwrap()
            ),
            "secondary inherited generator accumulator"
        );
        match step {
            Step::ReceiverBoot => fixed.receiver_boot = pair,
            _ => fixed.kinds[step.kind()] = pair,
        }
    }
    fixed
}

fn complete_payment(chain: &Chain, send: &UniformPair, gens: &PairGens) -> usize {
    let trusted = crate::carrier::TrustedProfile::from_parameters(
        chain.sender_context.relation,
        send.cp.shape,
        send.cp.as_ref(),
        send.cs.shape,
        send.cs.as_ref(),
        true,
    )
    .unwrap();
    let profile = payment_wire::Profile {
        pair: &trusted,
        scheme: chain.sender_context.root.scheme,
        relation: chain.sender_context.relation,
    };
    let encoded = payment_wire::encode(
        &profile,
        &chain.payment_originals(),
        &send.ip,
        &send.is,
        &send.pp,
        &send.ps,
        send.r,
    )
    .unwrap();
    let decoded = payment_wire::decode(&profile, &encoded).unwrap();
    let trace = trace_uniform(send.clone(), gens);
    assert_eq!(decoded.pair_bytes, incoming_encoding(&trace).bytes);
    assert_eq!(decoded.pair.pp.to_bytes(), send.pp.to_bytes());
    assert_eq!(decoded.pair.ps.to_bytes(), send.ps.to_bytes());
    assert_eq!(decoded.pair.ip.io, send.ip.io);
    assert_eq!(decoded.pair.is.io, send.is.io);
    eprintln!(
        "complete actual Payment: IPM1={} B, pair={} B, envelope={} B, companion={} B",
        encoded.len(),
        decoded.size.pair,
        decoded.size.envelope,
        decoded.size.companion
    );
    encoded.len()
}

#[test]
#[ignore = "heavy actual recursive program fixed point; run in the coordinated proof lane"]
fn complete_recursive_bootstrap_load_send_receive() {
    let seed_gens = empty_generators();
    let chain = chain_fixture::new(&seed_gens.p);
    let mut previous = generation(&chain, &seed_gens, None, 0);
    for hop in 1..=12 {
        let gens = previous.generators();
        let next = generation(&chain, &gens, Some(&previous), hop);
        if next.program_ids() == previous.program_ids() {
            let final_gens = next.generators();
            let final_chain = final_chain(&chain, &final_gens, next);
            let bytes = complete_payment(&chain, &final_chain.kinds[SEND], &final_gens);
            assert_eq!(chain.load.after_core[9], Fp::from(100));
            assert_eq!(chain.send.after_core[9], Fp::from(90));
            let sent = trace_uniform(final_chain.kinds[SEND].clone(), &final_gens);
            let received = crate::receive_fixture::from_send(
                &chain.receiver_bootstrap,
                &chain.receiver_key,
                &chain.send,
                &incoming_encoding(&sent).bytes,
                final_chain.receiver_boot.ip.io.clone().try_into().unwrap(),
            )
            .unwrap();
            assert_eq!(received.after_core[9], Fp::from(10));
            assert_eq!(
                final_chain.kinds[RECEIVE].ip.io[HEAD_IO],
                crate::monetary_state::commitment(&received.after_core, &received.after_rest),
                "the final accepted receiver head commits to the credited balance"
            );
            assert!(
                bytes <= payment_wire::QR_BUDGET,
                "complete Payment {bytes} B exceeds {} B",
                payment_wire::QR_BUDGET
            );
            return;
        }
        previous = next;
    }
    panic!("the exact four-kind programs did not reach a fixed point in twelve generations");
}

fn parser_relation<S: Scalar>(
    pair: &TracedPair,
    wire: &IncomingEncoding,
    expected: &[u8],
    bits: [u128; 2],
    primary: bool,
) -> (Built<S>, usize) {
    let mut values = vec![0u128; 8];
    values.extend(bits);
    values.extend(crate::proof_bytes::shared_words(&wire.bytes));
    let count = values.len();
    let n = count.next_power_of_two();
    let mut field_values: Vec<_> = values.into_iter().map(S::from_u128).collect();
    field_values.resize(n, S::ZERO);
    let mut w = Wiring::default();
    let family = w.family(0, 1);
    let shared = w.push(Array {
        name: "parser seam public shared values",
        family,
        rho: n.ilog2() as usize,
        r_count: count,
        fixed: false,
        lookups: vec![],
        widths: vec![128],
        rows: vec![],
        values: vec![field_values],
    });
    w.io_arrays.push((shared, 256));
    let raw = crate::proof_bytes::append_shared(&mut w, family, &wire.bytes, shared, 10);
    let selected = incoming_codec::select(&mut w, family, &raw, wire, pair, shared, 0, primary);
    for (i, b) in expected.iter().enumerate() {
        assert_eq!(
            w.arrays[selected.array].values[0][i],
            S::from(*b as u64),
            "selected byte {i}"
        );
    }
    let (a, p, c, t, copies) = w.finish();
    let built = UniformCircuit::build_with_copies(
        "total parser seam",
        a,
        p,
        c,
        t,
        copies,
        12,
        1024,
        b"total parser seam",
    );
    let selected_first = built.0.cell_index(selected.array, 0, 74);
    (built, selected_first)
}

fn relation_holds<S: Scalar>((c, i, w): &Built<S>) -> bool {
    use crate::r1cs::Circuit;
    let mut values = w.w.clone();
    let relation = c.instantiate(&c.shape, &[S::from(31), S::from(47)], Some(&mut values));
    let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
    let (a, b, c) = relation.abc(&c.shape, &z);
    a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
}

#[test]
fn fixed_profile_candidate_selection_matches_both_circuit_halves() {
    use crate::{
        carrier,
        r1cs::{Shape, ToyCircuit},
        spartan::{self, Link},
        verifier,
    };
    let p = Shape {
        label: "candidate parser P",
        sx: 10,
        row_bits: 2,
        col_bits: 8,
        k: 1,
    };
    let s = Shape {
        label: "candidate parser S",
        ..p
    };
    let profile =
        carrier::TrustedProfile::from_parameters([7; 32], p, &ToyCircuit, s, &ToyCircuit, true)
            .unwrap();
    let mut context = [Fp::ZERO; crate::bootstrap_context::FIELDS];
    context[crate::bootstrap_context::RELATION] = Fp::from_u128(u128::from_le_bytes([7; 16]));
    context[crate::bootstrap_context::RELATION + 1] = context[crate::bootstrap_context::RELATION];
    let inputs = carrier::ProjectedInputs::from_authenticated_fields(
        Fp::ONE,
        Fp::from(2),
        Fp::from(3),
        context,
        [Fp::ZERO; crate::bootstrap_context::LINEAGE_FIELDS],
        spartan::limbs_of_point::<Eq>(&Eq::generator().to_affine()),
        SEND,
    )
    .unwrap();
    let mut bp = vec![0u8; p.proof_bytes()];
    let mut bs = vec![0u8; s.proof_bytes()];
    bp[32 * (p.rows() + p.outer_blocks().len())] = 1;
    bs[32 * (s.rows() + s.outer_blocks().len())] = 1;
    let (original, ranges) =
        carrier::canonical_projected_template(&profile, &bp, &bs, Fq::ONE.to_repr()).unwrap();
    let (gp, gs) = (Gens::<Eq>::new(p.cols()), Gens::<Ep>::new(s.cols()));
    let malformed = [
        None,
        Some(42),
        Some(ranges.primary.start),
        Some(ranges.primary.start + 32 * (p.rows() + p.outer_blocks().len())),
        Some(ranges.secondary.start),
        Some(ranges.secondary.start + 32 * (s.rows() + s.outer_blocks().len())),
    ];
    for (case, mutation) in malformed.into_iter().enumerate() {
        let mut raw = original.clone();
        if let Some(at) = mutation {
            raw[at..at + 32].fill(255);
        }
        let decoded = carrier::decode_projected_candidate(&profile, &raw, &inputs).unwrap();
        let (expected, _) = carrier::canonical_projected_template(
            &profile,
            &decoded.pp.to_bytes(),
            &decoded.ps.to_bytes(),
            decoded.r,
        )
        .unwrap();
        if mutation.is_none() {
            assert_eq!(expected, raw);
        } else {
            assert_eq!(decoded.r, [0; 32]);
            assert!(expected[74..].iter().all(|b| *b == 0));
        }
        let r = decoded.r;
        let rlc = move |_: &[u8; 32]| r;
        let tr_p = verifier::trace(
            &p,
            &gp,
            &decoded.ip,
            &decoded.pp,
            &[],
            &Link::First {
                rho: decoded.rho,
                r,
            },
        );
        let tr_s = verifier::trace(
            &s,
            &gs,
            &decoded.is,
            &decoded.ps,
            &[],
            &Link::Second {
                d: decoded.d,
                rlc: &rlc,
            },
        );
        let pair = TracedPair {
            incoming_original: Some(raw.clone()),
            shape_p: p,
            inst_p: decoded.ip,
            claims_p: vec![],
            proof_p: decoded.pp,
            tr_p,
            shape_s: s,
            inst_s: decoded.is,
            claims_s: vec![],
            proof_s: decoded.ps,
            tr_s,
            acc_p: deferral::Claim {
                point: vec![],
                value: Fp::ZERO,
            },
            acc_s: deferral::Claim {
                point: vec![],
                value: Fq::ZERO,
            },
            kind: SEND,
        };
        let wire = incoming_encoding(&pair);
        let bits = incoming_codec::native_bits(&pair, &wire);
        assert_eq!(bits == [1, 1], mutation.is_none());
        let (mut primary, first) = parser_relation::<Fp>(&pair, &wire, &expected, bits, true);
        assert!(relation_holds(&primary), "primary total parser case {case}");
        primary.1.io[264] = Fp::ONE - primary.1.io[264];
        assert!(!relation_holds(&primary));
        primary.1.io[264] = Fp::ONE - primary.1.io[264];
        primary.2.w[first] += Fp::ONE;
        assert!(
            !relation_holds(&primary),
            "selected proof byte cannot be substituted"
        );
        drop(primary);
        let (mut secondary, _) = parser_relation::<Fq>(&pair, &wire, &expected, bits, false);
        assert!(
            relation_holds(&secondary),
            "secondary total parser case {case}"
        );
        if case == 1 {
            let (c, i, w) = &secondary;
            let g = Gens::<Ep>::new(c.shape.cols());
            let (proof, _) = spartan::prove_circuit(
                &c.shape,
                &g,
                i,
                w,
                &mut SpikeRng::new(b"false R parse actual proof"),
                c,
            );
            spartan::verify_circuit(&c.shape, &g, i, &proof.to_bytes(), c).unwrap();
            let mut forged = i.clone();
            forged.io[265] = Fq::ONE - forged.io[265];
            assert!(spartan::verify_circuit(&c.shape, &g, &forged, &proof.to_bytes(), c).is_err());
        }
        secondary.1.io[265] = Fq::ONE - secondary.1.io[265];
        assert!(!relation_holds(&secondary));
    }
}

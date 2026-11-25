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

use crate::native::PRODUCER_MATRIX_DIMENSIONS as DIMENSIONS;
/// Each offline compiler pass explicitly selects the shape it emits and imports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilerGroup {
    Three,
    Four,
}

impl CompilerGroup {
    pub(crate) fn import<F: Scalar>(
        self,
        bytes: &[u8],
        primary: bool,
    ) -> Result<UniformCircuit<F>, String> {
        match self {
            Self::Three => UniformCircuit::<F>::import_program(bytes, primary),
            Self::Four => UniformCircuit::<F>::import_four_variable_program(bytes, primary),
        }
    }

    fn of(p: &UniformCircuit<Fp>, s: &UniformCircuit<Fq>) -> Self {
        match (p.shape.k, s.shape.k) {
            (3, 3) => Self::Three,
            (4, 4) => Self::Four,
            k => panic!("unexpected compiler grouping {k:?}"),
        }
    }
}

#[derive(Clone, Debug)]
struct RunConfig {
    group: CompilerGroup,
    p_columns: usize,
    s_columns: usize,
    output: std::path::PathBuf,
}

impl RunConfig {
    fn from_environment() -> Self {
        let label =
            std::env::var("KAGEMUSHA_SPARTAN_TEST_LAYOUT").unwrap_or_else(|_| "p18-s17".to_owned());
        let (p_columns, s_columns) = match label.as_str() {
            "p18-s17" => (18, 17),
            "p17-s17" => (17, 17),
            "p18-s16" => (18, 16),
            _ => panic!("unknown test layout {label}; choose p18-s17, p17-s17 or p18-s16"),
        };
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let output = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| manifest.join("output/full-backend-layouts").join(&label));
        let output = if output.is_absolute() {
            output
        } else {
            manifest.join(output)
        };
        assert!(
            !output.exists(),
            "test output already exists: {}; select a new KAGEMUSHA_SPARTAN_TEST_OUTPUT to preserve earlier results",
            output.display()
        );
        std::fs::create_dir_all(output.parent().expect("test output parent")).unwrap();
        std::fs::create_dir(&output).expect("reserve a new per-run output directory");
        std::fs::write(
            output.join("layout.txt"),
            format!("layout={label}\ndimensions={DIMENSIONS:?}\n"),
        )
        .unwrap();
        Self {
            group: CompilerGroup::Three,
            p_columns,
            s_columns,
            output,
        }
    }
}

type Built<F> = (UniformCircuit<F>, Instance<F>, Witness<F>);

/// The compiler computes the pin directly on the in-memory relation before
/// encoding. The independent reader must reproduce it from the public bytes.
#[derive(Default)]
pub(crate) struct CompiledTape {
    pub(crate) pin: [u8; 32],
    pub(crate) bytes: Vec<u8>,
}

impl CompiledTape {
    pub(crate) fn capture<F: Scalar>(circuit: &UniformCircuit<F>) -> Self {
        let pin = circuit.program_digest();
        let bytes = circuit.export_program().expect("canonical public program");
        Self { pin, bytes }
    }
}

pub(crate) struct Generation {
    config: RunConfig,
    /// Selector order is Receive, Send, Bootstrap, Control.
    pub(crate) kinds: [UniformPair; 4],
    pub(crate) receiver_boot: UniformPair,
}

impl Generation {
    fn generators(&self) -> PairGens {
        next_gens_with_control_at(
            &self.kinds[RECEIVE],
            Some(&self.kinds[SEND]),
            Some(&self.kinds[BOOTSTRAP]),
            Some(&self.kinds[CONTROL]),
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

#[derive(Clone, Copy, Debug)]
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
            Self::Load => CONTROL,
            Self::Send => SEND,
            Self::Receive => RECEIVE,
        }
    }
}

fn build<F: Scalar>(
    config: &RunConfig,
    w: Wiring<F>,
    primary: bool,
    target: Option<Layout>,
) -> Built<F> {
    let columns = if primary {
        config.p_columns
    } else {
        config.s_columns
    };
    build_monetary_program_grouped(w, columns, primary, target, config.group)
}

/// Compile either curve with the label and transcript namespace accepted by
/// the compact public monetary-program encoder.
pub(crate) fn build_monetary_program<F: Scalar>(
    w: Wiring<F>,
    columns: usize,
    primary: bool,
    target: Option<Layout>,
) -> Built<F> {
    build_monetary_program_grouped(w, columns, primary, target, CompilerGroup::Three)
}

pub(crate) fn build_monetary_program_grouped<F: Scalar>(
    w: Wiring<F>,
    columns: usize,
    primary: bool,
    target: Option<Layout>,
    group: CompilerGroup,
) -> Built<F> {
    let (arrays, pi, counts, tables, copies) = w.finish();
    let builder = match group {
        CompilerGroup::Three => UniformCircuit::build_with_copies_at,
        CompilerGroup::Four => UniformCircuit::build_group_four_with_copies_at,
    };
    builder(
        if primary { "P-uniform" } else { "S-uniform" },
        arrays,
        pi,
        counts,
        tables,
        copies,
        columns,
        64,
        if primary {
            b"full backend P"
        } else {
            b"full backend S"
        },
        target,
    )
}

fn empty_generators<const KINDS: usize>(config: &RunConfig) -> PairGens {
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
        p: Gens::<Eq>::new(1 << config.p_columns),
        s: Gens::<Ep>::new(1 << config.s_columns),
        defer_p: MatPoly::with_min(
            std::array::from_fn::<_, KINDS, _>(|_| empty()),
            DIMENSIONS.0.0,
            DIMENSIONS.0.1,
        ),
        defer_s: MatPoly::with_min(
            std::array::from_fn::<_, KINDS, _>(|_| empty()),
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
    seed_secondary_with_claim(gens, kind, None)
}

fn seed_secondary_with_claim(
    gens: &PairGens,
    kind: usize,
    claim: Option<&crate::ipa::Claim<Ep>>,
) -> Wiring<Fq> {
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    let (_, pin) = kind_pin_in(&mut w, f, gens.defer_s.kinds.len(), kind);
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
    let values = if kind == CONTROL {
        vec![kind as u128, 0, 0]
    } else {
        vec![kind as u128]
    };
    let mu = Option::<Fq>::from(Fq::from_repr(nonnative::fq_bytes(&binding_mu(&values)))).unwrap();
    let shared = native_binding(&mut w, f, mu, &values);
    bind_mode_io(
        &mut w,
        f,
        (kind == CONTROL).then_some((crate::control::Mode::Load, shared, 1)),
    );
    deferral::expose_constant(&mut w, f, &gens.defer_s.empty_claim(), LATE_IO);
    if let Some(claim) = claim {
        generator_integration::expose_compiler_claim(&mut w, f, &gens.s, claim);
    } else {
        generator_integration::expose_empty(&mut w, f, &gens.s);
    }
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
        Step::Load => CompleteOperation::Control(
            crate::control::append(
                &mut w,
                f,
                &chain.sender_context.root.root,
                &crate::control::Witness::load(chain.load.clone()),
                &previous,
            )
            .unwrap(),
        ),
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
    finish_seed(w, f, gens, complete, step.kind(), None)
}

fn finish_seed(
    mut w: Wiring<Fp>,
    f: usize,
    gens: &PairGens,
    complete: CompleteOperation,
    kind: usize,
    claim: Option<&crate::ipa::Claim<Eq>>,
) -> Wiring<Fp> {
    kind_pin_in(&mut w, f, gens.defer_p.kinds.len(), kind);
    let is_control = kind == CONTROL;
    let values = if is_control {
        vec![kind as u128, 0, 0]
    } else {
        vec![kind as u128]
    };
    let shared = p_binding(&mut w, f, &binding_mu(&values), &values);
    let modes = bind_mode_io(
        &mut w,
        f,
        is_control.then_some((crate::control::Mode::Load, shared, 1)),
    );
    expose_operation(&mut w, f, &complete, modes);
    deferral::expose_constant(&mut w, f, &gens.defer_p.empty_claim(), LATE_IO);
    if let Some(claim) = claim {
        generator_integration::expose_compiler_claim(&mut w, f, &gens.p, claim);
    } else {
        generator_integration::expose_empty(&mut w, f, &gens.p);
    }
    let mut seen = std::collections::HashSet::new();
    w.tables.retain(|(_, t)| seen.insert(*t));
    w
}

fn recursive_pairs(previous: &Generation, gens: &PairGens, step: Step) -> Vec<TracedPair> {
    let selected: Vec<&UniformPair> = match step {
        Step::Load => vec![&previous.kinds[BOOTSTRAP]],
        Step::Send => vec![&previous.kinds[CONTROL]],
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
    let control;
    let operation = match step {
        Step::Load => {
            control = crate::control::Witness::load(chain.load.clone());
            OperationWitness::Control {
                root: &chain.sender_context.root.root,
                witness: &control,
            }
        }
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
    s_operation(
        step.kind(),
        (step.kind() == CONTROL).then_some(crate::control::Mode::Load),
        gens,
        &plan.pairs,
        plan.folds.as_ref().unwrap(),
    )
    .0
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

fn layouts(
    config: &RunConfig,
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
) -> (Layout, Layout) {
    let (p, s) = ordinary_requirements(config, chain, gens, previous);
    (
        bootstrap_layout::common_requirements(&p),
        bootstrap_layout::common_requirements(&s),
    )
}

fn ordinary_requirements(
    config: &RunConfig,
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
) -> (Vec<Requirements>, Vec<Requirements>) {
    let mut p = Vec::new();
    let mut s = Vec::new();
    // Each natural witness is released before constructing the next kind.
    for step in [Step::Receive, Step::Send, Step::SenderBoot, Step::Load] {
        let plan = previous.map(|p| OperationPlan::new(gens, p, step));
        let (c, _, _) = build(
            config,
            primary(chain, gens, plan.as_ref(), step),
            true,
            None,
        );
        p.push(Requirements::of(&c));
        drop(c);
        let (c, _, _) = build(config, secondary(gens, plan.as_ref(), step), false, None);
        s.push(Requirements::of(&c));
    }
    (p, s)
}

/// A completed proof needs only its public verifier program. Dropping the
/// compiler arrays here keeps later generations from retaining every private
/// witness and lookup table from earlier operations.
fn retain_public_program<F: Scalar + zeroize::Zeroize>(
    compiled: UniformCircuit<F>,
    primary: bool,
) -> Arc<UniformCircuit<F>> {
    let expected = (
        compiled.program_digest(),
        compiled.layout(),
        compiled.io_len,
    );
    let (operations, constants, fixed_fields) = compiled.public_program_counts();
    eprintln!(
        "retained {} public program: operations={operations}, constants={constants}, fixed_fields={fixed_fields}",
        if primary { "primary" } else { "secondary" },
    );
    let public = compiled.into_verifier_program();
    assert_eq!(
        (public.program_digest(), public.layout(), public.io_len),
        expected,
        "retained program preserves its exact authenticated relation and IO"
    );
    assert!(
        public.arrays.is_empty(),
        "no retained private compiler arrays"
    );
    Arc::new(public)
}

fn prove_step(
    config: &RunConfig,
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
    step: Step,
    layout: (Layout, Layout),
    generation: usize,
) -> UniformPair {
    let plan = previous.map(|p| OperationPlan::new(gens, p, step));
    let (cp, ip, mut wp) = build(
        config,
        primary(chain, gens, plan.as_ref(), step),
        true,
        Some(layout.0),
    );
    let (cs, is, mut ws) = build(
        config,
        secondary(gens, plan.as_ref(), step),
        false,
        Some(layout.1),
    );
    assert_eq!((cp.layout(), cs.layout()), layout);
    if generation == 1 && matches!(step, Step::Receive) {
        let independent = OperationPlan::new(gens, previous.unwrap(), step);
        let (other, _, mut witness) = build(
            config,
            primary(chain, gens, Some(&independent), step),
            true,
            Some(layout.0),
        );
        let difference = cp.public_program_differences(&other);
        assert_eq!(
            cp.program_digest(),
            other.program_digest(),
            "primary program must be independent of fresh private fold coins: {difference}",
        );
        zeroize::Zeroize::zeroize(&mut witness);
        drop((other.into_verifier_program(), witness));
        let (other, _, mut witness) = build(
            config,
            secondary(gens, Some(&independent), step),
            false,
            Some(layout.1),
        );
        let difference = cs.public_program_differences(&other);
        assert_eq!(
            cs.program_digest(),
            other.program_digest(),
            "secondary program must be independent of fresh private fold coins: {difference}",
        );
        zeroize::Zeroize::zeroize(&mut witness);
        drop((other.into_verifier_program(), witness));
        eprintln!("fresh independent private fold entropy preserves both exact Receive programs");
    }
    eprintln!(
        "full backend generation {generation} {step:?}: P shape={:?}, memory={:?}; S shape={:?}, memory={:?}",
        cp.shape,
        cp.proving_memory_accounting(),
        cs.shape,
        cs.proving_memory_accounting(),
    );
    eprintln!(
        "full backend generation {generation} {step:?}: P phase memory={:?}; S phase memory={:?}",
        cp.phase_memory_accounting(),
        cs.phase_memory_accounting(),
    );
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
    eprintln!(
        "full backend generation {generation} {step:?}: linked pair verified in {:.2}s, proofs {} + {} B",
        pair.secs,
        pair.pp.to_bytes().len(),
        pair.ps.to_bytes().len(),
    );
    zeroize::Zeroize::zeroize(&mut wp);
    zeroize::Zeroize::zeroize(&mut ws);
    drop((wp, ws));
    let cp = retain_public_program(cp, true);
    let cs = retain_public_program(cs, false);
    crate::linked_uniform(&cp, ip, &cs, is, pair, step.kind())
}

fn generation(
    config: &RunConfig,
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
    generation: usize,
) -> Generation {
    let layout = layouts(config, chain, gens, previous);
    generation_at(config, chain, gens, previous, generation, layout)
}

fn generation_at(
    config: &RunConfig,
    chain: &Chain,
    gens: &PairGens,
    previous: Option<&Generation>,
    generation: usize,
    layout: (Layout, Layout),
) -> Generation {
    eprintln!(
        "full backend generation {generation}: P {:#?}, S {:#?}",
        layout.0, layout.1
    );
    let kinds = [Step::Receive, Step::Send, Step::SenderBoot, Step::Load]
        .map(|step| prove_step(config, chain, gens, previous, step, layout, generation));
    let mut receiver_boot = prove_step(
        config,
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
        config: config.clone(),
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
    let config = fixed.config.clone();
    let expected = fixed.program_ids();
    let layout = (fixed.kinds[0].cp.layout(), fixed.kinds[0].cs.layout());
    for step in [
        Step::SenderBoot,
        Step::ReceiverBoot,
        Step::Load,
        Step::Send,
        Step::Receive,
    ] {
        let pair = prove_step(
            &config,
            chain,
            gens,
            Some(&fixed),
            step,
            layout,
            200 + step.kind(),
        );
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

fn complete_payment(
    config: &RunConfig,
    chain: &Chain,
    send: &UniformPair,
    gens: &PairGens,
) -> usize {
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
    let output = &config.output;
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("research-payment.ipm1"), &encoded).unwrap();
    std::fs::write(
        output.join("payment-size.txt"),
        format!(
            "IPM1={}\npair={}\nenvelope={}\ncompanion={}\nQR_budget={}\n",
            encoded.len(),
            decoded.size.pair,
            decoded.size.envelope,
            decoded.size.companion,
            payment_wire::QR_BUDGET,
        ),
    )
    .unwrap();
    encoded.len()
}

/// Prove that distributable public artifacts, with no retained witness
/// arrays, reproduce both native verifiers and all final accumulator checks.
fn imported_programs(final_chain: &Generation) -> Generation {
    use crate::spartan::{self, Link};
    let output = &final_chain.config.output;
    std::fs::create_dir_all(&output).unwrap();
    let kinds = std::array::from_fn(|kind| {
        let mut pair = final_chain.kinds[kind].clone();
        let primary = pair
            .cp
            .export_program()
            .expect("public primary program artifact");
        let secondary = pair
            .cs
            .export_program()
            .expect("public secondary program artifact");
        let cp = UniformCircuit::<Fp>::import_program(&primary, true).unwrap();
        let cs = UniformCircuit::<Fq>::import_program(&secondary, false).unwrap();
        assert_eq!(cp.program_digest(), pair.cp.program_digest());
        assert_eq!(cs.program_digest(), pair.cs.program_digest());
        std::fs::write(
            output.join(format!("research-kind-{kind}-primary.norito")),
            &primary,
        )
        .unwrap();
        std::fs::write(
            output.join(format!("research-kind-{kind}-secondary.norito")),
            &secondary,
        )
        .unwrap();
        eprintln!(
            "final public program kind {kind}: primary={} B secondary={} B",
            primary.len(),
            secondary.len()
        );
        pair.cp = Arc::new(cp);
        pair.cs = Arc::new(cs);
        pair
    });
    let mut receiver_boot = final_chain.receiver_boot.clone();
    receiver_boot.cp = kinds[BOOTSTRAP].cp.clone();
    receiver_boot.cs = kinds[BOOTSTRAP].cs.clone();
    let imported = Generation {
        config: final_chain.config.clone(),
        kinds,
        receiver_boot,
    };
    let gens = imported.generators();
    for pair in imported.kinds.iter().chain([&imported.receiver_boot]) {
        let r = pair.r;
        let rlc = move |_: &[u8; 32]| r;
        let d = spartan::verify_linked_circuit(
            &pair.cp.shape,
            &gens.p,
            &pair.ip,
            &pair.pp.to_bytes(),
            &Link::First { rho: pair.rho, r },
            pair.cp.as_ref(),
        )
        .unwrap();
        let rho = spartan::verify_linked_circuit(
            &pair.cs.shape,
            &gens.s,
            &pair.is,
            &pair.ps.to_bytes(),
            &Link::Second {
                d: pair.d,
                rlc: &rlc,
            },
            pair.cs.as_ref(),
        )
        .unwrap();
        assert_eq!((d, rho), (pair.d, pair.rho));
        assert_eq!(
            matrix_checks(pair, &gens),
            (true, true),
            "imported exact matrix programs"
        );
        let traced = trace_uniform(pair.clone(), &gens);
        assert!(crate::generator_claim::decide(
            &gens.p,
            &inherited_generator_claim(&traced.tr_p, &gens.p).unwrap()
        ));
        assert!(crate::generator_claim::decide(
            &gens.s,
            &inherited_generator_claim(&traced.tr_s, &gens.s).unwrap()
        ));
    }
    imported
}

fn malformed_receive_burn(chain: &Chain, gens: &PairGens, final_chain: &Generation) {
    use crate::bootstrap_context::{BURNED_TOTAL, CONTEXT_IO, FIELDS, LINEAGE_FIELDS, LINEAGE_IO};
    let send = &final_chain.kinds[SEND];
    let trusted = crate::carrier::TrustedProfile::from_parameters(
        chain.sender_context.relation,
        send.cp.shape,
        send.cp.as_ref(),
        send.cs.shape,
        send.cs.as_ref(),
        true,
    )
    .unwrap();
    let projection = crate::carrier::ProjectedInputs::from_authenticated_fields(
        send.ip.io[HEAD_IO],
        send.ip.io[STATEMENT_IO],
        send.ip.io[RECEIPT_IO],
        send.ip.io[CONTEXT_IO..CONTEXT_IO + FIELDS]
            .try_into()
            .unwrap(),
        send.ip.io[LINEAGE_IO..LINEAGE_IO + LINEAGE_FIELDS]
            .try_into()
            .unwrap(),
        send.ip.io[TABLE_IO..TABLE_IO + 4].try_into().unwrap(),
        SEND,
        [Fp::ZERO; 2],
    )
    .unwrap();
    let mut raw = incoming_encoding(&trace_uniform(send.clone(), gens)).bytes;
    raw[42..74].fill(255); // Noncanonical R in an otherwise exact fixed-profile frame.
    let decoded = crate::carrier::decode_projected_candidate(&trusted, &raw, &projection).unwrap();
    let incoming = UniformPair {
        cp: send.cp.clone(),
        ip: decoded.ip,
        pp: decoded.pp,
        cs: send.cs.clone(),
        is: decoded.is,
        ps: decoded.ps,
        d: decoded.d,
        rho: decoded.rho,
        r: decoded.r,
        kind: SEND,
    };
    let pairs = [
        trace_uniform(final_chain.receiver_boot.clone(), gens),
        trace_incoming_candidate(incoming, gens, raw.clone()),
    ];
    assert!(
        incoming_verdict_values(gens, &pairs[1])
            .iter()
            .any(|b| *b == 0)
    );
    let witness = crate::receive_fixture::from_send_with_validity(
        &chain.receiver_bootstrap,
        &chain.receiver_key,
        &chain.send,
        &raw,
        final_chain.receiver_boot.ip.io.clone().try_into().unwrap(),
        false,
    )
    .unwrap();
    let folds =
        GeneratorFolds::prepare(gens, &pairs, &crate::fs::ProofEntropy::fresh().unwrap()).unwrap();
    let (w, _) = p_operation(OperationWitness::Receive(&witness), gens, &pairs, &folds).unwrap();
    let layout = (
        final_chain.kinds[RECEIVE].cp.layout(),
        final_chain.kinds[RECEIVE].cs.layout(),
    );
    let (cp, ip, wp) = build(&final_chain.config, w, true, Some(layout.0));
    let (w, _) = s_operation(RECEIVE, None, gens, &pairs, &folds);
    let (cs, is, ws) = build(&final_chain.config, w, false, Some(layout.1));
    assert_eq!(
        cp.program_digest(),
        final_chain.kinds[RECEIVE].cp.program_digest(),
        "Burn uses the exact Receive program"
    );
    assert_eq!(
        cs.program_digest(),
        final_chain.kinds[RECEIVE].cs.program_digest()
    );
    let proof = crate::prove_linked_pair(
        "malformed R actual Receive Burn",
        250,
        &cp,
        &ip,
        &wp,
        &cs,
        &is,
        &ws,
    );
    assert!(proof.ok);
    assert_eq!(
        ip.io[LINEAGE_IO + BURNED_TOTAL],
        Fp::from(10),
        "invalid candidate irreversibly burns its credit"
    );
    let pair = crate::linked_uniform(&Arc::new(cp), ip, &Arc::new(cs), is, proof, RECEIVE);
    assert_eq!(matrix_checks(&pair, gens), (true, true));
    let traced = trace_uniform(pair, gens);
    assert!(crate::generator_claim::decide(
        &gens.p,
        &inherited_generator_claim(&traced.tr_p, &gens.p).unwrap()
    ));
    assert!(crate::generator_claim::decide(
        &gens.s,
        &inherited_generator_claim(&traced.tr_s, &gens.s).unwrap()
    ));
    let output = &final_chain.config.output;
    std::fs::write(output.join("research-malformed-incoming-pair.bin"), &raw).unwrap();
    eprintln!("actual malformed-R Receive verified under the same program; adjusted burn=10");
}

#[test]
#[ignore = "heavy actual recursive program fixed point; run in the coordinated proof lane"]
fn complete_recursive_bootstrap_load_send_receive() {
    run_recursive_chain(ChainRun::Qualify);
}

#[test]
#[ignore = "layout measurement only; selected layout must pass the complete native containment gate"]
fn measure_recursive_layout_candidate() {
    run_recursive_chain(ChainRun::Measure);
}

/// The retail build checks the current compiler against its independently signed carrier.
#[test]
#[ignore = "compiles the eight closed wallet programs for the application source check"]
fn compile_wallet_program_bundle() {
    run_recursive_chain(ChainRun::Compile);
}

/// Construct the ordinary monetary seed predecessors with the proposed three-coordinate
/// selector and both complete folded claims. The recursive compiler consumes these
/// private sizing witnesses on its first iteration. It cannot publish any seed tape.
#[test]
#[ignore = "proves each complete monetary sizing seed at the proposed five-kind dimensions"]
fn build_five_kind_monetary_seed_predecessors() {
    use crate::r1cs::Circuit;
    let config = RunConfig::from_environment();
    let gens = empty_generators::<{ crate::deferral::MAX_KINDS }>(&config);
    let chain = crate::native::device_fixture::chain(&gens.p);
    let seed = generation(&config, &chain, &gens, None, 0);
    for p in &seed.kinds {
        let kind = p.kind;
        let corner = crate::deferral::kind_corner(crate::deferral::MAX_KINDS, kind).unwrap();
        for (i, slot) in [KIND_IO, KIND_IO + 1, KIND_C2_IO].into_iter().enumerate() {
            assert_eq!(p.ip.io[slot], Fp::from(corner[i]));
            assert_eq!(p.is.io[slot], Fq::from(corner[i]));
        }
        assert_eq!(p.cp.wallet_io(), Some(TABLE_IO));
        assert_eq!(p.cs.wallet_io(), None);
        assert!(
            p.cp.late_slots()
                .starts_with(&(LATE_IO..LATE_IO + gens.defer_p.n() + 1).collect::<Vec<_>>())
        );
        assert!(
            p.cs.late_slots()
                .starts_with(&(LATE_IO..LATE_IO + gens.defer_s.n() + 1).collect::<Vec<_>>())
        );
        let traced = trace_uniform(p.clone(), &gens);
        assert_eq!(traced.acc_p, gens.defer_p.empty_claim());
        assert_eq!(traced.acc_s, gens.defer_s.empty_claim());
        assert_eq!(matrix_checks(p, &gens), (true, true));
        let pgen = inherited_generator_claim(&traced.tr_p, &gens.p).unwrap();
        let sgen = inherited_generator_claim(&traced.tr_s, &gens.s).unwrap();
        assert!(crate::generator_claim::decide(&gens.p, &pgen));
        assert!(crate::generator_claim::decide(&gens.s, &sgen));
    }
}

enum ChainRun {
    Compile,
    Measure,
    Qualify,
}

fn run_recursive_chain(mode: ChainRun) {
    let config = RunConfig::from_environment();
    eprintln!("full backend test config: {config:?}; dimensions={DIMENSIONS:?}");
    let seed_gens = empty_generators::<4>(&config);
    let chain = crate::native::device_fixture::chain(&seed_gens.p);
    let mut previous = generation(&config, &chain, &seed_gens, None, 0);
    let mut unchanged_geometry = 0;
    for hop in 1..=12 {
        let gens = previous.generators();
        let next = generation(&config, &chain, &gens, Some(&previous), hop);
        let before = previous.program_ids();
        let after = next.program_ids();
        let same_structure = previous.kinds.iter().zip(&next.kinds).all(|(a, b)| {
            a.cp.layout() == b.cp.layout()
                && a.cs.layout() == b.cs.layout()
                && a.cp.public_program_counts() == b.cp.public_program_counts()
                && a.cs.public_program_counts() == b.cs.public_program_counts()
        });
        unchanged_geometry = if same_structure {
            unchanged_geometry + 1
        } else {
            0
        };
        for kind in 0..4 {
            for side in 0..2 {
                let hex = |v: &[u8; 32]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
                eprintln!(
                    "generation {hop} kind {kind} side {side} program {} -> {} ({})",
                    hex(&before[kind][side]),
                    hex(&after[kind][side]),
                    if before[kind][side] == after[kind][side] {
                        "stable"
                    } else {
                        "changed"
                    },
                );
                if before[kind][side] != after[kind][side] {
                    let difference = if side == 0 {
                        previous.kinds[kind]
                            .cp
                            .public_program_differences(&next.kinds[kind].cp)
                    } else {
                        previous.kinds[kind]
                            .cs
                            .public_program_differences(&next.kinds[kind].cs)
                    };
                    eprintln!("generation {hop} kind {kind} side {side}: {difference}");
                }
            }
        }
        if after == before {
            // Equal digests are the fixed point; the complete programs must
            // also be exactly equal, byte for byte, in their canonical form.
            let differences =
                exact_program_differences(&program_set(&previous), &program_set(&next));
            assert!(
                differences.is_empty(),
                "generation {hop}: equal program digests but different complete programs: {differences:?}"
            );
            eprintln!(
                "generation {hop}: all eight complete public programs exactly equal to generation {}: canonical digests and canonical program bytes",
                hop - 1
            );
            if let Some((primary, secondary)) = crate::native::built_in_producer_programs() {
                assert_eq!(
                    after.map(|pair| pair[0]),
                    primary,
                    "published primary producer pins"
                );
                assert_eq!(
                    after.map(|pair| pair[1]),
                    secondary,
                    "published secondary producer pins"
                );
            }
            if matches!(mode, ChainRun::Compile) {
                let bundle = crate::native::ProgramBundle {
                    version: crate::native::FUTURE_ARTIFACT_VERSION,
                    primary: std::array::from_fn(|k| next.kinds[k].cp.export_program().unwrap()),
                    secondary: std::array::from_fn(|k| next.kinds[k].cs.export_program().unwrap()),
                };
                let bytes = bundle.encode().unwrap();
                std::fs::write(config.output.join("program-bundle.norito"), bytes).unwrap();
                return;
            }
            let final_gens = next.generators();
            // The fixed-point generation's own pairs are construction
            // artifacts over the final programs; the native boundary must
            // refuse each whose accumulators are false.
            let templates = template_artifacts(&next, &final_gens);
            let final_chain = final_chain(&chain, &final_gens, next);
            // Record the actual complete monetary bytes before the separate
            // program-distribution size gate, while requiring both gates below.
            let bytes = complete_payment(&config, &chain, &final_chain.kinds[SEND], &final_gens);
            drop(imported_programs(&final_chain));
            malformed_receive_burn(&chain, &final_gens, &final_chain);
            let artifacts = config.output.join("native-device");
            if matches!(mode, ChainRun::Qualify) {
                crate::native::test_full_chain_native(
                    &chain,
                    &final_chain.kinds,
                    &final_chain.receiver_boot,
                    &templates,
                    DIMENSIONS,
                    &artifacts,
                )
            } else {
                crate::native::test_measure_chain_native(
                    &chain,
                    &final_chain.kinds,
                    &final_chain.receiver_boot,
                    DIMENSIONS,
                    &artifacts,
                )
            }
            .expect("complete native Send/Receive prepare, resume, finalize and verify flow");
            drop(templates);
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
            let incoming = &final_chain.kinds[RECEIVE];
            let catalog = &final_chain.kinds[CONTROL];
            let refresh = crate::control::tests::recursive_refresh(
                &received.after_core,
                &received.after_rest,
                &incoming.ip.io,
                &chain.receiver_key,
                &chain.receiver_bootstrap,
            );
            let refreshed = prove_control_successor(
                &config,
                &final_gens,
                &chain.receiver_context.root.root,
                (&catalog.cp, &catalog.cs),
                incoming,
                &refresh,
            );
            assert_eq!(refresh.after_core[9], Fp::from(10));
            let unload = crate::control::Witness::unload(crate::unload::tests::recursive_unload(
                &refresh,
                refreshed.ip.io.clone().try_into().unwrap(),
                &chain.receiver_key,
            ));
            let redeemed = prove_control_successor(
                &config,
                &final_gens,
                &chain.receiver_context.root.root,
                (&catalog.cp, &catalog.cs),
                &refreshed,
                &unload,
            );
            assert_eq!(unload.after_core[9], Fp::from(6));
            assert_eq!(
                redeemed.ip.io[HEAD_IO],
                crate::monetary_state::commitment(&unload.after_core, &unload.after_rest)
            );
            assert!(
                bytes <= payment_wire::QR_BUDGET,
                "complete Payment {bytes} B exceeds {} B",
                payment_wire::QR_BUDGET
            );
            return;
        }
        assert!(
            unchanged_geometry < 2,
            "exact public programs changed after two generations of identical geometry and instruction counts; inspect the component differences above before further proofs"
        );
        previous = next;
    }
    panic!("the exact four-kind programs did not reach a fixed point in twelve generations");
}

/// Prove a semantic Control successor against the exact installed pair. The
/// fixed-point Load witness must produce both identical public programs for
/// Refresh and Unload. Both accumulated matrices and generators are decided.
fn prove_control_successor(
    config: &RunConfig,
    gens: &PairGens,
    root: &p256::Point,
    fixed: (&Arc<UniformCircuit<Fp>>, &Arc<UniformCircuit<Fq>>),
    predecessor: &UniformPair,
    witness: &crate::control::Witness,
) -> UniformPair {
    let prior = [trace_uniform(predecessor.clone(), gens)];
    let coins = crate::fs::ProofEntropy::fresh().expect("OS entropy for Control");
    let folds = GeneratorFolds::prepare(gens, &prior, &coins).unwrap();
    let mode = witness.mode;
    let (wp, _) = p_operation(
        OperationWitness::Control { root, witness },
        gens,
        &prior,
        &folds,
    )
    .unwrap();
    let (ws, _) = s_operation(CONTROL, Some(mode), gens, &prior, &folds);
    let (cp, ip, mut pw) = build(config, wp, true, Some(fixed.0.layout()));
    let (cs, is, mut sw) = build(config, ws, false, Some(fixed.1.layout()));
    let at = crate::control::MODE_IO;
    let bits = mode.bits();
    assert_eq!(&ip.io[at..at + 2], &bits.map(Fp::from));
    assert_eq!(&is.io[at..at + 2], &bits.map(Fq::from));
    assert!(
        cp.export_program().unwrap() == fixed.0.export_program().unwrap(),
        "exact Control P"
    );
    assert!(
        cs.export_program().unwrap() == fixed.1.export_program().unwrap(),
        "exact Control S"
    );
    let pair = crate::prove_linked_pair(
        "three-mode Control successor",
        300 + mode.tag() as usize,
        &cp,
        &ip,
        &pw,
        &cs,
        &is,
        &sw,
    );
    assert!(pair.ok, "both Control proofs verified");
    zeroize::Zeroize::zeroize(&mut pw);
    zeroize::Zeroize::zeroize(&mut sw);
    let result = crate::linked_uniform(fixed.0, ip, fixed.1, is, pair, CONTROL);
    assert_eq!(matrix_checks(&result, gens), (true, true));
    let trace = trace_uniform(result.clone(), gens);
    let p = inherited_generator_claim(&trace.tr_p, &gens.p).unwrap();
    let s = inherited_generator_claim(&trace.tr_s, &gens.s).unwrap();
    assert!(crate::generator_claim::decide(&gens.p, &p));
    assert!(crate::generator_claim::decide(&gens.s, &s));
    eprintln!(
        "full backend Control {mode:?}: exact catalog, linked pair and all four accumulators accepted"
    );
    result
}

type ProgramSet = [(Arc<UniformCircuit<Fp>>, Arc<UniformCircuit<Fq>>); 4];

fn program_set(generation: &Generation) -> ProgramSet {
    std::array::from_fn(|k| {
        (
            generation.kinds[k].cp.clone(),
            generation.kinds[k].cs.clone(),
        )
    })
}

/// Every difference between two complete four-kind program sets, P and S:
/// canonical program digest and the exact canonical exported program bytes
/// (instructions, constants, fixed values, geometry and IO ABI), not sampled
/// matrix evaluations. Each pair of exports is released before the next.
fn exact_program_differences(before: &ProgramSet, after: &ProgramSet) -> Vec<String> {
    fn side<F: Scalar>(
        kind: usize,
        name: &str,
        a: &UniformCircuit<F>,
        b: &UniformCircuit<F>,
        out: &mut Vec<String>,
    ) {
        if a.program_digest() != b.program_digest() {
            out.push(format!("kind {kind} {name}: canonical digest"));
        }
        match (a.export_program(), b.export_program()) {
            (Ok(x), Ok(y)) if x == y => {}
            (Ok(x), Ok(y)) => out.push(format!(
                "kind {kind} {name}: canonical program bytes ({} B and {} B)",
                x.len(),
                y.len()
            )),
            (x, y) => out.push(format!(
                "kind {kind} {name}: export {:?} / {:?}",
                x.err(),
                y.err()
            )),
        }
    }
    let mut out = Vec::new();
    for kind in 0..4 {
        side(kind, "P", &before[kind].0, &after[kind].0, &mut out);
        side(kind, "S", &before[kind].1, &after[kind].1, &mut out);
    }
    out
}

/// The fixed-point generation's non-Bootstrap pairs, each with whether its
/// accumulators hold under the final programs, decided here independently of
/// the native engine. They were proved over the final programs from warm-up
/// predecessors, so linked verification accepts them; at least one must carry
/// a false accumulator for the native refusals to be exercised.
fn template_artifacts(fixed: &Generation, gens: &PairGens) -> Vec<(UniformPair, bool)> {
    let out: Vec<_> = [CONTROL, SEND, RECEIVE]
        .into_iter()
        .map(|kind| {
            let pair = fixed.kinds[kind].clone();
            let (p, s) = matrix_checks(&pair, gens);
            let traced = trace_uniform(pair.clone(), gens);
            let gp = inherited_generator_claim(&traced.tr_p, &gens.p)
                .is_ok_and(|c| crate::generator_claim::decide(&gens.p, &c));
            let gs = inherited_generator_claim(&traced.tr_s, &gens.s)
                .is_ok_and(|c| crate::generator_claim::decide(&gens.s, &c));
            eprintln!(
                "construction artifact kind {kind}: matrix accumulators P {p}, S {s}; inherited generator accumulators P {gp}, S {gs}"
            );
            (pair, p && s && gp && gs)
        })
        .collect();
    assert!(
        out.iter().any(|(_, holds)| !holds),
        "the fixed-point generation must contain a construction artifact with a false accumulator"
    );
    out
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
        [Fp::ZERO; 2],
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

/// A minimal installable public program: `a * factor = b` over one cell
/// pair, with the monetary IO ABI's link slots.
fn tiny_program<F: Scalar + zeroize::Zeroize>(
    primary: bool,
    factor: u64,
) -> Arc<UniformCircuit<F>> {
    use crate::uniform::{Array, constant, own};
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    w.push(Array {
        name: "exact program comparison relation",
        family: f,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![255, 255],
        rows: vec![[own(0, 1), constant(vec![F::from(factor)]), own(1, 1)]],
        values: vec![vec![F::from(7)], vec![F::from(7 * factor)]],
    });
    for slot in if primary {
        vec![124, 125, 126, 127]
    } else {
        vec![124, 126]
    } {
        let a = w.push(Array {
            name: "exact program comparison link slot",
            family: f,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vec![F::ZERO]],
        });
        w.io_arrays.push((a, slot));
        w.link.io.push((a, slot));
    }
    w.link.io_values = Some(Box::new(move |_, _| {
        vec![F::ZERO; if primary { 4 } else { 2 }]
    }));
    let (a, p, n, t, c) = w.finish();
    let (circuit, _, _) = UniformCircuit::build_with_copies(
        if primary { "P-uniform" } else { "S-uniform" },
        a,
        p,
        n,
        t,
        c,
        10,
        64,
        b"exact program comparison",
    );
    Arc::new(circuit.into_verifier_program())
}

/// The fixed-point comparison sees every complete program, per kind and side:
/// identical sets have no difference, and one changed matrix constant in one
/// program is reported for exactly that program, by digest and by bytes.
#[test]
fn fixed_point_compares_all_eight_complete_programs_exactly() {
    let set = |changed: Option<(usize, bool)>| -> ProgramSet {
        std::array::from_fn(|kind| {
            let factor = |primary| {
                if changed == Some((kind, primary)) {
                    5
                } else {
                    3
                }
            };
            (
                tiny_program::<Fp>(true, factor(true)),
                tiny_program::<Fq>(false, factor(false)),
            )
        })
    };
    let base = set(None);
    assert_eq!(
        exact_program_differences(&base, &set(None)),
        Vec::<String>::new()
    );
    for kind in 0..4 {
        for primary in [true, false] {
            let differences = exact_program_differences(&base, &set(Some((kind, primary))));
            let name = if primary { "P" } else { "S" };
            assert_eq!(differences.len(), 2, "{differences:?}");
            assert!(differences[0].starts_with(&format!("kind {kind} {name}: canonical digest")));
            assert!(
                differences[1].starts_with(&format!("kind {kind} {name}: canonical program bytes"))
            );
        }
    }
}

/// Initial proof inputs for the ArchiveSent compiler use complete signed monetary
/// witnesses. As in the ordinary bootstrap compiler, the pre-existing authenticated
/// profiles are pinned constants in this first sizing pass only.
pub(crate) struct ArchiveTransferSeed<'a> {
    pub(crate) predecessor: [Fp; 64],
    pub(crate) incoming: Option<([Fp; 64], Fp)>,
    pub(crate) operation: OperationWitness<'a>,
    pub(crate) primary_opening: crate::ipa::Claim<Eq>,
    pub(crate) secondary_opening: crate::ipa::Claim<Ep>,
}

fn archive_seed_primary(generators: &PairGens, seed: ArchiveTransferSeed<'_>) -> Wiring<Fp> {
    let ArchiveTransferSeed {
        predecessor,
        incoming,
        operation,
        primary_opening,
        ..
    } = seed;
    let kind = operation.kind();
    let mut wiring = Wiring::default();
    let family = wiring.family(0, 1);
    let prior = VerifiedPredecessor::fixture(&mut wiring, family, predecessor);
    let incoming = incoming.map(|(profile, digest)| {
        IncomingProofFacts::fixture(&mut wiring, family, profile, Fp::ONE, digest)
    });
    pin_fixture_arrays(&mut wiring, 0);
    let complete = match operation {
        OperationWitness::Send(witness) => {
            assert!(incoming.is_none());
            CompleteOperation::Send(
                crate::send::append(&mut wiring, family, witness, &prior).unwrap(),
            )
        }
        OperationWitness::Receive(witness) => CompleteOperation::Receive(
            crate::receive::append(&mut wiring, family, witness, &prior, &incoming.unwrap())
                .unwrap(),
        ),
        _ => panic!("compiler transfer seeds are signed Send and Receive originals"),
    };
    finish_seed(
        wiring,
        family,
        generators,
        complete,
        kind,
        Some(&primary_opening),
    )
}

pub(crate) fn archive_seed_transfer(
    generators: &PairGens,
    layout: (Layout, Layout),
    seed: ArchiveTransferSeed<'_>,
    group: CompilerGroup,
) -> UniformPair {
    let kind = seed.operation.kind();
    let secondary = seed_secondary_with_claim(generators, kind, Some(&seed.secondary_opening));
    let wiring = archive_seed_primary(generators, seed);
    let (cp, ip, mut wp) = build_monetary_program_grouped(
        wiring,
        generators.p.g.len().ilog2() as usize,
        true,
        Some(layout.0),
        group,
    );
    let (cs, is, mut ws) = build_monetary_program_grouped(
        secondary,
        generators.s.g.len().ilog2() as usize,
        false,
        Some(layout.1),
        group,
    );
    let proof = crate::prove_linked_pair(
        "ArchiveSent compiler transfer seed",
        kind,
        &cp,
        &ip,
        &wp,
        &cs,
        &is,
        &ws,
    );
    assert!(proof.ok);
    zeroize::Zeroize::zeroize(&mut wp);
    zeroize::Zeroize::zeroize(&mut ws);
    let cp = retain_public_program(cp, true);
    let cs = retain_public_program(cs, false);
    crate::linked_uniform(&cp, ip, &cs, is, proof, kind)
}

/// The exact five-coordinate ordinary IO and both folds are already present
/// in genuine Bootstrap programs. At the proposed common target these tapes
/// determine the retail proof byte layout before proving a seed operation.
pub(crate) fn archive_seed_bootstrap_programs(
    primary: &UniformCircuit<Fp>,
    secondary: &UniformCircuit<Fq>,
    chain: &Chain,
) -> (PairGens, UniformCircuit<Fp>, UniformCircuit<Fq>) {
    let config = archive_config(primary, secondary);
    let empty = empty_generators::<{ crate::deferral::MAX_KINDS }>(&config);
    let p = build(
        &config,
        seed_primary(chain, &empty, Step::SenderBoot),
        true,
        Some(primary.layout()),
    )
    .0;
    let s = build(
        &config,
        seed_secondary(&empty, BOOTSTRAP),
        false,
        Some(secondary.layout()),
    )
    .0;
    assert_eq!(
        (p.layout(), s.layout()),
        (primary.layout(), secondary.layout())
    );
    (empty, p, s)
}

pub(crate) fn archive_seed_family(
    primary: &Arc<UniformCircuit<Fp>>,
    secondary: &Arc<UniformCircuit<Fq>>,
    chain: &Chain,
    generators: &PairGens,
) -> (UniformPair, UniformPair, UniformPair) {
    let cfg = archive_config(primary, secondary);
    let target = (primary.layout(), secondary.layout());
    let run = |step| prove_step(&cfg, chain, generators, None, step, target, 0);
    let boot = run(Step::SenderBoot);
    let receiver_boot = run(Step::ReceiverBoot);
    let control = run(Step::Load);
    (boot, receiver_boot, control)
}

fn archive_config(p: &UniformCircuit<Fp>, s: &UniformCircuit<Fq>) -> RunConfig {
    RunConfig {
        group: CompilerGroup::of(p, s),
        p_columns: p.shape.col_bits,
        s_columns: s.shape.col_bits,
        output: Default::default(),
    }
}

impl Generation {
    pub(crate) fn compiler_group(&self) -> CompilerGroup {
        self.config.group
    }

    pub(crate) fn transfer_seeds(kinds: [UniformPair; 4], receiver_boot: UniformPair) -> Self {
        let config = archive_config(&kinds[0].cp, &kinds[0].cs);
        assert_eq!(
            receiver_boot.cp.program_digest(),
            kinds[BOOTSTRAP].cp.program_digest()
        );
        assert_eq!(
            receiver_boot.cs.program_digest(),
            kinds[BOOTSTRAP].cs.program_digest()
        );
        Self {
            config,
            kinds,
            receiver_boot,
        }
    }

    pub(crate) fn archive_seed_requirements(
        chain: &Chain,
        columns: [usize; 2],
        group: CompilerGroup,
        send: ArchiveTransferSeed<'_>,
        receive: ArchiveTransferSeed<'_>,
    ) -> (Vec<Requirements>, Vec<Requirements>) {
        let config = RunConfig {
            group,
            p_columns: columns[0],
            s_columns: columns[1],
            output: Default::default(),
        };
        let empty = empty_generators::<{ crate::deferral::MAX_KINDS }>(&config);
        let mut p = Vec::with_capacity(4);
        let mut s = Vec::with_capacity(4);
        for (step, exact) in [
            (Step::Receive, Some(receive)),
            (Step::Send, Some(send)),
            (Step::SenderBoot, None),
            (Step::Load, None),
        ] {
            let secondary = seed_secondary_with_claim(
                &empty,
                step.kind(),
                exact.as_ref().map(|seed| &seed.secondary_opening),
            );
            let wire = match exact {
                Some(seed) => archive_seed_primary(&empty, seed),
                None => seed_primary(chain, &empty, step),
            };
            p.push(Requirements::of(&build(&config, wire, true, None).0));
            s.push(Requirements::of(&build(&config, secondary, false, None).0));
        }
        (p, s)
    }

    pub(crate) fn archive_ordinary_requirements(
        &self,
        chain: &Chain,
        gens: &PairGens,
    ) -> (Vec<Requirements>, Vec<Requirements>) {
        ordinary_requirements(&self.config, chain, gens, Some(self))
    }

    pub(crate) fn archive_targets(
        &self,
        chain: &Chain,
        gens: &PairGens,
        archive: [Requirements; 2],
        limits: [usize; 2],
    ) -> (Layout, Layout) {
        let (mut p, mut s) = ordinary_requirements(&self.config, chain, gens, Some(self));
        p.push(archive[0]);
        s.push(archive[1]);
        let target = |r: &[Requirements], cap| {
            eprintln!("five natural program requirements {r:?} fixed row limit {cap}");
            bootstrap_layout::common_export_layout(r, cap).unwrap()
        };
        (target(&p, limits[0]), target(&s, limits[1]))
    }

    pub(crate) fn next_archive_round(
        &self,
        chain: &Chain,
        gens: &PairGens,
        round: usize,
        target: (Layout, Layout),
    ) -> Self {
        generation_at(&self.config, chain, gens, Some(self), round, target)
    }

    pub(crate) fn final_wallet_chain(self, chain: &Chain, gens: &PairGens) -> Self {
        final_chain(chain, gens, self)
    }

    // Export all eight ordinary tapes at the common five-kind layout BEFORE
    // proving the round. The fifth tape for either curve comes from ArchiveSent.
    pub(crate) fn preflight_ordinary(
        &self,
        chain: &Chain,
        gens: &PairGens,
        target: (Layout, Layout),
    ) -> ([CompiledTape; 4], [CompiledTape; 4]) {
        let mut p = std::array::from_fn(|_| CompiledTape::default());
        let mut s = std::array::from_fn(|_| CompiledTape::default());
        for step in [Step::Receive, Step::Send, Step::SenderBoot, Step::Load] {
            let plan = OperationPlan::new(gens, self, step);
            let kind = step.kind();
            p[kind] = CompiledTape::capture(
                &build(
                    &self.config,
                    primary(chain, gens, Some(&plan), step),
                    true,
                    Some(target.0),
                )
                .0,
            );
            s[kind] = CompiledTape::capture(
                &build(
                    &self.config,
                    secondary(gens, Some(&plan), step),
                    false,
                    Some(target.1),
                )
                .0,
            );
            eprintln!(
                "five-kind program preflight {step:?}: P {} B, S {} B",
                p[kind].bytes.len(),
                s[kind].bytes.len()
            );
        }
        (p, s)
    }

    pub(crate) fn receive_witness(
        &self,
        chain: &Chain,
        gens: &PairGens,
    ) -> crate::receive::Witness {
        let pairs = recursive_pairs(self, gens, Step::Receive);
        let raw = incoming_encoding(&pairs[1]);
        let valid = incoming_verdict_values(gens, &pairs[1])
            .into_iter()
            .all(|b| b == 1);
        crate::receive_fixture::from_send_with_validity(
            &chain.receiver_bootstrap,
            &chain.receiver_key,
            &chain.send,
            &raw.bytes,
            pairs[0].inst_p.io.clone().try_into().unwrap(),
            valid,
        )
        .unwrap()
    }
}

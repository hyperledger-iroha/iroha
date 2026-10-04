//! Unit tests of the vendored-circuit export.
//!
//! `Mixed` exercises everything the golden circuits do not: simple and
//! complex selectors (compressed and not), `lookup` and `lookup_any`, global
//! constants, an instance cell read through `assign_advice_from_instance`, a
//! rational-free fixed coefficient column and copies across advice, fixed and
//! instance columns. Its exported keys must equal the vendored ones with and
//! without selector compression, and in oracle builds its native proofs must
//! equal the vendored proofs in both instance modes.

use std::marker::PhantomData;

use halo2_axiom::{
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::{AdviceCellValue, CellValue, MockProver},
    halo2curves::ff::{Field, PrimeField},
    plonk::{
        Advice as VAdvice, Circuit, Column as VColumn, ConstraintSystem as VCs, Error,
        Expression as VExpression, FirstPhase, Fixed as VFixed, Instance as VInstance, SecondPhase,
        Selector as VSelector, TableColumn, keygen_vk as v_keygen_vk, keygen_vk_custom,
    },
    poly::{Rotation as VRotation, commitment::ParamsProver, ipa::commitment::ParamsIPA},
};
use iroha_plonk::{
    cs::{ConstraintSystem, InstanceModeV1, PermutationError, ProofSuffixV1, TranscriptV1},
    pcs::ipa::PinnedParams,
};
use rayon::iter::ParallelIterator;

use super::*;
use crate::convert::{Pallas, Vesta, native_scalars};

/// Rows of the multiply-add chain.
const ROWS: usize = 12;
/// Domain size exponent of the tests.
const K: u32 = 6;

/// The chain `(a, b, c)` with `c = a b` on even rows and `c = a + 5 b` on odd
/// rows, `a_{i+1} = c_i`.
fn chain<F: PrimeField>(start: u64) -> Vec<(F, F, F)> {
    let mut a = F::from(start);
    (0..ROWS)
        .map(|row| {
            let b = F::from(3 + u64::try_from(row).expect("small"));
            let c = if row % 2 == 0 {
                a * b
            } else {
                a + F::from(5) * b
            };
            let out = (a, b, c);
            a = c;
            out
        })
        .collect()
}

/// The public input: the chain start and its last output.
fn public<F: PrimeField>(start: u64) -> Vec<F> {
    vec![F::from(start), chain::<F>(start).last().expect("rows").2]
}

/// Columns of [`Mixed`].
#[derive(Clone, Copy)]
struct MixedConfig {
    a: VColumn<VAdvice>,
    b: VColumn<VAdvice>,
    c: VColumn<VAdvice>,
    x: VColumn<VAdvice>,
    y: VColumn<VAdvice>,
    coeff: VColumn<VFixed>,
    t0: TableColumn,
    t1: TableColumn,
    instance: VColumn<VInstance>,
    s_mul: VSelector,
    s_add: VSelector,
    s_link: VSelector,
    s_sq: VSelector,
    q: VSelector,
}

/// A circuit with selectors, lookups, constants, an instance read and copies.
#[derive(Clone)]
struct Mixed<F> {
    start: u64,
    witness: bool,
    marker: PhantomData<F>,
}

impl<F> Mixed<F> {
    fn new(start: u64) -> Self {
        Self {
            start,
            witness: true,
            marker: PhantomData,
        }
    }
}

// The column names follow the algebra the gates state.
#[allow(clippy::many_single_char_names)]
impl<F: PrimeField> Circuit<F> for Mixed<F> {
    type Config = MixedConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            witness: false,
            ..self.clone()
        }
    }

    fn configure(meta: &mut VCs<F>) -> MixedConfig {
        let a = meta.advice_column();
        let b = meta.advice_column();
        let c = meta.advice_column();
        let x = meta.advice_column();
        let y = meta.advice_column();
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        let coeff = meta.fixed_column();
        let t0 = meta.lookup_table_column();
        let t1 = meta.lookup_table_column();
        let instance = meta.instance_column();
        meta.enable_equality(a);
        meta.enable_equality(b);
        meta.enable_equality(c);
        meta.enable_equality(instance);
        let s_mul = meta.selector();
        let s_add = meta.selector();
        let s_link = meta.selector();
        let s_sq = meta.selector();
        let q = meta.complex_selector();
        meta.create_gate("mul", |cells| {
            let s = cells.query_selector(s_mul);
            let a = cells.query_advice(a, VRotation::cur());
            let b = cells.query_advice(b, VRotation::cur());
            let c = cells.query_advice(c, VRotation::cur());
            vec![s * (a * b - c)]
        });
        meta.create_gate("add", |cells| {
            let s = cells.query_selector(s_add);
            let a = cells.query_advice(a, VRotation::cur());
            let b = cells.query_advice(b, VRotation::cur());
            let k = cells.query_fixed(coeff, VRotation::cur());
            let c = cells.query_advice(c, VRotation::cur());
            vec![s * ((a + k * b - c) * F::from(7))]
        });
        meta.create_gate("link", |cells| {
            let s = cells.query_selector(s_link);
            let next = cells.query_advice(a, VRotation::next());
            let c = cells.query_advice(c, VRotation::cur());
            vec![s * (next - c)]
        });
        meta.create_gate("square", |cells| {
            let s = cells.query_selector(s_sq);
            let x = cells.query_advice(x, VRotation::cur());
            let y = cells.query_advice(y, VRotation::cur());
            vec![s * (y - x.clone() * x)]
        });
        meta.lookup("pairs", |cells| {
            let q = cells.query_selector(q);
            let x = cells.query_advice(x, VRotation::cur());
            let y = cells.query_advice(y, VRotation::cur());
            vec![(q.clone() * x, t0), (q * y, t1)]
        });
        meta.lookup_any("range", |cells| {
            let q = cells.query_selector(q);
            let x = cells.query_advice(x, VRotation::cur());
            let t = cells.query_fixed(t0.inner(), VRotation::cur());
            vec![(q * (x + VExpression::Constant(F::ONE)), t)]
        });
        MixedConfig {
            a,
            b,
            c,
            x,
            y,
            coeff,
            t0,
            t1,
            instance,
            s_mul,
            s_add,
            s_link,
            s_sq,
            q,
        }
    }

    fn synthesize(&self, config: MixedConfig, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        layouter.assign_table(
            || "squares",
            |mut table| {
                for i in 0..16_u64 {
                    let offset = usize::try_from(i).expect("small");
                    table.assign_cell(|| "i", config.t0, offset, || Value::known(F::from(i)))?;
                    table.assign_cell(
                        || "i^2",
                        config.t1,
                        offset,
                        || Value::known(F::from(i * i)),
                    )?;
                }
                Ok(())
            },
        )?;
        let rows = chain::<F>(self.start);
        let witness = |value: F| {
            if self.witness {
                Value::known(value)
            } else {
                Value::unknown()
            }
        };
        let last = layouter.assign_region(
            || "chain",
            |mut region| {
                let mut last = None;
                for (row, (a, b, c)) in rows.iter().enumerate() {
                    if row % 2 == 0 {
                        config.s_mul.enable(&mut region, row)?;
                    } else {
                        config.s_add.enable(&mut region, row)?;
                        region.assign_fixed(config.coeff, row, F::from(5));
                    }
                    if row + 1 < rows.len() {
                        config.s_link.enable(&mut region, row)?;
                    }
                    if row == 0 {
                        region.assign_advice_from_instance(
                            || "start",
                            config.instance,
                            0,
                            config.a,
                            0,
                        )?;
                        region.assign_advice_from_constant(|| "three", config.b, 0, *b)?;
                    } else {
                        region.assign_advice(config.a, row, witness(*a));
                        region.assign_advice(config.b, row, witness(*b));
                    }
                    last = Some(region.assign_advice(config.c, row, witness(*c)).cell());
                    let small = (u64::try_from(row).expect("small") * 5 + 2) % 15;
                    config.s_sq.enable(&mut region, row)?;
                    config.q.enable(&mut region, row)?;
                    region.assign_advice(config.x, row, witness(F::from(small)));
                    region.assign_advice(config.y, row, witness(F::from(small * small)));
                }
                last.ok_or(Error::Synthesis)
            },
        )?;
        layouter.constrain_instance(last, config.instance, 1);
        Ok(())
    }
}

/// The vendored native value of a `MockProver` fixed cell.
fn fixed_cell<B: CurveBridge>(cell: &CellValue<B::VScalar>) -> NativeScalar<B> {
    match cell {
        CellValue::Assigned(value) => native_scalar::<B>(value),
        CellValue::Unassigned | CellValue::Poison(_) => NativeScalar::<B>::default(),
    }
}

/// The vendored advice column with `index`.
fn advice_column<F: Field>(index: usize) -> VColumn<VAdvice> {
    let mut cs = VCs::<F>::default();
    let mut column = cs.advice_column();
    for _ in 0..index {
        column = cs.advice_column();
    }
    column
}

/// Exports [`Mixed`] and checks the keys and tables against the vendored
/// ones with and without selector compression.
fn mixed_keys<B: CurveBridge>(compress: bool) {
    let circuit = Mixed::<B::VScalar>::new(3);
    let instances = vec![public::<B::VScalar>(3)];
    let vendored_params = ParamsIPA::<B::Vendored>::new(K);
    let vk = keygen_vk_custom(&vendored_params, &circuit.without_witnesses(), compress)
        .expect("vendored vk");
    assert_eq!(vendored_compress_selectors::<B>(&vk), compress);
    let exported = export_circuit::<B, _>(K, &circuit, &instances).expect("export");
    let (configured, _) = configure_vendored::<B::VScalar, Mixed<B::VScalar>>(&circuit);
    compare_constraint_systems::<B>(&configured, exported.constraint_system())
        .expect("configure-time system");
    let params = PinnedParams::<B::Native>::derive(K).expect("params");
    let config =
        vendored_keygen_config::<B>(&vk, TranscriptV1::Blake2bChallenge255, ProofSuffixV1::None);
    assert_eq!(config.compress_selectors, compress);
    let pk = exported.keygen(&params, &config).expect("native pk");
    compare_constraint_systems::<B>(vk.cs(), pk.constraint_system().constraint_system())
        .expect("finalized system");
    assert_eq!(
        pk.vk().to_bytes(),
        &vendored_vk_bytes::<B>(&vk)[..],
        "VK bytes"
    );

    let mock = MockProver::run(K, &circuit, instances.clone()).expect("mock");
    mock.assert_satisfied();
    // MockProver always compresses; compare its selector columns only then.
    let fixed = if compress {
        pk.fixed_values()
    } else {
        &pk.fixed_values()[..configured.num_fixed_columns()]
    };
    for (column, native) in fixed.iter().enumerate() {
        let vendored: Vec<_> = mock.fixed()[column].iter().map(fixed_cell::<B>).collect();
        assert_eq!(&vendored, native, "fixed column {column}");
    }
    let usable = (1 << K) - (configured.blinding_factors() + 1);
    for (column, native) in exported.advice().iter().enumerate() {
        let vendored = mock.advice_values(advice_column::<B::VScalar>(column));
        for row in 0..usable {
            let AdviceCellValue::Assigned(value) = &vendored[row] else {
                panic!("poisoned usable row");
            };
            assert_eq!(native[row], native_scalar::<B>(&value.as_ref().evaluate()));
        }
    }
    let permutation = exported.permutation().expect("copies");
    for (column, rows) in mock.permutation().mapping().enumerate() {
        let rows: Vec<_> = rows.collect();
        for (row, target) in rows.into_iter().enumerate() {
            assert_eq!(permutation.mapping(column, row), Some(target));
        }
    }
    let witness = exported.witness(&pk).expect("witness");
    assert_eq!(witness.instances(), exported.instances());
}

#[test]
fn mixed_circuit_keys_match_vendored() {
    for compress in [false, true] {
        mixed_keys::<Vesta>(compress);
        mixed_keys::<Pallas>(compress);
    }
}

#[test]
fn exported_tables_have_the_circuit_shape() {
    let circuit = Mixed::<<Vesta as CurveBridge>::VScalar>::new(5);
    let instances = vec![public(5)];
    let exported = export_circuit::<Vesta, _>(K, &circuit, &instances).expect("export");
    assert_eq!(exported.k(), K);
    let n = 1 << K;
    assert_eq!(exported.fixed().len(), 4);
    assert!(exported.fixed().iter().all(|column| column.len() == n));
    assert_eq!(exported.selectors().len(), 5);
    assert!(exported.selectors().iter().all(|rows| rows.len() == n));
    assert_eq!(exported.advice().len(), 5);
    assert_eq!(
        exported.instances(),
        &[native_scalars::<Vesta>(&instances[0])]
    );
    // The instance read, the constant and the output copy, at least.
    assert!(exported.copies().len() >= 3);
    assert_eq!(exported.constraint_system().instance_lengths(), &[2]);
    // Fixed coefficient 5 on the odd rows.
    assert_eq!(exported.fixed()[1][1], NativeScalar::<Vesta>::from(5));
    assert!(exported.selectors()[1][1] && !exported.selectors()[1][0]);
}

/// A vendored proof of [`Mixed`] in the instance mode `QUERY_INSTANCE`.
#[cfg(iroha_plonk_oracle)]
fn mixed_proofs<B: CurveBridge, const QUERY_INSTANCE: bool>(compress: bool) {
    use halo2_axiom::{
        plonk::{create_proof, keygen_pk as v_keygen_pk, verify_proof},
        poly::{
            VerificationStrategy,
            ipa::{
                commitment::IPACommitmentScheme,
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };
    use iroha_pasta::msm::MemoryBudget;
    use iroha_plonk::{
        prover::{ProverConfig, ProverRandomness, create_proof_oracle},
        verifier::verify_full_oracle,
    };
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    let circuit = Mixed::<B::VScalar>::new(3);
    let instances = vec![public::<B::VScalar>(3)];
    let vendored_params = ParamsIPA::<B::Vendored>::new(K);
    let vk = keygen_vk_custom(&vendored_params, &circuit.without_witnesses(), compress)
        .expect("vendored vk");
    let vendored_pk =
        v_keygen_pk(&vendored_params, vk, &circuit.without_witnesses()).expect("vendored pk");
    let columns: Vec<&[B::VScalar]> = instances.iter().map(Vec::as_slice).collect();
    let per_proof: [&[&[B::VScalar]]; 1] = [&columns];
    let mut transcript = Blake2bWrite::<_, B::Vendored, Challenge255<_>>::init(Vec::new());
    create_proof::<
        IPACommitmentScheme<B::Vendored>,
        ProverIPA<'_, B::Vendored, QUERY_INSTANCE, 0>,
        _,
        _,
        _,
        _,
    >(
        &vendored_params,
        &vendored_pk,
        std::slice::from_ref(&circuit),
        &per_proof,
        ChaCha20Rng::from_seed([42; 32]),
        &mut transcript,
    )
    .expect("vendored proof");
    let vendored = transcript.finalize();

    let exported = export_circuit::<B, _>(K, &circuit, &instances).expect("export");
    let params = PinnedParams::<B::Native>::derive(K).expect("params");
    let mut config = vendored_keygen_config::<B>(
        vendored_pk.get_vk(),
        TranscriptV1::Blake2bChallenge255,
        ProofSuffixV1::None,
    );
    if !QUERY_INSTANCE {
        config.instance_mode = InstanceModeV1::Direct;
    }
    let pk = exported.keygen(&params, &config).expect("native pk");
    let witness = exported.witness(&pk).expect("witness");
    let repr = vendored_transcript_repr::<B>(vendored_pk.get_vk());
    let native = create_proof_oracle(
        &params,
        &pk,
        &witness,
        ProverRandomness::fixed_seed_for_tests([42; 32]),
        ProverConfig::default(),
        repr,
    )
    .expect("native proof");
    assert_eq!(
        native,
        vendored,
        "{} compress={compress} committed={QUERY_INSTANCE}",
        B::NAME
    );
    verify_full_oracle(
        &params,
        pk.binding(),
        pk.vk(),
        exported.instances(),
        &vendored,
        MemoryBudget::DEFAULT,
        repr,
    )
    .expect("native verifier accepts the vendored proof");
    let mut reader = Blake2bRead::<_, B::Vendored, Challenge255<_>>::init(&native[..]);
    let strategy = SingleStrategy::<'_, B::Vendored, QUERY_INSTANCE, 0>::new(&vendored_params);
    verify_proof::<
        IPACommitmentScheme<B::Vendored>,
        VerifierIPA<'_, B::Vendored, QUERY_INSTANCE, 0>,
        _,
        _,
        _,
    >(
        &vendored_params,
        vendored_pk.get_vk(),
        strategy,
        &per_proof,
        &mut reader,
    )
    .expect("vendored verifier accepts the native proof");
}

#[cfg(iroha_plonk_oracle)]
#[test]
fn mixed_circuit_proofs_match_vendored() {
    for compress in [false, true] {
        mixed_proofs::<Vesta, true>(compress);
        mixed_proofs::<Pallas, true>(compress);
        mixed_proofs::<Vesta, false>(compress);
        mixed_proofs::<Pallas, false>(compress);
    }
}

/// A circuit with a second-phase advice column and a challenge.
#[derive(Clone, Default)]
struct TwoPhase;

impl Circuit<<Vesta as CurveBridge>::VScalar> for TwoPhase {
    type Config = ();
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self
    }

    fn configure(meta: &mut VCs<<Vesta as CurveBridge>::VScalar>) {
        let first = meta.advice_column_in(FirstPhase);
        let second = meta.advice_column_in(SecondPhase);
        let challenge = meta.challenge_usable_after(FirstPhase);
        meta.create_gate("phases", |cells| {
            let a = cells.query_advice(first, VRotation::cur());
            let b = cells.query_advice(second, VRotation::cur());
            let r = cells.query_challenge(challenge);
            vec![a * r - b]
        });
    }

    fn synthesize(
        &self,
        (): (),
        _: impl Layouter<<Vesta as CurveBridge>::VScalar>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

/// DEV-11 (spec section 14): later phases and challenges have no PIPA-v1 form and are rejected
/// (Hybrid instances and multi-circuit proofs have no API at all).
#[test]
fn multi_phase_circuits_and_challenges_are_rejected() {
    let (cs, ()) = configure_vendored::<_, TwoPhase>(&TwoPhase);
    assert!(matches!(
        export_constraint_system::<Vesta>(&cs, &[]),
        Err(ExportError::Phases)
    ));
    let poly = &cs.gates()[0].polynomials()[0];
    let VExpression::Sum(product, _) = poly else {
        panic!("a * r - b is a sum");
    };
    let VExpression::Product(_, challenge) = product.as_ref() else {
        panic!("a * r is a product");
    };
    assert!(matches!(
        export_expression::<Vesta>(challenge),
        Err(ExportError::Challenge)
    ));
    let second = cs.permutation().get_columns();
    assert!(second.is_empty());
    let mut with_equality = VCs::<<Vesta as CurveBridge>::VScalar>::default();
    with_equality.advice_column_in(FirstPhase);
    let late = with_equality.advice_column_in(SecondPhase);
    with_equality.enable_equality(late);
    let column = with_equality.permutation().get_columns()[0];
    assert!(matches!(export_column(&column), Err(ExportError::Phases)));
    assert!(matches!(
        export_circuit::<Vesta, _>(K, &TwoPhase, &[]),
        Err(ExportError::Phases)
    ));
}

#[test]
fn instance_counts_and_domain_sizes_are_checked() {
    let circuit = Mixed::<<Vesta as CurveBridge>::VScalar>::new(3);
    let (cs, _) = configure_vendored::<_, Mixed<_>>(&circuit);
    assert!(matches!(
        export_constraint_system::<Vesta>(&cs, &[]),
        Err(ExportError::InstanceColumns {
            expected: 1,
            found: 0
        })
    ));
    assert!(matches!(
        export_circuit::<Vesta, _>(2, &circuit, &[public(3)]),
        Err(ExportError::Rows { k: 2 })
    ));
    assert!(matches!(
        export_circuit::<Vesta, _>(K, &circuit, &[vec![]]),
        Err(ExportError::Synthesis(Error::BoundsFailure))
    ));
}

/// A misbehaving circuit: `mode` 0 assigns a fixed cell in the blinding
/// rows, 1 leaves an advice value unknown in the witness pass, 2 copies an
/// advice column without equality, 3 copies a cell in the blinding rows.
#[derive(Clone)]
struct Misuse {
    mode: u8,
}

impl Circuit<<Vesta as CurveBridge>::VScalar> for Misuse {
    type Config = (VColumn<VAdvice>, VColumn<VAdvice>, VColumn<VFixed>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure(meta: &mut VCs<<Vesta as CurveBridge>::VScalar>) -> Self::Config {
        let a = meta.advice_column();
        let b = meta.advice_column();
        let f = meta.fixed_column();
        meta.enable_equality(a);
        meta.create_gate("a = f", |cells| {
            vec![cells.query_advice(a, VRotation::cur()) - cells.query_fixed(f, VRotation::cur())]
        });
        (a, b, f)
    }

    fn synthesize(
        &self,
        (a, b, f): Self::Config,
        mut layouter: impl Layouter<<Vesta as CurveBridge>::VScalar>,
    ) -> Result<(), Error> {
        let mode = self.mode;
        layouter.assign_region(
            || "misuse",
            |mut region| {
                let one = Value::known(<Vesta as CurveBridge>::VScalar::ONE);
                let left = region.assign_advice(a, 0, one).cell();
                match mode {
                    0 => {
                        region.assign_fixed(f, (1 << K) - 1, <Vesta as CurveBridge>::VScalar::ONE);
                    }
                    1 => {
                        region.assign_advice(
                            a,
                            1,
                            Value::<<Vesta as CurveBridge>::VScalar>::unknown(),
                        );
                    }
                    2 => {
                        let right = region.assign_advice(b, 0, one).cell();
                        region.constrain_equal(left, right);
                    }
                    _ => {
                        let right = region.assign_advice(a, (1 << K) - 1, one).cell();
                        region.constrain_equal(left, right);
                    }
                }
                Ok(())
            },
        )
    }
}

#[test]
fn capture_reports_misuse() {
    let export = |mode| export_circuit::<Vesta, _>(K, &Misuse { mode }, &[]);
    assert!(matches!(
        export(0),
        Err(ExportError::Row {
            what: "fixed",
            row: 63,
            ..
        })
    ));
    assert!(matches!(
        export(1),
        Err(ExportError::UnknownAdvice { column: 0, row: 1 })
    ));
    let exported = export(2).expect("the vendored layouter records any copy");
    assert!(matches!(
        exported.permutation(),
        Err(ExportError::Permutation(
            PermutationError::ColumnNotInPermutation(_)
        ))
    ));
    assert!(matches!(
        export(3),
        Err(ExportError::Row { what: "copy", .. })
    ));
}

#[test]
fn witness_values_live_in_one_arena_per_column() {
    type F = <Vesta as CurveBridge>::VScalar;
    let circuit = Mixed::<F>::new(3);
    let (cs, _) = configure_vendored::<F, Mixed<F>>(&circuit);
    let n = 1 << K;
    let mut capture = Capture::new(Pass::Witness, K, n, &cs, &[]).expect("capture");
    assert!(capture.arenas.iter().all(Option::is_none));
    let first = capture.store(1, 4, VAssigned::Trivial(F::from(7)));
    assert_eq!(*first, VAssigned::Trivial(F::from(7)));
    let arena = capture.arenas[1].expect("column 1 arena");
    assert_eq!(arena.len(), n);
    assert!(core::ptr::eq(first, arena[4].get().expect("slot")));
    // A reassigned cell keeps its first reference valid and gets a new one.
    let second = capture.store(1, 4, VAssigned::Trivial(F::from(9)));
    assert_eq!(*first, VAssigned::Trivial(F::from(7)));
    assert_eq!(*second, VAssigned::Trivial(F::from(9)));
    assert!(!core::ptr::eq(first, second));
    // Another cell of the same column reuses the arena; other columns stay
    // unallocated; an out-of-range column falls back to a box.
    let other = capture.store(1, 5, VAssigned::Zero);
    assert!(core::ptr::eq(other, arena[5].get().expect("slot")));
    assert!(capture.arenas[0].is_none());
    let outside = capture.store(99, 0, VAssigned::Zero);
    assert_eq!(*outside, VAssigned::Zero);
}

#[test]
fn constraint_system_comparison_names_the_first_difference() {
    let circuit = Mixed::<<Vesta as CurveBridge>::VScalar>::new(3);
    let (cs, _) = configure_vendored::<_, Mixed<_>>(&circuit);
    let native = export_constraint_system::<Vesta>(&cs, &[2]).expect("export");
    assert_eq!(compare_constraint_systems::<Vesta>(&cs, &native), Ok(()));
    let mut extra = native.clone();
    extra.advice_column();
    assert_eq!(
        compare_constraint_systems::<Vesta>(&cs, &extra),
        Err(CsMismatch {
            part: "advice columns",
            index: None
        })
    );
    let mut gated = native.clone();
    gated.create_gate("extra", |cells| {
        vec![cells.query_advice(iroha_plonk::cs::Column::new(0, Advice), Rotation::cur())]
    });
    assert_eq!(
        compare_constraint_systems::<Vesta>(&cs, &gated),
        Err(CsMismatch {
            part: "gate count",
            index: None
        })
    );
    let empty = ConstraintSystem::<NativeScalar<Vesta>>::new();
    assert!(compare_constraint_systems::<Vesta>(&cs, &empty).is_err());
    let (compressed, _) = cs.compress_selectors(vec![vec![false; 1 << K]; 5]);
    assert!(compare_constraint_systems::<Vesta>(&compressed, &native).is_err());
    assert_eq!(
        CsMismatch {
            part: "gate",
            index: Some(2)
        }
        .to_string(),
        "gate 2 differs"
    );
    assert_eq!(
        CsMismatch {
            part: "degree",
            index: None
        }
        .to_string(),
        "degree differs"
    );
}

#[test]
fn vendored_flags_and_transcript_repr_are_exported() {
    let circuit = Mixed::<<Pallas as CurveBridge>::VScalar>::new(3);
    let params = ParamsIPA::<<Pallas as CurveBridge>::Vendored>::new(K);
    let plain = v_keygen_vk(&params, &circuit.without_witnesses()).expect("vk");
    assert!(!vendored_compress_selectors::<Pallas>(&plain));
    let config = vendored_keygen_config::<Pallas>(
        &plain,
        TranscriptV1::Blake2bChallenge255,
        ProofSuffixV1::None,
    );
    assert_eq!(config.transcript, TranscriptV1::Blake2bChallenge255);
    assert_eq!(config.instance_mode, InstanceModeV1::Committed);
    assert_eq!(config.proof_suffix, ProofSuffixV1::None);
    assert!(!config.compress_selectors);
    // The KAGEMUSHA path: the Poseidon transcript and the folded generator,
    // the same instance mode and selector choice.
    let kagemusha = vendored_keygen_config::<Pallas>(
        &plain,
        TranscriptV1::KagemushaPoseidonRp57,
        ProofSuffixV1::FoldedGenerator,
    );
    assert_eq!(kagemusha.transcript, TranscriptV1::KagemushaPoseidonRp57);
    assert_eq!(kagemusha.proof_suffix, ProofSuffixV1::FoldedGenerator);
    assert_eq!(kagemusha.instance_mode, InstanceModeV1::Committed);
    assert!(!kagemusha.compress_selectors);
    let compressed = keygen_vk_custom(&params, &circuit.without_witnesses(), true).expect("vk");
    assert!(vendored_compress_selectors::<Pallas>(&compressed));
    assert_eq!(
        vendored_transcript_repr::<Pallas>(&plain).to_repr(),
        plain.transcript_repr().to_repr()
    );
    assert_ne!(
        vendored_transcript_repr::<Pallas>(&plain),
        vendored_transcript_repr::<Pallas>(&compressed)
    );
    assert_eq!(vendored_vk_bytes::<Pallas>(&plain)[0], 0x02);
}

#[test]
fn errors_display_their_cause() {
    let errors = [
        ExportError::Synthesis(Error::Synthesis),
        ExportError::Phases,
        ExportError::Challenge,
        ExportError::Rows { k: 3 },
        ExportError::InstanceColumns {
            expected: 1,
            found: 2,
        },
        ExportError::Row {
            what: "fixed",
            row: 9,
            usable_rows: 8,
        },
        ExportError::Column {
            what: "advice",
            index: 4,
        },
        ExportError::UnknownAdvice { column: 1, row: 2 },
        ExportError::ConstraintSystem(CsError::EmptyGate {
            gate: "g".to_owned(),
        }),
        ExportError::QueryOrder,
        ExportError::from(PermutationError::RowOutOfBounds { row: 9, n: 8 }),
        ExportError::from(iroha_plonk::keys::KeyError::UnknownCurve),
    ];
    let texts: Vec<String> = errors.iter().map(ToString::to_string).collect();
    for (text, needle) in texts.iter().zip([
        "synthesis",
        "multi-phase",
        "challenge",
        "k = 3",
        "2 instance columns",
        "row 9",
        "advice column 4",
        "column 1 row 2",
        "native constraint system",
        "query tables",
        "copy constraint",
        "key generation",
    ]) {
        assert!(text.contains(needle), "{text:?} lacks {needle:?}");
    }
    assert!(matches!(
        ExportError::from(CsError::EmptyGate {
            gate: String::new()
        }),
        ExportError::ConstraintSystem(_)
    ));
}

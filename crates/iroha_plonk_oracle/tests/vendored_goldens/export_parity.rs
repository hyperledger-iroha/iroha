//! Export, key and parameter parity of every golden case (every build).
//!
//! For each `(family, curve, k)`:
//!
//! - the replayed configure-time constraint system equals the vendored one,
//!   and the native finalized (selector-compressed) system equals the
//!   vendored `VerifyingKey::cs`;
//! - the exported advice rows, the native configure-time fixed columns and
//!   the native permutation mapping equal the vendored `MockProver`'s; the
//!   selector columns are checked on their own, because `MockProver` always
//!   compresses selectors while the key follows the vendored keygen flag:
//!   against a native key generated with compression on (which must equal
//!   `MockProver`'s whole fixed table) and, uncompressed, against the exported
//!   activations. The golden circuits gate with fixed columns and declare no
//!   selectors, so `selector_tables_match_vendored` runs the same checks on a
//!   selector-gated circuit with and without compression;
//! - native key generation and parameter derivation give the same keys and
//!   bytes in Rayon pools of 1, 2, 4 and 7 threads (verifying-key bytes, the
//!   copy digest, and a SHA-256 over every fixed and `sigma` polynomial and
//!   every quotient coset), equal to the cached setup's;
//! - the native verifying-key bytes equal the vendored `0x02` bytes, and the
//!   strict native reader accepts the vendored bytes against the descriptor;
//! - the native parameter bytes equal the vendored `ParamsIPA` bytes and the
//!   pinned digest;
//! - the production `transcript_repr` (descriptor-bound) differs from the
//!   vendored one, which only oracle mode injects.

use halo2_axiom::halo2curves::ff::PrimeField;
use halo2_axiom::{
    dev::{AdviceCellValue, CellValue},
    poly::commitment::Params,
};
use iroha_plonk::{
    cs::{CurveV1, ProofSuffixV1, TranscriptV1, descriptor::pinned_params_digest},
    keys::{CosetPolynomial, DescriptorBinding, ProvingKey, VerifyingKey},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_oracle::{
    convert::{CurveBridge, NativeScalar, Pallas, Vesta, native_scalar},
    export::{compare_constraint_systems, vendored_keygen_config, vendored_vk_bytes},
    pools::same_on_each_pool,
};
use rayon::iter::ParallelIterator;
use sha2::{Digest, Sha256};

use crate::cases::{Family, Setup, setup};

/// The descriptor curve of `B`.
fn curve_of<B: CurveBridge>() -> CurveV1 {
    match B::NAME {
        "eq" => CurveV1::Vesta,
        _ => CurveV1::Pallas,
    }
}

/// The native value of a vendored `MockProver` fixed cell.
fn fixed_cell<B: CurveBridge>(cell: &CellValue<B::VScalar>) -> NativeScalar<B> {
    match cell {
        CellValue::Assigned(value) => native_scalar::<B>(value),
        CellValue::Unassigned | CellValue::Poison(_) => NativeScalar::<B>::default(),
    }
}

/// The vendored advice column with `index` (the vendored `Column::new` is
/// test-only, so the column is allocated on a scratch constraint system).
fn vendored_advice_column<B: CurveBridge>(
    index: usize,
) -> halo2_axiom::plonk::Column<halo2_axiom::plonk::Advice> {
    let mut cs = halo2_axiom::plonk::ConstraintSystem::<B::VScalar>::default();
    let mut column = cs.advice_column();
    for _ in 0..index {
        column = cs.advice_column();
    }
    column
}

/// Every export, key and parameter check of one setup.
fn check_setup<B: CurveBridge>(setup: &Setup<B>) {
    let label = format!("{}/{}/k{}", setup.family.label(), B::NAME, setup.k);
    let n = 1_usize << setup.k;

    // Constraint systems.
    compare_constraint_systems::<B>(&setup.vendored_cs, setup.exported.constraint_system())
        .unwrap_or_else(|mismatch| panic!("{label}: configure-time system: {mismatch}"));
    let finalized = setup.pk.constraint_system();
    compare_constraint_systems::<B>(
        setup.vendored_pk.get_vk().cs(),
        finalized.constraint_system(),
    )
    .unwrap_or_else(|mismatch| panic!("{label}: compressed system: {mismatch}"));
    let cs = finalized.constraint_system();
    assert_eq!(
        (cs.degree(), cs.permutation().columns().len()),
        setup.shape,
        "{label}: shape"
    );

    // Tables against the vendored MockProver.
    let usable = n - (cs.blinding_factors() + 1);
    for (column, exported) in setup.exported.advice().iter().enumerate() {
        let vendored = setup
            .mock
            .advice_values(vendored_advice_column::<B>(column));
        assert_eq!(exported.len(), n, "{label}: advice rows");
        for (row, value) in exported.iter().enumerate().take(usable) {
            let AdviceCellValue::Assigned(assigned) = &vendored[row] else {
                panic!("{label}: MockProver poisoned usable advice row {row}");
            };
            assert_eq!(
                *value,
                native_scalar::<B>(&assigned.as_ref().evaluate()),
                "{label}: advice column {column} row {row}"
            );
        }
        assert!(
            exported[usable..]
                .iter()
                .all(|value| *value == NativeScalar::<B>::default()),
            "{label}: blinding rows of the exported advice are zero"
        );
    }
    check_fixed_tables::<B>(
        &label,
        setup.vendored_cs.num_fixed_columns(),
        &setup.mock,
        &setup.exported,
        &setup.pk,
        &setup.params,
        vendored_keygen_config::<B>(
            setup.vendored_pk.get_vk(),
            TranscriptV1::Blake2bChallenge255,
            ProofSuffixV1::None,
        ),
    );
    let permutation = setup.exported.permutation().expect("replay copies");
    let vendored_mapping: Vec<Vec<(usize, usize)>> = setup
        .mock
        .permutation()
        .mapping()
        .map(ParallelIterator::collect)
        .collect();
    assert_eq!(vendored_mapping.len(), permutation.columns().len());
    for (column, rows) in vendored_mapping.iter().enumerate() {
        for (row, target) in rows.iter().enumerate() {
            assert_eq!(
                permutation.mapping(column, row),
                Some(*target),
                "{label}: permutation column {column} row {row}"
            );
        }
    }
    assert!(!setup.exported.copies().is_empty(), "{label}: copies");

    // Verifying key.
    let vendored_vk = vendored_vk_bytes::<B>(setup.vendored_pk.get_vk());
    assert_eq!(
        setup.pk.vk().to_bytes(),
        &vendored_vk[..],
        "{label}: VK bytes"
    );
    let binding = DescriptorBinding::decode(setup.pk.binding().encoded()).expect("descriptor");
    assert_eq!(&binding, setup.pk.binding());
    let read = VerifyingKey::<B::Native>::read(&vendored_vk, &binding).expect("strict VK read");
    assert_eq!(read.to_bytes(), setup.pk.vk().to_bytes());
    assert_ne!(
        *setup.pk.vk().transcript_repr(),
        setup.transcript_repr,
        "{label}: production transcript_repr is descriptor-bound, not the vendored Debug hash"
    );

    // Key generation and parameters at every pool size.
    let reference = key_fingerprint(&setup.pk);
    let pooled = same_on_each_pool(&format!("{label}: native keygen"), |_| {
        let config = vendored_keygen_config::<B>(
            setup.vendored_pk.get_vk(),
            TranscriptV1::Blake2bChallenge255,
            ProofSuffixV1::None,
        );
        key_fingerprint(&setup.exported.keygen(&setup.params, &config).expect("pk"))
    });
    assert_eq!(pooled, reference, "{label}: pooled keys equal the setup's");
    let params_bytes = setup.params.params().to_bytes();
    let pooled_params = same_on_each_pool(&format!("{label}: params"), |_| {
        PinnedParams::<B::Native>::derive(setup.k)
            .expect("derive")
            .params()
            .to_bytes()
    });
    assert_eq!(pooled_params, params_bytes, "{label}: pooled params");

    // Parameters.
    let mut vendored_params = Vec::new();
    setup
        .vendored_params
        .write(&mut vendored_params)
        .expect("write vendored params");
    assert_eq!(
        setup.params.params().to_bytes(),
        vendored_params,
        "{label}: params bytes"
    );
    assert_eq!(
        Some(setup.params.digest()),
        pinned_params_digest(curve_of::<B>(), setup.k),
        "{label}: pinned params digest"
    );
}

/// Checks the fixed tables of an export against the vendored `MockProver`:
/// the `configured` configure-time columns directly; the selector columns
/// on their own, because `MockProver` always compresses selectors while
/// `pk` follows `config`: a native key generated from the same export with
/// compression on must equal `MockProver`'s whole fixed table, and without
/// compression each selector column must hold the exported activations.
fn check_fixed_tables<B: CurveBridge>(
    label: &str,
    configured: usize,
    mock: &halo2_axiom::dev::MockProver<B::VScalar>,
    exported: &iroha_plonk_oracle::export::ExportedCircuit<B>,
    pk: &ProvingKey<B::Native>,
    params: &PinnedParams<B::Native>,
    config: iroha_plonk::keys::KeygenConfig,
) {
    let vendored_fixed: Vec<Vec<NativeScalar<B>>> = mock
        .fixed()
        .iter()
        .map(|column| column.iter().map(fixed_cell::<B>).collect())
        .collect();
    let descriptor = pk.binding().descriptor();
    assert_eq!(
        descriptor.selectors.first_column as usize, configured,
        "{label}: selector columns follow the configured ones"
    );
    assert_eq!(
        &vendored_fixed[..configured],
        &pk.fixed_values()[..configured],
        "{label}: configure-time fixed columns"
    );
    let mut compressed_config = config;
    compressed_config.compress_selectors = true;
    let compressed = exported
        .keygen(params, &compressed_config)
        .expect("compressed native key");
    assert_eq!(
        compressed.fixed_values(),
        vendored_fixed.as_slice(),
        "{label}: MockProver's compressed fixed table"
    );
    let selectors = exported.selectors();
    if descriptor.selectors.compress {
        assert_eq!(
            pk.fixed_values(),
            vendored_fixed.as_slice(),
            "{label}: compressed key"
        );
    } else {
        assert_eq!(
            pk.fixed_values().len(),
            configured + selectors.len(),
            "{label}: one column per selector"
        );
        let one = NativeScalar::<B>::from(1);
        for (selector, rows) in selectors.iter().enumerate() {
            let column = &pk.fixed_values()[configured + selector];
            for (row, active) in rows.iter().enumerate() {
                let expected = if *active {
                    one
                } else {
                    NativeScalar::<B>::default()
                };
                assert_eq!(
                    column[row], expected,
                    "{label}: selector {selector} row {row}"
                );
            }
        }
    }
}

/// A selector-gated circuit (the golden circuits have no selectors): two
/// simple selectors on alternating rows, which compression combines, a
/// complex selector in a lookup, and a fixed coefficient column.
#[derive(Clone, Default)]
struct Gated<F> {
    marker: core::marker::PhantomData<F>,
}

impl<F: PrimeField> halo2_axiom::plonk::Circuit<F> for Gated<F> {
    type Config = (
        [halo2_axiom::plonk::Column<halo2_axiom::plonk::Advice>; 2],
        halo2_axiom::plonk::Column<halo2_axiom::plonk::Fixed>,
        [halo2_axiom::plonk::Selector; 3],
        halo2_axiom::plonk::TableColumn,
    );
    type FloorPlanner = halo2_axiom::circuit::SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure(meta: &mut halo2_axiom::plonk::ConstraintSystem<F>) -> Self::Config {
        use halo2_axiom::poly::Rotation;
        let a = meta.advice_column();
        let b = meta.advice_column();
        let coeff = meta.fixed_column();
        let add = meta.selector();
        let mul = meta.selector();
        let range = meta.complex_selector();
        let table = meta.lookup_table_column();
        meta.create_gate("add", |cells| {
            let s = cells.query_selector(add);
            let a = cells.query_advice(a, Rotation::cur());
            let b = cells.query_advice(b, Rotation::cur());
            let c = cells.query_fixed(coeff, Rotation::cur());
            vec![s * (a + c - b)]
        });
        meta.create_gate("mul", |cells| {
            let s = cells.query_selector(mul);
            let a = cells.query_advice(a, Rotation::cur());
            let b = cells.query_advice(b, Rotation::cur());
            vec![s * (a.clone() * a - b)]
        });
        meta.lookup("range", |cells| {
            let s = cells.query_selector(range);
            let a = cells.query_advice(a, Rotation::cur());
            vec![(s * a, table)]
        });
        ([a, b], coeff, [add, mul, range], table)
    }

    fn synthesize(
        &self,
        ([a, b], coeff, [add, mul, range], table): Self::Config,
        mut layouter: impl halo2_axiom::circuit::Layouter<F>,
    ) -> Result<(), halo2_axiom::plonk::Error> {
        use halo2_axiom::circuit::Value;
        layouter.assign_table(
            || "range",
            |mut cells| {
                for value in 0..8_u64 {
                    let row = usize::try_from(value).expect("small");
                    cells.assign_cell(|| "v", table, row, || Value::known(F::from(value)))?;
                }
                Ok(())
            },
        )?;
        layouter.assign_region(
            || "gated",
            |mut region| {
                for row in 0..8_usize {
                    let x = F::from(u64::try_from(row % 4).expect("small"));
                    range.enable(&mut region, row)?;
                    region.assign_advice(a, row, Value::known(x));
                    if row % 2 == 0 {
                        add.enable(&mut region, row)?;
                        region.assign_fixed(coeff, row, F::from(3));
                        region.assign_advice(b, row, Value::known(x + F::from(3)));
                    } else {
                        mul.enable(&mut region, row)?;
                        region.assign_advice(b, row, Value::known(x * x));
                    }
                }
                Ok(())
            },
        )
    }
}

/// The fixed-table checks on [`Gated`] with and without selector
/// compression, on both curves: the selector path of [`check_fixed_tables`]
/// is not vacuous.
fn selector_tables<B: CurveBridge>() {
    use halo2_axiom::{
        dev::MockProver,
        plonk::Circuit as _,
        plonk::keygen_vk_custom,
        poly::{commitment::ParamsProver, ipa::commitment::ParamsIPA},
    };
    use iroha_plonk_oracle::export::{configure_vendored, export_circuit};
    let k = 6;
    let circuit = Gated::<B::VScalar>::default();
    let vendored_params = ParamsIPA::<B::Vendored>::new(k);
    let params = PinnedParams::<B::Native>::derive(k).expect("params");
    let exported = export_circuit::<B, _>(k, &circuit, &[]).expect("export");
    assert_eq!(exported.selectors().len(), 3);
    let mock = MockProver::run(k, &circuit, Vec::new()).expect("mock");
    mock.assert_satisfied();
    let (configured, _) = configure_vendored::<B::VScalar, Gated<B::VScalar>>(&circuit);
    for compress in [false, true] {
        let vk = keygen_vk_custom(&vendored_params, &circuit.without_witnesses(), compress)
            .expect("vendored vk");
        let config = vendored_keygen_config::<B>(
            &vk,
            TranscriptV1::Blake2bChallenge255,
            ProofSuffixV1::None,
        );
        assert_eq!(config.compress_selectors, compress);
        let pk = exported.keygen(&params, &config).expect("native pk");
        assert_eq!(pk.vk().to_bytes(), &vendored_vk_bytes::<B>(&vk)[..]);
        check_fixed_tables::<B>(
            &format!("gated/{}/compress={compress}", B::NAME),
            configured.num_fixed_columns(),
            &mock,
            &exported,
            &pk,
            &params,
            config,
        );
    }
}

#[test]
fn selector_tables_match_vendored() {
    selector_tables::<Vesta>();
    selector_tables::<Pallas>();
}

/// What a proving key computes: the verifying-key bytes, the copy digest,
/// and a SHA-256 over every fixed and `sigma` column (values and
/// coefficients) and every quotient coset of every cached polynomial.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyFingerprint {
    vk: Vec<u8>,
    copy_digest: [u8; 32],
    tables: String,
}

/// The [`KeyFingerprint`] of `pk`.
fn key_fingerprint<C: iroha_pasta::PastaCurve>(pk: &ProvingKey<C>) -> KeyFingerprint {
    let mut hasher = Sha256::new();
    let mut absorb = |column: &[C::ScalarExt]| {
        hasher.update(u64::try_from(column.len()).expect("fits").to_le_bytes());
        for value in column {
            hasher.update(value.to_repr());
        }
    };
    for column in pk
        .fixed_values()
        .iter()
        .chain(pk.fixed_polys())
        .chain(pk.permutation_values())
        .chain(pk.permutation_polys())
    {
        absorb(column);
    }
    let polys = (0..pk.fixed_values().len())
        .map(CosetPolynomial::Fixed)
        .chain((0..pk.permutation_values().len()).map(CosetPolynomial::Permutation))
        .chain([
            CosetPolynomial::L0,
            CosetPolynomial::LLast,
            CosetPolynomial::LActive,
        ]);
    for poly in polys {
        for coset in 0..pk.quotient_domain().pieces() {
            absorb(&pk.coset_values(poly, coset).expect("coset"));
        }
    }
    KeyFingerprint {
        vk: pk.vk().to_bytes().to_vec(),
        copy_digest: *pk.copy_digest(),
        tables: hex(&hasher.finalize()),
    }
}

/// Lowercase hex.
fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Runs [`check_setup`] for `family` over curve `B` at every `k`.
fn export_parity<B: CurveBridge>(family: Family, ks: &[u32]) {
    for &k in ks {
        check_setup(&setup::<B>(family, k));
    }
}

#[test]
fn sigma_eq_export_matches_vendored() {
    export_parity::<Vesta>(Family::Sigma, &[6, 9]);
}

#[test]
fn sigma_ep_export_matches_vendored() {
    export_parity::<Pallas>(Family::Sigma, &[6, 9]);
}

#[test]
fn wide_eq_export_matches_vendored() {
    export_parity::<Vesta>(Family::Wide, &[8, 10]);
}

#[test]
fn wide_ep_export_matches_vendored() {
    export_parity::<Pallas>(Family::Wide, &[8, 10]);
}

#[test]
#[ignore = "k = 11 golden; run in release"]
fn sigma_k11_export_matches_vendored() {
    export_parity::<Vesta>(Family::Sigma, &[11]);
    export_parity::<Pallas>(Family::Sigma, &[11]);
}

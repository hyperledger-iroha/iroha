//! Direct soundness controls and native geometry/proof comparisons for byte words.

use super::*;
use crate::zk::halo2_backend;
use halo2_proofs::{
    circuit::SimpleFloorPlanner,
    dev::MockProver,
    plonk::{Circuit, Instance},
};
use std::{sync::Arc, time::Instant};

#[derive(Clone)]
struct Arithmetic<const N: usize, const ROT: u32, const DEGREE: usize = 5, const ALL: bool = true> {
    values: Arc<Zeroizing<[u64; 3]>>,
    faults: Vec<Fault>,
    forge_source: bool,
    constant: bool,
    fail_after_load: bool,
    panic_after_load: bool,
}
impl<const N: usize, const ROT: u32, const DEGREE: usize, const ALL: bool>
    Arithmetic<N, ROT, DEGREE, ALL>
{
    fn new(values: [u64; 3]) -> Self {
        Self {
            values: Arc::new(Zeroizing::new(values)),
            faults: vec![],
            forge_source: false,
            constant: false,
            fail_after_load: false,
            panic_after_load: false,
        }
    }
    fn expected(&self) -> Vec<Scalar> {
        let [a, b, c] = **self.values;
        let value = if N == 4 {
            u64::from(
                ((a as u32).wrapping_add(b as u32).wrapping_add(c as u32) ^ a as u32)
                    .rotate_right(ROT),
            )
        } else {
            (a.wrapping_add(b).wrapping_add(c) ^ a).rotate_right(ROT)
        };
        (0..N)
            .map(|i| Scalar::from((value >> (8 * i)) & 255))
            .collect()
    }
}
impl<const N: usize, const ROT: u32, const DEGREE: usize, const ALL: bool> Circuit<Scalar>
    for Arithmetic<N, ROT, DEGREE, ALL>
{
    type Config = (ByteConfig, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self::new([0; 3])
    }
    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        let config = ByteConfig::configure(meta, DEGREE, ALL);
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        (config, instance)
    }
    fn synthesize(
        &self,
        (config, instance): Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), Error> {
        config.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "byte control",
            |mut region| {
                let mut words = ByteRegion {
                    config: &config,
                    region: &mut region,
                    offset: 0,
                    faults: &self.faults,
                };
                let mut a = words.load::<N>(self.values[0], self.constant)?;
                let b = words.load::<N>(self.values[1], false)?;
                let c = words.load::<N>(self.values[2], false)?;
                if self.fail_after_load {
                    return Err(Error::Synthesis);
                }
                assert!(!self.panic_after_load, "test-only synthesis unwind");
                if self.forge_source {
                    *a.value ^= 1;
                }
                let sum = words.add(&a, &b, &c)?;
                let xor = words.xor(&sum, &a)?;
                let out = words.rotate_right(&xor, ROT)?;
                assert_eq!(words.offset, if ROT % 8 == 0 { 5 * N } else { 6 * N });
                Ok(out.cells)
            },
        )?;
        for (row, cell) in output.into_iter().enumerate() {
            layouter.constrain_instance(cell, instance, row);
        }
        Ok(())
    }
}
fn positive<const N: usize, const ROT: u32>(values: [u64; 3]) {
    let circuit = Arithmetic::<N, ROT>::new(values);
    MockProver::run(13, &circuit, vec![circuit.expected()])
        .expect("layout")
        .assert_satisfied();
}

#[test]
fn byte_words_match_native_overflow_and_every_bit_rotation() {
    for values in [
        [0; 3],
        [u64::MAX; 3],
        [
            0x0123_4567_89ab_cdef,
            0xfedc_ba98_7654_3210,
            0x8080_7f7f_aa55_55aa,
        ],
    ] {
        positive::<8, 0>(values);
        positive::<8, 1>(values);
        positive::<8, 2>(values);
        positive::<8, 3>(values);
        positive::<8, 4>(values);
        positive::<8, 5>(values);
        positive::<8, 6>(values);
        positive::<8, 7>(values);
        positive::<8, 16>(values);
        positive::<8, 24>(values);
        positive::<8, 32>(values);
        positive::<8, 63>(values);
    }
    let values = [u64::from(u32::MAX); 3];
    positive::<4, 7>(values);
    positive::<4, 8>(values);
    positive::<4, 12>(values);
    positive::<4, 16>(values);
}

#[test]
fn byte_words_reject_real_assignments_carries_and_nibble_aliases() {
    let half = Scalar::from(2).invert().unwrap();
    assert_ne!(254, (3 * 255) & 255, "sum mutation must change the witness");
    for (label, row, column, value) in [
        ("byte overflow", 0, 0, Scalar::from(256)),
        ("fractional input", 0, 0, half),
        ("reserved load", 0, 1, Scalar::ONE),
        ("initial carry", 24, 4, Scalar::ONE),
        ("carry three", 24, 5, Scalar::from(3)),
        ("fractional carry", 24, 5, half),
        ("carry chain", 25, 4, Scalar::from(3)),
        ("terminal carry", 31, 5, Scalar::from(3)),
        ("sum", 24, 3, Scalar::from(254)),
        ("xor high", 32, 3, Scalar::from(16)),
        ("xor fractional high", 32, 3, half),
        ("xor output", 32, 2, Scalar::from(255)),
        ("rotation low", 40, 1, Scalar::from(128)),
        ("rotation high", 40, 2, half),
        ("rotation wrap", 47, 4, Scalar::from(127)),
        ("rotation reserved", 40, 5, Scalar::ONE),
    ] {
        let mut circuit = Arithmetic::<8, 63>::new([u64::MAX; 3]);
        circuit.faults.push(Fault { row, column, value });
        assert!(
            MockProver::run(13, &circuit, vec![circuit.expected()])
                .expect("layout")
                .verify()
                .is_err(),
            "surviving mutation: {label}"
        );
    }
    let mut circuit = Arithmetic::<8, 63>::new([42, 1, 2]);
    circuit.forge_source = true;
    assert!(
        MockProver::run(13, &circuit, vec![circuit.expected()])
            .expect("layout")
            .verify()
            .is_err()
    );
    circuit.forge_source = false;
    let mut public = circuit.expected();
    public[0] += Scalar::ONE;
    assert!(
        MockProver::run(13, &circuit, vec![public])
            .expect("layout")
            .verify()
            .is_err()
    );
}

#[derive(Clone)]
struct RotationOnly {
    faults: Vec<Fault>,
    constant: bool,
}
impl Circuit<Scalar> for RotationOnly {
    type Config = ByteConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            faults: vec![],
            constant: self.constant,
        }
    }
    fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
        ByteConfig::configure(meta, 5, false)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Scalar>,
    ) -> Result<(), Error> {
        config.load_table(&mut layouter)?;
        layouter.assign_region(
            || "isolated rotation",
            |mut region| {
                let mut words = ByteRegion {
                    config: &config,
                    region: &mut region,
                    offset: 0,
                    faults: &self.faults,
                };
                let word = words.load::<8>(1, self.constant)?;
                let _ = words.rotate_right(&word, 1)?;
                Ok(())
            },
        )
    }
}
#[test]
fn byte_rotation_rejects_fractional_split_without_output_constraints() {
    let half = Scalar::from(2).invert().unwrap();
    let circuit = RotationOnly {
        constant: false,
        faults: vec![
            Fault {
                row: 8,
                column: 1,
                value: Scalar::ZERO,
            },
            Fault {
                row: 8,
                column: 2,
                value: half,
            },
            Fault {
                row: 8,
                column: 3,
                value: half,
            },
            Fault {
                row: 15,
                column: 4,
                value: Scalar::ZERO,
            },
            Fault {
                row: 15,
                column: 3,
                value: Scalar::ZERO,
            },
        ],
    };
    let failures = MockProver::run(13, &circuit, vec![])
        .expect("layout")
        .verify()
        .expect_err("fractional split rejected");
    assert!(
        failures
            .iter()
            .all(|failure| format!("{failure:?}").starts_with("Lookup")),
        "{failures:?}"
    );
    let constant = RotationOnly {
        constant: true,
        faults: vec![Fault {
            row: 0,
            column: 0,
            value: Scalar::from(2),
        }],
    };
    assert!(
        MockProver::run(13, &constant, vec![])
            .expect("layout")
            .verify()
            .is_err()
    );
}

#[test]
fn byte_words_reject_overflow_and_clear_owned_values() {
    let unsupported = Arithmetic::<8, 2, 5, false>::new([1, 2, 3]);
    assert!(MockProver::run(12, &unsupported, vec![unsupported.expected()]).is_err());
    let circuit = Arithmetic::<4, 7>::new([u64::from(u32::MAX) + 1, 0, 0]);
    assert!(MockProver::run(13, &circuit, vec![circuit.expected()]).is_err());
    for (fail, panic) in [(false, false), (true, false), (false, true)] {
        let before = CLEARED_WORDS.with(std::cell::Cell::get);
        let mut circuit = Arithmetic::<8, 63>::new([u64::MAX; 3]);
        circuit.fail_after_load = fail;
        circuit.panic_after_load = panic;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            MockProver::run(13, &circuit, vec![circuit.expected()])
        }));
        if panic {
            assert!(result.is_err());
        } else if fail {
            assert!(result.expect("no unwind").is_err());
        } else {
            result
                .expect("no unwind")
                .expect("layout")
                .assert_satisfied();
        }
        assert!(CLEARED_WORDS.with(std::cell::Cell::get) >= before + 3);
    }
}

fn native_metrics<const DEGREE: usize>() {
    let circuit = Arithmetic::<8, 63, DEGREE, false>::new([u64::MAX, 0x1234_5678_9abc_def0, 42]);
    let mut meta = ConstraintSystem::default();
    let _ = Arithmetic::<8, 63, DEGREE, false>::configure(&mut meta);
    assert_eq!(meta.degree(), DEGREE);
    assert_eq!(meta.advice_queries().len(), 6);
    assert!(
        meta.advice_queries()
            .iter()
            .all(|(_, at)| *at == Rotation::cur())
    );
    assert_eq!(meta.lookups().len(), 2);
    assert_eq!(meta.permutation().get_columns().len(), 8);
    let start = Instant::now();
    let params = halo2_backend::params_new(12);
    let vk = halo2_backend::keygen_vk(&params, &circuit.without_witnesses()).expect("byte VK");
    let vk_bytes = halo2_backend::verifying_key_to_processed_bytes(&vk).len();
    let pk = halo2_backend::keygen_pk(&params, vk.clone(), &circuit.without_witnesses())
        .expect("byte PK");
    let keygen_ms = start.elapsed().as_secs_f64() * 1000.0;
    let public = circuit.expected();
    let columns: [&[Scalar]; 1] = [&public];
    let instances: [&[&[Scalar]]; 1] = [&columns];
    let start = Instant::now();
    let proof =
        halo2_backend::create_ipa_proof(&params, &pk, &[circuit], &instances).expect("byte proof");
    let prove_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    halo2_backend::verify_ipa_proof(&params, &vk, &proof, &instances).expect("byte verification");
    let verify_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "RAM_LFE_BYTE_METRICS k=12 degree={DEGREE} advice_columns=6 table_rows={HASH_TABLE_ROWS} operation_rows=48 lookup_arguments=2 permutation_columns=8 fixed_queries_before_selector_compression={} permutation_chunk_columns={} proof_bytes={} vk_bytes={vk_bytes} keygen_ms={keygen_ms:.3} prove_ms={prove_ms:.3} verify_ms={verify_ms:.3}",
        meta.fixed_queries().len(),
        DEGREE - 2,
        proof.len()
    );
    let mut bad = public.clone();
    bad[0] += Scalar::ONE;
    let wrong: [&[Scalar]; 1] = [&bad];
    assert!(halo2_backend::verify_ipa_proof(&params, &vk, &proof, &[&wrong]).is_err());
    let mut suffixed = proof.clone();
    suffixed.push(0);
    assert!(halo2_backend::verify_ipa_proof(&params, &vk, &suffixed, &instances).is_err());
    let mut changed = proof;
    changed[0] ^= 1;
    assert!(halo2_backend::verify_ipa_proof(&params, &vk, &changed, &instances).is_err());
}
#[test]
fn byte_native_proofs_measure_fixed_degree_tradeoffs() {
    native_metrics::<5>();
    native_metrics::<8>();
    native_metrics::<11>();
    native_metrics::<20>();
}

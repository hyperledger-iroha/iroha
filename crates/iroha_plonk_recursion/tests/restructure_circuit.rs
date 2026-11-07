//! Full k16 circuit/native restructuring parity and binding tests.

use ff::{Field, PrimeField};
use group::CurveAffine;
use iroha_pasta::{Ep, Eq, PastaCurve, PastaField, msm::MemoryBudget, poseidon::PoseidonField};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
    pcs::{
        ipa::{
            PinnedParams,
            commit::{self, Secrecy},
        },
        mle::evaluate,
    },
    transcript::generator_coordinates,
};
use iroha_plonk_gadgets::{
    bytes::{
        element::{decode_le_element, le_message_segments},
        tape::{BytesChip, BytesConfig},
    },
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FOLD_BODY_BYTES, FOLD_WITNESS_BYTES, FoldConfig, FoldTape, K,
    RestructureStatement,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_restructure,
    restructure_circuit::{RestructurePlan, RestructureStatementCells},
    verifier::{VerifierChip, VerifierConfig},
};
use zeroize::Zeroizing;

// Public deterministic fixtures only; these coins are never production entropy.
fn fixture_tape<C: PastaCurve>(start: u64) -> FoldTape<C> {
    let mut value = start;
    FoldTape::try_fill::<()>(|coin| {
        *coin = C::ScalarExt::from(value);
        value += 1;
        Ok(())
    })
    .expect("public fixture tape")
}

/// SplitMix64 public test data.
struct Data(u64);

impl Data {
    fn word(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn scalar<F: PastaField>(&mut self) -> F {
        F::from_raw_reduced(core::array::from_fn(|_| self.word()))
    }
}

#[derive(Clone, Debug)]
struct Config<C: PastaCurve> {
    verifier: VerifierConfig<C>,
    bytes: BytesConfig,
    public: Column<Instance>,
}

#[derive(Clone)]
struct RestructureCircuit<C: PastaCurve> {
    plan: RestructurePlan<C>,
    commitment: C::AffineExt,
    point: [C::ScalarExt; K],
    value: C::ScalarExt,
    proof: Zeroizing<Vec<u8>>,
    length: u32,
    known: bool,
}

impl<C: PastaCurve> RestructureCircuit<C> {
    fn witness<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }

    /// The structured claim as public instances: `G'` coordinates, then the
    /// two limbs of every challenge.
    fn instances(output: &AccumulatorT<C>) -> Vec<C::Base> {
        let (x, y) = generator_coordinates::<C>(output.g());
        let mut values = vec![x, y];
        for value in output.challenges() {
            values.extend(foreign_limbs(value).map(C::Base::from_u128));
        }
        values
    }
}

impl<C: PastaCurve> Circuit<C::Base> for RestructureCircuit<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let verifier = VerifierConfig::configure(meta);
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(2 + 2 * K);
        meta.enable_equality(public);
        Config {
            verifier,
            bytes,
            public,
        }
    }

    fn synthesize(
        &self,
        config: Config<C>,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let outputs = layouter.assign_region(
            || "k16 restructuring",
            |mut region| {
                let count = self.proof.len() / 32;
                let values = self
                    .proof
                    .iter()
                    .map(|byte| self.witness(*byte))
                    .collect::<Vec<_>>();
                let run = bytes.run(
                    &mut region,
                    &values,
                    &vec![16; 2 * count],
                    &le_message_segments(0, count),
                )?;
                let mut elements = Vec::with_capacity(count);
                for index in 0..count {
                    elements.push(decode_le_element(
                        &mut chip.uint(),
                        &mut region,
                        &run,
                        index * 32,
                    )?);
                }
                let commitment = chip
                    .witness_generator(&mut region, self.witness(self.commitment.to_curve()))?
                    .point()
                    .clone();
                let mut point = Vec::with_capacity(K);
                for coordinate in &self.point {
                    point.push(ScalarCells::assign_value(
                        &mut chip.uint(),
                        &mut region,
                        self.witness(*coordinate),
                    )?);
                }
                let value = ScalarCells::assign_value(
                    &mut chip.uint(),
                    &mut region,
                    self.witness(self.value),
                )?;
                let length = chip
                    .uint()
                    .assign::<32>(&mut region, self.witness(u128::from(self.length)))?;
                let statement = RestructureStatementCells {
                    commitment,
                    point,
                    value,
                };
                let output = chip.verify_restructure(
                    &mut region,
                    &self.plan,
                    &statement,
                    &elements,
                    &length,
                )?;
                let forwarded = FoldInputCells::from_claim(&mut chip, &mut region, &output.claim)?;
                let mut outputs = vec![forwarded.g().x().cell(), forwarded.g().y().cell()];
                for value in forwarded.challenges() {
                    outputs.extend([value.lo().cell(), value.hi().cell()]);
                }
                Ok(outputs)
            },
        )?;
        for (row, cell) in outputs.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.public, row)?;
        }
        Ok(())
    }
}

fn satisfied<C: PastaCurve>(circuit: &RestructureCircuit<C>, output: &AccumulatorT<C>) -> bool {
    check_circuit(
        circuit,
        16,
        &[RestructureCircuit::instances(output)],
        CheckMode::Strict,
    )
    .expect("circuit layout")
    .is_satisfied()
}

/// An honest restructuring and its circuit.
fn honest<C: PastaCurve>(seed: u64) -> (PinnedParams<C>, RestructureCircuit<C>, AccumulatorT<C>)
where
    C::Base: PoseidonField,
{
    let params = PinnedParams::<C>::derive(K as u32).expect("pinned params");
    let mut data = Data(seed);
    let witness: Vec<C::ScalarExt> = (0..1 << K).map(|_| data.scalar()).collect();
    let point: [C::ScalarExt; K] = core::array::from_fn(|_| data.scalar());
    let commitment = commit::commit(
        params.params(),
        &witness,
        &C::ScalarExt::ZERO,
        Secrecy::Secret,
        MemoryBudget::DEFAULT,
    )
    .expect("commit")
    .to_affine();
    let value = evaluate(&witness, &point).expect("2^16 entries");
    let statement = RestructureStatement {
        commitment,
        point,
        value,
    };
    let (proof, output) = create_restructure(
        &params,
        &statement,
        &witness,
        fixture_tape(seed),
        &FoldConfig::default(),
    )
    .expect("restructure");
    let private = proof.to_private_bytes();
    let mut bytes = Zeroizing::new(vec![0; FOLD_WITNESS_BYTES]);
    bytes.copy_from_slice(&private);
    let circuit = RestructureCircuit {
        plan: RestructurePlan::new(&params).expect("plan"),
        commitment,
        point,
        value,
        proof: bytes,
        length: u32::try_from(FOLD_WITNESS_BYTES).expect("small"),
        known: true,
    };
    (params, circuit, output)
}

fn run<C: PastaCurve>(seed: u64, every_field: bool)
where
    C::Base: PoseidonField,
{
    let (params, circuit, output) = honest::<C>(seed);
    assert!(satisfied(&circuit, &output), "honest restructuring");
    let other = AccumulatorT::new(*output.g(), [C::ScalarExt::ONE; K]).expect("claim");
    assert!(!satisfied(&circuit, &other), "another output claim");
    let mut changed = circuit.clone();
    changed.value += C::ScalarExt::ONE;
    assert!(!satisfied(&changed, &output), "value");
    if !every_field {
        return;
    }
    let mut changed = circuit.clone();
    changed.point[3] += C::ScalarExt::ONE;
    assert!(!satisfied(&changed, &output), "point");
    let mut changed = circuit.clone();
    changed.commitment = (circuit.commitment + params.params().w()).to_affine();
    assert!(!satisfied(&changed, &output), "commitment");
    for offset in [32, 32 * (2 * K + 1), FOLD_BODY_BYTES + 32] {
        let mut changed = circuit.clone();
        changed.proof[offset] ^= 1;
        assert!(!satisfied(&changed, &output), "byte {offset}");
    }
    let mut changed = circuit.clone();
    changed.length -= 1;
    assert!(!satisfied(&changed, &output), "length");
}

#[test]
#[ignore = "full k16 circuit; run optimized with --include-ignored"]
fn pallas_restructure_circuit_matches_native_and_binds_every_field() {
    run::<Ep>(21, true);
}

#[test]
#[ignore = "full k16 circuit; run optimized with --include-ignored"]
fn vesta_restructure_circuit_matches_native() {
    run::<Eq>(22, false);
}

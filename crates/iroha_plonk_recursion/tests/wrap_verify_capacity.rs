//! Lane rows of verifying a wrap-shaped k16 PIPA-R proof in circuit: the
//! inner circuit has the folding wrap P's chip layout (the shared verifier
//! lanes over `Fp`, a byte tape, and `PUBLIC` field instances) and nothing
//! else, so its descriptor, and with it the verification work, has P's shape.
//! Its Vesta proof is verified hard by an `Fq` circuit, as the final Omega
//! verifies P; the test prints the rows the verifier adds on every lane.

use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaField};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness,
    check::{CheckMode, check_circuit},
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance, InstanceType},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::bytes::{
    element::{decode_le_element, le_message_segments},
    tape::{BytesChip, BytesConfig},
};
use iroha_plonk_recursion::{
    codec::ScalarCells,
    verifier::{VerificationMode, VerifierChip, VerifierConfig, VerifierPlan},
};

/// Public instances of the wrap-shaped proof (P's crossing limbs, digests and
/// A_n's column are of this order).
const PUBLIC: usize = 200;

/// P's lanes and tape, with every table loaded and no other assignment.
#[derive(Clone, Default)]
struct WrapShape;

#[derive(Clone, Debug)]
struct WrapShapeConfig {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}

impl Circuit<Fp> for WrapShape {
    type Config = WrapShapeConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> WrapShapeConfig {
        let verifier = VerifierConfig::configure(meta);
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        let public = meta.instance_column(PUBLIC);
        WrapShapeConfig {
            verifier,
            bytes,
            public,
        }
    }

    fn synthesize(
        &self,
        config: WrapShapeConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let chip = VerifierChip::<Ep>::new(config.verifier);
        let bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let _ = config.public;
        Ok(())
    }
}

/// The Fq circuit that verifies the Vesta proof hard and records the rows the
/// verifier adds on each lane.
#[derive(Clone)]
struct Outer {
    plan: VerifierPlan<Eq>,
    key: iroha_plonk::VerifyingKey<Eq>,
    proof: Vec<u8>,
    instances: Vec<Fp>,
}

#[derive(Clone, Debug)]
struct OuterConfig {
    verifier: VerifierConfig<Eq>,
    bytes: BytesConfig,
}

thread_local! {
    static ROWS: std::cell::Cell<[[usize; 4]; 2]> = const { std::cell::Cell::new([[0; 4]; 2]) };
}

fn lanes(chip: &mut VerifierChip<Eq>) -> Result<[usize; 4], Error> {
    let lanes = chip.fold_lanes()?;
    Ok([
        lanes.ff.next_row(),
        lanes.ecc.next_row(),
        lanes.glue.next_row(),
        lanes.range.next_row(),
    ])
}

impl Circuit<Fq> for Outer {
    type Config = OuterConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> OuterConfig {
        let verifier = VerifierConfig::configure(meta);
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        OuterConfig { verifier, bytes }
    }

    fn synthesize(
        &self,
        config: OuterConfig,
        mut layouter: impl Layouter<Fq>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::<Eq>::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        layouter.assign_region(
            || "verify a wrap-shaped proof",
            |mut region| {
                let input: Vec<Value<u8>> =
                    self.proof.iter().map(|byte| Value::known(*byte)).collect();
                let run = bytes.run(
                    &mut region,
                    &input,
                    &vec![16; input.len() / 16],
                    &le_message_segments(0, input.len() / 32),
                )?;
                let mut proof = Vec::new();
                for index in 0..input.len() / 32 {
                    proof.push(decode_le_element(
                        &mut chip.uint(),
                        &mut region,
                        &run,
                        32 * index,
                    )?);
                }
                let mut instances = Vec::with_capacity(self.instances.len());
                for value in &self.instances {
                    instances.push(ScalarCells::<Eq>::assign_value(
                        &mut chip.uint(),
                        &mut region,
                        Value::known(*value),
                    )?);
                }
                let length = chip.uint().assign::<32>(
                    &mut region,
                    Value::known(u128::try_from(self.proof.len()).expect("small")),
                )?;
                let key = chip.constant_key(&mut region, &self.plan, &self.key)?;
                let before = lanes(&mut chip)?;
                let output = chip.verify(
                    &mut region,
                    &self.plan,
                    &key,
                    &[instances],
                    &proof,
                    &length,
                    VerificationMode::Hard,
                )?;
                let _ = output;
                let after = lanes(&mut chip)?;
                ROWS.with(|cell| cell.set([before, after]));
                Ok(())
            },
        )
    }
}

#[test]
#[ignore = "k16 keygen, proof and verifier circuit; run optimized"]
fn verifying_a_wrap_shaped_proof_in_circuit() {
    let params = PinnedParams::<Eq>::derive(16).expect("params");
    let pk = keygen_pk_v2(
        &params,
        &WrapShape,
        &KeygenConfigV2::pipa_r(vec![InstanceType::Field]),
    )
    .expect("wrap-shaped key");
    let instances: Vec<Fp> = (0..PUBLIC as u64).map(|i| Fp::from(i * 7 + 1)).collect();
    let witness = Witness::from_circuit(&pk, &WrapShape, &[instances.clone()]).expect("witness");
    let proof = create_proof_owned_with_claim(
        &params,
        &pk,
        witness,
        ProverRandomness::os(),
        ProverConfig::default(),
    )
    .expect("proof")
    .proof;
    let descriptor = pk.binding().descriptor();
    println!(
        "wrap-shaped descriptor: k {}, proof {} bytes, instance lengths {:?}",
        descriptor.k,
        proof.len(),
        descriptor.instance_lengths
    );
    let outer = Outer {
        plan: VerifierPlan::new(pk.binding().clone(), params).expect("plan"),
        key: pk.vk().clone(),
        proof,
        instances,
    };
    let report = check_circuit(&outer, 16, &[], CheckMode::Strict).expect("synthesis at k16");
    let [before, after] = ROWS.with(std::cell::Cell::get);
    println!(
        "in-circuit verification adds (FF, ECC, glue, range): {:?}; lanes end at {after:?}",
        [
            after[0] - before[0],
            after[1] - before[1],
            after[2] - before[2],
            after[3] - before[3]
        ]
    );
    assert!(
        report.is_satisfied(),
        "the honest wrap-shaped proof verifies hard"
    );
    let _ = (
        Fq::ONE,
        <Fp as PrimeField>::MODULUS,
        Fp::ZERO.to_canonical_limbs(),
    );
}

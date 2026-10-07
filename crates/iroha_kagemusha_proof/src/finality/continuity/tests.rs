//! Exact endpoint continuity, source binding, and explicit leaf obligations.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone)]
struct JoinCircuit {
    left: [u64; 6],
    right: [u64; 6],
    known: bool,
}
#[derive(Clone)]
struct Config {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}
impl Circuit<Fp> for JoinCircuit {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3).unwrap();
        let public = meta.instance_column(75);
        meta.enable_equality(public);
        Config { verifier, public }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let out = layouter.assign_region(
            || "contiguous source endpoints",
            |mut region| {
                let mut sides = Vec::new();
                for side in [self.left, self.right] {
                    let values = side.map(|x| {
                        if self.known {
                            Value::known(Fp::from(x))
                        } else {
                            Value::unknown()
                        }
                    });
                    let words = chip.uint().glue().witnesses(&mut region, &values)?;
                    let words = words.try_into().map_err(|_| Error::Synthesis)?;
                    sides.push(SourceEndpoints::from_words(&mut chip, &mut region, &words)?);
                }
                let joined = SourceEndpoints::join(&mut region, &sides[0], &sides[1])?;
                // Exercise the same canonical binding and explicit filler frame that
                // arithmetic leaves will use; this test grants no source semantics.
                let checkpoint = SourceCheckpoint::leaf(&mut chip, &mut region, joined)?;
                let frame = checkpoint.frame(&mut chip, &mut region)?;
                if frame.len() != 69 {
                    return Err(Error::Synthesis);
                }
                GlueChip::assert_equal(&mut region, &frame[0], checkpoint.digest())?;
                let binding = binding_digest(
                    &mut chip,
                    &mut region,
                    checkpoint.endpoints(),
                    checkpoint.pallas(),
                )?;
                GlueChip::assert_equal(&mut region, &frame[0], &binding)?;
                let mut output = checkpoint.endpoints().words().to_vec();
                output.extend(frame);
                Ok(output)
            },
        )?;
        for (i, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn accepts(circuit: &JoinCircuit) -> bool {
    let endpoints = [
        circuit.left[0],
        circuit.left[1],
        circuit.left[2],
        circuit.right[3],
        circuit.left[4],
        circuit.right[5],
    ]
    .map(Fp::from);
    let Ok(frame) = leaf_frame_native(endpoints) else {
        return false;
    };
    let mut public = endpoints.to_vec();
    public.extend(frame);
    let result = synthesize(circuit, 16, Some(&[public])).unwrap();
    check(&result.cs, &result.tables, CheckMode::Strict)
        .unwrap()
        .is_satisfied()
}
#[test]
fn source_endpoints_reject_gaps_reordering_context_switches_and_state_substitution() {
    let honest = JoinCircuit {
        left: [1, 2, 0, 3, 4, 5],
        right: [1, 2, 3, 7, 5, 6],
        known: true,
    };
    assert!(accepts(&honest));
    for index in [0, 1, 2, 4] {
        let mut forged = honest.clone();
        forged.right[index] += 1;
        assert!(!accepts(&forged), "changed right field {index}");
    }
    for right in [[1, 2, 3, 3, 5, 6], [1, 2, 3, 2, 5, 6], [0, 2, 3, 7, 5, 6]] {
        assert!(!accepts(&JoinCircuit {
            right,
            ..honest.clone()
        }));
    }
    assert!(!accepts(&JoinCircuit {
        left: honest.right,
        right: honest.left,
        known: true
    }));
    assert!(synthesize(&honest.without_witnesses(), 16, None).is_ok());
}

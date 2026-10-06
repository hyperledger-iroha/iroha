//! Canonical ordered roster commitment and membership substitution checks.
use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value},
};
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone)]
struct Membership {
    seat: u8,
    key: [u8; 48],
    path: [Fp; 5],
    root: Fp,
    known: bool,
}
#[derive(Clone)]
struct Config {
    verifier: VerifierConfig<Ep>,
    root: Column<Instance>,
}
impl Circuit<Fp> for Membership {
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
        let root = meta.instance_column(1);
        meta.enable_equality(root);
        Config { verifier, root }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let value = |x| {
            if self.known {
                Value::known(x)
            } else {
                Value::unknown()
            }
        };
        let root = layouter.assign_region(
            || "ordered roster key",
            |mut region| {
                let key = chip
                    .uint()
                    .glue()
                    .witnesses(
                        &mut region,
                        &self.key.map(|x| value(Fp::from(u64::from(x)))),
                    )?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let path = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &self.path.map(value))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let root = chip.uint().glue().witness(&mut region, value(self.root))?;
                verify_key_path(&mut chip, &mut region, self.seat, &key, &path, &root)?;
                Ok(root)
            },
        )?;
        layouter.constrain_instance(root.cell(), config.root, 0)
    }
}
fn accepts(circuit: &Membership) -> bool {
    check_circuit(circuit, 16, &[vec![circuit.root]], CheckMode::Strict)
        .is_ok_and(|report| report.is_satisfied())
}

#[test]
fn ordered_roster_paths_bind_full_keys_seats_siblings_and_zero_padding() {
    for n in [4, 31] {
        let keys = (0..n)
            .map(|i| core::array::from_fn(|j| (i * 48 + j) as u8))
            .collect::<Vec<_>>();
        let (root, paths) = key_tree_native(&keys).unwrap();
        for seat in [0, n - 1, 30] {
            let circuit = Membership {
                seat: seat as u8,
                key: *keys.get(seat).unwrap_or(&[0; 48]),
                path: paths[seat],
                root,
                known: true,
            };
            assert!(accepts(&circuit));
            let mut changed = circuit.clone();
            changed.key[47] ^= 1;
            assert!(!accepts(&changed));
        }
        let circuit = Membership {
            seat: 0,
            key: keys[0],
            path: paths[0],
            root,
            known: true,
        };
        let mut changed = circuit.clone();
        changed.seat = 1;
        assert!(!accepts(&changed));
        let mut changed = circuit;
        changed.path[4] += Fp::ONE;
        assert!(!accepts(&changed));
    }
    for n in [0, 1, 3, 5, 30, 32] {
        assert!(key_tree_native(&vec![[0; 48]; n]).is_err());
    }
}

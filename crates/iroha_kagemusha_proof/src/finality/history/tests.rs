//! Fixed genesis policy and exact state-bound history-join checks.

use super::*;
use crate::finality::continuity::SourceEndpoints;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_recursion::verifier::VerifierConfig;

fn anchor() -> HistoryAnchor {
    HistoryAnchor {
        network: [1; 32],
        instance: [2; 32],
        initial_context: [3; 32],
        initial_epoch: 0,
        parameters: [1000, 2000, 3000, 4000, 1 << 20, 100],
    }
}

#[test]
fn genesis_has_no_witness_selected_policy_and_every_anchor_field_is_bound() {
    let anchor = anchor();
    let source = GenesisSourceCircuit::new(anchor, Fp::from(77));
    let public = source.instances().unwrap();
    assert!(
        check_circuit(&source, 16, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    let original = synthesize(&source, 16, None).unwrap();
    let blank = synthesize(&source.without_witnesses(), 16, None).unwrap();
    assert_eq!(original.tables.fixed(), blank.tables.fixed());
    assert_eq!(original.tables.selectors(), blank.tables.selectors());
    assert_eq!(original.tables.permutation(), blank.tables.permutation());
    assert_eq!(
        original.tables.advice_assigned(),
        blank.tables.advice_assigned()
    );
    // The future wrapper identity is carried as advice, so its eventual value
    // cannot change this original source key or create a key-generation cycle.
    let other_key = GenesisSourceCircuit::new(anchor, Fp::from(78));
    let other_layout = synthesize(&other_key, 16, None).unwrap();
    assert_eq!(original.tables.fixed(), other_layout.tables.fixed());
    assert_eq!(original.tables.selectors(), other_layout.tables.selectors());
    assert_eq!(
        original.tables.permutation(),
        other_layout.tables.permutation()
    );
    assert_ne!(other_key.endpoints(), source.endpoints());
    assert!(
        !check_circuit(&other_key, 16, &public, CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    for at in 0..10 {
        let mut foreign = anchor;
        match at {
            0 => foreign.network[31] ^= 1,
            1 => foreign.instance[31] ^= 1,
            2 => foreign.initial_context[31] ^= 1,
            3 => foreign.initial_epoch += 1,
            _ => foreign.parameters[at - 4] += 1,
        }
        assert_ne!(foreign.digest(), anchor.digest());
        let substituted = GenesisSourceCircuit::new(foreign, Fp::from(77));
        assert_ne!(substituted.endpoints(), source.endpoints());
        let foreign_compiled = synthesize(&substituted, 16, None).unwrap();
        assert_ne!(original.tables.fixed(), foreign_compiled.tables.fixed());
        assert!(
            !check_circuit(&substituted, 16, &public, CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
    }
}

#[derive(Clone)]
struct Join {
    anchor: HistoryAnchor,
    children: [[Fp; 6]; 2],
    history_key: Fp,
    known: bool,
}
#[derive(Clone)]
struct Config {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}
impl Circuit<Fp> for Join {
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
        let public = meta.instance_column(6);
        meta.enable_equality(public);
        Config { verifier, public }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "history endpoint linkage",
            |mut region| {
                let anchor = chip
                    .uint()
                    .glue()
                    .constant(&mut region, self.anchor.digest())?;
                let mut endpoints = Vec::new();
                for values in self.children {
                    let words = chip.uint().glue().witnesses(
                        &mut region,
                        &values.map(|v| {
                            if self.known {
                                Value::known(v)
                            } else {
                                Value::unknown()
                            }
                        }),
                    )?;
                    endpoints.push(SourceEndpoints::from_words(
                        &mut chip,
                        &mut region,
                        &words.try_into().map_err(|_| Error::Synthesis)?,
                    )?);
                }
                let key = chip.uint().glue().witness(
                    &mut region,
                    if self.known {
                        Value::known(self.history_key)
                    } else {
                        Value::unknown()
                    },
                )?;
                Ok(append::join_cells(
                    &mut chip,
                    &mut region,
                    &anchor,
                    &key,
                    [&endpoints[0], &endpoints[1]],
                )?
                .words())
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

#[test]
fn history_join_requires_the_exact_anchor_envelopes_and_complete_state_continuity() {
    let anchor = anchor();
    let h = anchor.digest();
    let first = Fp::from(42);
    let last = Fp::from(73);
    let honest = Join {
        anchor,
        known: true,
        history_key: Fp::from(77),
        children: [
            [
                Fp::from(PREFIX_PROGRAM_ID),
                prefix_context(h, Fp::from(77)),
                Fp::ZERO,
                Fp::ONE,
                Fp::ZERO,
                first,
            ],
            [Fp::from(PROGRAM_ID), h, Fp::ZERO, Fp::ONE, first, last],
        ],
    };
    let public = vec![vec![
        Fp::from(PREFIX_PROGRAM_ID),
        prefix_context(h, Fp::from(77)),
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        last,
    ]];
    let accepts = |c: &Join| {
        check_circuit(c, 16, &public, CheckMode::Strict).is_ok_and(|report| report.is_satisfied())
    };
    assert!(accepts(&honest));
    let mut substituted_key = honest.clone();
    substituted_key.history_key += Fp::ONE;
    assert!(
        !accepts(&substituted_key),
        "the complete actual wrapper key is mandatory"
    );
    for side in 0..2 {
        for at in 0..6 {
            let mut forged = honest.clone();
            forged.children[side][at] += Fp::ONE;
            assert!(!accepts(&forged), "accepted child{side} field{at}");
        }
    }
    let known = synthesize(&honest, 16, Some(&public)).unwrap();
    let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.selectors(), unknown.tables.selectors());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
}

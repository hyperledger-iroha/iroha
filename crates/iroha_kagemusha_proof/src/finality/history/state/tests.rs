//! Canonical state encoding and forbidden Pending authority; no history acceptance.

use super::*;
use crate::finality::{continuity::SourcePairConfig, history::HistoryAnchorCells};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner, synthesize},
};

#[derive(Clone)]
struct StateProbe {
    anchor: HistoryAnchor,
    state: HistoryState,
    known: bool,
}
impl Circuit<Fp> for StateProbe {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier =
            iroha_plonk_recursion::verifier::VerifierConfig::configure_serialized_foreign_tagged(
                meta, 3,
            )
            .unwrap();
        let public = meta.instance_column(1);
        meta.enable_equality(public);
        SourcePairConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let state = layouter.assign_region(
            || "canonical history state",
            |mut region| {
                let anchor = HistoryAnchorCells::assign(&mut chip, &mut region, &self.anchor)?;
                HistoryStateCells::assign(
                    &mut chip,
                    &mut region,
                    &if self.known {
                        Value::known(self.state)
                    } else {
                        Value::unknown()
                    },
                    anchor.digest(),
                )
            },
        )?;
        layouter.constrain_instance(state.digest().cell(), config.public, 0)
    }
}
fn sample() -> StateProbe {
    // Fixed synthetic policy for state-codec checks; no signed genesis is claimed.
    let anchor = HistoryAnchor {
        network: [3; 32],
        instance: [7; 32],
        initial_context: [11; 32],
        initial_epoch: 0,
        parameters: [1, 2, 3, 4, 5, 6],
    };
    StateProbe {
        anchor,
        state: HistoryState::genesis(&anchor),
        known: true,
    }
}
fn accepts(probe: &StateProbe) -> bool {
    check_circuit(
        probe,
        16,
        &[vec![probe.state.digest(probe.anchor.digest())]],
        CheckMode::Strict,
    )
    .is_ok_and(|report| report.is_satisfied())
}
#[test]
fn canonical_genesis_ready_and_pending_states_match_native_commitments() {
    let initial = sample();
    assert!(initial.state.is_canonical());
    assert_eq!(initial.state.next_height, 2);
    assert_eq!(initial.state.current, initial.state.following);
    assert_eq!(initial.state.result, [0; 32]);
    assert_eq!(initial.state.tape_root, Fp::ZERO);
    assert_eq!(initial.state.frame_len, 0);
    assert!(accepts(&initial));
    let mut pending = initial.clone();
    pending.state.next_height = u64::from(u32::MAX) + 40;
    pending.state.following = HistorySlot {
        pending: true,
        boundary_height: pending.state.next_height,
        predecessor: initial.anchor.initial_context,
        parameters: [7, 8, 9, 10, 11, 12],
        ..HistorySlot::default()
    };
    pending.state.result = [19; 32];
    pending.state.tape_root = Fp::from(23);
    pending.state.frame_len = 65_536;
    assert!(pending.state.is_canonical());
    assert!(accepts(&pending));
    let a = synthesize(
        &pending,
        16,
        Some(&[vec![pending.state.digest(pending.anchor.digest())]]),
    )
    .unwrap();
    let b = synthesize(&pending.without_witnesses(), 16, None).unwrap();
    assert_eq!(a.tables.fixed(), b.tables.fixed());
    assert_eq!(a.tables.selectors(), b.tables.selectors());
    assert_eq!(a.tables.permutation(), b.tables.permutation());
    assert_eq!(a.tables.advice_assigned(), b.tables.advice_assigned());
}
#[test]
fn unused_variant_fields_pending_current_and_bad_bounds_are_rejected() {
    let base = sample();
    for fault in 0..8 {
        let mut changed = base.clone();
        match fault {
            0 => changed.state.current.boundary_height = 1,
            1 => changed.state.current.predecessor[0] = 1,
            2 => changed.state.following.boundary_height = 1,
            3 => changed.state.following.predecessor[31] = 1,
            4 => changed.state.following.pending = true,
            5 => {
                changed.state.current = HistorySlot {
                    pending: true,
                    ..HistorySlot::default()
                };
            }
            6 => changed.state.next_height = 1,
            7 => changed.state.frame_len = 65_537,
            _ => unreachable!(),
        }
        assert!(!changed.state.is_canonical());
        assert!(!accepts(&changed), "noncanonical state fault {fault}");
    }
    assert!(
        !check_circuit(
            &base,
            16,
            &[vec![base.state.digest(base.anchor.digest()) + Fp::ONE]],
            CheckMode::Strict
        )
        .unwrap()
        .is_satisfied()
    );
}

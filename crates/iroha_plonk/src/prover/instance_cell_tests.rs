//! The instance cell leaf ([`Expression::instance_cell`]): one public value
//! read on every row. Proofs that use one verify on both curves under both
//! Direct choices and bind that value (the constraint checker and the
//! verifier reject a changed instance), and a Committed-mode key refuses it.

use iroha_pasta::{Ep, Eq, PastaCurve, PastaField, poseidon::PoseidonField};

use crate::{
    check::{CheckMode, check_circuit},
    cs::{Advice, Column, ConstraintSystem, Expression, InstanceModeV1, Rotation, Selector},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
    keys::keygen_pk,
    pcs::ipa::PinnedParams,
    test_circuits::{CHOICES, K, keygen_config, setup},
};

/// `b = a * instances[0][1]` on every enabled row, the factor read through an
/// instance cell; instance column 0 holds `[start, factor]` and is never
/// queried as a column.
#[derive(Clone, Copy, Debug)]
struct Scaled {
    rows: usize,
    factor: u64,
}

impl<F: PastaField> Circuit<F> for Scaled {
    type Config = (Column<Advice>, Column<Advice>, Selector);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        *self
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let a = meta.advice_column();
        let b = meta.advice_column();
        let instance = meta.instance_column(2);
        let enabled = meta.selector();
        meta.create_gate("scaled", |cells| {
            let enabled = cells.query_selector(enabled);
            let a = cells.query_advice(a, Rotation::cur());
            let b = cells.query_advice(b, Rotation::cur());
            vec![enabled * (b - a * Expression::instance_cell(instance, 1))]
        });
        (a, b, enabled)
    }

    fn synthesize(
        &self,
        (a, b, enabled): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "rows",
            |mut region| {
                for row in 0..self.rows {
                    enabled.enable(&mut region, row)?;
                    let value = F::from(row as u64 + 2);
                    region.assign_advice(a, row, Value::known(value))?;
                    region.assign_advice(b, row, Value::known(value * F::from(self.factor)))?;
                }
                Ok(())
            },
        )
    }
}

const SCALED: Scaled = Scaled {
    rows: 9,
    factor: 11,
};

fn instances<F: PastaField>(factor: u64) -> Vec<Vec<F>> {
    vec![vec![F::from(5), F::from(factor)]]
}

fn bound<C>()
where
    C: PastaCurve,
    C::ScalarExt: PoseidonField,
    C::Base: PoseidonField,
{
    let right = instances::<C::ScalarExt>(SCALED.factor);
    let wrong = instances::<C::ScalarExt>(SCALED.factor + 1);
    let satisfied = |instances: &[Vec<C::ScalarExt>]| {
        check_circuit(&SCALED, K, instances, CheckMode::Strict)
            .expect("synthesis")
            .is_satisfied()
    };
    assert!(satisfied(&right));
    assert!(!satisfied(&wrong));
    let direct = CHOICES
        .into_iter()
        .filter(|choice| choice.1 == InstanceModeV1::Direct);
    for choice in direct {
        let setup = setup::<C, _>(&SCALED, choice);
        let proof = setup.prove(&SCALED, &right, 3).expect("proof");
        assert_eq!(setup.verify(&right, &proof), Ok(()), "{choice:?}");
        assert!(setup.verify(&wrong, &proof).is_err(), "{choice:?}");
        let forged = setup.prove(&SCALED, &wrong, 3);
        assert!(
            forged.map_or(true, |proof| setup.verify(&wrong, &proof).is_err()),
            "{choice:?}"
        );
    }
}

#[test]
fn instance_cells_verify_and_bind_their_value_on_both_curves() {
    bound::<Ep>();
    bound::<Eq>();
}

#[test]
fn committed_instances_refuse_instance_cells() {
    let params = PinnedParams::<Ep>::derive(K).expect("params");
    for choice in CHOICES
        .into_iter()
        .filter(|choice| choice.1 == InstanceModeV1::Committed)
    {
        assert!(
            keygen_pk(&params, &SCALED, &keygen_config(choice)).is_err(),
            "{choice:?}"
        );
    }
}

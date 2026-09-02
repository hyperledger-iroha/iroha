//! Field-neutral fixed-shape arithmetic kernel for all seven Offline Cash V1 operations.
//!
//! This is the executable balance/sequence/epoch portion of the final recursive state
//! circuit. It is intentionally not an [`super::OfflineCashRecursiveVerifierV1`] backend: the
//! production circuit must additionally constrain canonical state commitments, transport and
//! GuardBundle statement hashes, sparse-Merkle insertion, recursive predecessor verification, and
//! BGH19 delayed-history accumulation before an artifact release may use it for monetary proof.

use core::marker::PhantomData;

use ff::PrimeField;
use halo2_proofs::{
    circuit::{Layouter, V1, Value},
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Expression, Instance,
        Selector,
    },
    poly::Rotation,
};
use iroha_data_model::offline::OfflineCashPastaStateCommitmentV1;

use super::OfflineCashOperationV1;
use crate::zk::offline_cash_v1_poseidon::OfflineCashPoseidonFieldV1;

const MAIN_COLUMNS: usize = 25;
const OPERATION_INSTANCE_OFFSET: usize = 0;

const OP: usize = 0;
const S_BOOTSTRAP: usize = 1;
const S_MINT: usize = 2;
const S_SEND: usize = 3;
const S_RECEIVE: usize = 4;
const S_REDEEM: usize = 5;
const S_SUITE_UPGRADE: usize = 6;
const S_ROTATE: usize = 7;
const BALANCE_BEFORE: usize = 8;
const BALANCE_AFTER: usize = 9;
const AMOUNT: usize = 10;
const SEQUENCE_BEFORE: usize = 11;
const SEQUENCE_AFTER: usize = 12;
const EPOCH_BEFORE: usize = 13;
const EPOCH_AFTER: usize = 14;
const ROOT_BEFORE_LO: usize = 15;
const ROOT_BEFORE_HI: usize = 16;
const ROOT_AFTER_LO: usize = 17;
const ROOT_AFTER_HI: usize = 18;
const AMOUNT_INV: usize = 19;
const BOOTSTRAP_EPOCH_INV: usize = 20;
const ROOT_LO_INV: usize = 21;
const ROOT_HI_INV: usize = 22;
const ROOT_LO_NONZERO: usize = 23;
const ROOT_HI_NONZERO: usize = 24;

const RANGE_VALUES: [usize; 11] = [
    BALANCE_BEFORE,
    BALANCE_AFTER,
    AMOUNT,
    SEQUENCE_BEFORE,
    SEQUENCE_AFTER,
    EPOCH_BEFORE,
    EPOCH_AFTER,
    ROOT_BEFORE_LO,
    ROOT_BEFORE_HI,
    ROOT_AFTER_LO,
    ROOT_AFTER_HI,
];

/// Private field-neutral witness for the seven-operation arithmetic kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineCashOperationRelationWitnessV1 {
    /// Selected fixed-shape operation.
    pub operation: OfflineCashOperationV1,
    /// Consumed aggregate balance.
    pub balance_before: u128,
    /// Produced aggregate balance.
    pub balance_after: u128,
    /// Positive monetary amount, or zero for bootstrap/rotation.
    pub amount: u128,
    /// Consumed logical sequence.
    pub logical_sequence_before: u128,
    /// Produced logical sequence.
    pub logical_sequence_after: u128,
    /// Consumed hardware-epoch generation.
    pub hardware_epoch_before: u128,
    /// Produced hardware-epoch generation.
    pub hardware_epoch_after: u128,
    /// Consumed sparse-Merkle replay root.
    pub replay_root_before: OfflineCashPastaStateCommitmentV1,
    /// Produced sparse-Merkle replay root.
    pub replay_root_after: OfflineCashPastaStateCommitmentV1,
}

impl OfflineCashOperationRelationWitnessV1 {
    fn operation_tag(self) -> u64 {
        match self.operation {
            OfflineCashOperationV1::Bootstrap => 0,
            OfflineCashOperationV1::MintFold => 1,
            OfflineCashOperationV1::SendSplit => 2,
            OfflineCashOperationV1::ReceiveFold => 3,
            OfflineCashOperationV1::RedeemSplit => 4,
            OfflineCashOperationV1::SuiteUpgrade => 5,
            OfflineCashOperationV1::Rotate => 6,
        }
    }

    fn selectors(self) -> [u64; 7] {
        let mut selectors = [0; 7];
        selectors[usize::try_from(self.operation_tag()).expect("operation tag fits usize")] = 1;
        selectors
    }

    fn values<F: OfflineCashPoseidonFieldV1>(self) -> [u128; RANGE_VALUES.len()] {
        let (root_before_lo, root_before_hi) =
            digest_chunks(F::select_component(self.replay_root_before));
        let (root_after_lo, root_after_hi) =
            digest_chunks(F::select_component(self.replay_root_after));
        [
            self.balance_before,
            self.balance_after,
            self.amount,
            self.logical_sequence_before,
            self.logical_sequence_after,
            self.hardware_epoch_before,
            self.hardware_epoch_after,
            root_before_lo,
            root_before_hi,
            root_after_lo,
            root_after_hi,
        ]
    }
}

/// Halo2 configuration for the fixed seven-operation arithmetic kernel.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashOperationRelationConfigV1 {
    main: [Column<Advice>; MAIN_COLUMNS],
    range_bit: Column<Advice>,
    range_accumulator: Column<Advice>,
    operation_instance: Column<Instance>,
    q_main: Selector,
    q_bit: Selector,
    q_accumulate: Selector,
}

/// One field-neutral circuit shape used by both Eq/Fp and Ep/Fq state parities.
#[derive(Clone, Copy, Debug)]
pub struct OfflineCashOperationRelationCircuitV1<F> {
    witness: Option<OfflineCashOperationRelationWitnessV1>,
    marker: PhantomData<F>,
}

impl<F> Default for OfflineCashOperationRelationCircuitV1<F> {
    fn default() -> Self {
        Self {
            witness: None,
            marker: PhantomData,
        }
    }
}

impl<F> OfflineCashOperationRelationCircuitV1<F> {
    /// Construct one witnessed arithmetic relation.
    #[must_use]
    pub const fn new(witness: OfflineCashOperationRelationWitnessV1) -> Self {
        Self {
            witness: Some(witness),
            marker: PhantomData,
        }
    }
}

impl<F> Circuit<F> for OfflineCashOperationRelationCircuitV1<F>
where
    F: OfflineCashPoseidonFieldV1,
{
    type Config = OfflineCashOperationRelationConfigV1;
    // The arithmetic kernel uses one main region plus one range-check region for
    // each public u128 value.  The vendored `SimpleFloorPlanner` deliberately
    // overlays every region at row zero, which would make all range-check
    // terminal cells alias and spuriously equate unrelated witness values.  V1
    // gives these same-column regions disjoint rows while preserving the fixed
    // circuit shape.
    type FloorPlanner = V1;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let main = std::array::from_fn(|_| meta.advice_column());
        let range_bit = meta.advice_column();
        let range_accumulator = meta.advice_column();
        let operation_instance = meta.instance_column();
        let q_main = meta.selector();
        let q_bit = meta.selector();
        let q_accumulate = meta.selector();
        meta.enable_equality(main[OP]);
        for column in RANGE_VALUES {
            meta.enable_equality(main[column]);
        }
        meta.enable_equality(range_accumulator);
        meta.enable_equality(operation_instance);

        meta.create_gate("Offline Cash V1 seven-operation relation", |meta| {
            let q = meta.query_selector(q_main);
            let mut value = |column| meta.query_advice(main[column], Rotation::cur());
            let op = value(OP);
            let selectors = [
                value(S_BOOTSTRAP),
                value(S_MINT),
                value(S_SEND),
                value(S_RECEIVE),
                value(S_REDEEM),
                value(S_SUITE_UPGRADE),
                value(S_ROTATE),
            ];
            let one = Expression::Constant(F::ONE);
            let zero = Expression::Constant(F::ZERO);
            let selector_sum = selectors
                .iter()
                .cloned()
                .fold(zero.clone(), |sum, selector| sum + selector);
            let encoded_op = selectors
                .iter()
                .cloned()
                .enumerate()
                .fold(zero.clone(), |sum, (tag, selector)| {
                    sum + selector * Expression::Constant(F::from(tag as u64))
                });
            let bootstrap = selectors[0].clone();
            let mint = selectors[1].clone();
            let send = selectors[2].clone();
            let receive = selectors[3].clone();
            let redeem = selectors[4].clone();
            let suite_upgrade = selectors[5].clone();
            let rotate = selectors[6].clone();
            let inbound = mint.clone() + receive.clone();
            let outbound = send.clone() + redeem.clone();
            let monetary = inbound.clone() + outbound.clone();
            let ordinary = monetary.clone() + suite_upgrade.clone();
            let balance_before = value(BALANCE_BEFORE);
            let balance_after = value(BALANCE_AFTER);
            let amount = value(AMOUNT);
            let sequence_before = value(SEQUENCE_BEFORE);
            let sequence_after = value(SEQUENCE_AFTER);
            let epoch_before = value(EPOCH_BEFORE);
            let epoch_after = value(EPOCH_AFTER);
            let root_before_lo = value(ROOT_BEFORE_LO);
            let root_before_hi = value(ROOT_BEFORE_HI);
            let root_after_lo = value(ROOT_AFTER_LO);
            let root_after_hi = value(ROOT_AFTER_HI);
            let root_lo_delta = root_after_lo.clone() - root_before_lo.clone();
            let root_hi_delta = root_after_hi.clone() - root_before_hi.clone();
            let root_lo_nonzero = value(ROOT_LO_NONZERO);
            let root_hi_nonzero = value(ROOT_HI_NONZERO);
            let root_changed = root_lo_nonzero.clone() + root_hi_nonzero.clone()
                - root_lo_nonzero.clone() * root_hi_nonzero.clone();
            let mut constraints = selectors
                .iter()
                .cloned()
                .map(|selector| q.clone() * selector.clone() * (selector - one.clone()))
                .collect::<Vec<_>>();
            constraints.extend([
                q.clone() * (selector_sum - one.clone()),
                q.clone() * (op - encoded_op),
                q.clone() * bootstrap.clone() * balance_before.clone(),
                q.clone() * bootstrap.clone() * balance_after.clone(),
                q.clone() * bootstrap.clone() * amount.clone(),
                q.clone() * bootstrap.clone() * sequence_before.clone(),
                q.clone() * bootstrap.clone() * sequence_after.clone(),
                q.clone() * bootstrap.clone() * epoch_before.clone(),
                q.clone() * bootstrap.clone() * root_before_lo.clone(),
                q.clone() * bootstrap.clone() * root_before_hi.clone(),
                q.clone()
                    * bootstrap.clone()
                    * (epoch_after.clone() * value(BOOTSTRAP_EPOCH_INV) - one.clone()),
                q.clone()
                    * inbound.clone()
                    * (balance_after.clone() - balance_before.clone() - amount.clone()),
                q.clone()
                    * outbound.clone()
                    * (balance_before.clone() - balance_after.clone() - amount.clone()),
                q.clone()
                    * ordinary.clone()
                    * (sequence_after.clone() - sequence_before.clone() - one.clone()),
                q.clone() * rotate.clone() * sequence_after.clone(),
                q.clone() * ordinary.clone() * (epoch_after.clone() - epoch_before.clone()),
                q.clone()
                    * rotate.clone()
                    * (epoch_after.clone() - epoch_before.clone() - one.clone()),
                q.clone() * monetary.clone() * (amount.clone() * value(AMOUNT_INV) - one.clone()),
                q.clone() * (suite_upgrade.clone() + rotate.clone()) * amount,
                q.clone()
                    * (suite_upgrade.clone() + rotate.clone())
                    * (balance_after - balance_before),
                q.clone()
                    * (outbound.clone() + suite_upgrade.clone() + rotate.clone())
                    * (root_after_lo.clone() - root_before_lo.clone()),
                q.clone()
                    * (outbound + suite_upgrade + rotate)
                    * (root_after_hi.clone() - root_before_hi.clone()),
                q.clone() * root_lo_nonzero.clone() * (root_lo_nonzero.clone() - one.clone()),
                q.clone() * root_hi_nonzero.clone() * (root_hi_nonzero.clone() - one.clone()),
                q.clone() * (root_lo_delta.clone() * value(ROOT_LO_INV) - root_lo_nonzero.clone()),
                q.clone() * root_lo_delta * (one.clone() - root_lo_nonzero.clone()),
                q.clone() * (root_hi_delta.clone() * value(ROOT_HI_INV) - root_hi_nonzero.clone()),
                q.clone() * root_hi_delta * (one.clone() - root_hi_nonzero),
                q * inbound * (root_changed - one),
            ]);
            constraints
        });

        meta.create_gate("Offline Cash V1 u128 bit", |meta| {
            let q = meta.query_selector(q_bit);
            let bit = meta.query_advice(range_bit, Rotation::cur());
            vec![q * bit.clone() * (bit - Expression::Constant(F::ONE))]
        });
        meta.create_gate("Offline Cash V1 u128 accumulator", |meta| {
            let q = meta.query_selector(q_accumulate);
            let bit = meta.query_advice(range_bit, Rotation::cur());
            let before = meta.query_advice(range_accumulator, Rotation::cur());
            let after = meta.query_advice(range_accumulator, Rotation::next());
            vec![q * (after - before * Expression::Constant(F::from(2)) - bit)]
        });

        OfflineCashOperationRelationConfigV1 {
            main,
            range_bit,
            range_accumulator,
            operation_instance,
            q_main,
            q_bit,
            q_accumulate,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let witness = self.witness;
        let assigned = layouter.assign_region(
            || "Offline Cash V1 operation",
            |mut region| {
                config.q_main.enable(&mut region, 0)?;
                let mut raw = [0_u128; MAIN_COLUMNS];
                if let Some(witness) = witness {
                    raw[OP] = u128::from(witness.operation_tag());
                    for (target, selector) in raw[S_BOOTSTRAP..=S_ROTATE]
                        .iter_mut()
                        .zip(witness.selectors())
                    {
                        *target = u128::from(selector);
                    }
                    for (column, value) in RANGE_VALUES.into_iter().zip(witness.values::<F>()) {
                        raw[column] = value;
                    }
                    let (before_lo, before_hi) =
                        digest_chunks(F::select_component(witness.replay_root_before));
                    let (after_lo, after_hi) =
                        digest_chunks(F::select_component(witness.replay_root_after));
                    raw[ROOT_LO_NONZERO] = u128::from(before_lo != after_lo);
                    raw[ROOT_HI_NONZERO] = u128::from(before_hi != after_hi);
                }

                let mut cells = Vec::with_capacity(MAIN_COLUMNS);
                for (index, column) in config.main.into_iter().enumerate() {
                    let field_value = match (witness, index) {
                        (Some(witness), AMOUNT_INV) => inverse_or_zero::<F>(witness.amount),
                        (Some(witness), BOOTSTRAP_EPOCH_INV) => {
                            inverse_or_zero::<F>(witness.hardware_epoch_after)
                        }
                        (Some(witness), ROOT_LO_INV) => {
                            let (before, _) =
                                digest_chunks(F::select_component(witness.replay_root_before));
                            let (after, _) =
                                digest_chunks(F::select_component(witness.replay_root_after));
                            inverse_delta_or_zero::<F>(before, after)
                        }
                        (Some(witness), ROOT_HI_INV) => {
                            let (_, before) =
                                digest_chunks(F::select_component(witness.replay_root_before));
                            let (_, after) =
                                digest_chunks(F::select_component(witness.replay_root_after));
                            inverse_delta_or_zero::<F>(before, after)
                        }
                        (Some(_), _) => Value::known(field_from_u128::<F>(raw[index])),
                        (None, _) => Value::unknown(),
                    };
                    cells.push(region.assign_advice(column, 0, field_value));
                }
                Ok(cells)
            },
        )?;
        layouter.constrain_instance(
            assigned[OP].cell(),
            config.operation_instance,
            OPERATION_INSTANCE_OFFSET,
        );

        for (value_index, main_column) in RANGE_VALUES.into_iter().enumerate() {
            let value = witness.map(|witness| witness.values::<F>()[value_index]);
            let main_cell = assigned[main_column].clone();
            layouter.assign_region(
                || "Offline Cash V1 u128 range",
                |mut region| {
                    let mut accumulator = 0_u128;
                    for row in 0..128 {
                        config.q_bit.enable(&mut region, row)?;
                        config.q_accumulate.enable(&mut region, row)?;
                        let bit = value.map(|value| (value >> (127 - row)) & 1);
                        region.assign_advice(
                            config.range_bit,
                            row,
                            match bit {
                                Some(bit) => Value::known(F::from(bit as u64)),
                                None => Value::unknown(),
                            },
                        );
                        region.assign_advice(
                            config.range_accumulator,
                            row,
                            match value {
                                Some(_) => Value::known(field_from_u128::<F>(accumulator)),
                                None => Value::unknown(),
                            },
                        );
                        if let Some(bit) = bit {
                            accumulator = accumulator * 2 + bit;
                        }
                    }
                    Ok(main_cell.copy_advice(&mut region, config.range_accumulator, 128))
                },
            )?;
        }
        Ok(())
    }
}

fn digest_chunks(digest: [u8; 32]) -> (u128, u128) {
    let lo = u128::from_le_bytes(digest[..16].try_into().expect("fixed digest half"));
    let hi = u128::from_le_bytes(digest[16..].try_into().expect("fixed digest half"));
    (lo, hi)
}

fn field_from_u128<F: PrimeField + From<u64>>(value: u128) -> F {
    F::from(value as u64) + F::from((value >> 64) as u64) * F::from_u128(1_u128 << 64)
}

fn inverse_or_zero<F>(value: u128) -> Value<F>
where
    F: PrimeField + From<u64>,
{
    if value == 0 {
        Value::known(F::ZERO)
    } else {
        Value::known(field_from_u128::<F>(value).invert().unwrap())
    }
}

fn inverse_delta_or_zero<F>(before: u128, after: u128) -> Value<F>
where
    F: PrimeField + From<u64>,
{
    let delta = field_from_u128::<F>(after) - field_from_u128::<F>(before);
    if delta == F::ZERO {
        Value::known(F::ZERO)
    } else {
        Value::known(delta.invert().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    use super::*;
    use crate::zk::offline_cash_v1_poseidon::encode;

    fn root(value: u64) -> OfflineCashPastaStateCommitmentV1 {
        OfflineCashPastaStateCommitmentV1 {
            eq: encode(Fp::from(value)),
            ep: encode(Fq::from(value)),
        }
    }

    fn witness(operation: OfflineCashOperationV1) -> OfflineCashOperationRelationWitnessV1 {
        let mut witness = OfflineCashOperationRelationWitnessV1 {
            operation,
            balance_before: 9,
            balance_after: 9,
            amount: 0,
            logical_sequence_before: 8,
            logical_sequence_after: 9,
            hardware_epoch_before: 2,
            hardware_epoch_after: 2,
            replay_root_before: root(0x31),
            replay_root_after: root(0x31),
        };
        match operation {
            OfflineCashOperationV1::Bootstrap => {
                witness.balance_before = 0;
                witness.balance_after = 0;
                witness.logical_sequence_before = 0;
                witness.logical_sequence_after = 0;
                witness.hardware_epoch_before = 0;
                witness.hardware_epoch_after = 7;
                witness.replay_root_before = OfflineCashPastaStateCommitmentV1::ZERO;
            }
            OfflineCashOperationV1::MintFold | OfflineCashOperationV1::ReceiveFold => {
                witness.amount = 5;
                witness.balance_after = 14;
                witness.replay_root_after = root(0x32);
            }
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit => {
                witness.amount = 5;
                witness.balance_after = 4;
            }
            OfflineCashOperationV1::SuiteUpgrade => {}
            OfflineCashOperationV1::Rotate => {
                witness.logical_sequence_after = 0;
                witness.hardware_epoch_after = 3;
            }
        }
        witness
    }

    #[test]
    fn all_seven_relations_are_executable() {
        for operation in [
            OfflineCashOperationV1::Bootstrap,
            OfflineCashOperationV1::MintFold,
            OfflineCashOperationV1::SendSplit,
            OfflineCashOperationV1::ReceiveFold,
            OfflineCashOperationV1::RedeemSplit,
            OfflineCashOperationV1::SuiteUpgrade,
            OfflineCashOperationV1::Rotate,
        ] {
            let witness = witness(operation);
            let prover = MockProver::run(
                12,
                &OfflineCashOperationRelationCircuitV1::<Fp>::new(witness),
                vec![vec![Fp::from(witness.operation_tag())]],
            )
            .expect("fixed relation synthesizes");
            prover.assert_satisfied();
        }
    }

    #[test]
    fn overflow_and_zero_monetary_amount_are_rejected() {
        let mut overflow = witness(OfflineCashOperationV1::MintFold);
        overflow.balance_before = u128::MAX;
        overflow.balance_after = 4;
        let prover = MockProver::run(
            12,
            &OfflineCashOperationRelationCircuitV1::<Fp>::new(overflow),
            vec![vec![Fp::from(1)]],
        )
        .expect("overflow witness synthesizes before constraint failure");
        assert!(prover.verify().is_err());

        let mut zero = witness(OfflineCashOperationV1::SendSplit);
        zero.amount = 0;
        zero.balance_after = zero.balance_before;
        let prover = MockProver::run(
            12,
            &OfflineCashOperationRelationCircuitV1::<Fp>::new(zero),
            vec![vec![Fp::from(2)]],
        )
        .expect("zero witness synthesizes before constraint failure");
        assert!(prover.verify().is_err());
    }
}

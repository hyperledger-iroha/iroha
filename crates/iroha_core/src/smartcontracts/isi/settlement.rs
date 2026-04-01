//! Host execution for delivery-versus-payment and payment-versus-payment settlements.

#[cfg(feature = "telemetry")]
use std::time::Instant;

use iroha_data_model::{
    asset::AssetId,
    isi::{
        error::{InstructionEvaluationError, InstructionExecutionError},
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder,
            SettlementInstructionBox, SettlementLeg, SettlementPlan,
        },
    },
    prelude::*,
    query::error::FindError,
};
use iroha_primitives::numeric::{Numeric, NumericSpec};

use super::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use crate::smartcontracts::isi::error::MathError;
#[cfg(feature = "telemetry")]
use crate::sumeragi::status::SettlementOutcomeKind;

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub(crate) const SETTLEMENT_KIND_DVP: &str = "dvp";
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub(crate) const SETTLEMENT_KIND_PVP: &str = "pvp";

impl Execute for SettlementInstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            SettlementInstructionBox::Dvp(isi) => isi.execute(authority, stx),
            SettlementInstructionBox::Pvp(isi) => isi.execute(authority, stx),
        }
    }
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
fn settlement_failure_reason(err: &Error) -> &'static str {
    match err {
        InstructionExecutionError::InvariantViolation(message) => {
            let msg = message.as_ref();
            if msg.contains("non-zero") {
                "zero_quantity"
            } else if msg.contains("reciprocal") {
                "counterparty_mismatch"
            } else if msg.contains("not supported yet") {
                "unsupported_policy"
            } else if msg.contains("available") || msg.contains("requires") {
                "insufficient_funds"
            } else {
                "other"
            }
        }
        InstructionExecutionError::Find(_) => "missing_entity",
        InstructionExecutionError::Math(_) => "math_error",
        InstructionExecutionError::Evaluate(InstructionEvaluationError::Type(_)) => "type_error",
        _ => "other",
    }
}

#[allow(clippy::too_many_arguments)]
fn record_settlement_snapshot(
    stx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    settlement_id: &SettlementId,
    plan: SettlementPlan,
    metadata: Metadata,
    kind: SettlementKind,
    legs: Vec<SettlementLegSnapshot>,
    outcome: SettlementOutcomeRecord,
) {
    let mut ledger = stx
        .world
        .settlement_ledgers
        .get(settlement_id)
        .cloned()
        .unwrap_or_else(SettlementLedger::default);

    let block_height = stx._curr_block.height().get();
    let block_hash = stx._curr_block.hash();
    let executed_at_ms = u64::try_from(
        stx._curr_block
            .creation_time()
            .as_millis()
            .min(u128::from(u64::MAX)),
    )
    .unwrap_or(u64::MAX);

    ledger.push(SettlementLedgerEntry {
        settlement_id: settlement_id.clone(),
        kind,
        authority: authority.clone(),
        plan,
        metadata,
        block_height,
        block_hash,
        executed_at_ms,
        legs,
        outcome,
    });

    stx.world
        .settlement_ledgers
        .insert(settlement_id.clone(), ledger);
}

fn dvp_leg_snapshots(
    plan: SettlementPlan,
    outcome: &SettlementPairOutcome,
    delivery_leg: &SettlementLeg,
    payment_leg: &SettlementLeg,
) -> Vec<SettlementLegSnapshot> {
    let (delivery_committed, payment_committed) = dvp_committed(plan, outcome);
    vec![
        SettlementLegSnapshot {
            role: SettlementLegRole::Delivery,
            leg: delivery_leg.clone(),
            committed: delivery_committed,
        },
        SettlementLegSnapshot {
            role: SettlementLegRole::Payment,
            leg: payment_leg.clone(),
            committed: payment_committed,
        },
    ]
}

fn pvp_leg_snapshots(
    plan: SettlementPlan,
    outcome: &SettlementPairOutcome,
    primary_leg: &SettlementLeg,
    counter_leg: &SettlementLeg,
) -> Vec<SettlementLegSnapshot> {
    let (primary_committed, counter_committed) = pvp_committed(plan, outcome);
    vec![
        SettlementLegSnapshot {
            role: SettlementLegRole::Primary,
            leg: primary_leg.clone(),
            committed: primary_committed,
        },
        SettlementLegSnapshot {
            role: SettlementLegRole::Counter,
            leg: counter_leg.clone(),
            committed: counter_committed,
        },
    ]
}

fn ensure_account_exists(stx: &StateTransaction<'_, '_>, account: &AccountId) -> Result<(), Error> {
    stx.world.account(account).map_err(Error::from)?;
    Ok(())
}

fn ensure_leg_accounts(stx: &StateTransaction<'_, '_>, leg: &SettlementLeg) -> Result<(), Error> {
    ensure_account_exists(stx, leg.from())?;
    ensure_account_exists(stx, leg.to())?;
    Ok(())
}

fn resolve_settlement_leg_source_asset_id(
    stx: &StateTransaction<'_, '_>,
    leg: &SettlementLeg,
) -> Result<AssetId, Error> {
    if let Some(scoped_asset_id) = stx.world.assets.iter().find_map(|(asset_id, balance)| {
        (asset_id.definition() == leg.asset_definition_id()
            && asset_id.account() == leg.from()
            && balance
                .as_ref()
                .clone()
                .checked_sub(leg.quantity().clone())
                .is_some_and(|remaining| !remaining.mantissa().is_negative()))
        .then(|| asset_id.clone())
    }) {
        return Ok(scoped_asset_id);
    }

    stx.world.resolve_asset_id_for_current_scope(&AssetId::new(
        leg.asset_definition_id().clone(),
        leg.from().clone(),
    ))
}

fn resolve_settlement_leg_asset_ids(
    stx: &StateTransaction<'_, '_>,
    leg: &SettlementLeg,
) -> Result<(AssetId, AssetId), Error> {
    let withdraw = resolve_settlement_leg_source_asset_id(stx, leg)?;
    let deposit = AssetId::with_scope(
        leg.asset_definition_id().clone(),
        leg.to().clone(),
        withdraw.scope().clone(),
    );
    Ok((withdraw, deposit))
}

fn ensure_leg_funding(stx: &StateTransaction<'_, '_>, leg: &SettlementLeg) -> Result<(), Error> {
    let asset_id = resolve_settlement_leg_source_asset_id(stx, leg)?;
    let available = stx
        .world
        .assets
        .get(&asset_id)
        .map_or_else(Numeric::zero, |balance| balance.as_ref().clone());
    let remaining = available
        .clone()
        .checked_sub(leg.quantity().clone())
        .filter(|residual| !residual.mantissa().is_negative());
    if remaining.is_none() {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "settlement leg requires {} but only {} is available for {}",
                leg.quantity(),
                available,
                leg.from()
            )
            .into(),
        ));
    }
    Ok(())
}

fn ensure_leg_quantity(leg: &SettlementLeg) -> Result<(), Error> {
    if leg.quantity().is_zero() {
        return Err(InstructionExecutionError::InvariantViolation(
            "settlement legs must specify non-zero quantities".into(),
        ));
    }
    Ok(())
}

fn numeric_spec_for_leg(
    stx: &mut StateTransaction<'_, '_>,
    leg: &SettlementLeg,
) -> Result<NumericSpec, Error> {
    stx.numeric_spec_for(leg.asset_definition_id())
        .map_err(Error::from)
}

fn apply_settlement_leg(
    stx: &mut StateTransaction<'_, '_>,
    leg: &SettlementLeg,
    spec: NumericSpec,
) -> Result<(), Error> {
    assert_numeric_spec_with(leg.quantity(), spec)?;
    let (withdraw, deposit) = resolve_settlement_leg_asset_ids(stx, leg)?;
    withdraw_numeric_asset_exact(stx, &withdraw, leg.quantity())?;
    deposit_numeric_asset_exact(stx, &deposit, leg.quantity())?;
    Ok(())
}

fn rollback_settlement_leg(
    stx: &mut StateTransaction<'_, '_>,
    leg: &SettlementLeg,
) -> Result<(), Error> {
    let (source, destination) = resolve_settlement_leg_asset_ids(stx, leg)?;
    withdraw_numeric_asset_exact(stx, &destination, leg.quantity())?;
    deposit_numeric_asset_exact(stx, &source, leg.quantity())?;
    Ok(())
}

fn withdraw_numeric_asset_exact(
    stx: &mut StateTransaction<'_, '_>,
    id: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    let asset = stx
        .world
        .assets
        .get_mut(id)
        .ok_or_else(|| FindError::Asset(id.clone().into()))?;
    let quantity: &mut Numeric = &mut *asset;
    let candidate = quantity
        .clone()
        .checked_sub(amount.clone())
        .ok_or(MathError::NotEnoughQuantity)?;
    if candidate.mantissa().is_negative() {
        return Err(MathError::NotEnoughQuantity.into());
    }
    *quantity = candidate;
    if (**asset).is_zero() {
        assert!(stx.world.remove_asset_and_metadata(id).is_some());
    }
    Ok(())
}

fn deposit_numeric_asset_exact(
    stx: &mut StateTransaction<'_, '_>,
    id: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    let dst = stx.world.asset_or_insert_exact(id, Numeric::zero())?;
    let quantity: &mut Numeric = &mut *dst;
    *quantity = quantity
        .clone()
        .checked_add(amount.clone())
        .ok_or(MathError::Overflow)?;
    Ok(())
}

fn enforce_atomicity(plan: SettlementPlan) {
    match plan.atomicity() {
        SettlementAtomicity::AllOrNothing
        | SettlementAtomicity::CommitFirstLeg
        | SettlementAtomicity::CommitSecondLeg => {}
    }
}

fn log_atomicity_warning(stage: &str, rollback_err: &Error) {
    iroha_logger::warn!(
        error = %rollback_err,
        "failed to rollback {stage} settlement leg after error"
    );
}

#[derive(Clone, Copy, Debug, Default)]
struct SettlementPairOutcome {
    first_committed: bool,
    second_committed: bool,
    #[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
    fx_window_ms: Option<u64>,
}

#[derive(Debug)]
struct SettlementPairError {
    outcome: SettlementPairOutcome,
    error: Box<Error>,
}

impl SettlementPairError {
    fn new(outcome: SettlementPairOutcome, error: Error) -> Self {
        Self {
            outcome,
            error: Box::new(error),
        }
    }

    fn outcome(&self) -> &SettlementPairOutcome {
        &self.outcome
    }

    fn error(&self) -> &Error {
        self.error.as_ref()
    }

    fn into_error(self) -> Error {
        *self.error
    }
}

fn dvp_committed(plan: SettlementPlan, outcome: &SettlementPairOutcome) -> (bool, bool) {
    match plan.order() {
        SettlementExecutionOrder::DeliveryThenPayment => {
            (outcome.first_committed, outcome.second_committed)
        }
        SettlementExecutionOrder::PaymentThenDelivery => {
            (outcome.second_committed, outcome.first_committed)
        }
    }
}

fn pvp_committed(plan: SettlementPlan, outcome: &SettlementPairOutcome) -> (bool, bool) {
    match plan.order() {
        SettlementExecutionOrder::DeliveryThenPayment => {
            (outcome.first_committed, outcome.second_committed)
        }
        SettlementExecutionOrder::PaymentThenDelivery => {
            (outcome.second_committed, outcome.first_committed)
        }
    }
}

fn execute_settlement_pair(
    stx: &mut StateTransaction<'_, '_>,
    first: (&SettlementLeg, NumericSpec),
    second: (&SettlementLeg, NumericSpec),
    plan: SettlementPlan,
) -> Result<SettlementPairOutcome, SettlementPairError> {
    enforce_atomicity(plan);

    let mut outcome = SettlementPairOutcome::default();

    if let Err(err) = apply_settlement_leg(stx, first.0, first.1) {
        return Err(SettlementPairError::new(outcome, err));
    }
    outcome.first_committed = true;

    #[cfg(feature = "telemetry")]
    let first_leg_finished_at = Instant::now();

    if let Err(err) = apply_settlement_leg(stx, second.0, second.1) {
        match plan.atomicity() {
            SettlementAtomicity::AllOrNothing | SettlementAtomicity::CommitSecondLeg => {
                if let Err(rollback_err) = rollback_settlement_leg(stx, first.0) {
                    log_atomicity_warning("first", &rollback_err);
                } else {
                    outcome.first_committed = false;
                }
            }
            SettlementAtomicity::CommitFirstLeg => {
                // nothing to roll back; first leg intentionally committed
            }
        }
        return Err(SettlementPairError::new(outcome, err));
    }
    outcome.second_committed = true;
    #[cfg(feature = "telemetry")]
    {
        let elapsed_ms = first_leg_finished_at.elapsed().as_millis();
        let window_ms = u64::try_from(elapsed_ms).unwrap_or(u64::MAX);
        outcome.fx_window_ms = Some(window_ms);
    }
    Ok(outcome)
}

fn validate_dvp_preconditions(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    delivery_leg: &SettlementLeg,
    payment_leg: &SettlementLeg,
    plan: SettlementPlan,
) -> Result<(NumericSpec, NumericSpec), Error> {
    if delivery_leg.from() != authority {
        return Err(InstructionExecutionError::InvariantViolation(
            "DvP delivery leg must be authorised by the delivering account".into(),
        ));
    }
    if delivery_leg.to() != payment_leg.from() || payment_leg.to() != delivery_leg.from() {
        return Err(InstructionExecutionError::InvariantViolation(
            "DvP counterparties must be reciprocal across delivery and payment legs".into(),
        ));
    }

    ensure_leg_quantity(delivery_leg)?;
    ensure_leg_quantity(payment_leg)?;
    ensure_leg_accounts(stx, delivery_leg)?;
    ensure_leg_accounts(stx, payment_leg)?;
    let delivery_spec = numeric_spec_for_leg(stx, delivery_leg)?;
    let payment_spec = numeric_spec_for_leg(stx, payment_leg)?;
    assert_numeric_spec_with(delivery_leg.quantity(), delivery_spec)?;
    ensure_leg_funding(stx, delivery_leg)?;
    ensure_leg_funding(stx, payment_leg)?;
    enforce_atomicity(plan);

    Ok((delivery_spec, payment_spec))
}

fn validate_pvp_preconditions(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    primary_leg: &SettlementLeg,
    counter_leg: &SettlementLeg,
    plan: SettlementPlan,
) -> Result<(NumericSpec, NumericSpec), Error> {
    if primary_leg.from() != authority {
        return Err(InstructionExecutionError::InvariantViolation(
            "PvP primary leg must be authorised by the initiating account".into(),
        ));
    }
    if primary_leg.to() != counter_leg.from() || counter_leg.to() != primary_leg.from() {
        return Err(InstructionExecutionError::InvariantViolation(
            "PvP counterparties must be reciprocal across primary and counter legs".into(),
        ));
    }

    ensure_leg_quantity(primary_leg)?;
    ensure_leg_quantity(counter_leg)?;
    ensure_leg_accounts(stx, primary_leg)?;
    ensure_leg_accounts(stx, counter_leg)?;
    let primary_spec = numeric_spec_for_leg(stx, primary_leg)?;
    let counter_spec = numeric_spec_for_leg(stx, counter_leg)?;
    assert_numeric_spec_with(primary_leg.quantity(), primary_spec)?;
    assert_numeric_spec_with(counter_leg.quantity(), counter_spec)?;
    ensure_leg_funding(stx, primary_leg)?;
    ensure_leg_funding(stx, counter_leg)?;
    enforce_atomicity(plan);

    Ok((primary_spec, counter_spec))
}

pub(crate) fn admission_validate_dvp(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    instruction: &DvpIsi,
) -> Result<(), Error> {
    let _ = validate_dvp_preconditions(
        authority,
        stx,
        instruction.delivery_leg(),
        instruction.payment_leg(),
        *instruction.plan(),
    )?;
    Ok(())
}

pub(crate) fn admission_validate_pvp(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    instruction: &PvpIsi,
) -> Result<(), Error> {
    let _ = validate_pvp_preconditions(
        authority,
        stx,
        instruction.primary_leg(),
        instruction.counter_leg(),
        *instruction.plan(),
    )?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
impl Execute for DvpIsi {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let DvpIsi {
            settlement_id,
            delivery_leg,
            payment_leg,
            plan,
            metadata,
        } = self;

        let (delivery_spec, payment_spec) =
            match validate_dvp_preconditions(authority, stx, &delivery_leg, &payment_leg, plan) {
                Ok(specs) => specs,
                Err(err) => {
                    let reason = settlement_failure_reason(&err);
                    #[cfg(feature = "telemetry")]
                    {
                        stx.telemetry
                            .note_settlement_failure(SETTLEMENT_KIND_DVP, reason);
                        stx.telemetry.record_dvp_finality(
                            &settlement_id,
                            plan,
                            SettlementOutcomeKind::Failure,
                            Some(reason),
                            false,
                            false,
                        );
                    }
                    let legs = dvp_leg_snapshots(
                        plan,
                        &SettlementPairOutcome::default(),
                        &delivery_leg,
                        &payment_leg,
                    );
                    record_settlement_snapshot(
                        stx,
                        authority,
                        &settlement_id,
                        plan,
                        metadata.clone(),
                        SettlementKind::Dvp,
                        legs,
                        SettlementOutcomeRecord::Failure(SettlementFailureRecord {
                            reason: reason.to_string(),
                        }),
                    );
                    return Err(err);
                }
            };

        let first = match plan.order() {
            SettlementExecutionOrder::DeliveryThenPayment => (&delivery_leg, delivery_spec),
            SettlementExecutionOrder::PaymentThenDelivery => (&payment_leg, payment_spec),
        };
        let second = match plan.order() {
            SettlementExecutionOrder::DeliveryThenPayment => (&payment_leg, payment_spec),
            SettlementExecutionOrder::PaymentThenDelivery => (&delivery_leg, delivery_spec),
        };

        match execute_settlement_pair(stx, first, second, plan) {
            Ok(outcome) => {
                #[cfg(feature = "telemetry")]
                {
                    let (delivery_committed, payment_committed) = dvp_committed(plan, &outcome);
                    stx.telemetry.record_dvp_finality(
                        &settlement_id,
                        plan,
                        SettlementOutcomeKind::Success,
                        None,
                        delivery_committed,
                        payment_committed,
                    );
                    stx.telemetry.note_settlement_success(SETTLEMENT_KIND_DVP);
                }

                iroha_logger::info!(
                    %settlement_id,
                    delivery_from=%delivery_leg.from(),
                    delivery_to=%delivery_leg.to(),
                    payment_asset=%payment_leg.asset_definition_id(),
                    "DvP settlement executed"
                );

                let legs = dvp_leg_snapshots(plan, &outcome, &delivery_leg, &payment_leg);
                record_settlement_snapshot(
                    stx,
                    authority,
                    &settlement_id,
                    plan,
                    metadata.clone(),
                    SettlementKind::Dvp,
                    legs,
                    SettlementOutcomeRecord::Success(SettlementSuccessRecord {
                        first_committed: outcome.first_committed,
                        second_committed: outcome.second_committed,
                        fx_window_ms: outcome.fx_window_ms,
                    }),
                );

                Ok(())
            }
            Err(err) => {
                let reason = settlement_failure_reason(err.error());
                #[cfg(feature = "telemetry")]
                {
                    let (delivery_committed, payment_committed) =
                        dvp_committed(plan, err.outcome());
                    stx.telemetry
                        .note_settlement_failure(SETTLEMENT_KIND_DVP, reason);
                    stx.telemetry.record_dvp_finality(
                        &settlement_id,
                        plan,
                        SettlementOutcomeKind::Failure,
                        Some(reason),
                        delivery_committed,
                        payment_committed,
                    );
                }
                let legs = dvp_leg_snapshots(plan, err.outcome(), &delivery_leg, &payment_leg);
                record_settlement_snapshot(
                    stx,
                    authority,
                    &settlement_id,
                    plan,
                    metadata.clone(),
                    SettlementKind::Dvp,
                    legs,
                    SettlementOutcomeRecord::Failure(SettlementFailureRecord {
                        reason: reason.to_string(),
                    }),
                );
                Err(err.into_error())
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
impl Execute for PvpIsi {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let PvpIsi {
            settlement_id,
            primary_leg,
            counter_leg,
            plan,
            metadata,
        } = self;

        let (primary_spec, counter_spec) =
            match validate_pvp_preconditions(authority, stx, &primary_leg, &counter_leg, plan) {
                Ok(specs) => specs,
                Err(err) => {
                    let reason = settlement_failure_reason(&err);
                    #[cfg(feature = "telemetry")]
                    {
                        stx.telemetry
                            .note_settlement_failure(SETTLEMENT_KIND_PVP, reason);
                        stx.telemetry.record_pvp_finality(
                            &settlement_id,
                            plan,
                            SettlementOutcomeKind::Failure,
                            Some(reason),
                            false,
                            false,
                            None,
                        );
                    }
                    let legs = pvp_leg_snapshots(
                        plan,
                        &SettlementPairOutcome::default(),
                        &primary_leg,
                        &counter_leg,
                    );
                    record_settlement_snapshot(
                        stx,
                        authority,
                        &settlement_id,
                        plan,
                        metadata.clone(),
                        SettlementKind::Pvp,
                        legs,
                        SettlementOutcomeRecord::Failure(SettlementFailureRecord {
                            reason: reason.to_string(),
                        }),
                    );
                    return Err(err);
                }
            };

        let first = match plan.order() {
            SettlementExecutionOrder::DeliveryThenPayment => (&primary_leg, primary_spec),
            SettlementExecutionOrder::PaymentThenDelivery => (&counter_leg, counter_spec),
        };
        let second = match plan.order() {
            SettlementExecutionOrder::DeliveryThenPayment => (&counter_leg, counter_spec),
            SettlementExecutionOrder::PaymentThenDelivery => (&primary_leg, primary_spec),
        };

        match execute_settlement_pair(stx, first, second, plan) {
            Ok(outcome) => {
                #[cfg(feature = "telemetry")]
                {
                    let (primary_committed, counter_committed) = pvp_committed(plan, &outcome);
                    stx.telemetry.record_pvp_finality(
                        &settlement_id,
                        plan,
                        SettlementOutcomeKind::Success,
                        None,
                        primary_committed,
                        counter_committed,
                        outcome.fx_window_ms,
                    );
                    stx.telemetry.note_settlement_success(SETTLEMENT_KIND_PVP);
                }

                iroha_logger::info!(
                    %settlement_id,
                    primary_from=%primary_leg.from(),
                    primary_to=%primary_leg.to(),
                    counter_asset=%counter_leg.asset_definition_id(),
                    "PvP settlement executed"
                );

                let legs = pvp_leg_snapshots(plan, &outcome, &primary_leg, &counter_leg);
                record_settlement_snapshot(
                    stx,
                    authority,
                    &settlement_id,
                    plan,
                    metadata.clone(),
                    SettlementKind::Pvp,
                    legs,
                    SettlementOutcomeRecord::Success(SettlementSuccessRecord {
                        first_committed: outcome.first_committed,
                        second_committed: outcome.second_committed,
                        fx_window_ms: outcome.fx_window_ms,
                    }),
                );

                Ok(())
            }
            Err(err) => {
                let reason = settlement_failure_reason(err.error());
                #[cfg(feature = "telemetry")]
                {
                    let (primary_committed, counter_committed) = pvp_committed(plan, err.outcome());
                    stx.telemetry
                        .note_settlement_failure(SETTLEMENT_KIND_PVP, reason);
                    stx.telemetry.record_pvp_finality(
                        &settlement_id,
                        plan,
                        SettlementOutcomeKind::Failure,
                        Some(reason),
                        primary_committed,
                        counter_committed,
                        err.outcome().fx_window_ms,
                    );
                }
                let legs = pvp_leg_snapshots(plan, err.outcome(), &primary_leg, &counter_leg);
                record_settlement_snapshot(
                    stx,
                    authority,
                    &settlement_id,
                    plan,
                    metadata.clone(),
                    SettlementKind::Pvp,
                    legs,
                    SettlementOutcomeRecord::Failure(SettlementFailureRecord {
                        reason: reason.to_string(),
                    }),
                );
                Err(err.into_error())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        account::Account,
        asset::{
            Asset, AssetDefinition,
            prelude::{AssetDefinitionId, AssetId},
        },
        block::BlockHeader,
        common::Owned,
        domain::{Domain, DomainId},
        isi::error::InstructionEvaluationError,
        metadata::Metadata,
    };
    use iroha_primitives::numeric::{Numeric, NumericSpec};
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{kura::Kura, prelude::World, query::store::LiveQueryStore, state::State};

    #[test]
    fn enforce_atomicity_accepts_commit_variants() {
        for atomicity in [
            SettlementAtomicity::AllOrNothing,
            SettlementAtomicity::CommitFirstLeg,
            SettlementAtomicity::CommitSecondLeg,
        ] {
            let plan =
                SettlementPlan::new(SettlementExecutionOrder::DeliveryThenPayment, atomicity);
            super::enforce_atomicity(plan);
        }
    }

    fn settlement_state() -> (State, AssetDefinitionId, AssetDefinitionId) {
        settlement_state_with_balances(Numeric::from(10u32), Numeric::from(1_000u32))
    }

    fn settlement_state_with_balances(
        delivery_balance: Numeric,
        payment_balance: Numeric,
    ) -> (State, AssetDefinitionId, AssetDefinitionId) {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

        let delivery_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "bond".parse().unwrap(),
        );
        let payment_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "usd".parse().unwrap(),
        );

        let delivery_def = AssetDefinition::numeric(delivery_asset_id.clone()).build(&ALICE_ID);
        let payment_def = AssetDefinition::numeric(payment_asset_id.clone()).build(&ALICE_ID);

        let alice_delivery = Asset::new(
            AssetId::new(delivery_asset_id.clone(), ALICE_ID.clone()),
            delivery_balance,
        );
        let bob_payment = Asset::new(
            AssetId::new(payment_asset_id.clone(), BOB_ID.clone()),
            payment_balance,
        );

        let world = World::with_assets(
            [domain],
            [alice, bob],
            [delivery_def, payment_def],
            [alice_delivery, bob_payment],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        (state, delivery_asset_id, payment_asset_id)
    }

    fn settlement_state_with_payment_spec(
        payment_spec: NumericSpec,
    ) -> (State, AssetDefinitionId, AssetDefinitionId) {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

        let delivery_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "bond".parse().unwrap(),
        );
        let payment_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "usd".parse().unwrap(),
        );

        let delivery_def = AssetDefinition::new(delivery_asset_id.clone(), NumericSpec::integer())
            .build(&ALICE_ID);
        let payment_def =
            AssetDefinition::new(payment_asset_id.clone(), payment_spec).build(&ALICE_ID);

        let alice_delivery = Asset::new(
            AssetId::new(delivery_asset_id.clone(), ALICE_ID.clone()),
            Numeric::from(5u32),
        );
        let bob_payment = Asset::new(
            AssetId::new(payment_asset_id.clone(), BOB_ID.clone()),
            Numeric::from(2u32),
        );

        let world = World::with_assets(
            [domain],
            [alice, bob],
            [delivery_def, payment_def],
            [alice_delivery, bob_payment],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        (state, delivery_asset_id, payment_asset_id)
    }

    #[test]
    fn dvp_moves_assets_between_accounts() {
        let (state, delivery_def_id, payment_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let settlement_id: SettlementId = "dvp_trade".parse().unwrap();

        let delivery_leg = SettlementLeg::new(
            delivery_def_id.clone(),
            Numeric::from(10u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        );
        let payment_leg = SettlementLeg::new(
            payment_def_id.clone(),
            Numeric::from(1_000u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        );

        let plan = SettlementPlan::new(
            SettlementExecutionOrder::PaymentThenDelivery,
            SettlementAtomicity::AllOrNothing,
        );

        DvpIsi {
            settlement_id: settlement_id.clone(),
            delivery_leg,
            payment_leg,
            plan,
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("DvP execution succeeds");

        let alice_bond = AssetId::new(delivery_def_id.clone(), ALICE_ID.clone());
        let bob_bond = AssetId::new(delivery_def_id.clone(), BOB_ID.clone());
        assert!(
            stx.world.assets.get(&alice_bond).is_none(),
            "delivery asset should leave the seller"
        );
        assert_eq!(
            **stx.world.assets.get(&bob_bond).expect("buyer bond balance"),
            Numeric::from(10u32)
        );

        let alice_cash = AssetId::new(payment_def_id.clone(), ALICE_ID.clone());
        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_cash)
                .expect("seller payment balance"),
            Numeric::from(1_000u32)
        );
        assert!(
            stx.world.assets.get(&bob_cash).is_none(),
            "payment asset should be debited from the payer"
        );

        let ledger = stx
            .world
            .settlement_ledgers
            .get(&settlement_id)
            .cloned()
            .expect("settlement ledger entry recorded");
        assert_eq!(ledger.entries.len(), 1, "expected single settlement entry");
        let entry = ledger.entries.last().expect("entry present");
        assert!(entry.outcome.is_success(), "outcome should be success");
        assert_eq!(entry.kind, SettlementKind::Dvp);
        assert_eq!(entry.authority, ALICE_ID.clone());
        assert_eq!(entry.plan, plan);
        assert_eq!(entry.metadata, Metadata::default());
        assert_eq!(entry.block_height, stx._curr_block.height().get());
        assert_eq!(entry.block_hash, stx._curr_block.hash());
        assert_eq!(
            entry
                .legs
                .iter()
                .map(|leg| (leg.role, leg.committed))
                .collect::<Vec<_>>(),
            vec![
                (SettlementLegRole::Delivery, true),
                (SettlementLegRole::Payment, true)
            ]
        );
    }

    #[test]
    fn dvp_persists_balances_after_commit_in_dataspace_context() {
        let (state, delivery_def_id, payment_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut state_block = state.block(header);
        let mut stx = state_block.transaction();
        let dataspace = DataSpaceId::new(7);
        stx.current_dataspace_id = Some(dataspace);
        stx.world.current_dataspace_id = Some(dataspace);

        DvpIsi {
            settlement_id: "dvp_persisted".parse().unwrap(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Numeric::from(10u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Numeric::from(1_000u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::new(
                SettlementExecutionOrder::PaymentThenDelivery,
                SettlementAtomicity::AllOrNothing,
            ),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("DvP execution succeeds");

        stx.apply();
        state_block.commit().expect("commit state block");

        let view = state.view();
        let world = view.world();

        let alice_bond = AssetId::new(delivery_def_id.clone(), ALICE_ID.clone());
        let bob_bond = AssetId::new(delivery_def_id.clone(), BOB_ID.clone());
        let alice_cash = AssetId::new(payment_def_id.clone(), ALICE_ID.clone());
        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());

        assert!(
            world.asset(&alice_bond).is_err(),
            "seller delivery balance should stay debited after commit"
        );
        assert_eq!(
            world
                .asset(&bob_bond)
                .expect("buyer bond balance")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(10u32),
        );
        assert_eq!(
            world
                .asset(&alice_cash)
                .expect("seller cash balance")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(1_000u32),
        );
        assert!(
            world.asset(&bob_cash).is_err(),
            "payer cash balance should stay debited after commit"
        );
    }

    #[test]
    fn dvp_persists_partial_debits_after_commit() {
        let (state, delivery_def_id, payment_def_id) =
            settlement_state_with_balances(Numeric::from(100u32), Numeric::from(200u32));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut state_block = state.block(header);
        let mut stx = state_block.transaction();
        stx.current_dataspace_id = Some(DataSpaceId::new(7));
        stx.world.current_dataspace_id = Some(DataSpaceId::new(7));

        DvpIsi {
            settlement_id: "dvp_partial_commit".parse().unwrap(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Numeric::from(30u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Numeric::from(45u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::AllOrNothing,
            ),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("DvP execution succeeds");

        stx.apply();
        state_block.commit().expect("commit state block");

        let view = state.view();
        let world = view.world();

        let alice_bond = AssetId::new(delivery_def_id.clone(), ALICE_ID.clone());
        let bob_bond = AssetId::new(delivery_def_id.clone(), BOB_ID.clone());
        let alice_cash = AssetId::new(payment_def_id.clone(), ALICE_ID.clone());
        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());

        assert_eq!(
            world
                .asset(&alice_bond)
                .expect("seller delivery balance")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(70u32),
        );
        assert_eq!(
            world
                .asset(&bob_bond)
                .expect("buyer delivery balance")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(30u32),
        );
        assert_eq!(
            world
                .asset(&alice_cash)
                .expect("seller payment balance")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(45u32),
        );
        assert_eq!(
            world
                .asset(&bob_cash)
                .expect("buyer payment balance")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(155u32),
        );
    }

    #[test]
    fn dvp_uses_scoped_source_buckets_for_cross_dataspace_legs() {
        let ds1 = DataSpaceId::new(7);
        let ds2 = DataSpaceId::new(11);
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

        let delivery_def_id = AssetDefinitionId::new(
            domain_id.clone(),
            "bond".parse().expect("delivery asset name"),
        );
        let payment_def_id =
            AssetDefinitionId::new(domain_id, "usd".parse().expect("payment asset name"));

        let delivery_def = {
            let __asset_definition_id = delivery_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .with_balance_scope_policy(iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted)
        .build(&ALICE_ID);
        let payment_def = {
            let __asset_definition_id = payment_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .with_balance_scope_policy(iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted)
        .build(&ALICE_ID);

        let alice_delivery = Asset::new(
            AssetId::with_scope(
                delivery_def_id.clone(),
                ALICE_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(ds1),
            ),
            Numeric::from(10u32),
        );
        let bob_payment = Asset::new(
            AssetId::with_scope(
                payment_def_id.clone(),
                BOB_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(ds2),
            ),
            Numeric::from(1_000u32),
        );

        let world = World::with_assets(
            [domain],
            [alice, bob],
            [delivery_def, payment_def],
            [alice_delivery, bob_payment],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.current_dataspace_id = Some(ds1);
        stx.world.current_dataspace_id = Some(ds1);

        DvpIsi {
            settlement_id: "dvp_cross_scope".parse().unwrap(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Numeric::from(10u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Numeric::from(1_000u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::AllOrNothing,
            ),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("cross-dataspace DvP should resolve each leg against its source bucket");

        let alice_delivery_ds1 = AssetId::with_scope(
            delivery_def_id.clone(),
            ALICE_ID.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(ds1),
        );
        let bob_delivery_ds1 = AssetId::with_scope(
            delivery_def_id,
            BOB_ID.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(ds1),
        );
        let alice_payment_ds2 = AssetId::with_scope(
            payment_def_id.clone(),
            ALICE_ID.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(ds2),
        );
        let bob_payment_ds2 = AssetId::with_scope(
            payment_def_id,
            BOB_ID.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(ds2),
        );

        assert!(
            stx.world.asset(&alice_delivery_ds1).is_err(),
            "delivery source bucket should be debited in its original dataspace"
        );
        assert_eq!(
            stx.world
                .asset(&bob_delivery_ds1)
                .expect("delivery destination bucket")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(10u32),
        );
        assert_eq!(
            stx.world
                .asset(&alice_payment_ds2)
                .expect("payment destination bucket")
                .value()
                .as_ref()
                .clone(),
            Numeric::from(1_000u32),
        );
        assert!(
            stx.world.asset(&bob_payment_ds2).is_err(),
            "payment source bucket should be debited in its original dataspace"
        );
    }

    #[test]
    fn dvp_commit_first_keeps_delivery_on_payment_spec_error() {
        let (state, delivery_def_id, payment_def_id) =
            settlement_state_with_payment_spec(NumericSpec::fractional(2));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let settlement_id: SettlementId = "dvp_commit_first".parse().unwrap();

        let instruction = DvpIsi {
            settlement_id: settlement_id.clone(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Numeric::from(5u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                "1.001".parse::<Numeric>().expect("numeric"),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::CommitFirstLeg,
            ),
            metadata: Metadata::default(),
        };

        let err = instruction
            .execute(&ALICE_ID, &mut stx)
            .expect_err("scale violation should fail payment leg");
        assert!(
            matches!(
                err,
                InstructionExecutionError::Evaluate(InstructionEvaluationError::Type(_))
            ),
            "unexpected error: {err:?}"
        );

        let alice_delivery = AssetId::new(delivery_def_id.clone(), ALICE_ID.clone());
        let bob_delivery = AssetId::new(delivery_def_id.clone(), BOB_ID.clone());
        assert!(
            stx.world.assets.get(&alice_delivery).is_none(),
            "delivery leg should remain debited from seller"
        );
        assert_eq!(
            **stx
                .world
                .assets
                .get(&bob_delivery)
                .expect("buyer delivery balance"),
            Numeric::from(5u32),
            "buyer should retain delivered asset"
        );

        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());
        let alice_cash = AssetId::new(payment_def_id.clone(), ALICE_ID.clone());
        assert_eq!(
            **stx.world.assets.get(&bob_cash).expect("payer cash balance"),
            Numeric::from(2u32),
            "payer cash should be untouched"
        );
        assert!(
            stx.world.assets.get(&alice_cash).is_none(),
            "payment leg must not credit the seller"
        );

        let ledger = stx
            .world
            .settlement_ledgers
            .get(&settlement_id)
            .cloned()
            .expect("ledger entry recorded");
        let entry = ledger.entries.last().expect("latest settlement entry");
        match &entry.outcome {
            SettlementOutcomeRecord::Failure(failure) => assert_eq!(&failure.reason, "type_error"),
            other => panic!("expected failure outcome, found {other:?}"),
        }
        assert_eq!(entry.kind, SettlementKind::Dvp);
        assert_eq!(entry.authority, ALICE_ID.clone());
        assert_eq!(
            entry
                .legs
                .iter()
                .map(|leg| (leg.role, leg.committed))
                .collect::<Vec<_>>(),
            vec![
                (SettlementLegRole::Delivery, true),
                (SettlementLegRole::Payment, false)
            ]
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn dvp_commit_second_rolls_back_on_payment_spec_error() {
        let (state, delivery_def_id, payment_def_id) =
            settlement_state_with_payment_spec(NumericSpec::fractional(2));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let alice_delivery = AssetId::new(delivery_def_id.clone(), ALICE_ID.clone());
        let bob_delivery = AssetId::new(delivery_def_id.clone(), BOB_ID.clone());
        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());

        let initial_alice_delivery = stx
            .world
            .assets
            .get(&alice_delivery)
            .cloned()
            .map(Owned::into_inner)
            .expect("seller delivery balance");
        let initial_bob_delivery = stx
            .world
            .assets
            .get(&bob_delivery)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);
        let initial_bob_cash = stx
            .world
            .assets
            .get(&bob_cash)
            .cloned()
            .map(Owned::into_inner)
            .expect("payer cash balance");

        let settlement_id: SettlementId = "dvp_commit_second".parse().unwrap();

        let instruction = DvpIsi {
            settlement_id: settlement_id.clone(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Numeric::from(5u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                "1.001".parse::<Numeric>().expect("numeric"),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::CommitSecondLeg,
            ),
            metadata: Metadata::default(),
        };

        let err = instruction
            .execute(&ALICE_ID, &mut stx)
            .expect_err("scale violation should fail payment leg");
        assert!(
            matches!(
                err,
                InstructionExecutionError::Evaluate(InstructionEvaluationError::Type(_))
            ),
            "unexpected error: {err:?}"
        );

        let alice_after = stx
            .world
            .assets
            .get(&alice_delivery)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);
        let bob_delivery_after = stx
            .world
            .assets
            .get(&bob_delivery)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);
        let bob_cash_after = stx
            .world
            .assets
            .get(&bob_cash)
            .cloned()
            .map(Owned::into_inner)
            .expect("payer cash balance");

        assert_eq!(
            alice_after, initial_alice_delivery,
            "delivery leg should be rolled back for commit-second"
        );
        assert_eq!(
            bob_delivery_after, initial_bob_delivery,
            "buyer delivery balance should remain unchanged"
        );
        assert_eq!(
            bob_cash_after, initial_bob_cash,
            "payer cash balance should be unaffected"
        );

        let ledger = stx
            .world
            .settlement_ledgers
            .get(&settlement_id)
            .cloned()
            .expect("ledger entry recorded");
        let entry = ledger.entries.last().expect("latest settlement entry");
        match &entry.outcome {
            SettlementOutcomeRecord::Failure(failure) => assert_eq!(&failure.reason, "type_error"),
            other => panic!("expected failure outcome, found {other:?}"),
        }
        assert_eq!(
            entry
                .legs
                .iter()
                .map(|leg| (leg.role, leg.committed))
                .collect::<Vec<_>>(),
            vec![
                (SettlementLegRole::Delivery, false),
                (SettlementLegRole::Payment, false)
            ],
            "rollback must mark both legs uncommitted"
        );
    }

    #[test]
    fn dvp_failure_preserves_balances() {
        let (state, delivery_def_id, payment_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let alice_bond_id = AssetId::new(delivery_def_id.clone(), ALICE_ID.clone());
        let bob_cash_id = AssetId::new(payment_def_id.clone(), BOB_ID.clone());
        let initial_alice_bond = stx
            .world
            .assets
            .get(&alice_bond_id)
            .cloned()
            .map(Owned::into_inner)
            .expect("alice delivery balance");
        let initial_bob_cash = stx
            .world
            .assets
            .get(&bob_cash_id)
            .cloned()
            .map(Owned::into_inner)
            .expect("bob payment balance");

        let settlement_id: SettlementId = "dvp_fail".parse().unwrap();

        let instruction = DvpIsi {
            settlement_id: settlement_id.clone(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Numeric::from(5u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Numeric::from(2_000u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        };

        let err = instruction
            .execute(&ALICE_ID, &mut stx)
            .expect_err("insufficient payment leg must fail");
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvariantViolation(ref message)
                if message.contains("available")
            ),
            "unexpected error: {err:?}"
        );

        let alice_after = stx
            .world
            .assets
            .get(&alice_bond_id)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);
        let bob_cash_after = stx
            .world
            .assets
            .get(&bob_cash_id)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);

        assert_eq!(
            alice_after, initial_alice_bond,
            "seller bond balance changed"
        );
        assert_eq!(
            bob_cash_after, initial_bob_cash,
            "payer cash balance changed"
        );

        let ledger = stx
            .world
            .settlement_ledgers
            .get(&settlement_id)
            .cloned()
            .expect("ledger entry recorded");
        let entry = ledger.entries.last().expect("entry present");
        match &entry.outcome {
            SettlementOutcomeRecord::Failure(failure) => {
                assert_eq!(&failure.reason, "insufficient_funds")
            }
            other => panic!("expected failure outcome, found {other:?}"),
        }
        assert_eq!(entry.kind, SettlementKind::Dvp);
        assert_eq!(entry.authority, ALICE_ID.clone());
        assert_eq!(
            entry
                .legs
                .iter()
                .map(|leg| (leg.role, leg.committed))
                .collect::<Vec<_>>(),
            vec![
                (SettlementLegRole::Delivery, false),
                (SettlementLegRole::Payment, false)
            ]
        );
    }

    #[test]
    fn pvp_swaps_currencies_between_counterparties() {
        let (state, primary_def_id, counter_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let settlement_id: SettlementId = "pvp_fx".parse().unwrap();

        let primary_leg = SettlementLeg::new(
            primary_def_id.clone(),
            Numeric::from(10u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        );
        let counter_leg = SettlementLeg::new(
            counter_def_id.clone(),
            Numeric::from(100u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        );

        PvpIsi {
            settlement_id: settlement_id.clone(),
            primary_leg,
            counter_leg,
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("PvP execution succeeds");

        let alice_primary = AssetId::new(primary_def_id.clone(), ALICE_ID.clone());
        let bob_primary = AssetId::new(primary_def_id.clone(), BOB_ID.clone());
        assert!(
            stx.world.assets.get(&alice_primary).is_none(),
            "primary leg should debit initiating account"
        );
        assert_eq!(
            **stx
                .world
                .assets
                .get(&bob_primary)
                .expect("counterparty primary balance"),
            Numeric::from(10u32)
        );

        let alice_counter = AssetId::new(counter_def_id.clone(), ALICE_ID.clone());
        let bob_counter = AssetId::new(counter_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_counter)
                .expect("initiator counter balance"),
            Numeric::from(100u32)
        );

        let ledger = stx
            .world
            .settlement_ledgers
            .get(&settlement_id)
            .cloned()
            .expect("ledger entry recorded");
        assert_eq!(ledger.entries.len(), 1);
        let entry = ledger.entries.last().expect("entry present");
        assert!(entry.outcome.is_success());
        assert_eq!(entry.kind, SettlementKind::Pvp);
        assert_eq!(entry.authority, ALICE_ID.clone());
        assert_eq!(entry.plan, SettlementPlan::default());
        assert_eq!(entry.block_height, stx._curr_block.height().get());
        assert_eq!(entry.block_hash, stx._curr_block.hash());
        assert_eq!(
            entry
                .legs
                .iter()
                .map(|leg| (leg.role, leg.committed))
                .collect::<Vec<_>>(),
            vec![
                (SettlementLegRole::Primary, true),
                (SettlementLegRole::Counter, true)
            ]
        );
        assert_eq!(
            **stx
                .world
                .assets
                .get(&bob_counter)
                .expect("counterparty residual balance"),
            Numeric::from(900u32)
        );
    }

    #[test]
    fn pvp_failure_preserves_balances() {
        let (state, primary_def_id, counter_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let alice_primary_id = AssetId::new(primary_def_id.clone(), ALICE_ID.clone());
        let bob_counter_id = AssetId::new(counter_def_id.clone(), BOB_ID.clone());
        let alice_primary_before = stx
            .world
            .assets
            .get(&alice_primary_id)
            .cloned()
            .map(Owned::into_inner)
            .expect("initiator primary balance");
        let bob_counter_before = stx
            .world
            .assets
            .get(&bob_counter_id)
            .cloned()
            .map(Owned::into_inner)
            .expect("counterparty counter balance");

        let settlement_id: SettlementId = "pvp_fail".parse().unwrap();

        let instruction = PvpIsi {
            settlement_id: settlement_id.clone(),
            primary_leg: SettlementLeg::new(
                primary_def_id.clone(),
                Numeric::from(500u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            counter_leg: SettlementLeg::new(
                counter_def_id.clone(),
                Numeric::from(5_000u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        };

        let err = instruction
            .execute(&ALICE_ID, &mut stx)
            .expect_err("insufficient counter leg must fail");
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvariantViolation(ref message)
                if message.contains("available")
            ),
            "unexpected error: {err:?}"
        );

        let alice_primary_after = stx
            .world
            .assets
            .get(&alice_primary_id)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);
        let bob_counter_after = stx
            .world
            .assets
            .get(&bob_counter_id)
            .cloned()
            .map_or_else(Numeric::zero, Owned::into_inner);

        assert_eq!(alice_primary_after, alice_primary_before);
        assert_eq!(bob_counter_after, bob_counter_before);

        let ledger = stx
            .world
            .settlement_ledgers
            .get(&settlement_id)
            .cloned()
            .expect("ledger entry recorded");
        let entry = ledger.entries.last().expect("entry present");
        match &entry.outcome {
            SettlementOutcomeRecord::Failure(failure) => {
                assert_eq!(&failure.reason, "insufficient_funds")
            }
            other => panic!("expected failure outcome, found {other:?}"),
        }
        assert_eq!(entry.kind, SettlementKind::Pvp);
        assert_eq!(entry.authority, ALICE_ID.clone());
        assert_eq!(
            entry
                .legs
                .iter()
                .map(|leg| (leg.role, leg.committed))
                .collect::<Vec<_>>(),
            vec![
                (SettlementLegRole::Primary, false),
                (SettlementLegRole::Counter, false)
            ]
        );
    }

    #[test]
    fn admission_validate_dvp_rejects_insufficient_funds() {
        let (state, delivery_def_id, payment_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let instruction = DvpIsi {
            settlement_id: "dvp_insufficient".parse().unwrap(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id,
                Numeric::from(5u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id,
                Numeric::from(2_000u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        };

        let err = super::admission_validate_dvp(&ALICE_ID, &mut stx, &instruction)
            .expect_err("admission guard should reject insufficient payment leg");
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvariantViolation(ref message)
                    if message.contains("available")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn admission_validate_dvp_allows_funded_trade() {
        let (state, delivery_def_id, payment_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let instruction = DvpIsi {
            settlement_id: "dvp_ok".parse().unwrap(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id,
                Numeric::from(5u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id,
                Numeric::from(500u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        };

        super::admission_validate_dvp(&ALICE_ID, &mut stx, &instruction)
            .expect("admission guard should allow funded trades");
    }

    #[test]
    fn admission_validate_pvp_rejects_insufficient_funds() {
        let (state, primary_def_id, counter_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let instruction = PvpIsi {
            settlement_id: "pvp_insufficient".parse().unwrap(),
            primary_leg: SettlementLeg::new(
                primary_def_id,
                Numeric::from(500u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            counter_leg: SettlementLeg::new(
                counter_def_id,
                Numeric::from(5_000u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        };

        let err = super::admission_validate_pvp(&ALICE_ID, &mut stx, &instruction)
            .expect_err("admission guard should reject insufficient counter leg");
        assert!(
            matches!(
                err,
                InstructionExecutionError::InvariantViolation(ref message)
                    if message.contains("available")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn admission_validate_pvp_allows_funded_fx() {
        let (state, primary_def_id, counter_def_id) = settlement_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let instruction = PvpIsi {
            settlement_id: "pvp_ok".parse().unwrap(),
            primary_leg: SettlementLeg::new(
                primary_def_id,
                Numeric::from(5u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            counter_leg: SettlementLeg::new(
                counter_def_id,
                Numeric::from(500u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            plan: SettlementPlan::default(),
            metadata: Metadata::default(),
        };

        super::admission_validate_pvp(&ALICE_ID, &mut stx, &instruction)
            .expect("admission guard should allow funded FX settlements");
    }

    #[test]
    fn enforce_atomicity_accepts_commit_variants_for_pvp() {
        for atomicity in [
            SettlementAtomicity::AllOrNothing,
            SettlementAtomicity::CommitFirstLeg,
            SettlementAtomicity::CommitSecondLeg,
        ] {
            let plan =
                SettlementPlan::new(SettlementExecutionOrder::DeliveryThenPayment, atomicity);
            super::enforce_atomicity(plan);
        }
    }

    #[test]
    fn settlement_failure_reason_classifies_errors() {
        use iroha_data_model::{isi::error::MathError, query::error::FindError};

        let insufficient = InstructionExecutionError::InvariantViolation(
            "settlement leg requires 10 but only 5 is available".into(),
        );
        assert_eq!(
            super::settlement_failure_reason(&insufficient),
            "insufficient_funds"
        );

        let zero_qty = InstructionExecutionError::InvariantViolation(
            "settlement legs must specify non-zero quantities".into(),
        );
        assert_eq!(super::settlement_failure_reason(&zero_qty), "zero_quantity");

        let mismatch = InstructionExecutionError::InvariantViolation(
            "DvP counterparties must be reciprocal across delivery and payment legs".into(),
        );
        assert_eq!(
            super::settlement_failure_reason(&mismatch),
            "counterparty_mismatch"
        );

        let unsupported = InstructionExecutionError::InvariantViolation(
            "settlement atomicity policy `CommitFirstLeg` is not supported yet".into(),
        );
        assert_eq!(
            super::settlement_failure_reason(&unsupported),
            "unsupported_policy"
        );

        let find_missing = InstructionExecutionError::Find(FindError::Account(ALICE_ID.clone()));
        assert_eq!(
            super::settlement_failure_reason(&find_missing),
            "missing_entity"
        );

        let math = InstructionExecutionError::Math(MathError::Overflow);
        assert_eq!(super::settlement_failure_reason(&math), "math_error");
    }
}

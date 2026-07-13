//! Host execution for delivery-versus-payment and payment-versus-payment settlements.

use iroha_data_model::{
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetId},
    events::data::prelude::{ConfigurationEvent, ParameterChanged},
    isi::{
        error::{InstructionEvaluationError, InstructionExecutionError, InvalidParameterError},
        settlement::{
            DvpIsi, FxCorridorPolicy, FxCorridorPolicyRegistry, PvpIsi, SetFxCorridorPolicy,
            SettleFxCorridor, SettlementAtomicity, SettlementExecutionOrder,
            SettlementInstructionBox, SettlementLeg, SettlementPlan,
        },
    },
    prelude::*,
    query::error::FindError,
};
use iroha_executor_data_model::permission::settlement::{
    CanManageFxCorridors, CanSetFxCorridorPolicy, CanSettleFxCorridor,
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec, Quantity},
};

use super::*;
use crate::smartcontracts::isi::asset::isi::{
    assert_numeric_spec_with, execute_native_fx_numeric_asset_pair,
    validate_native_fx_numeric_asset_pair,
};
use crate::smartcontracts::isi::error::MathError;
#[cfg(feature = "telemetry")]
use crate::sumeragi::status::SettlementOutcomeKind;

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub(crate) const SETTLEMENT_KIND_DVP: &str = "dvp";
#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub(crate) const SETTLEMENT_KIND_PVP: &str = "pvp";
pub(crate) const CAN_SET_FX_CORRIDOR_POLICY: &str = "CanSetFxCorridorPolicy";
pub(crate) const CAN_SETTLE_FX_CORRIDOR: &str = "CanSettleFxCorridor";

impl Execute for SettlementInstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            SettlementInstructionBox::Dvp(isi) => isi.execute(authority, stx),
            SettlementInstructionBox::Pvp(isi) => isi.execute(authority, stx),
            SettlementInstructionBox::SetFxCorridorPolicy(isi) => isi.execute(authority, stx),
            SettlementInstructionBox::SettleFxCorridor(isi) => isi.execute(authority, stx),
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
    fx_corridor: Option<FxCorridorSettlementDetails>,
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
        fx_corridor,
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

fn fx_corridor_leg_snapshots(
    outcome: &SettlementPairOutcome,
    source_leg: &SettlementLeg,
    destination_leg: &SettlementLeg,
) -> Vec<SettlementLegSnapshot> {
    vec![
        SettlementLegSnapshot {
            role: SettlementLegRole::FxSource,
            leg: source_leg.clone(),
            committed: outcome.first_committed,
        },
        SettlementLegSnapshot {
            role: SettlementLegRole::FxDestination,
            leg: destination_leg.clone(),
            committed: outcome.second_committed,
        },
    ]
}

fn has_exact_permission(
    stx: &StateTransaction<'_, '_>,
    authority: &AccountId,
    required: &Permission,
) -> bool {
    if stx
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.contains(required))
    {
        return true;
    }

    stx.world
        .account_roles
        .iter()
        .filter_map(|(role_key, ())| {
            if &role_key.account == authority {
                stx.world.roles.get(&role_key.id)
            } else {
                None
            }
        })
        .any(|role| role.permissions().any(|permission| permission == required))
}

fn can_manage_fx_corridors(stx: &StateTransaction<'_, '_>, authority: &AccountId) -> bool {
    let manager: Permission = CanManageFxCorridors.into();
    has_exact_permission(stx, authority, &manager)
}

fn can_set_fx_corridor_policy(
    stx: &StateTransaction<'_, '_>,
    authority: &AccountId,
    policy_id: &Name,
) -> bool {
    let exact: Permission = CanSetFxCorridorPolicy {
        policy_id: policy_id.clone(),
    }
    .into();
    can_manage_fx_corridors(stx, authority) || has_exact_permission(stx, authority, &exact)
}

fn can_settle_fx_corridor(
    stx: &StateTransaction<'_, '_>,
    authority: &AccountId,
    policy_id: &Name,
) -> bool {
    let exact: Permission = CanSettleFxCorridor {
        policy_id: policy_id.clone(),
    }
    .into();
    can_manage_fx_corridors(stx, authority) || has_exact_permission(stx, authority, &exact)
}

fn invalid_fx_parameter(message: impl Into<String>) -> Error {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn fx_policy_registry(stx: &StateTransaction<'_, '_>) -> Result<FxCorridorPolicyRegistry, Error> {
    let parameters = stx.world.parameters.get();
    let Some(custom) = parameters
        .custom()
        .get(&FxCorridorPolicyRegistry::parameter_id())
    else {
        return Ok(FxCorridorPolicyRegistry::default());
    };
    FxCorridorPolicyRegistry::from_custom_parameter(custom)
        .map_err(|err| invalid_fx_parameter(format!("invalid FX corridor policy registry: {err}")))?
        .ok_or_else(|| invalid_fx_parameter("FX corridor policy registry id mismatch"))
}

fn fx_policy(stx: &StateTransaction<'_, '_>, policy_id: &Name) -> Result<FxCorridorPolicy, Error> {
    fx_policy_registry(stx)?
        .get(policy_id)
        .cloned()
        .ok_or_else(|| invalid_fx_parameter(format!("FX corridor policy `{policy_id}` not found")))
}

fn validate_fx_policy_entities(
    stx: &StateTransaction<'_, '_>,
    policy: &FxCorridorPolicy,
) -> Result<(), Error> {
    if let Some(message) = policy.invariant_error() {
        return Err(invalid_fx_parameter(message));
    }

    ensure_account_exists(stx, &policy.source_account)?;
    ensure_account_exists(stx, &policy.source_sink)?;
    ensure_account_exists(stx, &policy.destination_reserve)?;

    for asset_definition_id in [
        &policy.source_asset_definition_id,
        &policy.destination_asset_definition_id,
    ] {
        let definition = stx.world.asset_definition(asset_definition_id)?;
        if definition.balance_scope_policy() != AssetBalancePolicy::DataspaceRestricted {
            return Err(invalid_fx_parameter(format!(
                "FX corridor asset `{asset_definition_id}` must use DataspaceRestricted balances"
            )));
        }
    }

    Ok(())
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
            && balance.as_ref().checked_sub(leg.quantity()).is_ok())
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
        .map_or_else(Quantity::zero, |balance| balance.as_ref().clone());
    if available.checked_sub(leg.quantity()).is_err() {
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
    assert_numeric_spec_with(leg.quantity().as_numeric(), spec)?;
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
    amount: &Quantity,
) -> Result<(), Error> {
    let asset = stx
        .world
        .assets
        .get_mut(id)
        .ok_or_else(|| FindError::Asset(id.clone().into()))?;
    let quantity: &mut Quantity = &mut *asset;
    let candidate = quantity
        .checked_sub(amount)
        .map_err(|_| MathError::NotEnoughQuantity)?;
    *quantity = candidate;
    if (**asset).is_zero() {
        assert!(stx.world.remove_asset_and_metadata(id).is_some());
    }
    Ok(())
}

fn deposit_numeric_asset_exact(
    stx: &mut StateTransaction<'_, '_>,
    id: &AssetId,
    amount: &Quantity,
) -> Result<(), Error> {
    let is_nonzero = {
        let dst = stx.world.asset_or_insert_exact(id, Quantity::zero())?;
        let quantity: &mut Quantity = &mut *dst;
        *quantity = quantity
            .checked_add(amount)
            .map_err(|_| MathError::Overflow)?;
        !quantity.is_zero()
    };
    if is_nonzero {
        stx.world.track_nonzero_asset_holder(id);
    }
    Ok(())
}

fn scoped_fx_leg_asset_ids(leg: &SettlementLeg, dataspace: DataSpaceId) -> (AssetId, AssetId) {
    let scope = AssetBalanceScope::Dataspace(dataspace);
    (
        AssetId::with_scope(
            leg.asset_definition_id().clone(),
            leg.from().clone(),
            scope.clone(),
        ),
        AssetId::with_scope(leg.asset_definition_id().clone(), leg.to().clone(), scope),
    )
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
    Ok(outcome)
}

fn exact_fx_destination_amount(
    source_amount: &Quantity,
    destination_spec: NumericSpec,
    policy: &FxCorridorPolicy,
) -> Result<Quantity, Error> {
    let destination_amount = source_amount
        .try_mul_div_decimal_exact(
            &Numeric::from(policy.rate_numerator),
            &Numeric::from(policy.rate_denominator),
        )
        .map_err(|err| {
            invalid_fx_parameter(format!(
                "FX corridor rate does not produce an exact destination quantity: {err}"
            ))
        })?;
    assert_numeric_spec_with(destination_amount.as_numeric(), destination_spec)?;
    if destination_amount.is_zero() {
        return Err(invalid_fx_parameter(
            "FX corridor destination quantity must be non-zero",
        ));
    }
    Ok(destination_amount)
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
    assert_numeric_spec_with(delivery_leg.quantity().as_numeric(), delivery_spec)?;
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
    assert_numeric_spec_with(primary_leg.quantity().as_numeric(), primary_spec)?;
    assert_numeric_spec_with(counter_leg.quantity().as_numeric(), counter_spec)?;
    ensure_leg_funding(stx, primary_leg)?;
    ensure_leg_funding(stx, counter_leg)?;
    enforce_atomicity(plan);

    Ok((primary_spec, counter_spec))
}

fn validate_fx_settlement_preconditions(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    instruction: &SettleFxCorridor,
) -> Result<(FxCorridorPolicy, SettlementLeg, SettlementLeg), Error> {
    if !can_settle_fx_corridor(stx, authority, &instruction.policy_id) {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "not permitted: exact {CAN_SETTLE_FX_CORRIDOR} for policy `{}` is required",
                instruction.policy_id
            )
            .into(),
        ));
    }
    if stx
        .world
        .settlement_ledgers
        .get(&instruction.settlement_id)
        .is_some()
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "settlement id `{}` has already been committed",
                instruction.settlement_id
            )
            .into(),
        ));
    }

    let policy = fx_policy(stx, &instruction.policy_id)?;
    validate_fx_policy_entities(stx, &policy)?;
    if !policy.enabled {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("FX corridor policy `{}` is disabled", policy.policy_id).into(),
        ));
    }
    if authority != &policy.source_account {
        return Err(InstructionExecutionError::InvariantViolation(
            "FX corridor settlement must be authorised by the policy source account".into(),
        ));
    }
    if instruction.expected_policy_revision != policy.revision {
        return Err(invalid_fx_parameter(format!(
            "FX corridor policy revision mismatch: expected {}, active {}",
            instruction.expected_policy_revision, policy.revision
        )));
    }
    if instruction.source_asset_definition_id != policy.source_asset_definition_id
        || instruction.destination_asset_definition_id != policy.destination_asset_definition_id
    {
        return Err(invalid_fx_parameter(
            "FX corridor instruction assets do not match the active policy",
        ));
    }
    if instruction.recipient == policy.destination_reserve {
        return Err(invalid_fx_parameter(
            "FX corridor recipient must differ from the destination reserve",
        ));
    }
    ensure_account_exists(stx, &instruction.recipient)?;
    if instruction.source_amount.is_zero() {
        return Err(invalid_fx_parameter(
            "FX corridor source quantity must be positive",
        ));
    }

    let source_spec = stx
        .numeric_spec_for(&policy.source_asset_definition_id)
        .map_err(Error::from)?;
    let destination_spec = stx
        .numeric_spec_for(&policy.destination_asset_definition_id)
        .map_err(Error::from)?;
    assert_numeric_spec_with(instruction.source_amount.as_numeric(), source_spec)?;
    let destination_amount =
        exact_fx_destination_amount(&instruction.source_amount, destination_spec, &policy)?;

    let source_leg = SettlementLeg::new(
        policy.source_asset_definition_id.clone(),
        instruction.source_amount.clone(),
        policy.source_account.clone(),
        policy.source_sink.clone(),
    );
    let destination_leg = SettlementLeg::new(
        policy.destination_asset_definition_id.clone(),
        destination_amount,
        policy.destination_reserve.clone(),
        instruction.recipient.clone(),
    );
    let (source_id, source_destination_id) =
        scoped_fx_leg_asset_ids(&source_leg, policy.source_dataspace);
    let (destination_source_id, destination_id) =
        scoped_fx_leg_asset_ids(&destination_leg, policy.destination_dataspace);
    validate_native_fx_numeric_asset_pair(
        stx,
        authority,
        source_id,
        source_destination_id,
        source_leg.quantity().as_numeric().clone(),
        destination_source_id,
        destination_id,
        destination_leg.quantity().as_numeric().clone(),
    )?;

    Ok((policy, source_leg, destination_leg))
}

impl Execute for SetFxCorridorPolicy {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !can_set_fx_corridor_policy(stx, authority, &self.policy.policy_id) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "not permitted: exact {CAN_SET_FX_CORRIDOR_POLICY} for policy `{}` is required",
                    self.policy.policy_id
                )
                .into(),
            ));
        }
        validate_fx_policy_entities(stx, &self.policy)?;

        let mut registry = fx_policy_registry(stx)?;
        let expected_revision = match registry.get(&self.policy.policy_id) {
            Some(previous) => previous
                .revision
                .checked_add(1)
                .ok_or_else(|| invalid_fx_parameter("FX corridor policy revision overflow"))?,
            None => 1,
        };
        if self.policy.revision != expected_revision {
            return Err(invalid_fx_parameter(format!(
                "FX corridor policy revision must be {expected_revision}"
            )));
        }
        registry.upsert(self.policy);
        let next = registry.into_custom_parameter();
        let previous = {
            let parameters = stx.world.parameters.get_mut();
            let previous = parameters.custom().get(next.id()).cloned();
            parameters.set_parameter(Parameter::Custom(next.clone()));
            previous.unwrap_or_else(|| next.clone())
        };
        stx.world
            .emit_events(Some(ConfigurationEvent::Changed(ParameterChanged {
                old_value: Parameter::Custom(previous),
                new_value: Parameter::Custom(next),
            })));
        Ok(())
    }
}

pub(crate) fn admission_validate_fx_corridor(
    authority: &AccountId,
    stx: &mut StateTransaction<'_, '_>,
    instruction: &SettleFxCorridor,
) -> Result<(), Error> {
    let _ = validate_fx_settlement_preconditions(authority, stx, instruction)?;
    Ok(())
}

impl Execute for SettleFxCorridor {
    fn execute(
        self,
        authority: &AccountId,
        stx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let (policy, source_leg, destination_leg) =
            validate_fx_settlement_preconditions(authority, stx, &self)?;

        let (source_id, source_destination_id) =
            scoped_fx_leg_asset_ids(&source_leg, policy.source_dataspace);
        let (destination_source_id, destination_id) =
            scoped_fx_leg_asset_ids(&destination_leg, policy.destination_dataspace);
        execute_native_fx_numeric_asset_pair(
            stx,
            authority,
            source_id,
            source_destination_id,
            source_leg.quantity().as_numeric().clone(),
            destination_source_id,
            destination_id,
            destination_leg.quantity().as_numeric().clone(),
        )?;
        let outcome = SettlementPairOutcome {
            first_committed: true,
            second_committed: true,
            fx_window_ms: None,
        };

        let plan = SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            "fx_corridor_policy_id"
                .parse()
                .expect("valid FX corridor metadata key"),
            Json::new(policy.policy_id.to_string()),
        );
        metadata.insert(
            "fx_source_dataspace"
                .parse()
                .expect("valid FX source-dataspace metadata key"),
            Json::new(policy.source_dataspace.as_u64()),
        );
        metadata.insert(
            "fx_destination_dataspace"
                .parse()
                .expect("valid FX destination-dataspace metadata key"),
            Json::new(policy.destination_dataspace.as_u64()),
        );
        let legs = fx_corridor_leg_snapshots(&outcome, &source_leg, &destination_leg);
        let fx_corridor = FxCorridorSettlementDetails {
            policy_id: policy.policy_id.clone(),
            policy_revision: policy.revision,
            source_dataspace: policy.source_dataspace,
            destination_dataspace: policy.destination_dataspace,
            rate_numerator: policy.rate_numerator,
            rate_denominator: policy.rate_denominator,
            source_account: policy.source_account.clone(),
            source_sink: policy.source_sink.clone(),
            destination_reserve: policy.destination_reserve.clone(),
            recipient: self.recipient.clone(),
            source_asset_definition_id: policy.source_asset_definition_id.clone(),
            destination_asset_definition_id: policy.destination_asset_definition_id.clone(),
            source_amount: source_leg.quantity().clone(),
            destination_amount: destination_leg.quantity().clone(),
        };
        record_settlement_snapshot(
            stx,
            authority,
            &self.settlement_id,
            plan,
            metadata,
            SettlementKind::FxCorridor,
            legs,
            Some(fx_corridor),
            SettlementOutcomeRecord::Success(SettlementSuccessRecord {
                first_committed: outcome.first_committed,
                second_committed: outcome.second_committed,
                fx_window_ms: outcome.fx_window_ms,
            }),
        );

        iroha_logger::info!(
            settlement_id = %self.settlement_id,
            policy_id = %policy.policy_id,
            source_dataspace = policy.source_dataspace.as_u64(),
            destination_dataspace = policy.destination_dataspace.as_u64(),
            source_amount = %source_leg.quantity(),
            destination_amount = %destination_leg.quantity(),
            recipient = %self.recipient,
            "native FX corridor settlement executed"
        );
        Ok(())
    }
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
                        None,
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
                    None,
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
                    None,
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
                        None,
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
                    None,
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
                    None,
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
    use std::collections::BTreeSet;

    use iroha_data_model::{
        account::Account,
        asset::{
            Asset, AssetBalancePolicy, AssetDefinition,
            prelude::{AssetDefinitionId, AssetId},
        },
        block::BlockHeader,
        common::Owned,
        domain::{Domain, DomainId},
        isi::error::InstructionEvaluationError,
        metadata::Metadata,
    };
    use iroha_primitives::numeric::{Numeric, NumericSpec, Quantity};
    use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID, SAMPLE_GENESIS_ACCOUNT_ID};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{kura::Kura, prelude::World, query::store::LiveQueryStore, state::State};

    fn quantity(value: Numeric) -> Quantity {
        Quantity::try_from_numeric(value).expect("settlement fixture quantity must be non-negative")
    }

    fn assert_smart_contract_parameter_contains(error: InstructionExecutionError, expected: &str) {
        let InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            message,
        )) = error
        else {
            panic!("expected smart-contract parameter error, got {error:?}");
        };
        assert!(
            message.contains(expected),
            "expected `{message}` to contain `{expected}`"
        );
    }

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
        settlement_state_with_balances(Quantity::from(10u32), Quantity::from(1_000u32))
    }

    fn fx_corridor_state(
        source_balance: u32,
        destination_balance: u32,
        rate_numerator: u64,
        rate_denominator: u64,
        enabled: bool,
    ) -> (State, FxCorridorPolicy) {
        let domain_id = DomainId::try_new("fx", "universal").expect("FX domain");
        let source_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "aed".parse().expect("AED name"));
        let destination_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "pkr".parse().expect("PKR name"));
        let source_dataspace = DataSpaceId::new(10);
        let destination_dataspace = DataSpaceId::new(12);
        let policy_id: Name = "aed_to_pkr".parse().expect("policy id");

        let mut world = World::with_assets(
            [Domain::new(domain_id).build(&ALICE_ID)],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&ALICE_ID),
                Account::new(CARPENTER_ID.clone()).build(&ALICE_ID),
                Account::new(SAMPLE_GENESIS_ACCOUNT_ID.clone()).build(&ALICE_ID),
            ],
            [
                AssetDefinition::numeric(source_asset_definition_id.clone())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&ALICE_ID),
                AssetDefinition::numeric(destination_asset_definition_id.clone())
                    .with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted)
                    .build(&ALICE_ID),
            ],
            [
                Asset::new(
                    AssetId::with_scope(
                        source_asset_definition_id.clone(),
                        ALICE_ID.clone(),
                        AssetBalanceScope::Dataspace(source_dataspace),
                    ),
                    Quantity::from(source_balance),
                ),
                Asset::new(
                    AssetId::with_scope(
                        destination_asset_definition_id.clone(),
                        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                        AssetBalanceScope::Dataspace(destination_dataspace),
                    ),
                    Quantity::from(destination_balance),
                ),
            ],
            [],
        );
        world.account_permissions.insert(
            ALICE_ID.clone(),
            BTreeSet::from([
                Permission::from(CanSetFxCorridorPolicy {
                    policy_id: policy_id.clone(),
                }),
                Permission::from(CanSettleFxCorridor {
                    policy_id: policy_id.clone(),
                }),
            ]),
        );
        let policy = FxCorridorPolicy {
            policy_id,
            revision: 1,
            source_dataspace,
            source_account: ALICE_ID.clone(),
            source_asset_definition_id,
            source_sink: CARPENTER_ID.clone(),
            destination_dataspace,
            destination_reserve: SAMPLE_GENESIS_ACCOUNT_ID.clone(),
            destination_asset_definition_id,
            rate_numerator,
            rate_denominator,
            enabled,
        };
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        (state, policy)
    }

    fn fx_settlement(policy: &FxCorridorPolicy, id: &str, source_amount: u32) -> SettleFxCorridor {
        SettleFxCorridor {
            policy_id: policy.policy_id.clone(),
            expected_policy_revision: policy.revision,
            source_asset_definition_id: policy.source_asset_definition_id.clone(),
            destination_asset_definition_id: policy.destination_asset_definition_id.clone(),
            settlement_id: id.parse().expect("settlement id"),
            recipient: BOB_ID.clone(),
            source_amount: Quantity::from(source_amount),
        }
    }

    #[test]
    fn fx_corridor_settles_exact_rate_atomically_and_rejects_replay() {
        let (state, policy) = fx_corridor_state(10, 1_000, 76, 1, true);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        SetFxCorridorPolicy {
            policy: policy.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("policy registration succeeds");
        stx.tx_call_hash = Some(iroha_crypto::Hash::prehashed(
            [0xF1; iroha_crypto::Hash::LENGTH],
        ));
        let instruction = fx_settlement(&policy, "fx_001", 10);
        instruction
            .clone()
            .execute(&ALICE_ID, &mut stx)
            .expect("FX settlement succeeds");

        let balance = |id: AssetId| {
            stx.world
                .assets
                .get(&id)
                .map_or_else(Quantity::zero, |value| value.as_ref().clone())
        };
        assert_eq!(
            balance(AssetId::with_scope(
                policy.source_asset_definition_id.clone(),
                CARPENTER_ID.clone(),
                AssetBalanceScope::Dataspace(policy.source_dataspace),
            )),
            Quantity::from(10_u32)
        );
        assert_eq!(
            balance(AssetId::with_scope(
                policy.destination_asset_definition_id.clone(),
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                AssetBalanceScope::Dataspace(policy.destination_dataspace),
            )),
            Quantity::from(240_u32)
        );
        assert_eq!(
            balance(AssetId::with_scope(
                policy.destination_asset_definition_id.clone(),
                BOB_ID.clone(),
                AssetBalanceScope::Dataspace(policy.destination_dataspace),
            )),
            Quantity::from(760_u32)
        );
        let ledger = stx
            .world
            .settlement_ledgers
            .get(&instruction.settlement_id)
            .expect("FX outcome recorded");
        assert_eq!(ledger.entries.len(), 1);
        let receipt = &ledger.entries[0];
        assert_eq!(receipt.kind, SettlementKind::FxCorridor);
        assert!(receipt.outcome.is_success());
        assert!(receipt.legs.iter().all(|leg| leg.committed));
        let details = receipt
            .fx_corridor
            .as_ref()
            .expect("native FX receipt must retain exact policy and amount evidence");
        assert_eq!(details.policy_id, policy.policy_id);
        assert_eq!(details.policy_revision, policy.revision);
        assert_eq!(details.source_dataspace, policy.source_dataspace);
        assert_eq!(details.destination_dataspace, policy.destination_dataspace);
        assert_eq!(details.source_amount, Quantity::from(10_u32));
        assert_eq!(details.destination_amount, Quantity::from(760_u32));
        assert_eq!(details.recipient, BOB_ID.clone());

        let replay = instruction
            .execute(&ALICE_ID, &mut stx)
            .expect_err("settlement id replay must fail");
        assert!(replay.to_string().contains("already been committed"));
    }

    #[test]
    fn fx_corridor_rejects_policy_and_signed_intent_mismatches() {
        let (state, policy) = fx_corridor_state(10, 1_000, 76, 1, true);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        SetFxCorridorPolicy {
            policy: policy.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("policy registration succeeds");

        let mut wrong_revision = fx_settlement(&policy, "fx_wrong_revision", 1);
        wrong_revision.expected_policy_revision = 2;
        assert_smart_contract_parameter_contains(
            wrong_revision
                .execute(&ALICE_ID, &mut stx)
                .expect_err("revision mismatch must fail"),
            "revision mismatch",
        );

        let mut wrong_asset = fx_settlement(&policy, "fx_wrong_asset", 1);
        wrong_asset.source_asset_definition_id = policy.destination_asset_definition_id.clone();
        assert_smart_contract_parameter_contains(
            wrong_asset
                .execute(&ALICE_ID, &mut stx)
                .expect_err("asset mismatch must fail"),
            "do not match",
        );

        let mut reserve_recipient = fx_settlement(&policy, "fx_reserve_recipient", 1);
        reserve_recipient.recipient = policy.destination_reserve.clone();
        assert_smart_contract_parameter_contains(
            reserve_recipient
                .execute(&ALICE_ID, &mut stx)
                .expect_err("reserve recipient must fail"),
            "recipient",
        );

        stx.world.account_permissions.insert(
            BOB_ID.clone(),
            BTreeSet::from([Permission::from(CanSettleFxCorridor {
                policy_id: policy.policy_id.clone(),
            })]),
        );
        assert!(
            fx_settlement(&policy, "fx_wrong_authority", 1)
                .execute(&BOB_ID, &mut stx)
                .expect_err("misgranted permission must not bypass source ownership")
                .to_string()
                .contains("policy source account")
        );

        stx.world.account_permissions.insert(
            CARPENTER_ID.clone(),
            BTreeSet::from([Permission::from(CanSettleFxCorridor {
                policy_id: "another_corridor".parse().expect("other policy id"),
            })]),
        );
        assert!(
            fx_settlement(&policy, "fx_wrong_permission_scope", 1)
                .execute(&CARPENTER_ID, &mut stx)
                .expect_err("a permission for another corridor must fail closed")
                .to_string()
                .contains("exact CanSettleFxCorridor")
        );

        let mut revision_two = policy.clone();
        revision_two.revision = 3;
        assert_smart_contract_parameter_contains(
            SetFxCorridorPolicy {
                policy: revision_two,
            }
            .execute(&ALICE_ID, &mut stx)
            .expect_err("policy revision must be monotonic"),
            "must be 2",
        );

        let mut disabled = policy.clone();
        disabled.revision = 2;
        disabled.enabled = false;
        SetFxCorridorPolicy {
            policy: disabled.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("disabled policy revision is a valid governance update");
        assert!(
            fx_settlement(&disabled, "fx_disabled", 1)
                .execute(&ALICE_ID, &mut stx)
                .expect_err("disabled policy must reject settlement")
                .to_string()
                .contains("disabled")
        );
    }

    #[test]
    fn fx_corridor_policy_static_invariants_fail_closed() {
        let (_, policy) = fx_corridor_state(1, 76, 76, 1, true);
        let mut cases = Vec::new();
        let mut zero_revision = policy.clone();
        zero_revision.revision = 0;
        cases.push(zero_revision);
        let mut universal_source = policy.clone();
        universal_source.source_dataspace = DataSpaceId::UNIVERSAL;
        cases.push(universal_source);
        let mut same_dataspace = policy.clone();
        same_dataspace.destination_dataspace = same_dataspace.source_dataspace;
        cases.push(same_dataspace);
        let mut same_account = policy.clone();
        same_account.source_sink = same_account.source_account.clone();
        cases.push(same_account);
        let mut same_asset = policy.clone();
        same_asset.destination_asset_definition_id = same_asset.source_asset_definition_id.clone();
        cases.push(same_asset);
        let mut zero_rate = policy;
        zero_rate.rate_denominator = 0;
        cases.push(zero_rate);

        assert!(
            cases
                .iter()
                .all(|candidate| candidate.invariant_error().is_some())
        );
    }

    #[test]
    fn fx_corridor_preflight_preserves_source_on_non_exact_or_unfunded_payout() {
        let (state, policy) = fx_corridor_state(10, 1, 1, 3, true);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        SetFxCorridorPolicy {
            policy: policy.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("policy registration succeeds");
        assert_smart_contract_parameter_contains(
            fx_settlement(&policy, "fx_fractional", 1)
                .execute(&ALICE_ID, &mut stx)
                .expect_err("non-exact integer payout must fail"),
            "exact destination",
        );

        let source_id = AssetId::with_scope(
            policy.source_asset_definition_id.clone(),
            ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(policy.source_dataspace),
        );
        assert_eq!(
            **stx.world.assets.get(&source_id).expect("source unchanged"),
            Quantity::from(10_u32)
        );

        let mut policy_two = policy.clone();
        policy_two.revision = 2;
        policy_two.rate_numerator = 2;
        policy_two.rate_denominator = 1;
        SetFxCorridorPolicy {
            policy: policy_two.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("monotonic policy update succeeds");
        let error = fx_settlement(&policy_two, "fx_unfunded", 1)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("reserve preflight must fail");
        assert!(matches!(
            error,
            InstructionExecutionError::Math(MathError::NotEnoughQuantity)
        ));
        assert_eq!(
            **stx.world.assets.get(&source_id).expect("source unchanged"),
            Quantity::from(10_u32)
        );
    }

    #[test]
    fn fx_corridor_rejects_wrong_dataspace_and_frozen_reserve_without_partial_effects() {
        let (state, policy) = fx_corridor_state(10, 1_000, 76, 1, true);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let mut wrong_scope = policy.clone();
        wrong_scope.source_dataspace = DataSpaceId::new(11);
        SetFxCorridorPolicy {
            policy: wrong_scope.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("a well-formed policy may be published before its reserve is funded");
        let wrong_scope_instruction = fx_settlement(&wrong_scope, "fx_wrong_scope", 1);
        wrong_scope_instruction
            .clone()
            .execute(&ALICE_ID, &mut stx)
            .expect_err("funds in another dataspace must not satisfy the signed policy scope");
        let actual_source_id = AssetId::with_scope(
            policy.source_asset_definition_id.clone(),
            ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(policy.source_dataspace),
        );
        assert_eq!(
            **stx
                .world
                .assets
                .get(&actual_source_id)
                .expect("actual source remains funded"),
            Quantity::from(10_u32),
        );
        assert!(
            stx.world
                .settlement_ledgers
                .get(&wrong_scope_instruction.settlement_id)
                .is_none(),
            "a rejected cross-dataspace attempt must not emit a success receipt",
        );

        let (state, policy) = fx_corridor_state(10, 1_000, 76, 1, true);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        SetFxCorridorPolicy {
            policy: policy.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("policy registration succeeds");
        SetAssetTransferFreeze::new(
            policy.destination_reserve.clone(),
            policy.destination_asset_definition_id.clone(),
            true,
            Some("reserve safety hold".to_owned()),
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("asset owner may freeze the destination reserve");
        let frozen_instruction = fx_settlement(&policy, "fx_frozen_reserve", 1);
        frozen_instruction
            .clone()
            .execute(&ALICE_ID, &mut stx)
            .expect_err("ordinary transfer controls must gate the native payout leg");

        let source_id = AssetId::with_scope(
            policy.source_asset_definition_id.clone(),
            ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(policy.source_dataspace),
        );
        let reserve_id = AssetId::with_scope(
            policy.destination_asset_definition_id.clone(),
            policy.destination_reserve.clone(),
            AssetBalanceScope::Dataspace(policy.destination_dataspace),
        );
        let recipient_id = AssetId::with_scope(
            policy.destination_asset_definition_id.clone(),
            BOB_ID.clone(),
            AssetBalanceScope::Dataspace(policy.destination_dataspace),
        );
        assert_eq!(
            **stx.world.assets.get(&source_id).expect("source unchanged"),
            Quantity::from(10_u32),
        );
        assert_eq!(
            **stx
                .world
                .assets
                .get(&reserve_id)
                .expect("reserve unchanged"),
            Quantity::from(1_000_u32),
        );
        assert!(stx.world.assets.get(&recipient_id).is_none());
        assert!(
            stx.world
                .settlement_ledgers
                .get(&frozen_instruction.settlement_id)
                .is_none(),
            "a preflight rejection must not record a committed settlement receipt",
        );
    }

    fn settlement_state_with_balances(
        delivery_balance: Quantity,
        payment_balance: Quantity,
    ) -> (State, AssetDefinitionId, AssetDefinitionId) {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

        let delivery_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "bond".parse().unwrap(),
        );
        let payment_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
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
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

        let delivery_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "bond".parse().unwrap(),
        );
        let payment_asset_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "usd".parse().unwrap(),
        );

        let delivery_def = AssetDefinition::new(delivery_asset_id.clone(), NumericSpec::integer())
            .build(&ALICE_ID);
        let payment_def =
            AssetDefinition::new(payment_asset_id.clone(), payment_spec).build(&ALICE_ID);

        let alice_delivery = Asset::new(
            AssetId::new(delivery_asset_id.clone(), ALICE_ID.clone()),
            Quantity::from(5_u32),
        );
        let bob_payment = Asset::new(
            AssetId::new(payment_asset_id.clone(), BOB_ID.clone()),
            Quantity::from(2_u32),
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
            Quantity::from(10_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        );
        let payment_leg = SettlementLeg::new(
            payment_def_id.clone(),
            Quantity::from(1_000_u32),
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
            Quantity::from(10_u32)
        );

        let alice_cash = AssetId::new(payment_def_id.clone(), ALICE_ID.clone());
        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_cash)
                .expect("seller payment balance"),
            Quantity::from(1_000_u32)
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
                Quantity::from(10_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Quantity::from(1_000_u32),
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
            Quantity::from(10_u32),
        );
        assert_eq!(
            world
                .asset(&alice_cash)
                .expect("seller cash balance")
                .value()
                .as_ref()
                .clone(),
            Quantity::from(1_000_u32),
        );
        assert!(
            world.asset(&bob_cash).is_err(),
            "payer cash balance should stay debited after commit"
        );
    }

    #[test]
    fn dvp_persists_partial_debits_after_commit() {
        let (state, delivery_def_id, payment_def_id) =
            settlement_state_with_balances(Quantity::from(100u32), Quantity::from(200u32));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut state_block = state.block(header);
        let mut stx = state_block.transaction();
        stx.current_dataspace_id = Some(DataSpaceId::new(7));
        stx.world.current_dataspace_id = Some(DataSpaceId::new(7));

        DvpIsi {
            settlement_id: "dvp_partial_commit".parse().unwrap(),
            delivery_leg: SettlementLeg::new(
                delivery_def_id.clone(),
                Quantity::from(30_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Quantity::from(45_u32),
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
            Quantity::from(70_u32),
        );
        assert_eq!(
            world
                .asset(&bob_bond)
                .expect("buyer delivery balance")
                .value()
                .as_ref()
                .clone(),
            Quantity::from(30_u32),
        );
        assert_eq!(
            world
                .asset(&alice_cash)
                .expect("seller payment balance")
                .value()
                .as_ref()
                .clone(),
            Quantity::from(45_u32),
        );
        assert_eq!(
            world
                .asset(&bob_cash)
                .expect("buyer payment balance")
                .value()
                .as_ref()
                .clone(),
            Quantity::from(155_u32),
        );
    }

    #[test]
    fn dvp_uses_scoped_source_buckets_for_cross_dataspace_legs() {
        let ds1 = DataSpaceId::new(7);
        let ds2 = DataSpaceId::new(11);
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
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
            Quantity::from(10_u32),
        );
        let bob_payment = Asset::new(
            AssetId::with_scope(
                payment_def_id.clone(),
                BOB_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(ds2),
            ),
            Quantity::from(1_000_u32),
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
                Quantity::from(10_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Quantity::from(1_000_u32),
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
            Quantity::from(10_u32),
        );
        assert_eq!(
            stx.world
                .asset(&alice_payment_ds2)
                .expect("payment destination bucket")
                .value()
                .as_ref()
                .clone(),
            Quantity::from(1_000_u32),
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
                Quantity::from(5_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                quantity("1.001".parse::<Numeric>().expect("numeric")),
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
            Quantity::from(5_u32),
            "buyer should retain delivered asset"
        );

        let bob_cash = AssetId::new(payment_def_id.clone(), BOB_ID.clone());
        let alice_cash = AssetId::new(payment_def_id.clone(), ALICE_ID.clone());
        assert_eq!(
            **stx.world.assets.get(&bob_cash).expect("payer cash balance"),
            Quantity::from(2_u32),
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
            .map_or_else(Quantity::zero, Owned::into_inner);
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
                Quantity::from(5_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                quantity("1.001".parse::<Numeric>().expect("numeric")),
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
            .map_or_else(Quantity::zero, Owned::into_inner);
        let bob_delivery_after = stx
            .world
            .assets
            .get(&bob_delivery)
            .cloned()
            .map_or_else(Quantity::zero, Owned::into_inner);
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
                Quantity::from(5_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id.clone(),
                Quantity::from(2_000_u32),
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
            .map_or_else(Quantity::zero, Owned::into_inner);
        let bob_cash_after = stx
            .world
            .assets
            .get(&bob_cash_id)
            .cloned()
            .map_or_else(Quantity::zero, Owned::into_inner);

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
            Quantity::from(10_u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        );
        let counter_leg = SettlementLeg::new(
            counter_def_id.clone(),
            Quantity::from(100_u32),
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
            Quantity::from(10_u32)
        );

        let alice_counter = AssetId::new(counter_def_id.clone(), ALICE_ID.clone());
        let bob_counter = AssetId::new(counter_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_counter)
                .expect("initiator counter balance"),
            Quantity::from(100_u32)
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
            Quantity::from(900_u32)
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
                Quantity::from(500_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            counter_leg: SettlementLeg::new(
                counter_def_id.clone(),
                Quantity::from(5_000_u32),
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
            .map_or_else(Quantity::zero, Owned::into_inner);
        let bob_counter_after = stx
            .world
            .assets
            .get(&bob_counter_id)
            .cloned()
            .map_or_else(Quantity::zero, Owned::into_inner);

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
                Quantity::from(5_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id,
                Quantity::from(2_000_u32),
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
                Quantity::from(5_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            payment_leg: SettlementLeg::new(
                payment_def_id,
                Quantity::from(500_u32),
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
                Quantity::from(500_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            counter_leg: SettlementLeg::new(
                counter_def_id,
                Quantity::from(5_000_u32),
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
                Quantity::from(5_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            counter_leg: SettlementLeg::new(
                counter_def_id,
                Quantity::from(500_u32),
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

    #[test]
    fn settlement_leg_quantity_boundaries_reject_negative_and_zero_values() {
        let definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "usd".parse().expect("asset name"),
        );
        assert!(Quantity::try_from_numeric(Numeric::new(-1, 0)).is_err());

        let leg = SettlementLeg::new(
            definition_id,
            Quantity::zero(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        );

        assert!(matches!(
            super::ensure_leg_quantity(&leg),
            Err(InstructionExecutionError::InvariantViolation(_))
        ));
    }
}

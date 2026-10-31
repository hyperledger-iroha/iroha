//! Native UID state in the existing consensus state map.
//!
//! The existing `World` map is reused, but the persisted UID policy and usage
//! payload layouts change with purpose-aware counters. Retained-history replay
//! and snapshot restoration require qualification against the current layout;
//! no retired-layout decoder is provided. Policy and membership are owner-written;
//! generic contract state access cannot write these roots.
use super::*;
use iroha_data_model::{
    asset::{
        AssetBalanceScope,
        uid::{AssetUidBindingV1, AssetUidPolicyV1, AssetUidUsageV1},
    },
    isi::error::{AssetTransferAdmissionError, InstructionExecutionError, MathError},
};
use iroha_primitives::numeric::Quantity;

#[path = "asset_uid/wallet_holdings.rs"]
pub mod wallet_holdings;

#[path = "asset_uid/binding_custody.rs"]
pub mod binding_custody;

pub const POLICY_ROOT: &str = "asset_uid_policy_v1/";
pub const BINDING_ROOT: &str = "asset_uid_binding_v1/";
pub const USAGE_ROOT: &str = "asset_uid_usage_v1/";
type Error = InstructionExecutionError;

#[cfg(test)]
mod sccp_decode_tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};
    use iroha_data_model::asset::uid::AssetUidDailyWindowV1;

    fn check_original_decode_owner<T>(value: &T)
    where
        T: norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let state = blank_state();
        let mut block = state.block(header(2));
        let tx = block.transaction();
        let bytes = encode(value).unwrap();
        assert!(decode_with_owner::<T>(Some(&tx.world), &bytes).is_ok());
        assert!(decode_with_owner::<T>(Some(&tx.world), &bytes[..bytes.len() / 2]).is_err());
        assert!(
            !tx.is_execution_deferred(),
            "malformed policy is not a local failure"
        );
        let result =
            norito::with_decode_limits_scope(norito::DecodeLimits::new(0, 0, 0, 0, 0), || {
                decode_with_owner::<T>(Some(&tx.world), &bytes)
            });
        assert!(result.is_err());
        assert!(tx.is_execution_deferred());
        assert!(
            tx.borrow_execution_deferral()
                .unwrap()
                .decode_refusal()
                .is_some()
        );
    }

    #[test]
    fn binding_roster_and_usage_keep_decoder_refusals_separate_from_malformed_state() {
        let account = authority(1);
        let definition = iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id();
        check_original_decode_owner(&AssetUidBindingV1 {
            asset_definition_id: definition.clone(),
            account_id: account.clone(),
            uid: [7; 32],
        });
        check_original_decode_owner(&AssetUidPolicyV1 {
            asset_definition_id: definition.clone(),
            uid: [7; 32],
            revision: 1,
            accounts: BTreeSet::from([account]),
            holding_limit: Some(10_u32.into()),
            daily_window: AssetUidDailyWindowV1::UtcDay,
            daily_spend_limit: Some(5_u32.into()),
            daily_outgoing_limit: Some(7_u32.into()),
        });
        check_original_decode_owner(&AssetUidUsageV1 {
            asset_definition_id: definition,
            uid: [7; 32],
            daily_window: AssetUidDailyWindowV1::UtcDay,
            day_start_ms: 0,
            spend_amount: 2_u32.into(),
            outgoing_amount: 3_u32.into(),
        });
    }

    fn policy_control_fixture() -> AssetUidPolicyV1 {
        AssetUidPolicyV1 {
            asset_definition_id: iroha_data_model::sccp::escrow::sccp_taira_xor_asset_definition_id(
            ),
            uid: [7; 32],
            revision: 1,
            accounts: BTreeSet::from([authority(1)]),
            holding_limit: None,
            daily_window: AssetUidDailyWindowV1::UtcDay,
            daily_spend_limit: None,
            daily_outgoing_limit: None,
        }
    }

    #[test]
    fn sccp_policy_controls_wait_only_for_original_pool_and_retry_after_release() {
        use std::{
            future::Future,
            pin::Pin,
            task::{Context, Poll, Waker},
        };
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let state = blank_state();
        let mut block = state.block(header(2));
        let tx = block.transaction();
        let bytes = encode(&policy_control_fixture()).unwrap();
        let budget = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let mut registration = crate::unit_test_support::release_registration(&budget);
        // This test retains the World writer while observing returned capacity.
        // Native thread waits can observe credits before arbitrary task callbacks
        // are released with that writer's physical guards.
        registration.bind_thread(std::thread::current());
        let baseline = budget.reserved_bytes();
        let held = budget
            .try_reserve_bytes(budget.limit_bytes() - baseline)
            .unwrap();
        let foreign = iroha_allocation::AllocationBudget::new(budget.limit_bytes());
        let foreign_held = foreign.try_reserve_bytes(1).unwrap();

        // Header rejection has precedence over control admission, even with no pool capacity.
        assert!(decode_with_owner::<AssetUidPolicyV1>(Some(&tx.world), &bytes[..4]).is_err());
        assert!(!tx.is_execution_deferred());
        assert!(
            decode_with_owner::<AssetUidPolicyV1>(Some(&tx.world), &bytes).is_err(),
            "HC974 policy decoder controls must refuse on the original exhausted pool"
        );
        let reason = tx.borrow_execution_deferral().unwrap();
        let Some(iroha_allocation::AllocationRefusal::Capacity {
            requested_bytes,
            release,
            ..
        }) = reason.allocation_refusal()
        else {
            panic!("control admission must retain its original capacity refusal");
        };
        assert_eq!(
            *requested_bytes,
            norito::PreparedCanonicalDecode::allocation_layouts()
                .iter()
                .map(std::alloc::Layout::size)
                .sum::<usize>()
        );
        let mut wait = release.clone().wait_for_release(&mut registration);
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
        drop(foreign_held);
        assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
        drop(held);
        assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
        drop(wait);
        drop(reason);
        assert_eq!(budget.reserved_bytes(), baseline);
        drop(tx);
        let tx = block.transaction();
        let retry_baseline = budget.reserved_bytes();
        assert_eq!(
            decode_with_owner::<AssetUidPolicyV1>(Some(&tx.world), &bytes).unwrap(),
            policy_control_fixture()
        );
        assert_eq!(budget.reserved_bytes(), retry_baseline);
        assert!(!tx.is_execution_deferred());
    }

    #[test]
    fn sccp_policy_first_and_second_control_allocator_refusals_refund_original_pool() {
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let bytes = encode(&policy_control_fixture()).unwrap();
        let layout = norito::PreparedCanonicalDecode::allocation_layouts()[0];
        for occurrence in 0..2 {
            let state = blank_state();
            let mut block = state.block(header(2));
            let tx = block.transaction();
            let budget = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let baseline = budget.reserved_bytes();
            let (result, refused) = crate::test_allocations::refuse_layout_occurrence_during(
                layout,
                occurrence,
                || decode_with_owner::<AssetUidPolicyV1>(Some(&tx.world), &bytes),
            );
            assert!(
                refused,
                "selected canonical control must reach the native allocator"
            );
            assert!(result.is_err());
            let reason = tx.borrow_execution_deferral().unwrap();
            assert!(reason.allocator_refusal().is_some());
            assert!(
                reason.allocation_refusal().is_none(),
                "physical failure has no invented pool wait"
            );
            assert!(reason.decode_refusal().is_none());
            assert_eq!(
                budget.reserved_bytes(),
                baseline,
                "partial controls must refund after destruction"
            );
        }
    }
}

fn reject(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into())
}

fn digest(value: impl std::fmt::Display) -> String {
    hex::encode(Hash::new(value.to_string().as_bytes()).as_ref())
}

fn path(value: &str) -> StatePath {
    value.parse().expect("fixed native asset UID path is valid")
}

pub fn policy_key(definition: &AssetDefinitionId, uid: &[u8; 32]) -> StatePath {
    super::asset_context_keys::uid_policy_native(definition, uid)
}
pub fn binding_key(definition: &AssetDefinitionId, account: &AccountId) -> StatePath {
    super::asset_context_keys::uid_binding_native(definition, &account.to_string())
}

pub fn usage_key(definition: &AssetDefinitionId, uid: &[u8; 32], day: u64) -> StatePath {
    path(
        &(format!(
            "{USAGE_ROOT}{}/{}/{day}",
            digest(definition),
            hex::encode(uid)
        )),
    )
}

fn decode<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    decode_with_owner(None, bytes)
}

fn decode_with_owner<T>(owner: Option<&WorldTransaction<'_, '_>>, bytes: &[u8]) -> Result<T, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    crate::execution_attempt::canonical_policy_decode(
        owner,
        bytes,
        norito::canonical_decode_limits(bytes.len()),
    )
    .map_err(|error| {
        let rejected = |error: norito::CanonicalDecodeControlError| {
            reject(format!("invalid asset UID state: {error}"))
        };
        match owner {
            Some(owner) => owner.attempt_error_to_instruction_error(
                crate::execution_attempt::canonical_decode_control_attempt_error(error, rejected),
            ),
            None => rejected(error),
        }
    })
}

pub fn encode<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    norito::encode_canonical(value)
        .map_err(|e| reject(format!("cannot encode asset UID state: {e}")))
}

pub fn policy(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    uid: &[u8; 32],
) -> Result<Option<AssetUidPolicyV1>, Error> {
    policy_with_owner(world, definition, uid, None)
}

fn policy_with_owner(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    uid: &[u8; 32],
    owner: Option<&WorldTransaction<'_, '_>>,
) -> Result<Option<AssetUidPolicyV1>, Error> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&policy_key(definition, uid))
    else {
        return Ok(None);
    };
    let value: AssetUidPolicyV1 = decode_with_owner(owner, bytes)?;
    value.validate_shape().map_err(reject)?;
    if &value.asset_definition_id != definition || &value.uid != uid {
        return Err(reject("asset UID policy differs from its canonical key"));
    }
    Ok(Some(value))
}

pub fn binding(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    account: &AccountId,
) -> Result<Option<AssetUidBindingV1>, Error> {
    let Some(bytes) = world
        .smart_contract_state()
        .get(&binding_key(definition, account))
    else {
        return Ok(None);
    };
    let value: AssetUidBindingV1 = decode(bytes)?;
    validate_binding(&value, definition, account)?;
    Ok(Some(value))
}

fn validate_binding(
    value: &AssetUidBindingV1,
    definition: &AssetDefinitionId,
    account: &AccountId,
) -> Result<[u8; 32], Error> {
    if &value.asset_definition_id != definition
        || &value.account_id != account
        || value.uid == [0; 32]
    {
        return Err(reject(
            "asset UID membership differs from its canonical key",
        ));
    }
    Ok(value.uid)
}

pub fn member_policy(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    account: &AccountId,
) -> Result<Option<AssetUidPolicyV1>, Error> {
    member_policy_with_owner(world, definition, account, None)
}

/// Retain the transaction's decode owner across both membership and policy reads.
pub(super) fn member_policy_with_owner(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    account: &AccountId,
    owner: Option<&WorldTransaction<'_, '_>>,
) -> Result<Option<AssetUidPolicyV1>, Error> {
    let Some(uid) = binding_custody::binding_uid_with_owner(world, definition, account, owner)?
    else {
        return Ok(None);
    };
    let value = policy_with_owner(world, definition, &uid, owner)?
        .ok_or_else(|| reject("asset UID membership has no policy"))?;
    if !value.accounts.contains(account) {
        return Err(reject("asset UID membership is absent from policy roster"));
    }
    Ok(Some(value))
}

/// Resolve beneficial ownership of retained native custody to its payer.
/// Native escrow admission separately seals who may debit that custody.
fn beneficial_policy(
    world: &impl WorldReadOnly,
    id: &AssetId,
) -> Result<Option<AssetUidPolicyV1>, Error> {
    beneficial_policy_with_owner(world, id, None)
}

fn beneficial_policy_with_owner(
    world: &impl WorldReadOnly,
    id: &AssetId,
    owner: Option<&WorldTransaction<'_, '_>>,
) -> Result<Option<AssetUidPolicyV1>, Error> {
    if let Some(value) = member_policy_with_owner(world, id.definition(), id.account(), owner)? {
        return Ok(Some(value));
    }
    for (_, escrow) in world.asset_escrows().iter() {
        if escrow.asset_definition == *id.definition()
            && escrow.custody == *id.account()
            && !escrow.remaining_amount.is_zero()
        {
            return member_policy_with_owner(world, id.definition(), &escrow.seller, owner);
        }
    }
    Ok(None)
}

fn balance(
    world: &impl WorldReadOnly,
    overrides: &BTreeMap<AssetId, Quantity>,
    id: &AssetId,
) -> Quantity {
    overrides.get(id).cloned().unwrap_or_else(|| {
        world
            .assets()
            .get(id)
            .map_or_else(Quantity::zero, |value| value.as_ref().clone())
    })
}

/// Sum exact member balances and retained payer-owned native escrow reserves.
/// Custody balances, rather than a pending record mutation, determine the amount
/// during release/refund. Thus returning funds to the payer has zero net effect.
pub fn holdings(
    world: &impl WorldReadOnly,
    policy: &AssetUidPolicyV1,
    overrides: &BTreeMap<AssetId, Quantity>,
) -> Result<Quantity, Error> {
    let mut total = Quantity::zero();
    for account in &policy.accounts {
        let id = AssetId::new(policy.asset_definition_id.clone(), account.clone());
        total = total
            .checked_add(&balance(world, overrides, &id))
            .map_err(|_| MathError::Overflow)?;
        for escrow_id in
            super::range_bounds::index_members(world.asset_escrows_by_seller(), account)
        {
            let escrow = world
                .asset_escrows()
                .get(escrow_id)
                .ok_or_else(|| reject("asset UID escrow index is inconsistent"))?;
            if escrow.asset_definition == policy.asset_definition_id
                && !escrow.remaining_amount.is_zero()
            {
                let id = AssetId::new(policy.asset_definition_id.clone(), escrow.custody.clone());
                total = total
                    .checked_add(&balance(world, overrides, &id))
                    .map_err(|_| MathError::Overflow)?;
            }
        }
    }
    Ok(total)
}

pub fn check_holdings(
    world: &impl WorldReadOnly,
    policy: &AssetUidPolicyV1,
    overrides: &BTreeMap<AssetId, Quantity>,
) -> Result<(), Error> {
    if let Some(limit) = &policy.holding_limit {
        let total = holdings(world, policy, overrides)?;
        // A policy reduction can leave existing holdings above its new cap.
        // Only an increase is refused: spending, same-UID moves and refunds
        // must remain possible without inventing receiving headroom.
        if total > *limit && total > holdings(world, policy, &BTreeMap::new())? {
            return Err(Error::AssetTransferAdmission(
                AssetTransferAdmissionError::HoldingLimitExceeded(
                    format!("identity-wide asset holding {total} exceeds {limit}").into(),
                ),
            ));
        }
    }
    Ok(())
}

/// Mint/other direct credit must use aggregate headroom, even outside Transfer.
pub fn check_credit(
    world: &impl WorldReadOnly,
    id: &AssetId,
    candidate: &Quantity,
) -> Result<(), Error> {
    super::asset_uid_enrollment::require_active_beneficial(world, id)?;
    if let Some(policy) = beneficial_policy(world, id)? {
        if id.scope() != &AssetBalanceScope::Global {
            return Err(reject("asset UID policy requires exact Global balances"));
        }
        check_holdings(
            world,
            &policy,
            &BTreeMap::from([(id.clone(), candidate.clone())]),
        )?;
    }
    Ok(())
}

fn usage(
    world: &impl WorldReadOnly,
    policy: &AssetUidPolicyV1,
    day: u64,
) -> Result<AssetUidUsageV1, Error> {
    usage_with_owner(world, policy, day, None)
}

fn usage_with_owner(
    world: &impl WorldReadOnly,
    policy: &AssetUidPolicyV1,
    day: u64,
    owner: Option<&WorldTransaction<'_, '_>>,
) -> Result<AssetUidUsageV1, Error> {
    let Some(bytes) =
        world
            .smart_contract_state()
            .get(&usage_key(&policy.asset_definition_id, &policy.uid, day))
    else {
        return Ok(AssetUidUsageV1 {
            asset_definition_id: policy.asset_definition_id.clone(),
            uid: policy.uid,
            daily_window: policy.daily_window,
            day_start_ms: day,
            spend_amount: Quantity::zero(),
            outgoing_amount: Quantity::zero(),
        });
    };
    let value: AssetUidUsageV1 = decode_with_owner(owner, bytes)?;
    if value.spend_amount > value.outgoing_amount
        || value.asset_definition_id != policy.asset_definition_id
        || value.uid != policy.uid
        || value.daily_window != policy.daily_window
        || value.day_start_ms != day
    {
        return Err(reject("asset UID activity differs from its canonical key"));
    }
    Ok(value)
}

#[path = "asset_uid_activity.rs"]
mod activity;
pub use activity::MovementActivity;

/// Exact purpose provenance supplied only by the sealed native movement kernel.
pub struct Movement<'a> {
    pub(crate) source: &'a AssetId,
    pub(crate) destination: &'a AssetId,
    pub(crate) amount: &'a Quantity,
    pub(crate) activity: MovementActivity,
}

/// Validate a complete ordered occurrence against one exclusive World view.
///
/// Returning encoded writes lets the caller finish every fallible preflight
/// before writing balances. Every sibling wallet is checked against the same
/// exclusive transaction World; independent legs call this separately.
/// Decoder refusals remain bound to that transaction through the actual movement recheck.
pub fn prepare_movements(
    world: &WorldTransaction<'_, '_>,
    movements: &[Movement<'_>],
    now_ms: u64,
) -> Result<Vec<(StatePath, Vec<u8>)>, Error> {
    check_movements(world, movements, now_ms, Some(world))?
        .into_iter()
        .map(|(key, value)| encode(&value).map(|bytes| (key, bytes)))
        .collect()
}

/// Check an ordered occurrence without encoding usage writes that the caller discards.
///
/// Settlement performs this check before its actual native movements. It must retain
/// the cumulative balance and calendar checks of `prepare_movements`, but it neither
/// publishes the temporary usage rows nor needs an encoded copy of each row.
/// The original transaction retains local binding, policy and usage decode refusals.
/// TODO: admit the shared policy decoding and map scratch to the original execution
/// pool; removing unused encoded output alone does not close that resource boundary.
pub fn precheck_movements(
    transaction: &StateTransaction<'_, '_>,
    movements: &[Movement<'_>],
    now_ms: u64,
) -> Result<(), Error> {
    check_movements(
        transaction.world(),
        movements,
        now_ms,
        Some(&transaction.world),
    )
    .map(drop)
}

fn check_movements(
    world: &impl WorldReadOnly,
    movements: &[Movement<'_>],
    now_ms: u64,
    owner: Option<&WorldTransaction<'_, '_>>,
) -> Result<BTreeMap<StatePath, AssetUidUsageV1>, Error> {
    let mut balances = BTreeMap::new();
    let mut activities = BTreeMap::<StatePath, AssetUidUsageV1>::new();
    for movement in movements {
        let source_policy = beneficial_policy_with_owner(world, movement.source, owner)?;
        let destination_policy = beneficial_policy_with_owner(world, movement.destination, owner)?;
        if (source_policy.is_some() || destination_policy.is_some())
            && (movement.source.definition() != movement.destination.definition()
                || movement.source.scope() != &AssetBalanceScope::Global
                || movement.destination.scope() != &AssetBalanceScope::Global)
        {
            return Err(reject(
                "asset UID movement requires one exact Global definition",
            ));
        }
        let destination_ceiling = destination_policy
            .as_ref()
            .map(|policy| {
                policy
                    .holding_limit
                    .as_ref()
                    .map(|limit| {
                        holdings(world, policy, &balances).map(|before| before.max(limit.clone()))
                    })
                    .transpose()
            })
            .transpose()?
            .flatten();
        let source_after = balance(world, &balances, movement.source)
            .checked_sub(movement.amount)
            .map_err(|_| MathError::NotEnoughQuantity)?;
        balances.insert(movement.source.clone(), source_after);
        let destination_after = balance(world, &balances, movement.destination)
            .checked_add(movement.amount)
            .map_err(|_| MathError::Overflow)?;
        balances.insert(movement.destination.clone(), destination_after);
        if let (Some(value), Some(ceiling)) = (&destination_policy, destination_ceiling)
            && holdings(world, value, &balances)? > ceiling
        {
            return Err(Error::AssetTransferAdmission(
                AssetTransferAdmissionError::HoldingLimitExceeded(
                    "identity-wide asset holding increase exceeds its current cap".into(),
                ),
            ));
        }
        activity::charge_activity(world, movement, now_ms, &mut activities, owner)?;
    }
    Ok(activities)
}

/// Retain the allowed beneficial holding ceiling before funding new custody.
pub fn opening_holding_ceiling(
    world: &impl WorldReadOnly,
    source: &AssetId,
) -> Result<Option<Quantity>, Error> {
    let Some(policy) = member_policy(world, source.definition(), source.account())? else {
        return Ok(None);
    };
    policy
        .holding_limit
        .as_ref()
        .map(|limit| {
            holdings(world, &policy, &BTreeMap::new()).map(|before| before.max(limit.clone()))
        })
        .transpose()
}

/// Check the newly funded escrow before retaining its payer-owned reserve.
/// This closes compound atomic settlements that both fund custody and credit
/// another wallet of the same UID before the escrow record is installed.
pub fn check_opening(
    world: &impl WorldReadOnly,
    escrow: &iroha_data_model::escrow::AssetEscrowRecord,
    holding_ceiling: Option<&Quantity>,
) -> Result<(), Error> {
    let Some(value) = member_policy(world, &escrow.asset_definition, &escrow.seller)? else {
        return Ok(());
    };
    if let Some(limit) = holding_ceiling {
        let total = holdings(world, &value, &BTreeMap::new())?
            .checked_add(&escrow.amount)
            .map_err(|_| MathError::Overflow)?;
        if total > *limit {
            return Err(Error::AssetTransferAdmission(
                AssetTransferAdmissionError::HoldingLimitExceeded(
                    "conditional escrow reserve exceeds identity-wide holding limit".into(),
                ),
            ));
        }
    }
    Ok(())
}

/// Materialize exact owner-query rows from the captured native state.
pub fn views(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    selected_uid: Option<&[u8; 32]>,
    ledger_time_ms: u64,
    as_of_height: u64,
) -> Result<Vec<iroha_data_model::asset::uid::AssetUidPolicyViewV1>, Error> {
    let mut policies = Vec::new();
    if let Some(uid) = selected_uid {
        if let Some(value) = policy(world, definition, uid)? {
            policies.push(value);
        }
    } else {
        let prefix = format!("{POLICY_ROOT}{}/", digest(definition));
        for (key, bytes) in world.smart_contract_state().iter() {
            if key.as_ref().starts_with(&prefix) {
                let value: AssetUidPolicyV1 = decode(bytes)?;
                if &value.asset_definition_id != definition
                    || key != &policy_key(definition, &value.uid)
                {
                    return Err(reject("asset UID query encountered a noncanonical policy"));
                }
                value.validate_shape().map_err(reject)?;
                policies.push(value);
            }
        }
    }
    policies
        .into_iter()
        .map(|policy| {
            let day_start_ms = policy
                .daily_window
                .day_start_ms(ledger_time_ms)
                .map_err(reject)?;
            let beneficial_holdings = holdings(world, &policy, &BTreeMap::new())?;
            let usage = usage(world, &policy, day_start_ms)?;
            Ok(iroha_data_model::asset::uid::AssetUidPolicyViewV1 {
                policy,
                beneficial_holdings,
                wallet_holdings: None,
                day_start_ms,
                daily_spend_usage: usage.spend_amount,
                daily_outgoing_usage: usage.outgoing_amount,
                ledger_time_ms,
                as_of_height,
            })
        })
        .collect()
}

fn in_root(key: &StatePath, root: &str) -> bool {
    key.as_ref() == root.trim_end_matches('/') || key.as_ref().starts_with(root)
}

/// Native policy retains definitions/accounts so delete-and-recreate cannot
/// discard membership or daily usage.
pub fn retained(
    world: &impl WorldReadOnly,
    definition: Option<&AssetDefinitionId>,
    account: Option<&AccountId>,
) -> Result<bool, Error> {
    for (key, bytes) in world.smart_contract_state().iter() {
        if in_root(key, POLICY_ROOT) {
            let value: AssetUidPolicyV1 = decode(bytes)?;
            if definition.is_some_and(|id| id == &value.asset_definition_id)
                || account.is_some_and(|id| value.accounts.contains(id))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Validate additive state after restoring a snapshot, including old snapshots
/// with no UID roots. No defaults rewrite historical records or balances.
pub fn validate_persistence(world: &impl WorldReadOnly) -> Result<(), Error> {
    for (key, bytes) in world.smart_contract_state().iter() {
        if in_root(key, POLICY_ROOT) {
            let value: AssetUidPolicyV1 = decode(bytes)?;
            value.validate_shape().map_err(reject)?;
            if key != &policy_key(&value.asset_definition_id, &value.uid) {
                return Err(reject("noncanonical asset UID policy path"));
            }
            let definition = world.asset_definition(&value.asset_definition_id)?;
            if definition.balance_scope_policy()
                != iroha_data_model::asset::AssetBalancePolicy::Global
            {
                return Err(reject("asset UID policy requires a Global definition"));
            }
            for member in &value.accounts {
                world.account(member)?;
                if binding(world, &value.asset_definition_id, member)?
                    .is_none_or(|b| b.uid != value.uid)
                {
                    return Err(reject(
                        "asset UID policy is missing a reciprocal membership",
                    ));
                }
            }
        } else if in_root(key, BINDING_ROOT) {
            let value: AssetUidBindingV1 = decode(bytes)?;
            if key != &binding_key(&value.asset_definition_id, &value.account_id)
                || member_policy(world, &value.asset_definition_id, &value.account_id)?.is_none()
            {
                return Err(reject("orphan or noncanonical asset UID membership"));
            }
        } else if in_root(key, USAGE_ROOT) {
            let value: AssetUidUsageV1 = decode(bytes)?;
            if value.spend_amount > value.outgoing_amount
                || value
                    .daily_window
                    .day_start_ms(value.day_start_ms)
                    .map_err(reject)?
                    != value.day_start_ms
                || key != &usage_key(&value.asset_definition_id, &value.uid, value.day_start_ms)
                || policy(world, &value.asset_definition_id, &value.uid)?
                    .is_none_or(|policy| policy.daily_window != value.daily_window)
            {
                return Err(reject("orphan or noncanonical asset UID usage"));
            }
        }
    }
    Ok(())
}

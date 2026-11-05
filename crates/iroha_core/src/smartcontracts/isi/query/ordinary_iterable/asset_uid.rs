//! Bounded ownership and exact-asset scanning for the private UID roster query.
use super::*;
use crate::state::{asset_uid, range_bounds};
use iroha_data_model::{
    asset::{
        AssetId,
        uid::{AssetUidPolicyV1, AssetUidPolicyViewV1, AssetUidUsageV1},
    },
    query::asset_uid::FindAssetUidPoliciesV1,
};
use iroha_model_base::state_path::StatePath;
use iroha_primitives::numeric::Quantity;

#[allow(clippy::too_many_arguments)]
fn own_row(
    query: &FindAssetUidPoliciesV1,
    key: &StatePath,
    bytes: &[u8],
    world: &impl WorldReadOnly,
    network: &iroha_data_model::NetworkId,
    now: u64,
    as_of_height: u64,
    maximum: usize,
    allocated: usize,
    stats: &mut QueryExecutionStats,
    limits: OrdinaryQueryExecutionLimits,
) -> Result<(AssetUidPolicyViewV1, usize), Error> {
    if bytes.len() > maximum {
        return Err(Error::CapacityLimit);
    }
    let elements = maximum.checked_mul(8).ok_or(Error::CapacityLimit)?;
    let decode_limits = norito::DecodeLimits::new(elements, maximum, elements, allocated, 64);
    let (row, usage) = norito::core::with_decode_limits_measured(decode_limits, || {
        let policy =
            norito::decode_from_bytes_with_limits::<AssetUidPolicyV1>(bytes, decode_limits)
                .map_err(|_| Error::CapacityLimit)?;
        if policy.asset_definition_id != query.asset_definition_id
            || query.uid.is_some_and(|uid| uid != policy.uid)
            || *key != asset_uid::policy_key(&policy.asset_definition_id, &policy.uid)
        {
            return Err(Error::Conversion(
                "UID policy differs from exact source key".into(),
            ));
        }
        policy
            .validate_shape()
            .map_err(|message| Error::Conversion(message.into()))?;
        let day_start_ms = policy
            .daily_window
            .day_start_ms(now)
            .map_err(|message| Error::Conversion(message.into()))?;
        let work = limits.max_source_item_bytes();
        let budget = Some(limits.execution_budget());
        let mut beneficial_holdings = Quantity::zero();
        for account in &policy.accounts {
            stats.record_preflighted_item(work, budget)?;
            let asset = AssetId::new(
                policy.asset_definition_id.clone(),
                account
                    .try_clone_for_admission()
                    .map_err(|_| Error::CapacityLimit)?,
            );
            if let Some(amount) = world.assets().get(&asset) {
                beneficial_holdings = beneficial_holdings
                    .checked_add(amount.as_ref())
                    .map_err(|_| Error::CapacityLimit)?;
            }
            for escrow_id in range_bounds::index_members(world.asset_escrows_by_seller(), account) {
                stats.record_preflighted_item(work, budget)?;
                let escrow = world
                    .asset_escrows()
                    .get(escrow_id)
                    .ok_or_else(|| Error::Conversion("UID escrow index is inconsistent".into()))?;
                if escrow.asset_definition == policy.asset_definition_id
                    && !escrow.remaining_amount.is_zero()
                {
                    let asset = AssetId::new(
                        policy.asset_definition_id.clone(),
                        escrow
                            .custody
                            .try_clone_for_admission()
                            .map_err(|_| Error::CapacityLimit)?,
                    );
                    if let Some(amount) = world.assets().get(&asset) {
                        beneficial_holdings = beneficial_holdings
                            .checked_add(amount.as_ref())
                            .map_err(|_| Error::CapacityLimit)?;
                    }
                }
            }
        }
        let usage_key =
            asset_uid::usage_key(&policy.asset_definition_id, &policy.uid, day_start_ms);
        let (daily_spend_usage, daily_outgoing_usage) =
            if let Some(bytes) = world.smart_contract_state().get(&usage_key) {
                if bytes.len() > maximum {
                    return Err(Error::CapacityLimit);
                }
                let value =
                    norito::decode_from_bytes_with_limits::<AssetUidUsageV1>(bytes, decode_limits)
                        .map_err(|_| Error::CapacityLimit)?;
                if value.asset_definition_id != policy.asset_definition_id
                    || value.uid != policy.uid
                    || value.daily_window != policy.daily_window
                    || value.day_start_ms != day_start_ms
                {
                    return Err(Error::Conversion(
                        "UID usage differs from exact source key".into(),
                    ));
                }
                (value.spend_amount, value.outgoing_amount)
            } else {
                (Quantity::zero(), Quantity::zero())
            };
        let wallet_holdings = query
            .wallet_account_id
            .as_ref()
            .map(|account| {
                asset_uid::wallet_holdings::collect(
                    world,
                    network,
                    &policy,
                    account,
                    now,
                    maximum,
                    decode_limits,
                    || {
                        stats.record_preflighted_item(
                            work.checked_mul(3).ok_or(Error::CapacityLimit)?,
                            budget,
                        )
                    },
                )
            })
            .transpose()?;
        Ok(AssetUidPolicyViewV1 {
            policy,
            beneficial_holdings,
            wallet_holdings,
            day_start_ms,
            daily_spend_usage,
            daily_outgoing_usage,
            ledger_time_ms: now,
            as_of_height,
        })
    });
    Ok((row?, usage.total_allocated_bytes()))
}

/// Own only the admitted prefix and its completeness probe from one captured state.
pub(super) fn collect_asset_uid_policies(
    query: &FindAssetUidPoliciesV1,
    params: &QueryParams,
    mode: OrdinaryCursorMode,
    limits: OrdinaryQueryExecutionLimits,
    state: &impl StateReadOnly,
) -> Result<(ExactOwnedRows<AssetUidPolicyViewV1>, QueryExecutionStats), Error> {
    if query.wallet_account_id.is_some() && query.uid.is_none() {
        return Err(Error::Conversion(
            "wallet holdings require one exact selected UID".into(),
        ));
    }
    let now = state.authenticated_query_ledger_time_ms().ok_or_else(|| {
        Error::Conversion("UID policy query requires authenticated ledger time".into())
    })?;
    let as_of_height = u64::try_from(state.height()).map_err(|_| Error::CapacityLimit)?;
    let world = state.world();
    let network = state.network_id();
    let (maximum_rows, maximum, work) = trigger_source_limits(params, mode, limits)?;
    let start = asset_uid::policy_key(&query.asset_definition_id, &query.uid.unwrap_or([0; 32]));
    let end = asset_uid::policy_key(&query.asset_definition_id, &query.uid.unwrap_or([255; 32]));
    let source = world.smart_contract_state();
    let mut execution_stats = QueryExecutionStats::default();
    let mut selected = 0;
    // A view composes the policy, its holdings and usage and an optional wallet
    // product, so each row may grow to one composite row. Its stored policy and
    // usage rows still stay within one source item.
    let row_bytes = composite_row_bytes(limits)?;
    let row_maximum = usize::try_from(row_bytes).map_err(|_| Error::CapacityLimit)?;
    let single_graph = row_maximum
        .checked_sub(core::mem::size_of::<AssetUidPolicyViewV1>())
        .ok_or(Error::CapacityLimit)?;
    for (key, bytes) in source.range(start.clone()..=end.clone()).take(maximum_rows) {
        execution_stats.record_preflighted_item(work, Some(limits.execution_budget()))?;
        let (row, _) = own_row(
            query,
            key,
            bytes,
            world,
            network,
            now,
            as_of_height,
            maximum,
            single_graph,
            &mut execution_stats,
            limits,
        )?;
        drop(encode_bounded_frame(&row, row_maximum)?);
        selected += 1;
    }
    if query.wallet_account_id.is_some() && selected != 1 {
        return Err(Error::Conversion(
            "requested wallet UID policy is unavailable".into(),
        ));
    }
    // Rows on the page share the page envelope that every admitted policy
    // reserves. Rows past the page stay resident only in a stored cursor, whose
    // reservation covers `max_cursor_retained_items` source items. An ephemeral
    // probe row shares the page envelope.
    let fetch = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE)
        .get();
    let page_rows = match mode {
        OrdinaryCursorMode::Ephemeral => selected,
        OrdinaryCursorMode::Stored => {
            selected.min(usize::try_from(fetch).map_err(|_| Error::CapacityLimit)?)
        }
    };
    let page_bytes = limits
        .max_page_items()
        .checked_mul(limits.max_source_item_bytes())
        .ok_or(Error::CapacityLimit)?;
    let cursor_bytes = limits
        .max_cursor_retained_items()
        .checked_mul(limits.max_source_item_bytes())
        .ok_or(Error::CapacityLimit)?;
    let (mut page, page_slots) =
        RetainedDecodeBudget::shared::<AssetUidPolicyViewV1>(page_rows, page_bytes, row_bytes)?;
    let (mut cursor, cursor_slots) = RetainedDecodeBudget::shared::<AssetUidPolicyViewV1>(
        selected - page_rows,
        cursor_bytes,
        row_bytes,
    )?;
    let slot_bytes = page_slots
        .checked_add(cursor_slots)
        .ok_or(Error::CapacityLimit)?;
    let mut rows = ExactOwnedRows::new(selected, slot_bytes)?;
    for (index, (key, bytes)) in source.range(start..=end).take(selected).enumerate() {
        execution_stats.record_preflighted_item(work, Some(limits.execution_budget()))?;
        let graph = if index < page_rows {
            &mut page
        } else {
            &mut cursor
        };
        let (row, allocated) = own_row(
            query,
            key,
            bytes,
            world,
            network,
            now,
            as_of_height,
            maximum,
            graph.next_limit()?,
            &mut execution_stats,
            limits,
        )?;
        drop(encode_bounded_frame(&row, row_maximum)?);
        graph.record_decoded(allocated)?;
        rows.push(row)?;
    }
    Ok((rows.finish()?, execution_stats))
}

#[cfg(test)]
mod wallet_tests;

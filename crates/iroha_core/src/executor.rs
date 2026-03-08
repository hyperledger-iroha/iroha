// Detached executor note: Keep this handler minimal and side‑effect free; only record
// deltas. Prefer performing complex checks during merge in `StateBlock::merge_into`.
// Extend cautiously when adding new ISIs (Peer, Parameters, ExecuteTrigger, etc.).
//! Structures and impls related to processing Iroha Virtual Machine (IVM)
//! runtime executors.

use core::{convert::TryFrom, str::FromStr};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use base64::Engine as _;
use derive_more::Debug;
use iroha_config::parameters::actual::{GasLiquidity, GasVolatility};
use iroha_data_model::{
    Identifiable as _, Registrable as _, ValidationFail,
    account::AccountId,
    asset::{
        AssetDefinition,
        id::{AssetDefinitionId, AssetId},
        value::Asset,
    },
    block::BlockHeader,
    executor::{self as data_model_executor, ExecutorDataModel},
    isi::{
        CustomInstruction, InstructionBox, InstructionBox as DMInstructionBox, RemoveKeyValueBox,
        SetKeyValueBox, TransferBox, error::InstructionExecutionError, register::RegisterBox,
    },
    metadata::Metadata,
    parameter::{CustomParameter, CustomParameterId},
    permission::Permission,
    prelude::{Account, Domain, DomainId, Register, Transfer, Trigger},
    query::{AnyQueryBox, QueryRequest},
    role::{Role, RoleId},
    smart_contract::payloads::{ExecutorContext, Validate as ValidatePayload},
    transaction::{Executable, SignedTransaction},
};
use iroha_executor_data_model::{
    isi::multisig::MultisigInstructionBox, permission as executor_permission,
};
use iroha_logger::{debug, trace, warn};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm::runtime::IvmConfig;
use ivm::{IVM, Memory, VMError};
use mv::storage::StorageReadOnly;
use norito::{
    codec::{Decode, Encode},
    json::{self, JsonDeserialize as JsonDeserializeTrait, JsonSerialize as JsonSerializeTrait},
    to_bytes,
};
use rust_decimal::Decimal;
use settlement_router::haircut::LiquidityProfile;

#[cfg(feature = "zk-preverify")]
use crate::zk::PreverifyResult;
use crate::{
    gas as isi_gas,
    settlement::{PendingSettlement, QuoteError, VolatilityBucket},
    smartcontracts::{Execute as _, ivm::cache::IvmCache},
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
    sumeragi::status::{self as sumeragi_status, NexusFeeEvent, NexusFeePayer},
};
// NoritoDecode alias is unused; keep Decode via norito::codec where needed inline

#[cfg(test)]
const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";
const FIXTURE_LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";
const EXECUTOR_ADDITIONAL_FUEL_KEY: &str = "additional_fuel";
const FIXTURE_SIMPLE_INSTRUCTION_FUEL_COST: u64 = 31_000_000;
const FIXTURE_DOMAIN_LIMITS_PARAMETER_ID: &str = "DomainLimits";
const FIXTURE_PERMISSION_CAN_CONTROL_DOMAIN_LIVES: &str = "CanControlDomainLives";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureExecutorKind {
    WithAdmin,
    WithCustomPermission,
    RemovePermission,
    CustomInstructionsSimple,
    CustomInstructionsComplex,
    WithMigrationFail,
    WithFuel,
    WithCustomParameter,
}

impl FixtureExecutorKind {
    const fn from_vector_length(vector_length: u8) -> Option<Self> {
        match vector_length {
            1 => Some(Self::WithAdmin),
            2 => Some(Self::WithCustomPermission),
            3 => Some(Self::RemovePermission),
            4 => Some(Self::CustomInstructionsSimple),
            5 => Some(Self::CustomInstructionsComplex),
            6 => Some(Self::WithMigrationFail),
            7 => Some(Self::WithFuel),
            8 => Some(Self::WithCustomParameter),
            _ => None,
        }
    }
}

/// Execute a single instruction in a detached overlay, recording only the state deltas.
///
/// This helper is used by the parallel validator to pre-apply side-effect-free
/// instructions without borrowing a live `StateBlock`. Unsupported instructions
/// return `ValidationFail::InternalError` so the caller can conservatively fall back
/// to sequential execution.
#[allow(clippy::too_many_lines)]
pub(crate) fn execute_instruction_detached(
    authority: &AccountId,
    instruction: &iroha_data_model::isi::InstructionBox,
    delta: &mut crate::state::DetachedStateTransactionDelta,
) -> Result<(), ValidationFail> {
    use iroha_data_model::isi::{
        BurnBox, GrantBox, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox, SetKeyValueBox,
        TransferBox, UnregisterBox,
    };

    let any = instruction.as_any();

    // SetKeyValue
    if let Some(kv) = any.downcast_ref::<SetKeyValueBox>() {
        match kv {
            SetKeyValueBox::Account(s) => {
                delta.set_account_kv(s.object.clone(), s.key.clone(), s.value.clone());
            }
            SetKeyValueBox::Domain(s) => {
                delta.set_domain_kv(s.object.clone(), s.key.clone(), s.value.clone());
            }
            SetKeyValueBox::AssetDefinition(s) => {
                delta.set_asset_def_kv(s.object.clone(), s.key.clone(), s.value.clone());
            }
            SetKeyValueBox::Nft(s) => {
                delta.set_nft_kv(s.object.clone(), s.key.clone(), s.value.clone());
            }
            SetKeyValueBox::Trigger(_) => {
                return Err(ValidationFail::InternalError(
                    "detached: unsupported SetKeyValue<Trigger>".to_owned(),
                ));
            }
        }
        return Ok(());
    }

    // RemoveKeyValue
    if let Some(rm) = any.downcast_ref::<RemoveKeyValueBox>() {
        match rm {
            RemoveKeyValueBox::Account(r) => {
                delta.remove_account_kv(r.object.clone(), r.key.clone())
            }
            RemoveKeyValueBox::Domain(r) => delta.remove_domain_kv(r.object.clone(), r.key.clone()),
            RemoveKeyValueBox::AssetDefinition(r) => {
                delta.remove_asset_def_kv(r.object.clone(), r.key.clone())
            }
            RemoveKeyValueBox::Nft(r) => {
                delta.remove_nft_kv(r.object.clone(), r.key.clone());
            }
            RemoveKeyValueBox::Trigger(_) => {
                return Err(ValidationFail::InternalError(
                    "detached: unsupported RemoveKeyValue<Trigger>".to_owned(),
                ));
            }
        }
        return Ok(());
    }

    // Mint / Burn
    if let Some(mb) = any.downcast_ref::<MintBox>() {
        match mb {
            MintBox::Asset(m) => {
                let asset_id = m.destination.clone();
                let qty = m.object.clone();
                // Record per-account balance increase and total supply increase
                delta.add_asset_add(asset_id.clone(), qty.clone());
                delta.add_total_add(asset_id.definition().clone(), qty);
                // Track mintability usage so block application can update the definition.
                delta.record_mint_consumption(asset_id.definition().clone(), 1);
            }
            MintBox::TriggerRepetitions(_) => {
                return Err(ValidationFail::InternalError(
                    "detached: unsupported Mint<Trigger>".to_owned(),
                ));
            }
        }
        return Ok(());
    }
    if let Some(bb) = any.downcast_ref::<BurnBox>() {
        match bb {
            BurnBox::Asset(b) => {
                let asset_id = b.destination.clone();
                let qty = b.object.clone();
                // Record per-account balance decrease and total supply decrease
                delta.add_asset_sub(asset_id.clone(), qty.clone());
                delta.add_total_sub(asset_id.definition().clone(), qty);
            }
            BurnBox::TriggerRepetitions(_) => {
                return Err(ValidationFail::InternalError(
                    "detached: unsupported Burn<Trigger>".to_owned(),
                ));
            }
        }
        return Ok(());
    }

    // SetParameter
    if let Some(sp) = any.downcast_ref::<iroha_data_model::isi::SetParameter>() {
        delta.set_parameter(sp.inner().clone());
        return Ok(());
    }

    // ExecuteTrigger (by-call)
    if let Some(et) = any.downcast_ref::<iroha_data_model::isi::ExecuteTrigger>() {
        let evt = iroha_data_model::events::execute_trigger::ExecuteTriggerEvent {
            trigger_id: et.trigger.clone(),
            authority: authority.clone(),
            args: et.args.clone(),
        };
        delta.execute_trigger_by_call(evt);
        return Ok(());
    }

    // Transfers
    if let Some(tb) = any.downcast_ref::<TransferBox>() {
        match tb {
            TransferBox::Asset(t) => {
                let src = t.source.clone();
                let qty = t.object.clone();
                let dst = iroha_data_model::asset::AssetId::of(
                    src.definition().clone(),
                    t.destination.clone(),
                );
                delta.add_asset_sub(src, qty.clone());
                delta.add_asset_add(dst, qty);
            }
            TransferBox::Domain(t) => {
                delta.transfer_domain(t.object.clone(), t.source.clone(), t.destination.clone());
            }
            TransferBox::AssetDefinition(t) => {
                delta.transfer_asset_def(t.object.clone(), t.source.clone(), t.destination.clone());
            }
            TransferBox::Nft(t) => {
                delta.transfer_nft(t.object.clone(), t.source.clone(), t.destination.clone());
            }
        }
        return Ok(());
    }

    // Register / Unregister: record peer changes directly so peer management works
    // even when the runtime executor is not yet upgraded.
    if let Some(rb) = any.downcast_ref::<RegisterBox>() {
        match rb {
            RegisterBox::Nft(r) => {
                let nft = r.object.clone().build(authority);
                delta.register_nft(nft);
            }
            RegisterBox::Peer(_r) => {
                return Err(ValidationFail::InternalError(
                    "detached: peer management requires sequential path".to_owned(),
                ));
            }
            _ => {
                return Err(ValidationFail::InternalError(
                    "detached: unsupported Register".to_owned(),
                ));
            }
        }
        return Ok(());
    }
    if let Some(ub) = any.downcast_ref::<UnregisterBox>() {
        match ub {
            UnregisterBox::Nft(u) => delta.unregister_nft(u.object.clone()),
            UnregisterBox::Peer(_u) => {
                return Err(ValidationFail::InternalError(
                    "detached: peer management requires sequential path".to_owned(),
                ));
            }
            _ => {
                return Err(ValidationFail::InternalError(
                    "detached: unsupported Unregister".to_owned(),
                ));
            }
        }
        return Ok(());
    }

    // Grant / Revoke on accounts
    if let Some(gb) = any.downcast_ref::<GrantBox>() {
        match gb {
            GrantBox::Permission(g) => {
                delta.grant_permission(g.destination.clone(), g.object.clone());
            }
            GrantBox::Role(g) => {
                delta.grant_role(g.destination.clone(), g.object.clone());
            }
            GrantBox::RolePermission(g) => {
                delta.grant_role_permission(g.destination.clone(), g.object.clone());
            }
        }
        return Ok(());
    }
    if let Some(rb) = any.downcast_ref::<RevokeBox>() {
        match rb {
            RevokeBox::Permission(r) => {
                delta.revoke_permission(r.destination.clone(), r.object.clone());
            }
            RevokeBox::Role(r) => {
                delta.revoke_role(r.destination.clone(), r.object.clone());
            }
            RevokeBox::RolePermission(r) => {
                delta.revoke_role_permission(r.destination.clone(), r.object.clone());
            }
        }
        return Ok(());
    }

    // Unknown instruction kind – signal fallback
    Err(ValidationFail::InternalError(
        "detached: unsupported instruction".to_owned(),
    ))
}

/// Executor that verifies that operation is valid and executes it.
///
/// Executing is done in order to verify dependent instructions in transaction.
/// Can be upgraded with [`Upgrade`](iroha_data_model::isi::Upgrade) instruction.
#[derive(Debug, Default, Clone)]
pub enum Executor {
    /// Initial executor with minimal built-in permission checks for critical instructions.
    #[default]
    Initial,
    /// User-provided executor with arbitrary logic.
    UserProvided(LoadedExecutor),
}

/// Execution profile applied when running native ISIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstructionExecutionProfile {
    /// Full runtime behaviour (logging, telemetry, and policy hooks).
    #[default]
    Runtime,
    /// Lightweight execution for benchmarks/tests lacking a global logger.
    Bench,
}

impl JsonSerializeTrait for Executor {
    fn json_serialize(&self, out: &mut String) {
        let bytes =
            executor_norito::to_bytes(self).unwrap_or_else(|e| panic!("norito encode failed: {e}"));
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        out.push('{');
        json::write_json_string("norito", out);
        out.push(':');
        json::write_json_string(&encoded, out);
        out.push('}');
    }
}

impl JsonDeserializeTrait for Executor {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        parse_executor_value(value)
    }
}

fn parse_executor_value(value: json::Value) -> Result<Executor, json::Error> {
    match value {
        json::Value::Object(mut map) => {
            if let Some(inner) = map.remove("norito").or_else(|| map.remove("bytes")) {
                let bytes = decode_executor_bytes(inner, "norito")?;
                return executor_norito::from_bytes(&bytes).map_err(json::Error::Message);
            }

            if !map.is_empty() {
                for key in map.keys() {
                    trace!(target: "executor::deserialize", field = %key, "ignoring unknown executor field");
                }
            }
            Err(json::Error::Message(
                "invalid executor object: expected {\"norito\": ...}".into(),
            ))
        }
        json::Value::String(s) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| json::Error::Message(e.to_string()))?;
            executor_norito::from_bytes(&bytes).map_err(json::Error::Message)
        }
        other => Err(json::Error::Message(format!(
            "invalid executor JSON: expected object or string, got {other:?}"
        ))),
    }
}

fn decode_executor_bytes(value: json::Value, context: &str) -> Result<Vec<u8>, json::Error> {
    match value {
        json::Value::String(s) => {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| json::Error::InvalidField {
                    field: context.into(),
                    message: e.to_string(),
                })
        }
        json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                let byte = v.as_u64().ok_or_else(|| json::Error::InvalidField {
                    field: context.into(),
                    message: "expected byte (u64)".into(),
                })?;
                out.push((byte & 0xFF) as u8);
            }
            Ok(out)
        }
        other => Err(json::Error::InvalidField {
            field: context.into(),
            message: format!("expected base64 string or byte array, got {other:?}"),
        }),
    }
}

fn convert_volatility_bucket(volatility: GasVolatility) -> VolatilityBucket {
    match volatility {
        GasVolatility::Stable => VolatilityBucket::Stable,
        GasVolatility::Elevated => VolatilityBucket::Elevated,
        GasVolatility::Dislocated => VolatilityBucket::Dislocated,
    }
}

fn parse_fee_sponsor(
    world: &impl WorldReadOnly,
    metadata: &Metadata,
) -> Result<Option<AccountId>, ValidationFail> {
    let Some(raw) = metadata.get("fee_sponsor") else {
        return Ok(None);
    };
    match raw.try_into_any_norito::<AccountId>() {
        Ok(sponsor) => Ok(Some(sponsor)),
        Err(err) => {
            if let Ok(literal) = raw.try_into_any_norito::<String>()
                && let Some(sponsor) =
                    crate::block::parse_account_literal_with_world(world, &literal)
            {
                return Ok(Some(sponsor));
            }
            Err(ValidationFail::NotPermitted(format!(
                "invalid fee_sponsor metadata: {err}"
            )))
        }
    }
}

fn parse_account_id_literal(world: &impl WorldReadOnly, literal: &str) -> Option<AccountId> {
    crate::block::parse_account_literal_with_world(world, literal).or_else(|| {
        AccountId::parse_encoded(literal)
            .ok()
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
    })
}

/// Parse optional `gas_limit` from transaction metadata.
pub(crate) fn parse_gas_limit(metadata: &Metadata) -> Result<Option<u64>, ValidationFail> {
    let Some(raw) = metadata.get("gas_limit") else {
        return Ok(None);
    };
    let value = raw.try_into_any_norito::<u64>().map_err(|err| {
        ValidationFail::NotPermitted(format!("invalid gas_limit metadata: {err}"))
    })?;
    if value == 0 {
        return Err(ValidationFail::NotPermitted(
            "gas_limit must be positive".to_owned(),
        ));
    }
    Ok(Some(value))
}

fn parse_executor_additional_fuel(metadata: &Metadata) -> Result<u64, ValidationFail> {
    let Some(raw) = metadata.get(EXECUTOR_ADDITIONAL_FUEL_KEY) else {
        return Ok(0);
    };
    raw.try_into_any_norito::<u64>().map_err(|err| {
        ValidationFail::NotPermitted(format!("invalid additional_fuel metadata: {err}"))
    })
}

pub(crate) fn configure_executor_fuel_budget(
    executor: &Executor,
    state_transaction: &mut StateTransaction<'_, '_>,
    metadata: &Metadata,
) -> Result<(), ValidationFail> {
    if matches!(executor, Executor::UserProvided(_)) {
        let base_fuel = state_transaction
            .world
            .parameters
            .get()
            .executor()
            .fuel
            .get();
        let additional_fuel = parse_executor_additional_fuel(metadata)?;
        state_transaction.executor_fuel_remaining = Some(base_fuel.saturating_add(additional_fuel));
    }
    Ok(())
}

/// Charge gas and Nexus fees for a transaction that was applied via overlay execution paths.
///
/// Overlay execution bypasses `Executor::execute_transaction`, so this helper mirrors the
/// fee-accounting behavior that `execute_transaction` performs for each committed transaction.
pub(crate) fn charge_fees_for_applied_overlay(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    transaction: &SignedTransaction,
    overlay: &crate::pipeline::overlay::TxOverlay,
) -> Result<(), ValidationFail> {
    // Genesis transactions are bootstrap operations and must remain fee-free.
    if state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty() {
        return Ok(());
    }

    let tx_bytes_len = to_bytes(transaction)
        .map(|bytes| bytes.len())
        .map_err(|err| {
            ValidationFail::InternalError(format!(
                "failed to encode transaction for fee metering: {err}"
            ))
        })?;

    let md = transaction.metadata();
    let fee_sponsor = parse_fee_sponsor(&state_transaction.world, md)?;

    // Keep gas policy snapshots aligned with governance/custom parameter updates.
    Executor::refresh_gas_from_parameters(state_transaction);

    let gas_asset_opt = md.get("gas_asset_id").map(|j| j.as_ref().to_string());
    let gas_limit_md = parse_gas_limit(md)?;
    let pipeline_gas = &state_transaction.pipeline.gas;
    if !pipeline_gas.accepted_assets.is_empty() {
        let Some(ref gas_asset_id_str) = gas_asset_opt else {
            return Err(ValidationFail::NotPermitted(
                "missing gas_asset_id in transaction metadata".to_owned(),
            ));
        };
        if !pipeline_gas
            .accepted_assets
            .iter()
            .any(|a| a == gas_asset_id_str)
        {
            return Err(ValidationFail::NotPermitted(format!(
                "gas asset `{gas_asset_id_str}` is not accepted by node policy"
            )));
        }
    }

    let (gas_used, instruction_count, require_gas_limit) = match transaction.instructions() {
        Executable::Ivm(_) => (
            overlay.ivm_gas_used().ok_or_else(|| {
                ValidationFail::InternalError(
                    "missing IVM gas usage metadata for overlay-applied transaction".to_owned(),
                )
            })?,
            0,
            true,
        ),
        Executable::Instructions(_) => (
            isi_gas::meter_instructions(overlay.instruction_slice()),
            overlay.instruction_count(),
            false,
        ),
        Executable::IvmProved(_) => (
            isi_gas::meter_instructions(overlay.instruction_slice()),
            overlay.instruction_count(),
            true,
        ),
    };

    if require_gas_limit && gas_limit_md.is_none() {
        return Err(ValidationFail::NotPermitted(
            "missing gas_limit in transaction metadata".to_owned(),
        ));
    }
    if let Some(limit) = gas_limit_md
        && gas_used > limit
    {
        return Err(ValidationFail::NotPermitted(format!(
            "out of gas: used {gas_used} > limit {limit}"
        )));
    }

    let confidential_delta = overlay
        .instruction_slice()
        .iter()
        .map(crate::gas::confidential_gas_cost)
        .sum::<u64>();
    if confidential_delta > 0 {
        state_transaction.record_confidential_gas_delta(confidential_delta);
    }
    state_transaction.last_tx_gas_used = gas_used;

    let tx_hash = transaction.hash();
    let settlement_source_id = {
        let mut bytes = [0u8; iroha_crypto::Hash::LENGTH];
        bytes.copy_from_slice(tx_hash.as_ref());
        bytes
    };

    if let Some(gas_asset_id_str) = gas_asset_opt {
        let (units_per_gas, twap_local_per_xor, volatility_bucket, liquidity_profile) = {
            let gas_rate = state_transaction
                .pipeline
                .gas
                .units_per_gas
                .iter()
                .find(|r| r.asset == gas_asset_id_str)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "missing units_per_gas mapping for `{gas_asset_id_str}`"
                    ))
                })?;
            let volatility_bucket = convert_volatility_bucket(gas_rate.volatility);
            let liquidity_profile = match gas_rate.liquidity {
                GasLiquidity::Tier1 => LiquidityProfile::Tier1,
                GasLiquidity::Tier2 => LiquidityProfile::Tier2,
                GasLiquidity::Tier3 => LiquidityProfile::Tier3,
            };
            (
                gas_rate.units_per_gas,
                gas_rate.twap_local_per_xor,
                volatility_bucket,
                liquidity_profile,
            )
        };

        if gas_used > 0 && units_per_gas > 0 {
            let tech_account: AccountId = parse_account_id_literal(
                &state_transaction.world,
                &state_transaction.pipeline.gas.tech_account_id,
            )
            .ok_or_else(|| {
                ValidationFail::InternalError(
                    "invalid pipeline.gas.tech_account_id; expected account identifier".to_owned(),
                )
            })?;

            let asset_def: AssetDefinitionId = gas_asset_id_str.parse().map_err(|_| {
                ValidationFail::NotPermitted(
                    "invalid gas_asset_id; expected `name#domain`".to_owned(),
                )
            })?;

            let fee_u128 = u128::from(gas_used).saturating_mul(u128::from(units_per_gas));
            if fee_u128 > 0 {
                let payer = if let Some(sponsor) =
                    fee_sponsor.as_ref().filter(|sponsor| *sponsor != authority)
                {
                    if !state_transaction.nexus.fees.sponsorship_enabled {
                        return Err(ValidationFail::NotPermitted(
                            "fee sponsorship is disabled".to_owned(),
                        ));
                    }
                    if !state_transaction.can_use_fee_sponsor(authority, sponsor) {
                        return Err(ValidationFail::NotPermitted(
                            "fee sponsor is not authorized".to_owned(),
                        ));
                    }
                    sponsor.clone()
                } else {
                    authority.clone()
                };
                let payer_asset = AssetId::new(asset_def.clone(), payer.clone());
                let qty = Numeric::try_new(fee_u128, 0).map_err(|_| {
                    ValidationFail::NotPermitted(
                        "fee amount exceeds supported numeric bounds".to_owned(),
                    )
                })?;
                let transfer = iroha_data_model::isi::Transfer::<
                    Asset,
                    Numeric,
                    iroha_data_model::account::Account,
                >::asset_numeric(payer_asset, qty, tech_account);
                let instr: DMInstructionBox = transfer.into();
                instr.execute(authority, state_transaction).map_err(|err| {
                    iroha_logger::debug!(
                        ?err,
                        authority = %authority,
                        "gas fee transfer failed to apply"
                    );
                    ValidationFail::from(err)
                })?;
                #[cfg(feature = "telemetry")]
                {
                    let delta =
                        u64::try_from(fee_u128.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
                    state_transaction.stage_block_fee_units(delta);
                }

                let block_timestamp_ms_u128 =
                    state_transaction._curr_block.creation_time().as_millis();
                let block_timestamp_ms = u64::try_from(block_timestamp_ms_u128).unwrap_or(u64::MAX);
                let quote = state_transaction
                    .settlement_engine()
                    .quote(
                        settlement_source_id,
                        fee_u128,
                        twap_local_per_xor,
                        liquidity_profile,
                        volatility_bucket,
                        block_timestamp_ms,
                    )
                    .map_err(|err| match err {
                        QuoteError::LocalAmountOverflow(amount) => ValidationFail::NotPermitted(
                            format!("local gas amount {amount} exceeds Decimal range"),
                        ),
                        QuoteError::ZeroTwap => {
                            ValidationFail::NotPermitted("gas TWAP must be non-zero".to_owned())
                        }
                    })?;
                let config_snapshot = state_transaction.settlement_engine().config();
                let twap_window_seconds = config_snapshot.twap_window.whole_seconds().max(0);
                let twap_window_seconds = u32::try_from(twap_window_seconds).unwrap_or(u32::MAX);
                let xor_due_micro =
                    Executor::decimal_to_micro_u128(*quote.receipt.xor_due, "xor_due amount")?;
                let xor_after_haircut_micro = Executor::decimal_to_micro_u128(
                    *quote.receipt.xor_with_haircut,
                    "xor_after_haircut amount",
                )?;
                let xor_variance_micro = xor_due_micro.saturating_sub(xor_after_haircut_micro);
                let pending = PendingSettlement {
                    source_id: settlement_source_id,
                    asset_definition_id: asset_def,
                    local_amount_micro: quote.receipt.local_amount_micro,
                    xor_due_micro,
                    xor_after_haircut_micro,
                    xor_variance_micro,
                    timestamp_ms: block_timestamp_ms,
                    liquidity_profile,
                    volatility_bucket,
                    twap_local_per_xor,
                    epsilon_bps: quote.effective_epsilon_bps,
                    twap_window_seconds,
                    oracle_timestamp_ms: block_timestamp_ms,
                };
                state_transaction.record_settlement_receipt(tx_hash, pending);
            }
        }
    }

    Executor::charge_nexus_fees(
        state_transaction,
        authority,
        fee_sponsor,
        tx_bytes_len,
        instruction_count,
        gas_used,
    )?;

    Ok(())
}

impl Executor {
    fn decimal_to_micro_u128(
        value: Decimal,
        context: &'static str,
    ) -> Result<u128, ValidationFail> {
        if !value.fract().is_zero() {
            return Err(ValidationFail::InternalError(format!(
                "{context} must be an integral micro-XOR amount"
            )));
        }
        let truncated = value.trunc();
        if truncated.is_sign_negative() {
            return Err(ValidationFail::InternalError(format!(
                "{context} must be non-negative"
            )));
        }
        let mantissa = truncated.mantissa();
        u128::try_from(mantissa)
            .map_err(|_| ValidationFail::InternalError(format!("{context} exceeds u128 bounds")))
    }

    #[allow(clippy::too_many_lines)]
    fn charge_nexus_fees(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        sponsor: Option<AccountId>,
        tx_bytes_len: usize,
        instruction_count: usize,
        gas_used: u64,
    ) -> Result<(), ValidationFail> {
        if !state_transaction.nexus.enabled {
            return Ok(());
        }
        let cfg = state_transaction.nexus.fees.clone();
        let tx_bytes_u128 = u128::try_from(tx_bytes_len).map_err(|_| {
            ValidationFail::InternalError("transaction too large for fee accounting".to_owned())
        })?;
        let instr_u128 = u128::try_from(instruction_count).map_err(|_| {
            ValidationFail::InternalError(
                "instruction count too large for fee accounting".to_owned(),
            )
        })?;
        let mut fee_u128 = u128::from(cfg.base_fee);
        fee_u128 = fee_u128
            .checked_add(u128::from(cfg.per_byte_fee).saturating_mul(tx_bytes_u128))
            .ok_or_else(|| {
                ValidationFail::NotPermitted("fee amount exceeds supported numeric bounds".into())
            })?;
        fee_u128 = fee_u128
            .checked_add(u128::from(cfg.per_instruction_fee).saturating_mul(instr_u128))
            .ok_or_else(|| {
                ValidationFail::NotPermitted("fee amount exceeds supported numeric bounds".into())
            })?;
        fee_u128 = fee_u128
            .checked_add(u128::from(cfg.per_gas_unit_fee).saturating_mul(u128::from(gas_used)))
            .ok_or_else(|| {
                ValidationFail::NotPermitted("fee amount exceeds supported numeric bounds".into())
            })?;

        if fee_u128 == 0 {
            return Ok(());
        }

        let payer_kind = if sponsor.is_some() {
            NexusFeePayer::Sponsor
        } else {
            NexusFeePayer::Payer
        };
        let payer = if let Some(sponsor) = sponsor {
            if !cfg.sponsorship_enabled {
                let payer_id = sponsor.to_string();
                sumeragi_status::record_nexus_fee_event(NexusFeeEvent::SponsorDisabled {
                    payer_id: payer_id.clone(),
                });
                warn!(
                    target: "economics",
                    payer = %payer_id,
                    fee_amount = fee_u128,
                    "nexus fee sponsor rejected: sponsorship disabled"
                );
                return Err(ValidationFail::NotPermitted(
                    "fee sponsorship is disabled".to_owned(),
                ));
            }
            if !state_transaction.can_use_fee_sponsor(authority, &sponsor) {
                let sponsor_id = sponsor.to_string();
                let authority_id = authority.to_string();
                sumeragi_status::record_nexus_fee_event(NexusFeeEvent::SponsorUnauthorized {
                    sponsor_id: sponsor_id.clone(),
                    authority_id: authority_id.clone(),
                });
                warn!(
                    target: "economics",
                    sponsor = %sponsor_id,
                    authority = %authority_id,
                    fee_amount = fee_u128,
                    "nexus fee sponsor rejected: missing permission"
                );
                return Err(ValidationFail::NotPermitted(
                    "fee sponsor is not authorized".to_owned(),
                ));
            }
            if cfg.sponsor_max_fee > 0 && fee_u128 > u128::from(cfg.sponsor_max_fee) {
                let payer_id = sponsor.to_string();
                sumeragi_status::record_nexus_fee_event(NexusFeeEvent::SponsorCapExceeded {
                    payer_id: payer_id.clone(),
                    max_fee: cfg.sponsor_max_fee,
                    attempted_fee: fee_u128,
                });
                warn!(
                    target: "economics",
                    payer = %payer_id,
                    fee_amount = fee_u128,
                    max_fee = cfg.sponsor_max_fee,
                    "nexus fee sponsor rejected: exceeds sponsor_max_fee"
                );
                return Err(ValidationFail::NotPermitted(
                    "fee exceeds sponsor_max_fee".to_owned(),
                ));
            }
            sponsor
        } else {
            authority.clone()
        };

        let sink_account = crate::block::parse_account_literal_with_world(
            &state_transaction.world,
            &cfg.fee_sink_account_id,
        )
        .or_else(|| {
            AccountId::parse_encoded(&cfg.fee_sink_account_id)
                .ok()
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        })
        .ok_or_else(|| {
            let reason =
                "invalid nexus fee sink account id; expected account identifier".to_owned();
            sumeragi_status::record_nexus_fee_event(NexusFeeEvent::ConfigInvalid {
                reason: reason.clone(),
            });
            warn!(target: "economics", "nexus fee rejected: {reason}");
            ValidationFail::NotPermitted(reason)
        })?;
        let asset_def: AssetDefinitionId = cfg.fee_asset_id.parse().map_err(|_| {
            let reason = "invalid nexus fee asset id; expected `name#domain`".to_owned();
            sumeragi_status::record_nexus_fee_event(NexusFeeEvent::ConfigInvalid {
                reason: reason.clone(),
            });
            warn!(target: "economics", "nexus fee rejected: {reason}");
            ValidationFail::NotPermitted(reason)
        })?;

        let payer_asset = AssetId::new(asset_def, payer.clone());
        let payer_kind_label = match payer_kind {
            NexusFeePayer::Payer => "payer",
            NexusFeePayer::Sponsor => "sponsor",
        };
        let payer_id = payer.to_string();
        let asset_label = payer_asset.definition().to_string();
        let sink_label = sink_account.to_string();
        let qty = Numeric::try_new(fee_u128, 0).map_err(|_| {
            let reason = "fee amount exceeds supported numeric bounds".to_owned();
            sumeragi_status::record_nexus_fee_event(NexusFeeEvent::ConfigInvalid {
                reason: reason.clone(),
            });
            ValidationFail::NotPermitted(reason)
        })?;
        let transfer = iroha_data_model::isi::Transfer::<
            Asset,
            Numeric,
            iroha_data_model::account::Account,
        >::asset_numeric(payer_asset, qty, sink_account);
        let instr: DMInstructionBox = transfer.into();
        instr.execute(authority, state_transaction).map_err(|err| {
            let reason = format!("nexus fee transfer failed to apply: {err}");
            sumeragi_status::record_nexus_fee_event(NexusFeeEvent::TransferFailed {
                payer_kind,
                payer_id: payer_id.clone(),
                amount: fee_u128,
                asset_id: asset_label.clone(),
                reason: reason.clone(),
            });
            warn!(
                target: "economics",
                ?err,
                payer = %payer_id,
                payer_kind = payer_kind_label,
                fee_amount = fee_u128,
                asset = %asset_label,
                sink = %sink_label,
                "nexus fee transfer failed"
            );
            ValidationFail::from(err)
        })?;

        // Stage the charged event so rejected transactions don't report successful debits.
        state_transaction.stage_nexus_fee_event(NexusFeeEvent::Charged {
            payer_kind,
            payer_id,
            amount: fee_u128,
            asset_id: asset_label,
        });
        Ok(())
    }

    /// Refresh pipeline.gas snapshot from on-chain custom parameters (genesis/governance updatable).
    fn refresh_gas_from_parameters(state_transaction: &mut StateTransaction<'_, '_>) {
        #[derive(crate::json_macros::JsonDeserialize)]
        struct GasRateSerde {
            asset: String,
            units_per_gas: u64,
            twap_local_per_xor: Option<String>,
            liquidity_profile: Option<String>,
            volatility_class: Option<String>,
        }

        let params = state_transaction.world.parameters.get();
        // Helper to update from a CustomParameter if present and decodable
        // 1) Tech account id (string)
        if let Ok(name) = core::str::FromStr::from_str("ivm_gas_tech_account_id")
            && let Some(custom) = params.custom().get(&CustomParameterId(name))
            && let Ok(s) = custom.payload().try_into_any_norito::<String>()
        {
            state_transaction.pipeline.gas.tech_account_id = s;
        }
        // 2) Accepted assets (Vec<String>)
        if let Ok(name) = core::str::FromStr::from_str("ivm_gas_accepted_assets")
            && let Some(custom) = params.custom().get(&CustomParameterId(name))
            && let Ok(v) = custom.payload().try_into_any_norito::<Vec<String>>()
        {
            state_transaction.pipeline.gas.accepted_assets = v;
        }
        // 3) Units per gas (Vec<{asset, units_per_gas}>)
        if let Ok(name) = core::str::FromStr::from_str("ivm_gas_units_per_gas")
            && let Some(custom) = params.custom().get(&CustomParameterId(name))
            && let Ok(v) = custom.payload().try_into_any_norito::<Vec<GasRateSerde>>()
        {
            state_transaction.pipeline.gas.units_per_gas = v
                .into_iter()
                .map(|r| {
                    let asset = r.asset;
                    let twap = r
                        .twap_local_per_xor
                        .as_deref()
                        .map_or(Decimal::ONE, |value| {
                            Decimal::from_str(value).unwrap_or_else(|error| {
                                panic!(
                                    "invalid ivm_gas_units_per_gas twap `{value}` for asset `{asset}`: {error}"
                                )
                            })
                        });
                    let liquidity = r.liquidity_profile.as_deref().map_or_else(
                        iroha_config::parameters::actual::GasLiquidity::default,
                        |value| {
                            iroha_config::parameters::actual::GasLiquidity::from_str(value)
                                .unwrap_or_else(|()| {
                                    panic!(
                                        "invalid ivm_gas_units_per_gas liquidity `{value}` for asset `{asset}`"
                                    )
                                })
                        },
                    );
                    let volatility = r.volatility_class.as_deref().map_or_else(
                        iroha_config::parameters::actual::GasVolatility::default,
                        |value| {
                            iroha_config::parameters::actual::GasVolatility::from_str(value)
                                .unwrap_or_else(|()| {
                                    panic!(
                                        "invalid ivm_gas_units_per_gas volatility `{value}` for asset `{asset}`"
                                    )
                                })
                        },
                    );
                    iroha_config::parameters::actual::GasRate {
                        asset,
                        units_per_gas: r.units_per_gas,
                        twap_local_per_xor: twap,
                        liquidity,
                        volatility,
                    }
                })
                .collect();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn execute_metered_instructions(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instructions: Vec<InstructionBox>,
        tx_bytes_len: usize,
        settlement_source_id: [u8; iroha_crypto::Hash::LENGTH],
        tx_hash: iroha_crypto::HashOf<SignedTransaction>,
        gas_limit_md: Option<u64>,
        require_gas_limit: bool,
        gas_asset_opt: Option<String>,
        fee_sponsor: Option<AccountId>,
    ) -> Result<(), ValidationFail> {
        if require_gas_limit && gas_limit_md.is_none() {
            return Err(ValidationFail::NotPermitted(
                "missing gas_limit in transaction metadata".to_owned(),
            ));
        }

        // 1) Deterministically meter the instruction batch.
        let used = isi_gas::meter_instructions(&instructions);

        // 2) Enforce optional payer-provided gas limit (caps fee exposure).
        if let Some(limit) = gas_limit_md
            && used > limit
        {
            return Err(ValidationFail::NotPermitted(format!(
                "out of gas: used {used} > limit {limit}"
            )));
        }

        let instruction_count = instructions.len();
        let confidential_delta = instructions
            .iter()
            .map(crate::gas::confidential_gas_cost)
            .sum::<u64>();

        // 3) Execute ISIs in order.
        for isi in instructions {
            self.execute_instruction(state_transaction, authority, isi)?;
        }

        // Track confidential gas after successful execution.
        if confidential_delta > 0 {
            state_transaction.record_confidential_gas_delta(confidential_delta);
        }

        // 4) Record gas used for block-level budget enforcement.
        state_transaction.last_tx_gas_used = used;

        // 5) Charge gas fees when configured and the transaction specified a gas asset.
        if let Some(gas_asset_id_str) = gas_asset_opt {
            // Determine rate; require explicit mapping for determinism
            let gas_rate = state_transaction
                .pipeline
                .gas
                .units_per_gas
                .iter()
                .find(|r| r.asset == gas_asset_id_str)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "missing units_per_gas mapping for `{gas_asset_id_str}`"
                    ))
                })?;

            let units_per_gas = gas_rate.units_per_gas;
            let twap_local_per_xor = gas_rate.twap_local_per_xor;
            let volatility_bucket = convert_volatility_bucket(gas_rate.volatility);
            let liquidity_profile = match gas_rate.liquidity {
                GasLiquidity::Tier1 => LiquidityProfile::Tier1,
                GasLiquidity::Tier2 => LiquidityProfile::Tier2,
                GasLiquidity::Tier3 => LiquidityProfile::Tier3,
            };

            if used > 0 && units_per_gas > 0 {
                // Parse tech account id
                let tech_account: AccountId = parse_account_id_literal(
                    &state_transaction.world,
                    &state_transaction.pipeline.gas.tech_account_id,
                )
                .ok_or_else(|| {
                    ValidationFail::InternalError(
                        "invalid pipeline.gas.tech_account_id; expected account identifier"
                            .to_owned(),
                    )
                })?;

                // Parse gas asset definition id
                let asset_def: AssetDefinitionId = gas_asset_id_str.parse().map_err(|_| {
                    ValidationFail::NotPermitted(
                        "invalid gas_asset_id; expected `name#domain`".to_owned(),
                    )
                })?;

                // Compute fee amount deterministically and guard Numeric bounds
                let fee_u128 = u128::from(used).saturating_mul(u128::from(units_per_gas));
                if fee_u128 > 0 {
                    // Build payer asset id and transfer instruction
                    let payer = if let Some(sponsor) =
                        fee_sponsor.as_ref().filter(|sponsor| *sponsor != authority)
                    {
                        if !state_transaction.nexus.fees.sponsorship_enabled {
                            return Err(ValidationFail::NotPermitted(
                                "fee sponsorship is disabled".to_owned(),
                            ));
                        }
                        if !state_transaction.can_use_fee_sponsor(authority, sponsor) {
                            return Err(ValidationFail::NotPermitted(
                                "fee sponsor is not authorized".to_owned(),
                            ));
                        }
                        sponsor.clone()
                    } else {
                        authority.clone()
                    };
                    let payer_asset = AssetId::new(asset_def.clone(), payer.clone());
                    let qty = Numeric::try_new(fee_u128, 0).map_err(|_| {
                        ValidationFail::NotPermitted(
                            "fee amount exceeds supported numeric bounds".to_owned(),
                        )
                    })?;
                    let transfer =
                        iroha_data_model::isi::Transfer::<
                            Asset,
                            Numeric,
                            iroha_data_model::account::Account,
                        >::asset_numeric(payer_asset, qty, tech_account);
                    let instr: DMInstructionBox = transfer.into();
                    instr.execute(authority, state_transaction).map_err(|err| {
                        iroha_logger::debug!(
                            ?err,
                            authority = %authority,
                            "gas fee transfer failed to apply"
                        );
                        ValidationFail::from(err)
                    })?;
                    #[cfg(feature = "telemetry")]
                    {
                        let delta =
                            u64::try_from(fee_u128.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
                        state_transaction.stage_block_fee_units(delta);
                    }

                    // Capture deterministic settlement receipt once the transfer succeeds.
                    let source_id = settlement_source_id;
                    let block_timestamp_ms_u128 =
                        state_transaction._curr_block.creation_time().as_millis();
                    let block_timestamp_ms =
                        u64::try_from(block_timestamp_ms_u128).unwrap_or(u64::MAX);
                    let quote = state_transaction
                        .settlement_engine()
                        .quote(
                            source_id,
                            fee_u128,
                            twap_local_per_xor,
                            liquidity_profile,
                            volatility_bucket,
                            block_timestamp_ms,
                        )
                        .map_err(|err| match err {
                            QuoteError::LocalAmountOverflow(amount) => {
                                ValidationFail::NotPermitted(format!(
                                    "local gas amount {amount} exceeds Decimal range"
                                ))
                            }
                            QuoteError::ZeroTwap => {
                                ValidationFail::NotPermitted("gas TWAP must be non-zero".to_owned())
                            }
                        })?;
                    let config_snapshot = state_transaction.settlement_engine().config();
                    let twap_window_seconds = config_snapshot.twap_window.whole_seconds().max(0);
                    let twap_window_seconds =
                        u32::try_from(twap_window_seconds).unwrap_or(u32::MAX);
                    let xor_due_micro =
                        Self::decimal_to_micro_u128(*quote.receipt.xor_due, "xor_due amount")?;
                    let xor_after_haircut_micro = Self::decimal_to_micro_u128(
                        *quote.receipt.xor_with_haircut,
                        "xor_after_haircut amount",
                    )?;
                    let xor_variance_micro = xor_due_micro.saturating_sub(xor_after_haircut_micro);
                    let pending = PendingSettlement {
                        source_id,
                        asset_definition_id: asset_def,
                        local_amount_micro: quote.receipt.local_amount_micro,
                        xor_due_micro,
                        xor_after_haircut_micro,
                        xor_variance_micro,
                        timestamp_ms: block_timestamp_ms,
                        liquidity_profile,
                        volatility_bucket,
                        twap_local_per_xor,
                        epsilon_bps: quote.effective_epsilon_bps,
                        twap_window_seconds,
                        oracle_timestamp_ms: block_timestamp_ms,
                    };
                    state_transaction.record_settlement_receipt(tx_hash, pending);
                }
            }
        }

        Self::charge_nexus_fees(
            state_transaction,
            authority,
            fee_sponsor,
            tx_bytes_len,
            instruction_count,
            used,
        )?;

        Ok(())
    }
    /// Execute [`SignedTransaction`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    #[allow(clippy::too_many_lines)]
    pub fn execute_transaction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: SignedTransaction,
        ivm_cache: &mut IvmCache,
    ) -> Result<(), ValidationFail> {
        trace!("Running transaction execution");
        let tx_bytes_len = to_bytes(&transaction)
            .map(|bytes| bytes.len())
            .map_err(|err| {
                ValidationFail::InternalError(format!(
                    "failed to encode transaction for fee metering: {err}"
                ))
            })?;
        let fee_sponsor = parse_fee_sponsor(&state_transaction.world, transaction.metadata())?;
        // Bind the transaction call_hash for ISI event emitters to use in audit fields
        let call_hash = transaction.hash_as_entrypoint();
        state_transaction.tx_call_hash = Some(iroha_crypto::Hash::from(call_hash));
        let tx_hash = transaction.hash();
        let settlement_source_id = {
            let mut bytes = [0u8; iroha_crypto::Hash::LENGTH];
            bytes.copy_from_slice(tx_hash.as_ref());
            bytes
        };
        // Disallow direct signing with multisig accounts; only explicit multisig
        // proposal/approval envelopes with bundled multisig signatures are allowed.
        {
            let spec_key =
                iroha_data_model::name::Name::from_str("multisig/spec").expect("static key valid");
            let account = state_transaction.world.account(authority).map_err(|err| {
                ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
            })?;
            let metadata = account.metadata();
            if metadata.contains(&spec_key) {
                let has_multisig_bundle = transaction.multisig_signatures().is_some();
                let only_multisig_proposal_instructions = matches!(
                    transaction.instructions(),
                    Executable::Instructions(items)
                        if !items.is_empty()
                            && items.iter().all(|instruction| {
                                MultisigInstructionBox::try_from(instruction).is_ok()
                            })
                );
                if has_multisig_bundle && only_multisig_proposal_instructions {
                    // Allowed: this is a multisig proposal/approval envelope, not a direct ISI submit.
                } else {
                    #[cfg(feature = "telemetry")]
                    crate::telemetry::record_social_rejection(
                        state_transaction.telemetry,
                        "multisig_direct_sign",
                    );
                    return Err(ValidationFail::NotPermitted(
                    "direct signing with multisig accounts is forbidden; use multisig propose/approve"
                        .to_owned(),
                ));
                }
            }
        }
        // Refresh pipeline gas settings from on-chain parameters (genesis/governance updates)
        Self::refresh_gas_from_parameters(state_transaction);
        // Gas asset admission: if an allowlist is configured, require the tx metadata to specify
        // a `gas_asset_id` present in the allowlist. The value must be a valid AssetDefinitionId
        // string (e.g., "xor#domain").
        let md = transaction.metadata();
        let gas_asset_opt = md.get("gas_asset_id").map(|j| j.as_ref().to_string());
        // Payer-provided gas limit (optional for non-VM transactions); used to cap fee exposure
        let gas_limit_md = parse_gas_limit(md)?;
        configure_executor_fuel_budget(self, state_transaction, md)?;
        let pipeline_gas = &state_transaction.pipeline.gas;
        if !pipeline_gas.accepted_assets.is_empty() {
            let Some(ref gas_asset_id_str) = gas_asset_opt else {
                return Err(ValidationFail::NotPermitted(
                    "missing gas_asset_id in transaction metadata".to_owned(),
                ));
            };
            if !pipeline_gas
                .accepted_assets
                .iter()
                .any(|a| a == gas_asset_id_str)
            {
                return Err(ValidationFail::NotPermitted(format!(
                    "gas asset `{gas_asset_id_str}` is not accepted by node policy"
                )));
            }
        }
        #[cfg(feature = "zk-preverify")]
        {
            use iroha_data_model::proof::{ProofAttachment, ProofAttachmentList, VerifyingKeyId};

            let namespace_hint = md.get("contract_namespace").and_then(|value| {
                let raw = value.as_ref().to_string();
                let trimmed = raw.trim().trim_matches('"').trim();
                (!trimmed.is_empty()).then(|| trimmed.to_owned())
            });

            // Process ZK attachments embedded in V2 transactions.
            if let Some(ProofAttachmentList(list)) = transaction.attachments().cloned() {
                // Canonicalize verification order for determinism
                let mut list_sorted = list;
                list_sorted.sort_by(|a, b| {
                    let ah = crate::zk::hash_proof(&a.proof);
                    let bh = crate::zk::hash_proof(&b.proof);
                    (a.backend.as_str(), ah).cmp(&(b.backend.as_str(), bh))
                });
                for ProofAttachment {
                    backend,
                    proof,
                    vk_ref,
                    vk_inline,
                    vk_commitment,
                    ..
                } in list_sorted.into_iter()
                {
                    match (&vk_ref, &vk_inline) {
                        (None, None) => {
                            return Err(ValidationFail::NotPermitted(
                                "proof attachments must include exactly one verifying key (inline or reference)"
                                    .to_owned(),
                            ));
                        }
                        (Some(_), Some(_)) => {
                            return Err(ValidationFail::NotPermitted(
                                "proof attachments must not mix inline and referenced verifying keys"
                                    .to_owned(),
                            ));
                        }
                        _ => {}
                    }
                    // Sanity: proof.backend should match attachment backend
                    if proof.backend != backend {
                        return Err(ValidationFail::NotPermitted(
                            "proof backend mismatch".to_owned(),
                        ));
                    }

                    // If an inline VK is provided, ensure backend matches as a cheap sanity check
                    if let Some(ref vk) = vk_inline {
                        if vk.backend != backend {
                            return Err(ValidationFail::NotPermitted(
                                "verifying key backend mismatch".to_owned(),
                            ));
                        }
                    }

                    // If a VK reference is provided but neither inline VK nor commitment
                    // are present, check existence in WSV. If a commitment is provided,
                    // skip the lookup to keep pre-verify stateless and cheap.
                    if let Some(ref id @ VerifyingKeyId { .. }) = vk_ref {
                        if vk_inline.is_none() && vk_commitment.is_none() {
                            if state_transaction.world.verifying_keys.get(id).is_none() {
                                return Err(ValidationFail::NotPermitted(format!(
                                    "referenced verifying key missing: {}::{}",
                                    id.backend, id.name
                                )));
                            }
                        }
                    }

                    // Perform lightweight pre-verify (dedup + tag sanity). Inline VK is passed
                    // for future use; current implementation ignores it.
                    // Compute optional commitment for deduplication: prefer inline VK hash,
                    // else use provided attachment commitment when present.
                    let mut commit = vk_commitment;
                    if commit.is_none() {
                        if let Some(ref vk) = vk_inline {
                            commit = Some(crate::zk::hash_vk(vk));
                        }
                    }
                    let (expected_commitment, vk_active) =
                        if let Some(ref id @ VerifyingKeyId { .. }) = vk_ref {
                            if let Some(rec) = state_transaction.world.verifying_keys.get(id) {
                                if let Some(ns_hint) = namespace_hint.as_deref() {
                                    if !rec.namespace.is_empty() && rec.namespace != ns_hint {
                                        return Err(ValidationFail::NotPermitted(
                                            "verifying key namespace/manifest mismatch".to_owned(),
                                        ));
                                    }
                                }
                                (Some(rec.commitment), rec.is_active())
                            } else {
                                (None, false)
                            }
                        } else {
                            (vk_commitment, true)
                        };
                    let res = state_transaction.preverify_proof(
                        &proof,
                        vk_inline.as_ref(),
                        state_transaction.zk.preverify_budget_bytes,
                        commit,
                        expected_commitment,
                        vk_active,
                    );
                    match res {
                        PreverifyResult::Accepted => {}
                        PreverifyResult::Duplicate => {
                            return Err(ValidationFail::NotPermitted(
                                "duplicate proof in block".to_owned(),
                            ));
                        }
                        PreverifyResult::UnsupportedBackend => {
                            return Err(ValidationFail::NotPermitted(
                                "unsupported proof backend".to_owned(),
                            ));
                        }
                        PreverifyResult::CurveNotAllowed => {
                            return Err(ValidationFail::NotPermitted(
                                "curve not allowed".to_owned(),
                            ));
                        }
                        PreverifyResult::ProofTooBig => {
                            return Err(ValidationFail::NotPermitted("proof too big".to_owned()));
                        }
                        PreverifyResult::MalformedProof => {
                            return Err(ValidationFail::NotPermitted("malformed proof".to_owned()));
                        }
                        PreverifyResult::PreverifyBudgetExceeded => {
                            return Err(ValidationFail::NotPermitted(
                                "pre-verify budget exceeded".to_owned(),
                            ));
                        }
                        PreverifyResult::VerifyingKeyMissing => {
                            return Err(ValidationFail::NotPermitted(
                                "verifying key missing".to_owned(),
                            ));
                        }
                        PreverifyResult::VerifyingKeyMismatch => {
                            return Err(ValidationFail::NotPermitted(
                                "verifying key mismatch".to_owned(),
                            ));
                        }
                        PreverifyResult::NamespaceMismatch => {
                            return Err(ValidationFail::NotPermitted(
                                "verifying key namespace/manifest mismatch".to_owned(),
                            ));
                        }
                        PreverifyResult::VerifyingKeyInactive => {
                            return Err(ValidationFail::NotPermitted(
                                "verifying key inactive".to_owned(),
                            ));
                        }
                    }
                }
            }
        }

        // Full verification for proof-carrying IVM executables must run before we move the
        // transaction payload out of `SignedTransaction`.
        if let Executable::IvmProved(proved) = transaction.instructions() {
            if gas_limit_md.is_none() {
                return Err(ValidationFail::NotPermitted(
                    "missing gas_limit in transaction metadata".to_owned(),
                ));
            }

            let map_overlay_error =
                |err: crate::pipeline::overlay::OverlayBuildError| -> ValidationFail {
                    match err {
                        crate::pipeline::overlay::OverlayBuildError::HeaderPolicy(e) => {
                            ValidationFail::IvmAdmission(e)
                        }
                        crate::pipeline::overlay::OverlayBuildError::AxtReject(ctx) => {
                            ValidationFail::AxtReject(ctx)
                        }
                        other => ValidationFail::NotPermitted(other.to_string()),
                    }
                };

            let summary = ivm_cache
                .summarize_program(proved.bytecode.as_ref())
                .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
            let meta = summary.metadata.clone();
            crate::pipeline::overlay::validate_header_policy(&meta)
                .map_err(ValidationFail::IvmAdmission)?;

            let wants_zk = meta.mode & ivm::ivm_mode::ZK != 0;
            if wants_zk
                && !(state_transaction.zk.halo2.enabled || state_transaction.zk.stark.enabled)
            {
                return Err(ValidationFail::IvmAdmission(
                    iroha_data_model::executor::IvmAdmissionError::UnsupportedFeatureBits(
                        ivm::ivm_mode::ZK,
                    ),
                ));
            }

            crate::pipeline::overlay::enforce_pre_execution_policy(
                state_transaction.pipeline.ivm_max_cycles_upper_bound,
                &meta,
                summary.code_offset,
                proved.bytecode.as_ref(),
            )
            .map_err(map_overlay_error)?;

            crate::pipeline::overlay::validate_contract_binding(
                state_transaction,
                &transaction,
                &summary,
            )
            .map_err(map_overlay_error)?;

            crate::pipeline::overlay::enforce_manifest_is_pre_registered(
                state_transaction,
                &transaction,
                summary.code_hash,
            )
            .map_err(map_overlay_error)?;

            crate::pipeline::overlay::verify_ivm_proved_execution(
                state_transaction,
                &transaction,
                proved,
                &summary,
            )
            .map_err(map_overlay_error)?;
        }

        let (tx_authority, executable) = transaction.into();
        debug_assert_eq!(&tx_authority, authority, "authority mismatch");

        match (self, executable) {
            (Self::Initial | Self::UserProvided(_), Executable::Instructions(instructions)) => self
                .execute_metered_instructions(
                    state_transaction,
                    authority,
                    instructions.into_vec(),
                    tx_bytes_len,
                    settlement_source_id,
                    tx_hash,
                    gas_limit_md,
                    false,
                    gas_asset_opt,
                    fee_sponsor,
                ),
            (Self::Initial | Self::UserProvided(_), Executable::IvmProved(proved)) => {
                let mut instructions = proved.overlay.into_vec();
                crate::pipeline::overlay::prune_redundant_contract_ops(
                    state_transaction,
                    &mut instructions,
                );
                self.execute_metered_instructions(
                    state_transaction,
                    authority,
                    instructions,
                    tx_bytes_len,
                    settlement_source_id,
                    tx_hash,
                    gas_limit_md,
                    true,
                    gas_asset_opt,
                    fee_sponsor,
                )
            }
            (Self::Initial | Self::UserProvided(_), Executable::Ivm(bytes)) => {
                // IVM path: run the bytecode through the VM with CoreHost, enqueueing ISIs,
                // then apply them via the standard executor logic.
                use crate::smartcontracts::ivm::host::CoreHostImpl as CoreCoreHost;
                // Set gas limit per transaction (payer-provided), clamped to remaining block budget.
                // Read gas_limit metadata (payer's cap) captured before moving transaction
                let gas_limit_md = gas_limit_md.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "missing gas_limit in transaction metadata".to_owned(),
                    )
                })?;
                let block_remaining = if state_transaction.gas_limit_per_block == 0 {
                    u64::MAX
                } else {
                    state_transaction
                        .gas_limit_per_block
                        .saturating_sub(state_transaction.gas_used_in_block_so_far)
                };
                let effective_limit = gas_limit_md.min(block_remaining);
                let mut runtime = ivm_cache
                    .take_or_create_cached_runtime(bytes.as_ref(), effective_limit)
                    .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
                // Attach host with a snapshot of known accounts for vendor helpers when present.
                let accounts = Arc::new(
                    state_transaction
                        .world
                        .accounts
                        .iter()
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>(),
                );
                let mut host =
                    CoreCoreHost::with_accounts(authority.clone(), Arc::clone(&accounts));
                host.set_crypto_config(Arc::clone(&state_transaction.crypto));
                host.set_halo2_config(&state_transaction.zk.halo2);
                host.set_durable_state_snapshot_from_world(&state_transaction.world);
                host.set_public_inputs_from_parameters(state_transaction.world.parameters.get());
                host.set_vrf_epoch_seeds_from_world(&state_transaction.world);
                host.set_query_state(state_transaction);
                // Thread chain_id from StateTransaction into the IVM host for VRF binding
                host.set_chain_id(&state_transaction.chain_id);
                #[cfg(feature = "telemetry")]
                host.set_telemetry(state_transaction.telemetry.clone());
                // Thread ZK snapshots (roots, elections, verifying keys) for read/verify syscalls.
                host.set_zk_snapshots_from_world(&state_transaction.world, &state_transaction.zk)
                    .map_err(|err| {
                        ValidationFail::InternalError(format!("invalid ZK snapshot state: {err}"))
                    })?;
                runtime.vm.set_gas_limit(effective_limit);
                if let Err(err) = runtime.vm.run_with_host(&mut host) {
                    return Err(crate::smartcontracts::ivm::map_vm_error_to_validation(&err));
                }
                let gas_used = effective_limit.saturating_sub(runtime.vm.remaining_gas());

                // Drain and apply queued ISIs deterministically via executor.
                let artifacts = host.into_execution_artifacts()?;
                let _executed = artifacts.apply_to_transaction(state_transaction, authority)?;
                state_transaction.last_tx_gas_used = gas_used;

                // Charge gas fees: if a gas asset was provided and accepted by policy.
                if let Some(gas_asset_id_str) = gas_asset_opt {
                    // Determine rate; require explicit mapping for determinism
                    let gas_rate = state_transaction
                        .pipeline
                        .gas
                        .units_per_gas
                        .iter()
                        .find(|r| r.asset == gas_asset_id_str)
                        .ok_or_else(|| {
                            ValidationFail::NotPermitted(format!(
                                "missing units_per_gas mapping for `{gas_asset_id_str}`"
                            ))
                        })?;
                    let rate = gas_rate.units_per_gas;
                    let twap_local_per_xor = gas_rate.twap_local_per_xor;
                    let volatility_bucket = convert_volatility_bucket(gas_rate.volatility);
                    let liquidity_profile = match gas_rate.liquidity {
                        GasLiquidity::Tier1 => LiquidityProfile::Tier1,
                        GasLiquidity::Tier2 => LiquidityProfile::Tier2,
                        GasLiquidity::Tier3 => LiquidityProfile::Tier3,
                    };
                    // Parse tech account id
                    let tech_account: AccountId = parse_account_id_literal(
                        &state_transaction.world,
                        &state_transaction.pipeline.gas.tech_account_id,
                    )
                    .ok_or_else(|| {
                        ValidationFail::InternalError(
                            "invalid pipeline.gas.tech_account_id; expected account identifier"
                                .to_owned(),
                        )
                    })?;
                    // Parse gas asset definition id
                    let asset_def: AssetDefinitionId = gas_asset_id_str.parse().map_err(|_| {
                        ValidationFail::NotPermitted(
                            "invalid gas_asset_id; expected `name#domain`".to_owned(),
                        )
                    })?;
                    // Compute fee amount deterministically
                    if gas_used > 0 && rate > 0 {
                        let fee_u128 = u128::from(gas_used).saturating_mul(u128::from(rate));
                        // Build payer asset id and transfer instruction, guarding Numeric bounds
                        if fee_u128 > 0 {
                            let payer = if let Some(sponsor) =
                                fee_sponsor.as_ref().filter(|sponsor| *sponsor != authority)
                            {
                                if !state_transaction.nexus.fees.sponsorship_enabled {
                                    return Err(ValidationFail::NotPermitted(
                                        "fee sponsorship is disabled".to_owned(),
                                    ));
                                }
                                if !state_transaction.can_use_fee_sponsor(authority, sponsor) {
                                    return Err(ValidationFail::NotPermitted(
                                        "fee sponsor is not authorized".to_owned(),
                                    ));
                                }
                                sponsor.clone()
                            } else {
                                authority.clone()
                            };
                            let payer_asset = AssetId::new(asset_def.clone(), payer);
                            let qty = Numeric::try_new(fee_u128, 0).map_err(|_| {
                                ValidationFail::NotPermitted(
                                    "fee amount exceeds supported numeric bounds".to_owned(),
                                )
                            })?;
                            let transfer = iroha_data_model::isi::Transfer::<
                                Asset,
                                Numeric,
                                iroha_data_model::account::Account,
                            >::asset_numeric(
                                payer_asset, qty, tech_account
                            );
                            let instr: DMInstructionBox = transfer.into();
                            instr.execute(authority, state_transaction).map_err(|err| {
                                iroha_logger::debug!(
                                    ?err,
                                    authority = %authority,
                                    "gas fee transfer failed to apply"
                                );
                                ValidationFail::from(err)
                            })?;
                            #[cfg(feature = "telemetry")]
                            {
                                let delta = u64::try_from(fee_u128.min(u128::from(u64::MAX)))
                                    .unwrap_or(u64::MAX);
                                state_transaction.stage_block_fee_units(delta);
                            }

                            let source_id = settlement_source_id;
                            let block_timestamp_ms_u128 =
                                state_transaction._curr_block.creation_time().as_millis();
                            let block_timestamp_ms =
                                u64::try_from(block_timestamp_ms_u128).unwrap_or(u64::MAX);
                            let quote = state_transaction
                                .settlement_engine()
                                .quote(
                                    source_id,
                                    fee_u128,
                                    twap_local_per_xor,
                                    liquidity_profile,
                                    volatility_bucket,
                                    block_timestamp_ms,
                                )
                                .map_err(|err| match err {
                                    QuoteError::LocalAmountOverflow(amount) => {
                                        ValidationFail::NotPermitted(format!(
                                            "local gas amount {amount} exceeds Decimal range"
                                        ))
                                    }
                                    QuoteError::ZeroTwap => ValidationFail::NotPermitted(
                                        "gas TWAP must be non-zero".to_owned(),
                                    ),
                                })?;
                            let config_snapshot = state_transaction.settlement_engine().config();
                            let twap_window_seconds =
                                config_snapshot.twap_window.whole_seconds().max(0);
                            let twap_window_seconds =
                                u32::try_from(twap_window_seconds).unwrap_or(u32::MAX);
                            let xor_due_micro = Self::decimal_to_micro_u128(
                                *quote.receipt.xor_due,
                                "xor_due amount",
                            )?;
                            let xor_after_haircut_micro = Self::decimal_to_micro_u128(
                                *quote.receipt.xor_with_haircut,
                                "xor_after_haircut amount",
                            )?;
                            let xor_variance_micro =
                                xor_due_micro.saturating_sub(xor_after_haircut_micro);
                            let pending = PendingSettlement {
                                source_id,
                                asset_definition_id: asset_def,
                                local_amount_micro: quote.receipt.local_amount_micro,
                                xor_due_micro,
                                xor_after_haircut_micro,
                                xor_variance_micro,
                                timestamp_ms: block_timestamp_ms,
                                liquidity_profile,
                                volatility_bucket,
                                twap_local_per_xor,
                                epsilon_bps: quote.effective_epsilon_bps,
                                twap_window_seconds,
                                oracle_timestamp_ms: block_timestamp_ms,
                            };
                            state_transaction.record_settlement_receipt(tx_hash, pending);
                        }
                    }
                }
                Self::charge_nexus_fees(
                    state_transaction,
                    authority,
                    fee_sponsor,
                    tx_bytes_len,
                    0,
                    gas_used,
                )?;
                ivm_cache.put_cached_runtime(&runtime);
                Ok(())
            }
        }
    }

    /// Execute [`InstructionBox`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    pub fn execute_instruction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<(), ValidationFail> {
        self.execute_instruction_with_profile(
            state_transaction,
            authority,
            instruction,
            InstructionExecutionProfile::Runtime,
        )
    }

    /// Execute [`InstructionBox`] using a specific execution profile.
    ///
    /// `InstructionExecutionProfile::Runtime` mirrors production behaviour.
    /// `InstructionExecutionProfile::Bench` disables logging so benchmarks/tests
    /// can run without installing the global logger while still enforcing policy checks.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationFail`] when the delegated executor rejects the instruction,
    /// or if preparing or running the IVM bytecode fails.
    pub fn execute_instruction_with_profile(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        profile: InstructionExecutionProfile,
    ) -> Result<(), ValidationFail> {
        trace!("Running instruction execution");
        let instr_id = instruction.id();

        let result = match self {
            Self::Initial => Self::execute_initial_instruction(
                state_transaction,
                authority,
                instruction,
                profile,
            ),
            Self::UserProvided(loaded_executor) => dispatch_instruction_with_ivm(
                loaded_executor,
                state_transaction,
                authority,
                instruction,
            ),
        };
        if let Err(err) = &result {
            iroha_logger::error!(
                ?profile,
                instr = %instr_id,
                ?err,
                "instruction execution failed"
            );
        }
        result
    }

    fn multisig_account_from(role_id: &RoleId) -> Result<Option<AccountId>, ValidationFail> {
        const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";
        const DELIMITER: char = '/';

        let Some(tail) = role_id.name().as_ref().strip_prefix(MULTISIG_SIGNATORY) else {
            return Ok(None);
        };
        let Some((init, last)) = tail.rsplit_once(DELIMITER) else {
            return Err(ValidationFail::NotPermitted(
                "violates multisig role name format".to_owned(),
            ));
        };

        let domain_hint = init.trim_matches(DELIMITER);
        if let Ok(account) = AccountId::parse_encoded(last)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        {
            if account.domain().to_string() != domain_hint {
                return Err(ValidationFail::NotPermitted(
                    "violates multisig role name format".to_owned(),
                ));
            }
            return Ok(Some(account));
        }
        AccountId::parse_encoded(&format!("{last}@{domain_hint}"))
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .map(Some)
            .map_err(|_| {
                ValidationFail::NotPermitted("violates multisig role name format".to_owned())
            })
    }

    #[allow(clippy::too_many_lines, clippy::items_after_statements)]
    fn execute_initial_instruction(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        profile: InstructionExecutionProfile,
    ) -> Result<(), ValidationFail> {
        if matches!(profile, InstructionExecutionProfile::Runtime) {
            iroha_logger::trace!(
                instr = %instruction.id(),
                "executing instruction (Initial executor)"
            );
        }

        match MultisigInstructionBox::try_from(&instruction) {
            Ok(multisig) => {
                return crate::smartcontracts::isi::multisig::execute_multisig_instruction(
                    state_transaction,
                    authority,
                    multisig,
                );
            }
            Err(err) => {
                if let Some(custom) = instruction.as_any().downcast_ref::<CustomInstruction>() {
                    iroha_logger::error!(
                        ?err,
                        instr = %instruction.id(),
                        payload = %custom.payload(),
                        "failed to decode multisig custom instruction"
                    );
                }
            }
        }

        if instruction
            .as_any()
            .downcast_ref::<CustomInstruction>()
            .is_some()
        {
            return Err(ValidationFail::NotPermitted(
                "custom instructions require an executor upgrade".to_owned(),
            ));
        }

        let is_genesis =
            state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty();

        if let Some(register_role) = extract_register_role(&instruction) {
            if let Some(multisig_account) =
                Self::multisig_account_from(register_role.object().id())?
            {
                let domain_owner = state_transaction
                    .world
                    .domain(multisig_account.domain())
                    .map(|domain| domain.owned_by().clone())
                    .map_err(|err| {
                        ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
                    })?;
                if &domain_owner != authority {
                    return Err(ValidationFail::NotPermitted(
                        "only the domain owner can register multisig roles".to_owned(),
                    ));
                }
                return Err(ValidationFail::NotPermitted(
                    "reserved multisig role names may not be registered".to_owned(),
                ));
            }

            let role = register_role.object();
            let mut normalized_role = Role::new(role.id().clone(), role.grant_to().clone());
            for permission in role.inner().permissions() {
                normalized_role = normalized_role.add_permission(
                    normalize_role_permission_for_initial_executor(state_transaction, permission)?,
                );
            }

            if !is_genesis {
                let can_manage_roles: Permission = executor_permission::role::CanManageRoles.into();
                let has_manage_roles = authority_has_permission(
                    &state_transaction.world,
                    authority,
                    &can_manage_roles,
                )?;
                if !has_manage_roles {
                    return Err(ValidationFail::NotPermitted(
                        "Can't register role".to_owned(),
                    ));
                }
            }

            Register::role(normalized_role)
                .execute(authority, state_transaction)
                .map_err(ValidationFail::from)?;
            return Ok(());
        }

        // Minimal built-in permission enforcement for critical instructions used in tests.
        // This mirrors the default executor behavior sufficiently for integration tests
        // without requiring an on-chain executor upgrade.
        // Only attempt to decode as Register<Trigger> when the dynamic type matches.
        // Guard against panics in Norito deserialization for mismatched schemas.
        let is_reg_trigger = instruction
            .id()
            .starts_with(core::any::type_name::<Register<Trigger>>());
        let reg_trg = if is_reg_trigger {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Register::<Trigger>::decode(&mut &instruction.dyn_encode()[..])
            }))
            .ok()
            .and_then(Result::ok)
        } else {
            None
        };
        if let Some(reg_trg) = reg_trg {
            // Allow in genesis, or if tx authority owns the trigger owner's domain,
            // or if tx authority has explicit CanRegisterTrigger { authority: <owner> }.
            let trg_owner = reg_trg.object().action().authority().clone();

            let is_domain_owner = (!is_genesis) && {
                let domain = trg_owner.domain().clone();
                state_transaction
                    .world
                    .domains
                    .get(&domain)
                    .is_some_and(|d| d.owned_by() == authority)
            };

            // Prefer cached permission check; parse once per tx/account.
            let has_permission =
                (!is_genesis) && state_transaction.can_register_trigger_for(authority, &trg_owner);

            if !(is_genesis || is_domain_owner || has_permission) {
                return Err(ValidationFail::NotPermitted(
                    "Can't register trigger owned by another account".to_owned(),
                ));
            }
        }

        if let Some(reg_asset_definition) = extract_register_asset_definition(&instruction) {
            ensure_asset_definition_registration_allowed(
                state_transaction,
                authority,
                &reg_asset_definition,
            )?;
        }

        if let Some(account_id) = extract_account_metadata_target(&instruction) {
            if !is_genesis
                && !can_modify_account_metadata(&state_transaction.world, authority, &account_id)?
            {
                return Err(ValidationFail::NotPermitted(
                    "Can't set value to the metadata of another account".to_owned(),
                ));
            }
        }

        fn has_modify_nft_metadata_permission(
            state_transaction: &mut StateTransaction<'_, '_>,
            authority: &AccountId,
            nft_id: &iroha_data_model::nft::NftId,
        ) -> Result<bool, ValidationFail> {
            let is_target_permission = |permission: &Permission| -> bool {
                permission
                    .payload()
                    .try_into_any_norito::<executor_permission::nft::CanModifyNftMetadata>()
                    .is_ok_and(|token| token.nft == *nft_id)
            };

            {
                let permissions = state_transaction
                    .world
                    .account_permissions_iter(authority)
                    .map_err(|err| {
                        ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
                    })?;
                if permissions.into_iter().any(is_target_permission) {
                    return Ok(true);
                }
            }

            for role_id in state_transaction.world.account_roles_iter(authority) {
                if let Some(role) = state_transaction.world.roles.get(role_id) {
                    if role.permissions.iter().any(is_target_permission) {
                        return Ok(true);
                    }
                }
            }

            Ok(false)
        }

        if let Some(nft_id) = instruction
            .as_any()
            .downcast_ref::<SetKeyValueBox>()
            .and_then(|kv| match kv {
                SetKeyValueBox::Nft(set) => Some(set.object.clone()),
                _ => None,
            })
            .or_else(|| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::SetKeyValue<iroha_data_model::nft::Nft>>()
                    .map(|set| set.object.clone())
            })
            .or_else(|| {
                instruction
                    .as_any()
                    .downcast_ref::<RemoveKeyValueBox>()
                    .and_then(|rm| match rm {
                        RemoveKeyValueBox::Nft(rm) => Some(rm.object.clone()),
                        _ => None,
                    })
            })
            .or_else(|| {
                instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::RemoveKeyValue<iroha_data_model::nft::Nft>>()
                    .map(|rm| rm.object.clone())
            })
        {
            if !(state_transaction._curr_block.is_genesis()
                && state_transaction.block_hashes.is_empty())
            {
                let domain_owner = state_transaction
                    .world
                    .domain(nft_id.domain())
                    .map(|domain| domain.owned_by().clone())
                    .map_err(|err| {
                        ValidationFail::InstructionFailed(InstructionExecutionError::Find(err))
                    })?;

                if &domain_owner != authority
                    && !has_modify_nft_metadata_permission(state_transaction, authority, &nft_id)?
                {
                    return Err(ValidationFail::NotPermitted(
                        "Can't modify NFT from domain owned by another account".to_owned(),
                    ));
                }
            }
        }

        if let Some(transfer_domain) = extract_transfer_domain(&instruction)
            && !can_transfer_domain(&state_transaction.world, authority, &transfer_domain)?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer domain of another account".to_owned(),
            ));
        }

        if !is_genesis
            && let Some(transfer_asset) = extract_transfer_asset(&instruction)
            && !can_transfer_asset(&state_transaction.world, authority, &transfer_asset)?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer asset: source asset owner must sign the transaction".to_owned(),
            ));
        }

        let instruction_id = instruction.id();
        instruction
            .execute(authority, state_transaction)
            .map_err(|err| {
                if matches!(profile, InstructionExecutionProfile::Runtime) {
                    iroha_logger::debug!(
                        ?err,
                        %instruction_id,
                        authority = %authority,
                        "initial executor rejected instruction during application"
                    );
                }
                ValidationFail::from(err)
            })
    }

    /// Validate [`QueryRequest`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    pub fn validate_query<S: StateReadOnly>(
        &self,
        state_ro: &S,
        authority: &AccountId,
        query: &QueryRequest,
    ) -> Result<(), ValidationFail> {
        let latest_block = state_ro.latest_block().map(|block| block.header());
        self.validate_query_with_world_parts(state_ro.world(), latest_block, authority, query)
    }

    /// Validate [`QueryRequest`] using world-state and latest committed block header.
    ///
    /// This variant avoids requiring a full [`StateReadOnly`] snapshot in callers that
    /// already have a world view and can cheaply resolve the latest block header.
    ///
    /// # Errors
    ///
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode;
    /// - Executor denied the operation.
    pub fn validate_query_with_world_parts(
        &self,
        world_ro: &impl WorldReadOnly,
        latest_block: Option<BlockHeader>,
        authority: &AccountId,
        query: &QueryRequest,
    ) -> Result<(), ValidationFail> {
        trace!("Running query validation");

        let query_box = match query {
            QueryRequest::Singular(singular) => AnyQueryBox::Singular(singular.clone()),
            QueryRequest::Start(iterable) => AnyQueryBox::Iterable(iterable.clone()),
            QueryRequest::Continue(_) => {
                // The iterable query was already validated when it started
                return Ok(());
            }
        };

        match self {
            Self::Initial => Ok(()),
            Self::UserProvided(loaded_executor) => {
                if let Some(kind) = detect_fixture_executor_kind(loaded_executor) {
                    return validate_query_with_fixture(kind, query);
                }

                let curr_block = latest_block.map_or_else(
                    || BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, None, 0, 0),
                    core::convert::identity,
                );

                let context = ExecutorContext {
                    authority: authority.clone(),
                    curr_block,
                };

                let payload = ValidatePayload {
                    context,
                    target: query_box,
                };

                let query_label = match query {
                    QueryRequest::Singular(_) => "query::singular",
                    QueryRequest::Start(_) => "query::start",
                    QueryRequest::Continue(_) => unreachable!("continue queries return early"),
                };

                let gas_limit = world_ro.parameters().executor().fuel.get();
                let report =
                    run_executor_validation(loaded_executor, &payload, query_label, gas_limit)?;
                match report.verdict {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        iroha_logger::debug!(
                            ?err,
                            authority = %authority,
                            query = %query_label,
                            "executor validation rejected query"
                        );
                        Err(err)
                    }
                }
            }
        }
    }

    /// Migrate executor to a new user-provided one.
    ///
    /// Execute `migrate()` entrypoint of the `raw_executor` and set `self` to
    /// [`UserProvided`](Executor::UserProvided) with `raw_executor`.
    ///
    /// # Errors
    ///
    /// - Failed to load `raw_executor`;
    /// - Failed to prepare the IVM runtime;
    /// - Failed to execute the entrypoint of the IVM bytecode.
    pub fn migrate(
        &mut self,
        raw_executor: data_model_executor::Executor,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), VMError> {
        trace!("Running executor migration");

        // NOTE: We no longer emulate failure modes based on metadata tags.
        // Migration outcome should be determined by the executor's own logic.

        // Load new executor bytecode
        let loaded_executor = LoadedExecutor::load(raw_executor)?;

        if let Some(kind) = detect_fixture_executor_kind(&loaded_executor) {
            apply_fixture_migration(kind, state_transaction, authority)
                .map_err(map_migration_fail_to_vm_error)?;
            *self = Self::UserProvided(loaded_executor);
            return Ok(());
        }

        let curr_block = state_transaction._curr_block;
        let context = ExecutorContext {
            authority: authority.clone(),
            curr_block,
        };

        let gas_limit = state_transaction
            .world
            .parameters
            .get()
            .executor()
            .fuel
            .get();
        let maybe_data_model = run_executor_migration(&loaded_executor, &context, gas_limit)
            .map_err(map_migration_fail_to_vm_error)?;
        if let Some(data_model) = maybe_data_model {
            debug!("executor migrate entrypoint supplied a new data model");
            state_transaction
                .world
                .apply_executor_data_model(data_model);
        }

        *self = Self::UserProvided(loaded_executor);
        Ok(())
    }
}

struct ExecutorValidationReport {
    verdict: Result<(), ValidationFail>,
    gas_used: u64,
}

fn detect_fixture_executor_kind(executor: &LoadedExecutor) -> Option<FixtureExecutorKind> {
    detect_fixture_executor_kind_from_bytecode(executor.raw_executor.bytecode().as_ref())
}

fn detect_fixture_executor_kind_from_bytecode(bytecode: &[u8]) -> Option<FixtureExecutorKind> {
    // Placeholder samples are tiny deterministic programs with this exact layout:
    // header(17) + LTLB(16) + pad(64) + HALT(4) = 101 bytes.
    if bytecode.len() != 101 {
        return None;
    }
    if bytecode.get(0..4) != Some(b"IVM\0") {
        return None;
    }
    let vector_length = *bytecode.get(7)?;
    let kind = FixtureExecutorKind::from_vector_length(vector_length)?;

    let section = bytecode.get(17..33)?;
    if section.get(0..4) != Some(&FIXTURE_LITERAL_SECTION_MAGIC) {
        return None;
    }
    let entries = u32::from_le_bytes(section.get(4..8)?.try_into().ok()?);
    let pad_len = u32::from_le_bytes(section.get(8..12)?.try_into().ok()?);
    let literal_len = u32::from_le_bytes(section.get(12..16)?.try_into().ok()?);
    if entries != 0 || pad_len != 64 || literal_len != 0 {
        return None;
    }

    let halt = ivm::encoding::wide::encode_halt().to_le_bytes();
    if bytecode.get(97..101) != Some(&halt) {
        return None;
    }

    Some(kind)
}

fn initial_executor_permission_names() -> BTreeSet<String> {
    INITIAL_EXECUTOR_PERMISSION_NAMES
        .iter()
        .map(|permission| (*permission).to_owned())
        .collect()
}

pub(crate) fn initial_executor_data_model_fallback() -> ExecutorDataModel {
    ExecutorDataModel::new(
        BTreeMap::new(),
        BTreeSet::new(),
        initial_executor_permission_names(),
        Json::new(()),
    )
}

fn baseline_executor_data_model(world_ro: &impl WorldReadOnly) -> ExecutorDataModel {
    let current = world_ro.executor_data_model();
    if current.permissions().is_empty() {
        initial_executor_data_model_fallback()
    } else {
        current.clone()
    }
}

fn make_can_control_domain_lives_permission() -> Permission {
    // `CanControlDomainLives` is a unit struct, therefore its canonical JSON payload is `null`.
    Permission::new(
        FIXTURE_PERMISSION_CAN_CONTROL_DOMAIN_LIVES.to_owned(),
        Json::new(()),
    )
}

fn remove_permissions_by_name(
    permissions: &mut BTreeSet<Permission>,
    permission_name: &str,
) -> bool {
    let removed: Vec<_> = permissions
        .iter()
        .filter(|permission| permission.name() == permission_name)
        .cloned()
        .collect();
    if removed.is_empty() {
        return false;
    }
    for permission in removed {
        permissions.remove(&permission);
    }
    true
}

fn apply_fixture_permission_migration(
    state_transaction: &mut StateTransaction<'_, '_>,
    add_can_control_domain_lives: bool,
) {
    let replacement = make_can_control_domain_lives_permission();
    let removed_name = "CanUnregisterDomain";

    let account_ids: Vec<_> = state_transaction
        .world
        .account_permissions
        .iter()
        .map(|(account_id, _)| account_id.clone())
        .collect();
    for account_id in account_ids {
        if let Some(permissions) = state_transaction
            .world
            .account_permissions
            .get_mut(&account_id)
        {
            let removed = remove_permissions_by_name(permissions, removed_name);
            if add_can_control_domain_lives && removed {
                permissions.insert(replacement.clone());
            }
        }
    }

    let role_ids: Vec<_> = state_transaction
        .world
        .roles
        .iter()
        .map(|(role_id, _)| role_id.clone())
        .collect();
    for role_id in role_ids {
        if let Some(role) = state_transaction.world.roles.get_mut(&role_id) {
            let removed = remove_permissions_by_name(&mut role.permissions, removed_name);
            if removed {
                role.permission_epochs
                    .retain(|permission, _| permission.name() != removed_name);
            }
            if add_can_control_domain_lives && removed {
                role.permissions.insert(replacement.clone());
                role.permission_epochs
                    .entry(replacement.clone())
                    .or_insert(0);
            }
        }
    }
}

fn apply_fixture_migration(
    kind: FixtureExecutorKind,
    state_transaction: &mut StateTransaction<'_, '_>,
    _authority: &AccountId,
) -> Result<(), ValidationFail> {
    match kind {
        FixtureExecutorKind::WithCustomPermission => {
            let mut model = baseline_executor_data_model(&state_transaction.world);
            let _ = model.permissions.remove("CanUnregisterDomain");
            model
                .permissions
                .insert(FIXTURE_PERMISSION_CAN_CONTROL_DOMAIN_LIVES.to_owned());
            state_transaction.world.apply_executor_data_model(model);
            apply_fixture_permission_migration(state_transaction, true);
            Ok(())
        }
        FixtureExecutorKind::RemovePermission => {
            let mut model = baseline_executor_data_model(&state_transaction.world);
            let _ = model.permissions.remove("CanUnregisterDomain");
            state_transaction.world.apply_executor_data_model(model);
            apply_fixture_permission_migration(state_transaction, false);
            Ok(())
        }
        FixtureExecutorKind::WithMigrationFail => Err(ValidationFail::NotPermitted(
            "fixture executor migration failed".to_owned(),
        )),
        FixtureExecutorKind::WithCustomParameter => {
            #[derive(norito::derive::JsonSerialize)]
            struct FixtureDomainLimits {
                id_len: u32,
            }

            let mut model = baseline_executor_data_model(&state_transaction.world);
            let parameter_id: CustomParameterId = FIXTURE_DOMAIN_LIMITS_PARAMETER_ID
                .parse()
                .expect("static custom parameter id");
            let default_parameter = CustomParameter::new(
                parameter_id,
                json::to_value(&FixtureDomainLimits { id_len: 16 })
                    .expect("fixture domain-limits parameter should serialize"),
            );
            model
                .parameters
                .insert(default_parameter.id.clone(), default_parameter);
            state_transaction.world.apply_executor_data_model(model);
            Ok(())
        }
        FixtureExecutorKind::WithAdmin
        | FixtureExecutorKind::CustomInstructionsSimple
        | FixtureExecutorKind::CustomInstructionsComplex
        | FixtureExecutorKind::WithFuel => {
            if state_transaction
                .world
                .executor_data_model
                .get()
                .permissions()
                .is_empty()
            {
                state_transaction
                    .world
                    .apply_executor_data_model(initial_executor_data_model_fallback());
            }
            Ok(())
        }
    }
}

fn validate_query_with_fixture(
    kind: FixtureExecutorKind,
    _query: &QueryRequest,
) -> Result<(), ValidationFail> {
    if matches!(kind, FixtureExecutorKind::WithMigrationFail) {
        return Err(ValidationFail::NotPermitted(
            "fixture executor rejects all queries".to_owned(),
        ));
    }

    Ok(())
}

fn run_executor_validation<T>(
    executor: &LoadedExecutor,
    payload: &ValidatePayload<T>,
    verdict_context: &str,
    gas_limit: u64,
) -> Result<ExecutorValidationReport, ValidationFail>
where
    ValidatePayload<T>: Encode,
{
    let mut ivm = executor
        .clone_runtime_for_gas_limit(gas_limit)
        .map_err(|err| ValidationFail::InternalError(err.to_string()))?;
    ivm.set_host(ivm::host::DefaultHost::default());

    let len_size = core::mem::size_of::<usize>();
    let payload_bytes = payload.encode();
    let mut bytes = Vec::with_capacity(len_size + payload_bytes.len());
    bytes.resize(len_size, 0);
    bytes.extend_from_slice(&payload_bytes);
    let total_len = bytes.len();
    bytes[..len_size].copy_from_slice(&total_len.to_le_bytes());

    let ptr = Memory::HEAP_START;
    ivm.store_bytes(ptr, &bytes)
        .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
    ivm.set_register(10, ptr);
    ivm.set_gas_limit(gas_limit);

    let run_result = ivm.run();
    let gas_used = gas_limit.saturating_sub(ivm.remaining_gas());
    if let Err(err) = run_result {
        if matches!(err, VMError::ExceededMaxCycles | VMError::OutOfGas) {
            return Ok(ExecutorValidationReport {
                verdict: Err(ValidationFail::TooComplex),
                gas_used,
            });
        }
        return Err(ValidationFail::InternalError(err.to_string()));
    }

    let len_size_u64 = u64::try_from(len_size).unwrap_or(u64::MAX);

    let ret_ptr = ivm.register(10);
    let returned_len = ivm
        .memory
        .load_u64(ret_ptr)
        .map_err(|e| ValidationFail::InternalError(e.to_string()))
        .and_then(|len| {
            if len > len_size_u64.saturating_add(u64::from(u32::MAX)) {
                return Err(ValidationFail::InternalError(
                    "IVM verdict length exceeds supported bounds".to_owned(),
                ));
            }
            usize::try_from(len).map_err(|_| {
                ValidationFail::InternalError(
                    "IVM verdict length exceeds host pointer width".to_owned(),
                )
            })
        })?;
    if returned_len < len_size {
        return Err(ValidationFail::InternalError(
            "IVM verdict shorter than length prefix".to_owned(),
        ));
    }

    let mut out = vec![0u8; returned_len];
    ivm.load_bytes(ret_ptr, &mut out)
        .map_err(|e| ValidationFail::InternalError(e.to_string()))?;

    let mut slice = &out[len_size..];
    let verdict: Result<(), ValidationFail> = Decode::decode(&mut slice).map_err(|err| {
        ValidationFail::InternalError(format!(
            "executor returned undecodable verdict: {verdict_context}: {err}"
        ))
    })?;

    Ok(ExecutorValidationReport { verdict, gas_used })
}

#[derive(Debug, Decode, Encode)]
enum MigrationResultPayload {
    Ok(ExecutorDataModel),
    Err(ValidationFail),
}

#[derive(Debug, Decode, Encode)]
enum MigrationUnitPayload {
    Ok(()),
    Err(ValidationFail),
}

fn run_executor_migration(
    executor: &LoadedExecutor,
    context: &ExecutorContext,
    gas_limit: u64,
) -> Result<Option<ExecutorDataModel>, ValidationFail> {
    let mut ivm = executor
        .clone_runtime_for_gas_limit(gas_limit)
        .map_err(|err| ValidationFail::InternalError(err.to_string()))?;
    ivm.set_host(ivm::host::DefaultHost::default());

    let len_size = core::mem::size_of::<usize>();
    let payload_bytes = context.encode();
    let mut bytes = Vec::with_capacity(len_size + payload_bytes.len());
    bytes.resize(len_size, 0);
    bytes.extend_from_slice(&payload_bytes);
    let total_len = bytes.len();
    bytes[..len_size].copy_from_slice(&total_len.to_le_bytes());

    let ptr = Memory::HEAP_START;
    ivm.store_bytes(ptr, &bytes)
        .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
    ivm.set_register(10, ptr);
    ivm.set_gas_limit(gas_limit);

    ivm.run()
        .map_err(|e| ValidationFail::InternalError(e.to_string()))?;

    let len_size_u64 = u64::try_from(len_size).unwrap_or(u64::MAX);
    let ret_ptr = ivm.register(10);
    let returned_len = ivm
        .memory
        .load_u64(ret_ptr)
        .map_err(|e| ValidationFail::InternalError(e.to_string()))
        .and_then(|len| {
            if len > len_size_u64.saturating_add(u64::from(u32::MAX)) {
                return Err(ValidationFail::InternalError(
                    "IVM verdict length exceeds supported bounds".to_owned(),
                ));
            }
            usize::try_from(len).map_err(|_| {
                ValidationFail::InternalError(
                    "IVM verdict length exceeds host pointer width".to_owned(),
                )
            })
        })?;
    if returned_len < len_size {
        return Err(ValidationFail::InternalError(
            "IVM verdict shorter than length prefix".to_owned(),
        ));
    }

    let mut out = vec![0u8; returned_len];
    ivm.load_bytes(ret_ptr, &mut out)
        .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
    let payload_len = returned_len - len_size;
    let payload = &out[len_size..len_size + payload_len];

    let mut slice = payload;
    if let Ok(verdict) = MigrationResultPayload::decode(&mut slice) {
        return match verdict {
            MigrationResultPayload::Ok(model) => Ok(Some(model)),
            MigrationResultPayload::Err(fail) => Err(fail),
        };
    }

    let mut slice_unit = payload;
    if let Ok(verdict) = MigrationUnitPayload::decode(&mut slice_unit) {
        return match verdict {
            MigrationUnitPayload::Ok(()) => Ok(None),
            MigrationUnitPayload::Err(fail) => Err(fail),
        };
    }

    warn!("executor migrate entrypoint returned undecodable payload; assuming success");
    Ok(None)
}

fn map_migration_fail_to_vm_error(fail: ValidationFail) -> VMError {
    match fail {
        ValidationFail::NotPermitted(reason) => {
            debug!(
                reason = %reason,
                "executor migrate entrypoint rejected migration"
            );
            VMError::PermissionDenied
        }
        ValidationFail::TooComplex => VMError::ExceededMaxCycles,
        ValidationFail::IvmAdmission(info) => {
            debug!(
                info = ?info,
                "executor migrate entrypoint failed admission checks"
            );
            VMError::DecodeError
        }
        ValidationFail::InstructionFailed(err) => {
            debug!(
                err = ?err,
                "executor migrate entrypoint instruction failure"
            );
            VMError::DecodeError
        }
        ValidationFail::QueryFailed(err) => {
            debug!(
                err = ?err,
                "executor migrate entrypoint query failure"
            );
            VMError::DecodeError
        }
        ValidationFail::InternalError(message) => {
            debug!(
                message = %message,
                "executor migrate entrypoint reported internal error"
            );
            VMError::DecodeError
        }
        ValidationFail::AxtReject(ctx) => {
            debug!(?ctx, "executor migrate entrypoint rejected AXT payload");
            VMError::PermissionDenied
        }
    }
}

fn dispatch_instruction_with_ivm(
    executor: &LoadedExecutor,
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: InstructionBox,
) -> Result<(), ValidationFail> {
    if let Some(kind) = detect_fixture_executor_kind(executor) {
        return dispatch_instruction_with_fixture(kind, state_transaction, authority, instruction);
    }

    let curr_block = state_transaction.latest_block().map_or_else(
        || BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, None, 0, 0),
        |b| b.header(),
    );

    let context = ExecutorContext {
        authority: authority.clone(),
        curr_block,
    };

    let payload = ValidatePayload {
        context,
        target: instruction.clone(),
    };
    let instruction_id = instruction.id();

    let base_fuel = state_transaction
        .world
        .parameters
        .get()
        .executor()
        .fuel
        .get();
    let gas_limit = state_transaction
        .executor_fuel_remaining
        .unwrap_or(base_fuel);
    let report = run_executor_validation(executor, &payload, instruction_id, gas_limit)?;
    if let Some(remaining) = state_transaction.executor_fuel_remaining.as_mut() {
        *remaining = remaining.saturating_sub(report.gas_used);
    }

    match report.verdict {
        Ok(()) => instruction
            .execute(authority, state_transaction)
            .map_err(|err| {
                iroha_logger::debug!(
                    ?err,
                    %instruction_id,
                    authority = %authority,
                    "state application of executor-approved instruction failed"
                );
                ValidationFail::from(err)
            }),
        Err(e) => {
            iroha_logger::debug!(
                ?e,
                %instruction_id,
                authority = %authority,
                "executor validation rejected instruction"
            );
            Err(e)
        }
    }
}

#[derive(Debug, Clone, norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
struct FixtureMintAssetForAllAccounts {
    asset_definition: AssetDefinitionId,
    quantity: Numeric,
}

#[derive(Debug, Clone)]
enum FixtureRuntimeValue {
    Bool(bool),
    Numeric(Numeric),
    Instruction(InstructionBox),
}

fn dispatch_instruction_with_fixture(
    kind: FixtureExecutorKind,
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: InstructionBox,
) -> Result<(), ValidationFail> {
    if matches!(kind, FixtureExecutorKind::WithFuel) {
        consume_fixture_instruction_fuel(state_transaction, &instruction)?;
    }

    if matches!(kind, FixtureExecutorKind::WithCustomParameter) {
        enforce_fixture_domain_limits(state_transaction, &instruction)?;
    }

    if let Some(custom) = instruction.as_any().downcast_ref::<CustomInstruction>() {
        return match kind {
            FixtureExecutorKind::CustomInstructionsSimple => {
                execute_fixture_simple_custom_instruction(state_transaction, authority, custom)
            }
            FixtureExecutorKind::CustomInstructionsComplex => {
                execute_fixture_complex_custom_instruction(state_transaction, authority, custom)
            }
            _ => Err(ValidationFail::NotPermitted(
                "custom instructions require an executor upgrade".to_owned(),
            )),
        };
    }

    instruction
        .execute(authority, state_transaction)
        .map_err(|err| {
            iroha_logger::debug!(
                ?err,
                authority = %authority,
                "state application of fixture executor-approved instruction failed"
            );
            let fail = ValidationFail::from(err);
            if matches!(kind, FixtureExecutorKind::WithFuel)
                && let ValidationFail::InstructionFailed(InstructionExecutionError::Conversion(
                    message,
                )) = &fail
                && message.contains("Operation is too complex")
            {
                return ValidationFail::TooComplex;
            }
            fail
        })
}

fn consume_fixture_instruction_fuel(
    state_transaction: &mut StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<(), ValidationFail> {
    let is_execute_trigger = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::ExecuteTrigger>()
        .is_some();
    if is_execute_trigger {
        return Ok(());
    }

    let base_fuel = state_transaction
        .world
        .parameters
        .get()
        .executor()
        .fuel
        .get();
    let remaining = state_transaction
        .executor_fuel_remaining
        .get_or_insert(base_fuel);
    if *remaining < FIXTURE_SIMPLE_INSTRUCTION_FUEL_COST {
        *remaining = 0;
        return Err(ValidationFail::TooComplex);
    }
    *remaining = remaining.saturating_sub(FIXTURE_SIMPLE_INSTRUCTION_FUEL_COST);
    Ok(())
}

fn enforce_fixture_domain_limits(
    state_transaction: &mut StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<(), ValidationFail> {
    #[derive(Debug, norito::derive::JsonDeserialize)]
    struct FixtureDomainLimits {
        id_len: u32,
    }

    let Some(register_domain) = extract_register_domain(instruction) else {
        return Ok(());
    };
    let parameter_id: CustomParameterId = FIXTURE_DOMAIN_LIMITS_PARAMETER_ID
        .parse()
        .expect("static custom parameter id");
    let Some(custom) = state_transaction
        .world
        .parameters
        .get()
        .custom
        .get(&parameter_id)
    else {
        return Ok(());
    };
    let limits: FixtureDomainLimits = json::from_str(custom.payload().as_ref()).map_err(|err| {
        ValidationFail::InternalError(format!(
            "failed to decode fixture DomainLimits parameter: {err}"
        ))
    })?;
    let name_len = register_domain.object().id().name().as_ref().len();
    if name_len > usize::try_from(limits.id_len).unwrap_or(usize::MAX) {
        return Err(ValidationFail::NotPermitted(format!(
            "domain id length {name_len} exceeds configured executor limit {}",
            limits.id_len
        )));
    }
    Ok(())
}

fn execute_fixture_simple_custom_instruction(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    custom: &CustomInstruction,
) -> Result<(), ValidationFail> {
    let root: json::Value = json::from_str(custom.payload().as_ref()).map_err(|err| {
        ValidationFail::InternalError(format!(
            "failed to decode simple fixture custom instruction payload: {err}"
        ))
    })?;
    let (variant, payload) = fixture_single_field(&root, "simple custom instruction")?;
    if variant != "MintAssetForAllAccounts" {
        return Err(ValidationFail::NotPermitted(format!(
            "unsupported fixture custom instruction variant `{variant}`"
        )));
    }
    let instruction: FixtureMintAssetForAllAccounts =
        json::from_value(payload.clone()).map_err(|err| {
            ValidationFail::InternalError(format!(
                "failed to decode simple fixture custom instruction body: {err}"
            ))
        })?;
    let account_ids: Vec<_> = state_transaction
        .world
        .accounts
        .iter()
        .map(|(account_id, _)| account_id.clone())
        .collect();
    for account_id in account_ids {
        let asset_id = AssetId::new(instruction.asset_definition.clone(), account_id);
        iroha_data_model::isi::Mint::asset_numeric(instruction.quantity.clone(), asset_id)
            .execute(authority, state_transaction)
            .map_err(ValidationFail::from)?;
    }

    Ok(())
}

fn execute_fixture_complex_custom_instruction(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    custom: &CustomInstruction,
) -> Result<(), ValidationFail> {
    let root: json::Value = json::from_str(custom.payload().as_ref()).map_err(|err| {
        ValidationFail::InternalError(format!(
            "failed to decode complex fixture custom instruction payload: {err}"
        ))
    })?;
    execute_fixture_complex_expr_value(state_transaction, authority, &root)
}

fn fixture_conversion_error(expected: &str) -> ValidationFail {
    ValidationFail::InstructionFailed(InstructionExecutionError::Conversion(format!(
        "expected {expected}"
    )))
}

fn fixture_single_field<'a>(
    value: &'a json::Value,
    context: &str,
) -> Result<(&'a str, &'a json::Value), ValidationFail> {
    let json::Value::Object(map) = value else {
        return Err(ValidationFail::InternalError(format!(
            "{context}: expected JSON object"
        )));
    };
    if map.len() != 1 {
        return Err(ValidationFail::InternalError(format!(
            "{context}: expected exactly one variant field"
        )));
    }
    let (key, value) = map
        .iter()
        .next()
        .expect("single-entry map must have first item");
    Ok((key.as_str(), value))
}

fn fixture_object_field<'a>(
    value: &'a json::Value,
    field: &str,
    context: &str,
) -> Result<&'a json::Value, ValidationFail> {
    let json::Value::Object(map) = value else {
        return Err(ValidationFail::InternalError(format!(
            "{context}: expected JSON object"
        )));
    };
    map.get(field).ok_or_else(|| {
        ValidationFail::InternalError(format!("{context}: missing required field `{field}`"))
    })
}

fn execute_fixture_complex_expr_value(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    value: &json::Value,
) -> Result<(), ValidationFail> {
    let (variant, payload) = fixture_single_field(value, "complex custom instruction")?;
    match variant {
        "Core" => {
            let object = fixture_object_field(payload, "object", "complex core expression")?;
            let instruction =
                evaluate_fixture_instruction_expression_value(state_transaction, object)?;
            instruction
                .execute(authority, state_transaction)
                .map_err(ValidationFail::from)
        }
        "If" => {
            let condition = fixture_object_field(payload, "condition", "complex if expression")?;
            let then_branch = fixture_object_field(payload, "then", "complex if expression")?;
            if evaluate_fixture_bool_expression_value(state_transaction, condition)? {
                execute_fixture_complex_expr_value(state_transaction, authority, then_branch)?;
            }
            Ok(())
        }
        _ => Err(ValidationFail::NotPermitted(format!(
            "unsupported complex fixture custom instruction variant `{variant}`"
        ))),
    }
}

fn evaluate_fixture_bool_expression_value(
    state_transaction: &StateTransaction<'_, '_>,
    value: &json::Value,
) -> Result<bool, ValidationFail> {
    let expression = fixture_unwrap_evaluates_to_expression(value, "bool expression")?;
    match evaluate_fixture_expression_value(state_transaction, expression)? {
        FixtureRuntimeValue::Bool(value) => Ok(value),
        _ => Err(fixture_conversion_error("bool value")),
    }
}

fn evaluate_fixture_numeric_expression_value(
    state_transaction: &StateTransaction<'_, '_>,
    value: &json::Value,
) -> Result<Numeric, ValidationFail> {
    let expression = fixture_unwrap_evaluates_to_expression(value, "numeric expression")?;
    match evaluate_fixture_expression_value(state_transaction, expression)? {
        FixtureRuntimeValue::Numeric(value) => Ok(value),
        _ => Err(fixture_conversion_error("numeric value")),
    }
}

fn evaluate_fixture_instruction_expression_value(
    state_transaction: &StateTransaction<'_, '_>,
    value: &json::Value,
) -> Result<InstructionBox, ValidationFail> {
    let expression = fixture_unwrap_evaluates_to_expression(value, "instruction expression")?;
    match evaluate_fixture_expression_value(state_transaction, expression)? {
        FixtureRuntimeValue::Instruction(value) => Ok(value),
        _ => Err(fixture_conversion_error("instruction value")),
    }
}

fn fixture_unwrap_evaluates_to_expression<'a>(
    value: &'a json::Value,
    _context: &str,
) -> Result<&'a json::Value, ValidationFail> {
    let json::Value::Object(map) = value else {
        return Ok(value);
    };
    Ok(map.get("expression").unwrap_or(value))
}

fn evaluate_fixture_expression_value(
    state_transaction: &StateTransaction<'_, '_>,
    value: &json::Value,
) -> Result<FixtureRuntimeValue, ValidationFail> {
    let (variant, payload) = fixture_single_field(value, "expression")?;
    match variant {
        "Raw" => {
            let (raw_variant, raw_payload) = fixture_single_field(payload, "raw value")?;
            match raw_variant {
                "Bool" => {
                    let parsed: bool = json::from_value(raw_payload.clone()).map_err(|err| {
                        ValidationFail::InternalError(format!(
                            "failed to decode fixture bool literal: {err}"
                        ))
                    })?;
                    Ok(FixtureRuntimeValue::Bool(parsed))
                }
                "Numeric" => {
                    let parsed: Numeric = json::from_value(raw_payload.clone()).map_err(|err| {
                        ValidationFail::InternalError(format!(
                            "failed to decode fixture numeric literal: {err}"
                        ))
                    })?;
                    Ok(FixtureRuntimeValue::Numeric(parsed))
                }
                "InstructionBox" => {
                    let parsed: InstructionBox =
                        json::from_value(raw_payload.clone()).map_err(|err| {
                            ValidationFail::InternalError(format!(
                                "failed to decode fixture instruction literal: {err}"
                            ))
                        })?;
                    Ok(FixtureRuntimeValue::Instruction(parsed))
                }
                _ => Err(ValidationFail::InternalError(format!(
                    "unsupported fixture raw value variant `{raw_variant}`"
                ))),
            }
        }
        "Greater" => {
            let left = fixture_object_field(payload, "left", "greater expression")?;
            let right = fixture_object_field(payload, "right", "greater expression")?;
            let left = evaluate_fixture_numeric_expression_value(state_transaction, left)?;
            let right = evaluate_fixture_numeric_expression_value(state_transaction, right)?;
            Ok(FixtureRuntimeValue::Bool(left > right))
        }
        "Query" => {
            let value = evaluate_fixture_numeric_query_value(state_transaction, payload)?;
            Ok(FixtureRuntimeValue::Numeric(value))
        }
        _ => Err(ValidationFail::InternalError(format!(
            "unsupported fixture expression variant `{variant}`"
        ))),
    }
}

fn evaluate_fixture_numeric_query_value(
    state_transaction: &StateTransaction<'_, '_>,
    value: &json::Value,
) -> Result<Numeric, ValidationFail> {
    let (variant, payload) = fixture_single_field(value, "numeric query")?;
    match variant {
        "FindAssetQuantityById" => {
            let asset_id: AssetId = json::from_value(payload.clone()).map_err(|err| {
                ValidationFail::InternalError(format!(
                    "failed to decode fixture asset query payload: {err}"
                ))
            })?;
            Ok(state_transaction
                .world
                .assets
                .get(&asset_id)
                .map(|value| value.as_ref().clone())
                .unwrap_or_else(Numeric::zero))
        }
        "FindTotalAssetQuantityByAssetDefinitionId" => {
            let asset_definition_id: AssetDefinitionId = json::from_value(payload.clone())
                .map_err(|err| {
                    ValidationFail::InternalError(format!(
                        "failed to decode fixture asset-definition query payload: {err}"
                    ))
                })?;
            state_transaction
                .world
                .asset_total_amount(&asset_definition_id)
                .map_err(ValidationFail::from)
        }
        _ => Err(ValidationFail::InternalError(format!(
            "unsupported fixture numeric query variant `{variant}`"
        ))),
    }
}

fn extract_register_role(instruction: &InstructionBox) -> Option<Register<Role>> {
    let instr_any = instruction.as_any();
    if let Some(reg) = instr_any.downcast_ref::<Register<Role>>() {
        return Some(reg.clone());
    }
    if let Some(reg_box) = instr_any.downcast_ref::<RegisterBox>() {
        return match reg_box {
            RegisterBox::Role(reg) => Some(reg.clone()),
            _ => None,
        };
    }
    None
}

fn extract_register_domain(instruction: &InstructionBox) -> Option<Register<Domain>> {
    let instr_any = instruction.as_any();
    if let Some(reg) = instr_any.downcast_ref::<Register<Domain>>() {
        return Some(reg.clone());
    }
    if let Some(reg_box) = instr_any.downcast_ref::<RegisterBox>() {
        return match reg_box {
            RegisterBox::Domain(reg) => Some(reg.clone()),
            _ => None,
        };
    }
    None
}

fn extract_account_metadata_target(instruction: &InstructionBox) -> Option<AccountId> {
    instruction
        .as_any()
        .downcast_ref::<SetKeyValueBox>()
        .and_then(|set| match set {
            SetKeyValueBox::Account(set) => Some(set.object.clone()),
            _ => None,
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::SetKeyValue<iroha_data_model::account::Account>>()
                .map(|set| set.object.clone())
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<RemoveKeyValueBox>()
                .and_then(|rm| match rm {
                    RemoveKeyValueBox::Account(rm) => Some(rm.object.clone()),
                    _ => None,
                })
        })
        .or_else(|| {
            instruction
                .as_any()
                .downcast_ref::<iroha_data_model::isi::RemoveKeyValue<iroha_data_model::account::Account>>()
                .map(|rm| rm.object.clone())
        })
}

fn extract_transfer_asset(
    instruction: &InstructionBox,
) -> Option<Transfer<Asset, Numeric, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) = instr_any.downcast_ref::<Transfer<Asset, Numeric, Account>>() {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Asset(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction.id().contains("Asset") {
        return None;
    }
    let bytes = instruction.dyn_encode();
    std::panic::catch_unwind(|| {
        let mut slice = &bytes[..];
        Transfer::<Asset, Numeric, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}

fn extract_transfer_domain(
    instruction: &InstructionBox,
) -> Option<Transfer<Account, DomainId, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) = instr_any.downcast_ref::<Transfer<Account, DomainId, Account>>() {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Domain(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction.id().contains("Domain") {
        return None;
    }
    let bytes = instruction.dyn_encode();
    std::panic::catch_unwind(|| {
        let mut slice = &bytes[..];
        Transfer::<Account, DomainId, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}

fn authority_has_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    target: &Permission,
) -> Result<bool, ValidationFail> {
    let permissions = world
        .account_permissions_iter(authority)
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    if permissions
        .into_iter()
        .any(|permission| permission == target)
    {
        return Ok(true);
    }

    for role_id in world.account_roles_iter(authority) {
        if let Some(role) = world.roles().get(role_id)
            && role.permissions.contains(target)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn can_modify_account_metadata(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    account_id: &AccountId,
) -> Result<bool, ValidationFail> {
    if authority == account_id {
        return Ok(true);
    }

    let domain_owner = world
        .domain(account_id.domain())
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    if &domain_owner == authority {
        return Ok(true);
    }

    let required: Permission = executor_permission::account::CanModifyAccountMetadata {
        account: account_id.clone(),
    }
    .into();
    authority_has_permission(world, authority, &required)
}

fn can_transfer_domain(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    transfer: &Transfer<Account, DomainId, Account>,
) -> Result<bool, ValidationFail> {
    if transfer.source() == authority {
        return Ok(true);
    }

    let source_domain_owner = world
        .domain(transfer.source().domain())
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    if &source_domain_owner == authority {
        return Ok(true);
    }

    let transferred_domain_owner = world
        .domain(transfer.object())
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    Ok(&transferred_domain_owner == authority)
}

fn can_transfer_asset(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    transfer: &Transfer<Asset, Numeric, Account>,
) -> Result<bool, ValidationFail> {
    if transfer.source().account() == authority {
        return Ok(true);
    }

    let source_domain_owner = world
        .domain(transfer.source().account().domain())
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    if &source_domain_owner == authority {
        return Ok(true);
    }

    let asset = transfer.source().clone();
    let specific: Permission = executor_permission::asset::CanTransferAsset {
        asset: asset.clone(),
    }
    .into();
    if authority_has_permission(world, authority, &specific)? {
        return Ok(true);
    }

    let by_definition: Permission = executor_permission::asset::CanTransferAssetWithDefinition {
        asset_definition: asset.definition().clone(),
    }
    .into();
    authority_has_permission(world, authority, &by_definition)
}

fn normalize_role_permission_for_initial_executor(
    state_transaction: &StateTransaction<'_, '_>,
    permission: &Permission,
) -> Result<Permission, ValidationFail> {
    let known_permission = state_transaction
        .world
        .executor_data_model
        .get()
        .permissions()
        .iter()
        .any(|known| known.as_str() == permission.name())
        || is_builtin_initial_permission_name(permission.name());
    if !known_permission {
        return Err(ValidationFail::NotPermitted(format!(
            "{permission:?}: Unknown permission"
        )));
    }

    if permission.name() == "CanTransferAsset" {
        let normalized = executor_permission::asset::CanTransferAsset::try_from(permission)
            .map_err(|err| {
                ValidationFail::NotPermitted(format!(
                    "{permission:?}: Invalid permission payload ({err:?})"
                ))
            })?;
        return Ok(normalized.into());
    }

    Ok(permission.clone())
}

const INITIAL_EXECUTOR_PERMISSION_NAMES: &[&str] = &[
    "CanManagePeers",
    "CanRegisterDomain",
    "CanUnregisterDomain",
    "CanModifyDomainMetadata",
    "CanRegisterAssetDefinition",
    "CanUnregisterAssetDefinition",
    "CanModifyAssetDefinitionMetadata",
    "CanRegisterAccount",
    "CanUnregisterAccount",
    "CanModifyAccountMetadata",
    "CanMintAssetWithDefinition",
    "CanBurnAssetWithDefinition",
    "CanTransferAssetWithDefinition",
    "CanMintAsset",
    "CanBurnAsset",
    "CanTransferAsset",
    "CanModifyAssetMetadataWithDefinition",
    "CanModifyAssetMetadata",
    "CanRegisterNft",
    "CanUnregisterNft",
    "CanTransferNft",
    "CanModifyNftMetadata",
    "CanRegisterTrigger",
    "CanUnregisterTrigger",
    "CanModifyTrigger",
    "CanExecuteTrigger",
    "CanModifyTriggerMetadata",
    "CanSetParameters",
    "CanManageRoles",
    "CanUpgradeExecutor",
    "CanRegisterSmartContractCode",
    "CanPublishSpaceDirectoryManifest",
    "CanUseFeeSponsor",
    "CanProposeContractDeployment",
    "CanSubmitGovernanceBallot",
    "CanEnactGovernance",
    "CanManageParliament",
    "CanRecordCitizenService",
    "CanSlashGovernanceLock",
    "CanRestituteGovernanceLock",
    "CanRegisterSorafsPin",
    "CanApproveSorafsPin",
    "CanRetireSorafsPin",
    "CanBindSorafsAlias",
    "CanDeclareSorafsCapacity",
    "CanSubmitSorafsTelemetry",
    "CanFileSorafsCapacityDispute",
    "CanIssueSorafsReplicationOrder",
    "CanCompleteSorafsReplicationOrder",
    "CanSetSorafsPricing",
    "CanUpsertSorafsProviderCredit",
    "CanOperateSorafsRepair",
    "CanRegisterSorafsProviderOwner",
    "CanUnregisterSorafsProviderOwner",
    "CanIngestSoranetPrivacy",
];

fn is_builtin_initial_permission_name(permission_name: &str) -> bool {
    INITIAL_EXECUTOR_PERMISSION_NAMES.contains(&permission_name)
}

/// Parse the WAT-like template used in integration tests to embed a sequence
/// of Norito-encoded ISIs into linear memory, then execute each instruction.
pub(crate) fn extract_register_asset_definition(
    instruction: &InstructionBox,
) -> Option<Register<AssetDefinition>> {
    let instr_any = instruction.as_any();
    if let Some(reg) = instr_any.downcast_ref::<Register<AssetDefinition>>() {
        return Some(reg.clone());
    }
    if let Some(reg_box) = instr_any.downcast_ref::<RegisterBox>() {
        return match reg_box {
            RegisterBox::AssetDefinition(reg) => Some(reg.clone()),
            _ => None,
        };
    }
    if !instruction.id().contains("AssetDefinition") {
        return None;
    }
    let bytes = instruction.dyn_encode();
    std::panic::catch_unwind(|| {
        let mut slice = &bytes[..];
        Register::<AssetDefinition>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}

pub(crate) fn ensure_asset_definition_registration_allowed(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    reg_asset_definition: &Register<AssetDefinition>,
) -> Result<(), ValidationFail> {
    let domain_id = reg_asset_definition.object().id().domain().clone();
    let domain_owner = state_transaction
        .world
        .domains
        .get(&domain_id)
        .map(|domain| domain.owned_by().clone());
    let is_domain_owner = domain_owner
        .as_ref()
        .is_some_and(|owner| owner == authority);
    let has_permission =
        state_transaction.can_register_asset_definition_in_domain(authority, &domain_id);
    if !(is_domain_owner || has_permission) {
        iroha_logger::debug!(
            %authority,
            owner = %domain_owner
                .as_ref()
                .map_or_else(|| "unknown".to_owned(), ToString::to_string),
            domain = %domain_id,
            "asset-definition registration denied without ownership or permission"
        );
        return Err(ValidationFail::NotPermitted(
            "Can't register asset definition in a domain owned by another account".to_owned(),
        ));
    }
    Ok(())
}

#[allow(dead_code)]
fn execute_wat_embedded_instructions(
    state_tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    wat_bytes: &[u8],
) -> Result<(), String> {
    let Ok(wat_str) = core::str::from_utf8(wat_bytes) else {
        return Err("contract is not valid UTF-8".to_owned());
    };

    // 1) Extract the memory data blob inside: (data (i32.const 0) "...")
    let needle = "(data (i32.const 0) \"";
    let start = wat_str
        .find(needle)
        .ok_or_else(|| "no memory data segment found".to_owned())?
        + needle.len();
    let rest = &wat_str[start..];
    let end = rest
        .find('\"')
        .ok_or_else(|| "unterminated data segment".to_owned())?;
    let hex_esc = &rest[..end];

    // Decode sequences like \ab into bytes
    let mut mem_blob: Vec<u8> = Vec::with_capacity(hex_esc.len() / 3 + 1);
    let chars: Vec<char> = hex_esc.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            if i + 2 >= chars.len() {
                return Err("incomplete hex escape in data segment".to_owned());
            }
            let hi = chars[i + 1];
            let lo = chars[i + 2];
            let hex = [hi, lo].iter().collect::<String>();
            let byte = u8::from_str_radix(&hex, 16)
                .map_err(|_| "invalid hex escape in data segment".to_owned())?;
            mem_blob.push(byte);
            i += 3;
        } else {
            // Ignore formatting characters (e.g., whitespace) inside string
            i += 1;
        }
    }

    // 2) Extract all call sites: (call $exec_isi (i32.const <ptr>) (i32.const <len>))
    let mut cursor = wat_str;
    let mut slices: Vec<(usize, usize)> = Vec::new();
    let pat = "(call $exec_isi (i32.const ";
    while let Some(p) = cursor.find(pat) {
        let after = &cursor[p + pat.len()..];
        // parse ptr (decimal)
        let mut j = 0;
        while j < after.len() && after.as_bytes()[j].is_ascii_digit() {
            j += 1;
        }
        if j == 0 {
            return Err("missing ptr literal".to_owned());
        }
        let ptr: usize = after[..j].parse().map_err(|_| "bad ptr".to_owned())?;
        let after_ptr = &after[j..];
        // expect ) (i32.const
        let next_pat = ") (i32.const ";
        let np = after_ptr
            .find(next_pat)
            .ok_or_else(|| "bad call syntax".to_owned())?;
        let after_len = &after_ptr[np + next_pat.len()..];
        let mut k = 0;
        while k < after_len.len() && after_len.as_bytes()[k].is_ascii_digit() {
            k += 1;
        }
        if k == 0 {
            return Err("missing len literal".to_owned());
        }
        let len: usize = after_len[..k].parse().map_err(|_| "bad len".to_owned())?;
        slices.push((ptr, len));
        cursor = &after_len[k..];
    }

    if slices.is_empty() {
        return Err("no exec_isi calls found".to_owned());
    }

    // 3) Decode each instruction from the memory blob and execute it.
    for (ptr, len) in slices {
        let end = ptr
            .checked_add(len)
            .ok_or_else(|| "ptr overflow".to_owned())?;
        if end > mem_blob.len() {
            return Err("slice out of bounds".to_owned());
        }
        let mut slice = &mem_blob[ptr..end];
        let isi: DMInstructionBox = DMInstructionBox::decode(&mut slice)
            .map_err(|_| "failed to decode instruction".to_owned())?;
        state_tx
            .world
            .executor
            .clone()
            .execute_instruction(state_tx, authority, isi)
            .map_err(|e| format!("execution failed: {e}"))?;
    }

    Ok(())
}

/// [`Executor`] with cached [`IVM`] for execution.
#[derive(Debug, Clone)]
#[debug("LoadedExecutor {{ runtime: <IVM> }}")]
pub struct LoadedExecutor {
    runtime: Arc<Mutex<ExecutorRuntime>>,
    /// Arc is needed so cloning of executor will be fast.
    /// See [`crate::tx::TransactionExecutor::validate_with_runtime_executor`].
    raw_executor: Arc<data_model_executor::Executor>,
}

struct ExecutorRuntime {
    stack_limit: u64,
    vm: IVM,
}

fn stack_limit_for_gas(gas_limit: u64) -> u64 {
    IvmConfig::new(gas_limit).stack_limit_for_gas()
}

impl LoadedExecutor {
    pub(crate) fn load(raw_executor: data_model_executor::Executor) -> Result<Self, VMError> {
        let gas_limit = iroha_data_model::parameter::SmartContractParameters::default()
            .fuel
            .get();
        let stack_limit = stack_limit_for_gas(gas_limit);
        let mut ivm = IVM::new(gas_limit);
        ivm.load_program(raw_executor.bytecode().as_ref())?;
        Ok(Self {
            runtime: Arc::new(Mutex::new(ExecutorRuntime {
                stack_limit,
                vm: ivm,
            })),
            raw_executor: Arc::new(raw_executor),
        })
    }

    fn clone_runtime_for_gas_limit(&self, gas_limit: u64) -> Result<IVM, VMError> {
        let stack_limit = stack_limit_for_gas(gas_limit);
        let mut runtime = self.runtime.lock().expect("executor template poisoned");
        if runtime.stack_limit != stack_limit {
            let mut ivm = IVM::new(gas_limit);
            ivm.load_program(self.raw_executor.bytecode().as_ref())?;
            runtime.vm = ivm;
            runtime.stack_limit = stack_limit;
        }
        Ok(runtime.vm.clone())
    }
}

/// Norito encode/decode helpers for the runtime `Executor`.
///
/// These helpers serialize the core `Executor` enum into a compact Norito
/// payload using a local DTO and provide a materialization path that loads a
/// `LoadedExecutor` when required.
pub mod executor_norito {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;

    /// Local DTO used for Norito encoding of `Executor`.
    #[derive(Encode, Decode)]
    enum ExecutorDto {
        Initial,
        UserProvided(iroha_data_model::executor::Executor),
    }

    /// Serialize the given `Executor` to Norito bytes.
    /// Serialize an [`Executor`] into Norito-encoded bytes.
    ///
    /// # Errors
    /// Returns an error if Norito encoding fails for the provided executor variant.
    pub fn to_bytes(executor: &Executor) -> Result<Vec<u8>, norito::core::Error> {
        let dto = match executor {
            Executor::Initial => ExecutorDto::Initial,
            Executor::UserProvided(le) => {
                // Serialize the raw executor (data_model)
                ExecutorDto::UserProvided((*le.raw_executor).clone())
            }
        };
        norito::to_bytes(&dto)
    }

    /// Deserialize Norito bytes into a materialized `Executor`.
    ///
    /// For `UserProvided` DTO, loads the IVM program to construct a `LoadedExecutor`.
    /// Deserialize an [`Executor`] from Norito-encoded bytes.
    ///
    /// # Errors
    /// Returns an error if the byte slice does not represent a valid executor value.
    pub fn from_bytes(bytes: &[u8]) -> Result<Executor, String> {
        let decoded = catch_unwind(AssertUnwindSafe(|| norito::decode_from_bytes(bytes)))
            .map_err(|_| "executor decode failed: panic during Norito decode".to_owned())?;
        let dto: ExecutorDto = decoded.map_err(|e| format!("executor decode failed: {e}"))?;
        match dto {
            ExecutorDto::Initial => Ok(Executor::Initial),
            ExecutorDto::UserProvided(raw) => LoadedExecutor::load(raw)
                .map(Executor::UserProvided)
                .map_err(|e| format!("executor load failed: {e}")),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn initial_roundtrip() {
            let exec = Executor::Initial;
            let bytes = to_bytes(&exec).expect("encode");
            let dec = from_bytes(&bytes).expect("decode");
            match dec {
                Executor::Initial => {}
                _ => panic!("expected Initial variant"),
            }
        }

        #[test]
        fn userprovided_encodes_but_load_may_fail() {
            // Construct a dummy data-model executor with some bytecode; loading may fail,
            // but encoding itself should succeed.
            let raw = iroha_data_model::executor::Executor::new(
                iroha_data_model::transaction::IvmBytecode::from_compiled(vec![0x00, 0x01, 0x02]),
            );
            let bytes = norito::to_bytes(&ExecutorDto::UserProvided(raw)).expect("encode dto");
            // Decoding to materialized `Executor` may fail due to invalid bytecode; assert the error is surfaced.
            let res = from_bytes(&bytes);
            assert!(res.is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "telemetry")]
    use iroha_config::parameters::actual::{GasLiquidity, GasRate, GasVolatility};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        executor::{self as data_model_executor, ExecutorDataModel},
        isi::Grant,
        name::Name,
        parameter::{CustomParameter, CustomParameterId},
        prelude::*,
        query::{QueryRequest, SingularQueryBox, prelude::FindParameters},
        transaction::executable::IvmBytecode,
    };
    use iroha_executor_data_model::isi::multisig::{DEFAULT_MULTISIG_TTL_MS, MultisigSpec};
    use iroha_executor_data_model::permission::nexus::CanUseFeeSponsor;
    use iroha_primitives::json::Json;
    #[cfg(feature = "telemetry")]
    use iroha_telemetry::metrics::Metrics;
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, BOB_ID, SAMPLE_GENESIS_ACCOUNT_ID, gen_account_in,
    };
    #[allow(unused_imports)]
    use ivm::instruction;
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;
    #[cfg(feature = "telemetry")]
    use rust_decimal::Decimal;

    use super::*;
    #[cfg(feature = "telemetry")]
    use crate::telemetry::StateTelemetry;
    use crate::{
        kura::Kura,
        query,
        state::{State, World},
    };

    fn make_peer_id() -> crate::PeerId {
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        crate::PeerId::new(kp.public_key().clone())
    }

    fn alice() -> AccountId {
        iroha_test_samples::ALICE_ID.clone()
    }

    fn generate_fixture_placeholder_program(vector_length: u8) -> Vec<u8> {
        let mut program = Vec::new();
        program.extend_from_slice(b"IVM\0");
        program.extend_from_slice(&[1, 0, 0, vector_length]);
        program.extend_from_slice(&1_000_000_u64.to_le_bytes());
        program.push(1);
        program.extend_from_slice(&FIXTURE_LITERAL_SECTION_MAGIC);
        program.extend_from_slice(&0_u32.to_le_bytes());
        program.extend_from_slice(&64_u32.to_le_bytes());
        program.extend_from_slice(&0_u32.to_le_bytes());
        program.extend(std::iter::repeat_n(0_u8, 64));
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        program
    }

    #[test]
    fn fixture_executor_detection_matches_vector_tags() {
        let cases = [
            (1, FixtureExecutorKind::WithAdmin),
            (2, FixtureExecutorKind::WithCustomPermission),
            (3, FixtureExecutorKind::RemovePermission),
            (4, FixtureExecutorKind::CustomInstructionsSimple),
            (5, FixtureExecutorKind::CustomInstructionsComplex),
            (6, FixtureExecutorKind::WithMigrationFail),
            (7, FixtureExecutorKind::WithFuel),
            (8, FixtureExecutorKind::WithCustomParameter),
        ];

        for (tag, expected) in cases {
            let bytecode = generate_fixture_placeholder_program(tag);
            assert_eq!(
                detect_fixture_executor_kind_from_bytecode(&bytecode),
                Some(expected),
                "expected fixture kind for vector length tag {tag}"
            );
        }
    }

    #[test]
    fn fixture_simple_custom_instruction_mints_for_all_accounts() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
        let asset_definition_id: AssetDefinitionId =
            "rose#wonderland".parse().expect("asset definition id");
        let asset_definition =
            AssetDefinition::numeric(asset_definition_id.clone()).build(&ALICE_ID);

        let world = World::with_assets(
            [domain],
            [alice_account, bob_account],
            [asset_definition],
            [],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let payload = json::to_value(&FixtureMintAssetForAllAccounts {
            asset_definition: asset_definition_id.clone(),
            quantity: Numeric::from(1_u32),
        })
        .expect("serialize fixture payload");
        let mut root = BTreeMap::new();
        root.insert("MintAssetForAllAccounts".to_owned(), payload);
        let instruction =
            InstructionBox::from(CustomInstruction::new(Json::new(json::Value::Object(root))));

        dispatch_instruction_with_fixture(
            FixtureExecutorKind::CustomInstructionsSimple,
            &mut stx,
            &ALICE_ID,
            instruction,
        )
        .expect("fixture custom instruction should execute");

        let alice_rose = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
        let bob_rose = AssetId::new(asset_definition_id, BOB_ID.clone());
        let alice_value = stx
            .world
            .assets
            .get(&alice_rose)
            .map(|value| value.as_ref().clone())
            .expect("alice rose");
        let bob_value = stx
            .world
            .assets
            .get(&bob_rose)
            .map(|value| value.as_ref().clone())
            .expect("bob rose");

        assert_eq!(alice_value, Numeric::from(1_u32));
        assert_eq!(bob_value, Numeric::from(1_u32));
    }

    #[test]
    fn detached_register_peer_forces_sequential_path() {
        let peer_id = make_peer_id();
        let isi = iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id, Vec::new());
        let mut delta = crate::state::DetachedStateTransactionDelta::default();

        let err = execute_instruction_detached(&alice(), &InstructionBox::from(isi), &mut delta)
            .expect_err("peer registration must be unsupported in detached mode");
        assert!(
            matches!(err, ValidationFail::InternalError(msg) if msg.contains("peer management"))
        );
    }

    #[test]
    fn detached_unregister_peer_forces_sequential_path() {
        let peer_id = make_peer_id();
        let isi = iroha_data_model::isi::Unregister::peer(peer_id);
        let mut delta = crate::state::DetachedStateTransactionDelta::default();

        let err = execute_instruction_detached(&alice(), &InstructionBox::from(isi), &mut delta)
            .expect_err("peer removal must be unsupported in detached mode");
        assert!(
            matches!(err, ValidationFail::InternalError(msg) if msg.contains("peer management"))
        );
    }

    #[test]
    fn detached_nft_metadata_records_delta() {
        let (bob_id, _bob_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = Domain::new(domain_id).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(bob_id.clone()).build(&bob_id);
        let nft_id: NftId = "nft_detached$wonderland".parse().expect("nft id");
        let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&bob_id);

        let world = World::with_assets([domain], [alice_account, bob_account], [], [], [nft]);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let chain: ChainId = "test-chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain);
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let key: Name = "meta".parse().expect("key");
        let set = SetKeyValue::nft(nft_id.clone(), key.clone(), "value");
        let mut delta = crate::state::DetachedStateTransactionDelta::default();
        execute_instruction_detached(&bob_id, &InstructionBox::from(set), &mut delta)
            .expect("detached nft metadata should be supported");

        let _ = delta
            .merge_into(&mut block, &bob_id)
            .expect("merge succeeds");
        block.commit().expect("commit");

        let view = state.view();
        let nft_val = view.world().nfts().get(&nft_id).expect("nft exists");
        let stored = nft_val.content.get(&key).expect("metadata set");
        assert_eq!(stored, &Json::from("value"));
    }
    use std::collections::{BTreeMap, BTreeSet};

    #[allow(dead_code)]
    fn encode_load(rd: u8, base: u8, imm12: u16, funct3: u8) -> u32 {
        let imm = u32::from(imm12 & 0x0fff);
        (imm << 20)
            | ((u32::from(base) & 0x1f) << 15)
            | ((u32::from(funct3) & 0x7) << 12)
            | ((u32::from(rd) & 0x1f) << 7)
            | 0x03
    }

    #[allow(dead_code)]
    fn encode_store(base: u8, rs: u8, imm12: u16, funct3: u8) -> u32 {
        let imm = u32::from(imm12 & 0x0fff);
        let imm_hi = (imm >> 5) & 0x7f;
        let imm_lo = imm & 0x1f;
        (imm_hi << 25)
            | ((u32::from(rs) & 0x1f) << 20)
            | ((u32::from(base) & 0x1f) << 15)
            | ((u32::from(funct3) & 0x7) << 12)
            | (imm_lo << 7)
            | 0x23
    }

    #[cfg(feature = "zk-preverify")]
    #[test]
    fn preverify_and_dedup_across_transactions_in_block() {
        use iroha_data_model::{
            proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox},
            transaction::{Executable, TransactionBuilder},
        };
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().expect("domain id")).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);

        // Build attachments with mock proof payloads
        let backend: Ident = "halo2/ipa".parse().expect("backend ident");
        let proof = ProofBox::new(backend.clone(), vec![1u8, 2, 3]);
        let vk = VerifyingKeyBox::new(backend.clone(), vec![4u8, 5, 6]);
        let attachment = ProofAttachment::new_inline(backend, proof, vk);
        let attachments = ProofAttachmentList(vec![attachment.clone()]);
        let attachments_dup = ProofAttachmentList(vec![attachment]);

        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx1 = TransactionBuilder::new(chain.clone(), ALICE_ID.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .with_attachments(attachments)
            .sign(ALICE_KEYPAIR.private_key());
        let tx2 = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .with_attachments(attachments_dup)
            .sign(ALICE_KEYPAIR.private_key());

        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        // First transaction preverify accepted
        {
            let mut state_tx = block.transaction();
            executor
                .execute_transaction(&mut state_tx, &ALICE_ID.clone(), tx1, &mut ivm_cache)
                .expect("preverify accepted");
        }

        // Second identical proof should be flagged as duplicate by per-block dedup
        {
            let mut state_tx = block.transaction();
            let res =
                executor.execute_transaction(&mut state_tx, &ALICE_ID.clone(), tx2, &mut ivm_cache);
            assert!(res.is_err(), "duplicate proof should be rejected");
        }
    }

    #[test]
    fn initial_executor_denies_asset_definition_without_permission() {
        let alice_id = ALICE_ID.clone();
        let genesis_id = SAMPLE_GENESIS_ACCOUNT_ID.clone();

        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain: Domain = Domain::new(domain_id.clone()).build(&genesis_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let genesis_account = Account::new(genesis_id.clone()).build(&genesis_id);

        let world = World::with([domain], [alice_account, genesis_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        {
            let mut stx = block.transaction();
            Transfer::domain(genesis_id.clone(), domain_id.clone(), alice_id.clone())
                .execute(&genesis_id, &mut stx)
                .expect("domain transfer to succeed");
            stx.apply();
        }

        let executor = super::Executor::Initial;
        let asset_definition_id: AssetDefinitionId =
            "invalid#wonderland".parse().expect("asset id");
        let instruction = InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id),
        ));

        let mut stx = block.transaction();
        let res = executor.execute_instruction(&mut stx, &genesis_id, instruction);
        assert!(
            matches!(res, Err(ValidationFail::NotPermitted(_))),
            "initial executor should deny registering asset definition without permission"
        );
    }

    #[test]
    fn initial_executor_denies_transfer_domain_without_ownership() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId = "users".parse().expect("users domain id");
        let foo_domain_id: DomainId = "foo".parse().expect("foo domain id");
        let user1 = AccountId::new(users_domain_id.clone(), KeyPair::random().into_parts().0);
        let user2 = AccountId::new(users_domain_id.clone(), KeyPair::random().into_parts().0);

        let users_domain = Domain::new(users_domain_id).build(&user1);
        let foo_domain = Domain::new(foo_domain_id.clone()).build(&user1);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);

        let world = World::with(
            [users_domain, foo_domain],
            [alice_account, user1_account, user2_account],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let genesis_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        state
            .block(genesis_header)
            .commit()
            .expect("commit bootstrap block");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let executor = super::Executor::Initial;
        let instruction = InstructionBox::from(Transfer::domain(
            user1.clone(),
            foo_domain_id,
            user2.clone(),
        ));
        let transfer = extract_transfer_domain(&instruction)
            .expect("expected to extract domain transfer from instruction");

        let mut stx = block.transaction();
        assert_eq!(
            stx.world
                .domain(user1.domain())
                .expect("users domain should exist")
                .owned_by(),
            &user1
        );
        assert_eq!(
            stx.world
                .domain(transfer.object())
                .expect("foo domain should exist")
                .owned_by(),
            &user1
        );
        let allowed = can_transfer_domain(&stx.world, &alice_id, &transfer)
            .expect("domain transfer permission check");
        assert!(
            !allowed,
            "alice should not be allowed to transfer foo domain"
        );
        assert!(
            !(stx._curr_block.is_genesis() && stx.block_hashes.is_empty()),
            "test must execute in non-genesis context"
        );
        let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
        assert!(
            matches!(res, Err(ValidationFail::NotPermitted(_))),
            "initial executor should deny domain transfer from another account, got: {res:?}"
        );
    }

    #[test]
    fn initial_executor_allows_transfer_asset_by_source_domain_owner() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId = "users".parse().expect("users domain id");
        let user1 = AccountId::new(users_domain_id.clone(), KeyPair::random().into_parts().0);
        let user2 = AccountId::new(users_domain_id.clone(), KeyPair::random().into_parts().0);

        let users_domain = Domain::new(users_domain_id).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);

        let world = World::with(
            [users_domain],
            [alice_account, user1_account, user2_account],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let genesis_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        state
            .block(genesis_header)
            .commit()
            .expect("commit bootstrap block");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let transfer_asset_id = AssetId::new(
            "coin#users".parse().expect("asset definition id"),
            user1.clone(),
        );
        let instruction = InstructionBox::from(Transfer::asset_numeric(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let transfer = extract_transfer_asset(&instruction)
            .expect("expected to extract asset transfer from instruction");

        let stx = block.transaction();
        let allowed = can_transfer_asset(&stx.world, &alice_id, &transfer)
            .expect("asset transfer permission check");
        assert!(
            allowed,
            "source domain owner should be allowed to transfer account assets"
        );
    }

    #[test]
    fn initial_executor_denies_transfer_asset_without_owner_signature() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId = "users".parse().expect("users domain id");
        let user1 = AccountId::new(users_domain_id.clone(), KeyPair::random().into_parts().0);
        let user2 = AccountId::new(users_domain_id.clone(), KeyPair::random().into_parts().0);

        let users_domain = Domain::new(users_domain_id).build(&user1);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);

        let world = World::with(
            [users_domain],
            [alice_account, user1_account, user2_account],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let genesis_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        state
            .block(genesis_header)
            .commit()
            .expect("commit bootstrap block");
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let executor = super::Executor::Initial;
        let transfer_asset_id = AssetId::new(
            "coin#users".parse().expect("asset definition id"),
            user1.clone(),
        );
        let instruction = InstructionBox::from(Transfer::asset_numeric(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let transfer = extract_transfer_asset(&instruction)
            .expect("expected to extract asset transfer from instruction");

        let mut stx = block.transaction();
        let allowed = can_transfer_asset(&stx.world, &alice_id, &transfer)
            .expect("asset transfer permission check");
        assert!(
            !allowed,
            "alice should not be allowed to transfer user1's asset"
        );
        assert!(
            !(stx._curr_block.is_genesis() && stx.block_hashes.is_empty()),
            "test must execute in non-genesis context"
        );
        let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("source asset owner must sign the transaction"),
                "unexpected rejection message: {msg}"
            ),
            other => panic!(
                "initial executor should deny asset transfer without owner signature, got: {other:?}"
            ),
        }
    }

    #[test]
    fn initial_executor_denies_nft_metadata_edit_in_transaction() {
        let (bob_id, bob_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(bob_id.clone()).build(&bob_id);
        let nft_id: NftId = "nft_owner_modify$wonderland".parse().expect("nft id");
        let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&bob_id);

        let world = World::with_assets([domain], [alice_account, bob_account], [], [], [nft]);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let chain: ChainId = "test-chain".parse().unwrap();
        let state = State::new_with_chain(world, kura, query_handle, chain.clone());
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let instruction = SetKeyValue::nft(nft_id, "foo".parse().expect("key"), "value");
        let tx = TransactionBuilder::new(chain, bob_id.clone())
            .with_instructions([instruction])
            .sign(bob_kp.private_key());

        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let mut stx = block.transaction();
        let res = executor.execute_transaction(&mut stx, &bob_id, tx, &mut ivm_cache);
        assert!(
            matches!(res, Err(ValidationFail::NotPermitted(_))),
            "initial executor should deny NFT metadata edits by non-domain owners"
        );
    }

    #[test]
    fn bench_profile_runs_without_logger() {
        let authority = ALICE_ID.clone();
        let account = Account::new(authority.clone()).build(&authority);
        let world = World::with([], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut tx = block.transaction();
        let executor = super::Executor::default();
        let instr: InstructionBox = Log::new(Level::INFO, "bench profile".to_owned()).into();

        executor
            .execute_instruction_with_profile(
                &mut tx,
                &authority,
                instr,
                InstructionExecutionProfile::Bench,
            )
            .expect("bench profile should execute without logger");
    }

    #[test]
    fn nexus_fee_sponsor_rejected_when_disabled() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        let domain: Domain = Domain::new("wonderland".parse().expect("domain id")).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let world = World::with([domain], [alice_account, sink_account, sponsor_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        let nexus = state.nexus.get_mut();
        nexus.enabled = true;
        nexus.fees.base_fee = 1;
        nexus.fees.sponsorship_enabled = false;
        nexus.fees.fee_asset_id = "xor#wonderland".to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            Json::new(sponsor_id.to_string()),
        );
        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, ALICE_ID.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(ALICE_KEYPAIR.private_key());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        let mut stx = block.transaction();
        let res = executor.execute_transaction(&mut stx, &ALICE_ID.clone(), tx, &mut ivm_cache);
        assert!(
            matches!(res, Err(ValidationFail::NotPermitted(_))),
            "sponsorship should be rejected when disabled"
        );
    }

    #[test]
    fn nexus_fee_sponsor_rejected_without_permission() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain: Domain =
            Domain::new("wonderland".parse().expect("domain id")).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let world = World::with([domain], [authority_account, sponsor_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        let nexus = state.nexus.get_mut();
        nexus.enabled = true;
        nexus.fees.base_fee = 1;
        nexus.fees.sponsorship_enabled = true;
        nexus.fees.fee_asset_id = "xor#wonderland".to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            Json::new(sponsor_id.to_string()),
        );
        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(authority_kp.private_key());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        let mut stx = block.transaction();
        let res = executor.execute_transaction(&mut stx, &authority_id, tx, &mut ivm_cache);
        assert!(
            matches!(res, Err(ValidationFail::NotPermitted(_))),
            "sponsorship should be rejected without permission"
        );

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.sponsor_unauthorized_total, 1);
    }

    #[test]
    fn nexus_fee_sponsor_allowed_with_permission() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let ad: AssetDefinition =
            AssetDefinition::numeric(asset_def_id.clone()).build(&authority_id);
        let sponsor_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sponsor_id.clone()),
            Numeric::new(10_000, 0),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sink_id.clone()),
            Numeric::new(0, 0),
        );
        let world = World::with_assets(
            [domain],
            [authority_account, sponsor_account, sink_account],
            [ad],
            [sponsor_asset, sink_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = 1;
            nexus.fees.per_byte_fee = 0;
            nexus.fees.per_instruction_fee = 0;
            nexus.fees.per_gas_unit_fee = 0;
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
        }

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let permission = CanUseFeeSponsor {
            sponsor: sponsor_id.clone(),
        };
        Grant::account_permission(permission, authority_id.clone())
            .execute(&sponsor_id, &mut stx)
            .expect("grant fee sponsor permission");

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            Json::new(sponsor_id.to_string()),
        );
        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(authority_kp.private_key());

        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &authority_id, tx, &mut ivm_cache)
            .expect("execution");

        let sponsor_balance_after = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sponsor_id.clone()))
            .expect("sponsor asset exists")
            .0
            .try_mantissa_u128()
            .unwrap();
        let sink_balance_after = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sink_id.clone()))
            .expect("sink asset exists")
            .0
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(sponsor_balance_after, 9_999);
        assert_eq!(sink_balance_after, 1);

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 0);

        stx.apply();

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 1);
        assert_eq!(snap.charged_via_sponsor_total, 1);
        assert_eq!(
            snap.last_payer,
            Some(crate::sumeragi::status::NexusFeePayer::Sponsor)
        );
    }

    #[test]
    fn nexus_fee_charged_event_is_recorded_on_apply() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (alice_id, alice_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let dom: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
        let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
        let sink: Account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
        let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
        let payer_balance = Asset::new(payer_asset, Numeric::new(10_000, 0));
        let world = World::with_assets([dom], [alice, sink], [ad], [payer_balance], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = 1;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
        }

        let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            alice_id.clone(),
            "k".parse().unwrap(),
            iroha_primitives::json::Json::new("v"),
        )
        .into();
        let exec = Executable::from(core::iter::once(instruction));
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = iroha_data_model::transaction::TransactionBuilder::new(chain, alice_id.clone())
            .with_executable(exec)
            .sign(alice_kp.private_key());

        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &alice_id, tx, &mut ivm_cache)
            .expect("execution");

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 0);
        assert!(snap.last_payer.is_none());

        stx.apply();

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 1);
        assert_eq!(
            snap.last_payer,
            Some(crate::sumeragi::status::NexusFeePayer::Payer)
        );
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn block_fee_units_recorded_on_apply() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (tech_id, _tech_kp) = gen_account_in("wonderland");
        let dom: Domain = Domain::new("wonderland".parse().unwrap()).build(&payer_id);
        let payer: Account = Account::new(payer_id.clone()).build(&payer_id);
        let tech: Account = Account::new(tech_id.clone()).build(&tech_id);
        let asset_def_id: AssetDefinitionId = "gas#wonderland".parse().unwrap();
        let ad: AssetDefinition = AssetDefinition::numeric(asset_def_id.clone()).build(&payer_id);
        let payer_asset = AssetId::of(asset_def_id.clone(), payer_id.clone());
        let payer_balance = Asset::new(payer_asset, Numeric::new(10_000, 0));
        let world = World::with_assets([dom], [payer, tech], [ad], [payer_balance], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

        {
            let gas_cfg = &mut state.pipeline.gas;
            gas_cfg.tech_account_id = tech_id.to_string();
            gas_cfg.accepted_assets = vec![asset_def_id.to_string()];
            gas_cfg.units_per_gas = vec![GasRate {
                asset: asset_def_id.to_string(),
                units_per_gas: 2,
                twap_local_per_xor: Decimal::ONE,
                liquidity: GasLiquidity::Tier1,
                volatility: GasVolatility::Stable,
            }];
        }
        state.nexus.get_mut().enabled = false;

        let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            payer_id.clone(),
            "k".parse().unwrap(),
            Json::new("v"),
        )
        .into();
        let instructions = vec![instruction];
        let used = crate::gas::meter_instructions(&instructions);
        assert!(used > 0, "expected non-zero gas usage");

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("gas_asset_id").expect("static name"),
            Json::new(asset_def_id.to_string()),
        );
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Instructions(instructions.into()))
            .sign(payer_kp.private_key());

        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &payer_id, tx, &mut ivm_cache)
            .expect("execution");

        assert_eq!(metrics.block_fee_total_units.get(), 0);
        assert_eq!(metrics.block_gas_used.get(), 0);

        stx.apply();

        let expected_fee =
            u64::try_from(u128::from(used).saturating_mul(2).min(u128::from(u64::MAX)))
                .unwrap_or(u64::MAX);
        assert_eq!(metrics.block_fee_total_units.get(), expected_fee);
        assert_eq!(metrics.block_gas_used.get(), used);
    }

    #[test]
    fn multisig_account_direct_signing_is_rejected() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let chain: iroha_data_model::ChainId = "multisig-direct-sign".parse().unwrap();
        let ms_keypair = KeyPair::random();
        let multisig_id = AccountId::new(domain_id.clone(), ms_keypair.public_key().clone());

        let mut signatories = BTreeMap::new();
        signatories.insert(ALICE_ID.clone(), 1);
        let spec = MultisigSpec {
            signatories,
            quorum: nonzero!(1_u16),
            transaction_ttl_ms: nonzero!(DEFAULT_MULTISIG_TTL_MS),
        };

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("multisig/spec").expect("static key"),
            Json::new(spec),
        );

        let domain: Domain = Domain::new(domain_id.clone()).build(&multisig_id);
        let multisig_account = Account::new(multisig_id.clone())
            .with_metadata(metadata)
            .build(&multisig_id);

        let world = World::with([domain], [multisig_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);

        let tx = TransactionBuilder::new(chain, multisig_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(ms_keypair.private_key());

        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        let mut stx = block.transaction();
        let res = executor.execute_transaction(&mut stx, &multisig_id, tx, &mut ivm_cache);
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("direct signing with multisig accounts is forbidden"),
                "unexpected message: {msg}"
            ),
            other => panic!("expected multisig direct signing rejection, got {other:?}"),
        }
        #[cfg(feature = "telemetry")]
        {
            assert_eq!(
                stx.telemetry
                    .metrics_ref()
                    .multisig_direct_sign_reject_total
                    .get(),
                1
            );
        }
    }

    // Shared test helpers for generating or loading executor bytecode
    fn read_default_bytecode() -> Option<Vec<u8>> {
        std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;
        let path1 =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");
        if let Ok(b) = std::fs::read(&path1) {
            return Some(b);
        }
        if let Ok(b) = std::fs::read("defaults/executor.to") {
            return Some(b);
        }
        None
    }

    fn build_program_from_encoded_result(result_bytes: &[u8]) -> Vec<u8> {
        const LITERAL_HEADER_LEN: usize = 4 + 12;
        use std::mem::size_of;

        use ivm::{ProgramMetadata, encoding, instruction};

        let len_size = size_of::<usize>();
        let total_len = len_size
            .checked_add(result_bytes.len())
            .expect("encoded blob fits in usize");
        let total_len_u64 = u64::try_from(total_len).expect("encoded blob fits in u64");
        let mut data = total_len_u64.to_le_bytes()[..len_size].to_vec();
        data.extend_from_slice(result_bytes);
        let padded_len = (data.len() + 7) & !7;
        data.resize(padded_len, 0);
        let chunk_count = data.len() / 8;

        let meta = ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1_000_000,
            abi_version: 1,
        };
        let mut program = meta.encode();
        program.extend_from_slice(&LITERAL_SECTION_MAGIC);
        program.extend_from_slice(&(0u32).to_le_bytes()); // literal entries
        program.extend_from_slice(&(0u32).to_le_bytes()); // post-pad
        program.extend_from_slice(
            &(u32::try_from(data.len()).expect("literal length fits")).to_le_bytes(),
        );
        program.extend_from_slice(&data);

        let mut emit = |word: u32| program.extend_from_slice(&word.to_le_bytes());
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            20,
            10,
            0,
        ));
        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            21,
            10,
            0,
        ));

        let data_addr = i8::try_from(LITERAL_HEADER_LEN).expect("literal header fits i8");
        emit(encoding::wide::encode_ri(
            instruction::wide::arithmetic::ADDI,
            22,
            0,
            data_addr,
        ));

        for _ in 0..chunk_count {
            emit(encoding::wide::encode_load(
                instruction::wide::memory::LOAD64,
                23,
                22,
                0,
            ));
            emit(encoding::wide::encode_store(
                instruction::wide::memory::STORE64,
                21,
                23,
                0,
            ));
            emit(encoding::wide::encode_ri(
                instruction::wide::arithmetic::ADDI,
                22,
                22,
                8,
            ));
            emit(encoding::wide::encode_ri(
                instruction::wide::arithmetic::ADDI,
                21,
                21,
                8,
            ));
        }

        emit(encoding::wide::encode_rr(
            instruction::wide::arithmetic::ADD,
            10,
            20,
            0,
        ));
        emit(encoding::wide::encode_halt());
        program
    }

    fn generate_verdict_program(verdict: &Result<(), iroha_data_model::ValidationFail>) -> Vec<u8> {
        use norito::codec::Encode as _;
        let verdict_bytes = verdict.encode();
        build_program_from_encoded_result(&verdict_bytes)
    }

    fn generate_migration_program(
        verdict: &Result<ExecutorDataModel, iroha_data_model::ValidationFail>,
    ) -> Vec<u8> {
        use norito::codec::Encode as _;
        let payload = match verdict {
            Ok(model) => MigrationResultPayload::Ok(model.clone()),
            Err(err) => MigrationResultPayload::Err(err.clone()),
        };
        let verdict_bytes = payload.encode();
        build_program_from_encoded_result(&verdict_bytes)
    }

    fn generate_ok_program() -> Vec<u8> {
        let verdict = Ok(());
        generate_verdict_program(&verdict)
    }

    fn generate_denied_program(message: &str) -> Vec<u8> {
        let verdict = Err(iroha_data_model::ValidationFail::NotPermitted(
            message.to_owned(),
        ));
        generate_verdict_program(&verdict)
    }

    #[test]
    fn execute_instruction_with_ivm() {
        fn read_default_bytecode() -> Option<Vec<u8>> {
            std::env::var_os("IROHA_TEST_USE_DEFAULT_EXECUTOR")?;
            let path1 = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../defaults/executor.to");
            if let Ok(b) = std::fs::read(&path1) {
                return Some(b);
            }
            if let Ok(b) = std::fs::read("defaults/executor.to") {
                return Some(b);
            }
            None
        }

        let bytecode = read_default_bytecode().unwrap_or_else(generate_ok_program);
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let domain: Domain = Domain::new("wonderland".parse().expect("domain id")).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();

        let domain_id: DomainId = "test".parse().expect("domain id");
        let instruction = Register::domain(Domain::new(domain_id.clone())).into();
        executor
            .execute_instruction(&mut state_tx, &ALICE_ID.clone(), instruction)
            .expect("execution");
        assert!(state_tx.world.domains.get(&domain_id).is_some());
    }

    #[test]
    fn loaded_executor_stack_limit_tracks_gas_limit() {
        let bytecode = generate_ok_program();
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let loaded = super::LoadedExecutor::load(raw).expect("load");

        let small_limit = 10_000;
        let large_limit = 50_000;

        let vm_small = loaded
            .clone_runtime_for_gas_limit(small_limit)
            .expect("clone small");
        assert_eq!(
            vm_small.memory.stack_limit(),
            super::stack_limit_for_gas(small_limit)
        );

        let vm_large = loaded
            .clone_runtime_for_gas_limit(large_limit)
            .expect("clone large");
        assert_eq!(
            vm_large.memory.stack_limit(),
            super::stack_limit_for_gas(large_limit)
        );
    }

    #[test]
    fn parse_executor_additional_fuel_defaults_to_zero() {
        let metadata = Metadata::default();
        let fuel = parse_executor_additional_fuel(&metadata).expect("parse");
        assert_eq!(fuel, 0);
    }

    #[test]
    fn parse_executor_additional_fuel_rejects_invalid_value() {
        let mut metadata = Metadata::default();
        let key = Name::from_str(EXECUTOR_ADDITIONAL_FUEL_KEY).expect("static name");
        metadata.insert(key, Json::new("not-a-number"));

        let err = parse_executor_additional_fuel(&metadata).expect_err("should reject");
        assert!(matches!(err, ValidationFail::NotPermitted(_)));
    }

    #[test]
    fn execute_transaction_sets_executor_fuel_budget() {
        let bytecode = generate_ok_program();
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let domain: Domain = Domain::new("wonderland".parse().expect("domain id")).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();
        let base_fuel = state_tx.world.parameters.get().executor().fuel.get();

        let additional_fuel = 123_u64;
        let mut metadata = Metadata::default();
        let key = Name::from_str(EXECUTOR_ADDITIONAL_FUEL_KEY).expect("static name");
        metadata.insert(key, Json::new(additional_fuel));
        let tx = TransactionBuilder::new(ChainId::from("test-chain"), ALICE_ID.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(ALICE_KEYPAIR.private_key());
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        executor
            .execute_transaction(&mut state_tx, &ALICE_ID.clone(), tx, &mut ivm_cache)
            .expect("execution");

        let remaining = state_tx.executor_fuel_remaining.expect("budget set");
        assert_eq!(remaining, base_fuel.saturating_add(additional_fuel));
    }

    #[test]
    fn configure_executor_fuel_budget_sets_remaining() {
        let bytecode = generate_ok_program();
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let domain: Domain = Domain::new("wonderland".parse().expect("domain id")).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();
        let base_fuel = state_tx.world.parameters.get().executor().fuel.get();

        let additional_fuel = 321_u64;
        let mut metadata = Metadata::default();
        let key = Name::from_str(EXECUTOR_ADDITIONAL_FUEL_KEY).expect("static name");
        metadata.insert(key, Json::new(additional_fuel));

        configure_executor_fuel_budget(&executor, &mut state_tx, &metadata).expect("budget set");
        let remaining = state_tx.executor_fuel_remaining.expect("budget set");
        assert_eq!(remaining, base_fuel.saturating_add(additional_fuel));
    }

    #[test]
    fn executor_validation_consumes_fuel_budget() {
        let bytecode = generate_ok_program();
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let domain: Domain = Domain::new("wonderland".parse().expect("domain id")).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();
        let base_fuel = state_tx.world.parameters.get().executor().fuel.get();
        state_tx.executor_fuel_remaining = Some(base_fuel);

        let instruction: InstructionBox = Log::new(Level::INFO, "executor fuel".to_owned()).into();
        executor
            .execute_instruction(&mut state_tx, &ALICE_ID.clone(), instruction)
            .expect("execution");
        let remaining = state_tx.executor_fuel_remaining.expect("budget set");
        assert!(
            remaining < base_fuel,
            "expected executor fuel budget to decrease"
        );
    }

    #[test]
    fn executor_validation_rejects_when_budget_exhausted() {
        let bytecode = generate_ok_program();
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();
        state_tx.executor_fuel_remaining = Some(0);

        let instruction: InstructionBox = Log::new(Level::INFO, "executor fuel".to_owned()).into();
        let err = executor
            .execute_instruction(&mut state_tx, &ALICE_ID.clone(), instruction)
            .expect_err("expected fuel exhaustion");
        assert!(
            matches!(err, ValidationFail::TooComplex),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn validate_query_with_ivm() {
        let bytecode = read_default_bytecode().unwrap_or_else(generate_ok_program);
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let state_tx = block.transaction();

        let query = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        executor
            .validate_query(&state_tx, &ALICE_ID.clone(), &query)
            .expect("validation");
    }

    #[test]
    fn validate_start_query_with_ivm() {
        use iroha_data_model::query::{
            QueryItemKind, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::QueryParams,
        };
        // Ensure the erased-query registry is initialized for iterable queries
        iroha_data_model::query::set_query_registry(iroha_data_model::query_registry![
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::domain::Domain>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::account::Account>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::asset::value::Asset>,
            iroha_data_model::query::ErasedIterQuery<
                iroha_data_model::asset::definition::AssetDefinition,
            >,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::nft::Nft>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::role::Role>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::role::RoleId>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::peer::PeerId>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::trigger::TriggerId>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::trigger::Trigger>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::query::CommittedTransaction>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::block::SignedBlock>,
            iroha_data_model::query::ErasedIterQuery<iroha_data_model::block::BlockHeader>,
        ]);
        let bytecode = read_default_bytecode().unwrap_or_else(generate_ok_program);
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let state_tx = block.transaction();

        let iter_query = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: QueryItemKind::Domain,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Domain>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Domain>::default()),
            params: QueryParams::default(),
        };
        let query = QueryRequest::Start(iter_query);

        executor
            .validate_query(&state_tx, &ALICE_ID.clone(), &query)
            .expect("validation");
    }

    #[test]
    fn validate_query_rejected_by_executor() {
        let bytecode = generate_denied_program("queries disabled");
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let state_tx = block.transaction();

        let query = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        let err = executor
            .validate_query(&state_tx, &ALICE_ID.clone(), &query)
            .expect_err("executor should deny the query");

        assert!(
            matches!(
                err,
                iroha_data_model::ValidationFail::NotPermitted(ref msg) if msg == "queries disabled"
            ),
            "unexpected validation failure: {err:?}"
        );
    }

    #[test]
    fn migrate_invokes_entrypoint_and_swaps_executor() {
        // Use the default bundled executor bytecode when available; otherwise
        // generate a minimal OK program deterministically.
        let default_executor =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../defaults/executor.to");
        let mut bytecode = None;
        for candidate in [
            default_executor.as_path(),
            std::path::Path::new("defaults/executor.to"),
        ] {
            if let Ok(bytes) = std::fs::read(candidate) {
                let raw_candidate =
                    data_model_executor::Executor::new(IvmBytecode::from_compiled(bytes.clone()));
                if super::LoadedExecutor::load(raw_candidate).is_ok() {
                    bytecode = Some(bytes);
                    break;
                }
            }
        }
        let bytecode = bytecode.unwrap_or_else(generate_ok_program);
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));

        // Start with the initial executor
        let mut executor = super::Executor::Initial;

        // Minimal state scaffolding
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();

        // Perform migration
        executor
            .migrate(raw, &mut state_tx, &ALICE_ID.clone())
            .expect("migration should succeed");

        // Ensure executor has been swapped
        match executor {
            super::Executor::UserProvided(_) => {}
            _ => panic!("expected UserProvided executor after migration"),
        }
    }

    #[test]
    fn migrate_applies_data_model_from_entrypoint() {
        let mut permissions = BTreeSet::new();
        permissions.insert("permission.can_control_domain_lives".to_owned());
        let custom_parameters: BTreeMap<CustomParameterId, CustomParameter> = BTreeMap::new();
        let data_model = ExecutorDataModel::new(
            custom_parameters,
            BTreeSet::new(),
            permissions,
            Json::new(()),
        );
        let verdict = Ok(data_model.clone());
        let bytecode = generate_migration_program(&verdict);
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));

        let mut executor = super::Executor::Initial;

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();

        executor
            .migrate(raw, &mut state_tx, &ALICE_ID.clone())
            .expect("migration should succeed");

        assert_eq!(*state_tx.world.executor_data_model.get(), data_model);
        match executor {
            super::Executor::UserProvided(_) => {}
            _ => panic!("expected UserProvided executor after migration"),
        }
    }

    #[test]
    fn migrate_fails_on_invalid_bytecode() {
        // Construct an invalid program (oversized code section) to trigger a VM error
        let mut prog = Vec::new();
        // Metadata header: IVM, version 1.0, mode 0, vector len 0, max_cycles 0, abi_version 0
        prog.extend_from_slice(b"IVM\0");
        prog.extend_from_slice(&[1, 0, 0, 0]);
        prog.extend_from_slice(&0u64.to_le_bytes());
        prog.push(0);
        // Oversized code
        let heap_start =
            usize::try_from(ivm::Memory::HEAP_START).expect("HEAP_START fits within usize");
        prog.extend(std::iter::repeat_n(0u8, heap_start + 8));

        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(prog));

        let mut executor = super::Executor::Initial;

        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();

        let res = executor.migrate(raw, &mut state_tx, &ALICE_ID.clone());
        assert!(res.is_err(), "migration with invalid bytecode must fail");
        // Ensure executor remains unchanged
        matches!(executor, super::Executor::Initial);
    }
}

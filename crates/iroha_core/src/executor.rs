// Detached executor note: Keep this handler minimal and side‑effect free; only record
// deltas. Prefer performing complex checks during merge in `StateBlock::merge_into`.
// Extend cautiously when adding new ISIs (Peer, Parameters, ExecuteTrigger, etc.).
//! Structures and impls related to processing Iroha Virtual Machine (IVM)
//! runtime executors.

use core::{
    convert::TryFrom,
    ops::{Deref, DerefMut},
    str::FromStr,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

use base64::Engine as _;
use derive_more::Debug;
use iroha_config::parameters::actual::{GasLiquidity, GasVolatility, NexusFees};
use iroha_data_model::{
    Identifiable as _, Registrable as _, ValidationFail,
    account::{AccountId, address::AccountAddress},
    asset::{
        AssetBalancePolicy, AssetDefinition,
        id::{AssetBalanceScope, AssetDefinitionId, AssetId},
        value::Asset,
    },
    block::{BlockHeader, consensus::NexusFeeScheduleInputs},
    executor::{self as data_model_executor, ExecutorDataModel},
    isi::{
        CustomInstruction, GrantBox, InstructionBox, InstructionBox as DMInstructionBox,
        RemoveKeyValueBox, RevokeBox, SetKeyValueBox, TransferBox,
        error::InstructionExecutionError, mint_burn::MintBox, register::RegisterBox,
    },
    metadata::Metadata,
    name::Name,
    nexus::{
        DataSpaceId, FeeSponsorContractSelector, FeeSponsorExecutableKind, FeeSponsorPolicy,
        FeeSponsorPolicyId, FeeSponsorRule, FeeSponsorRuleEffect,
        VERIFIED_LANE_RELAY_STATE_KEY_PREFIX, VerifiedLaneRelayRecord,
        VerifiedNexusFeeBudgetRecord,
    },
    parameter::{CustomParameter, CustomParameterId},
    permission::Permission,
    prelude::{Account, Burn, Domain, DomainId, Register, Transfer, Trigger},
    query::{AnyQueryBox, QueryRequest},
    role::{Role, RoleId},
    smart_contract::payloads::{ExecutorContext, Validate as ValidatePayload},
    transaction::{Executable, SignedTransaction, executable::ContractInvocation},
};
use iroha_executor_data_model::{
    isi::multisig::MultisigInstructionBox, permission as executor_permission,
};
use iroha_logger::{debug, trace, warn};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec, Quantity},
};
use ivm::runtime::IvmConfig;
use ivm::{IVM, Memory, RuntimeTemplate, VMError};
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
    settlement::{PendingNexusFeeReceipt, PendingSettlement, QuoteError, VolatilityBucket},
    smartcontracts::{
        Execute as _, code,
        ivm::cache::{ExecutableProgramSummary, IvmCache},
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
    sumeragi::status::{self as sumeragi_status, NexusFeeEvent, NexusFeePayer},
};
// NoritoDecode alias is unused; keep Decode via norito::codec where needed inline

#[cfg(test)]
const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";

#[cfg(test)]
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
    program.extend_from_slice(&(0u32).to_le_bytes());
    program.extend_from_slice(&(0u32).to_le_bytes());
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

#[cfg(test)]
fn generate_verdict_program(verdict: &Result<(), ValidationFail>) -> Vec<u8> {
    let verdict_bytes = verdict.encode();
    build_program_from_encoded_result(&verdict_bytes)
}

/// Build a user executor that rejects every validation request with a stable message.
#[cfg(test)]
pub(crate) fn denying_executor_for_testing(message: &str) -> Executor {
    let verdict = Err(ValidationFail::NotPermitted(message.to_owned()));
    let bytecode = generate_verdict_program(&verdict);
    let raw = data_model_executor::Executor::new(
        iroha_data_model::transaction::executable::IvmBytecode::from_compiled(bytecode),
    );
    Executor::UserProvided(LoadedExecutor::load(raw).expect("load deny-all test executor"))
}

const EXECUTOR_ADDITIONAL_FUEL_KEY: &str = "additional_fuel";
const SORA_V2_CLAIM_TX_HASH_METADATA_KEY: &str = "sora_v2_claim_tx_hash";
const SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY: &str = "sora_nexus_claim_recipient";
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
                let qty = m.object.clone().into_numeric();
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
                let qty = b.object.clone().into_numeric();
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
                let qty = t.object.clone().into_numeric();
                delta.transfer_asset(src, t.destination.clone(), qty);
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
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    metadata: &Metadata,
) -> Result<Option<AccountId>, ValidationFail> {
    let Some(raw) = metadata.get("fee_sponsor") else {
        return Ok(None);
    };
    match raw.try_into_any_norito::<AccountId>() {
        Ok(sponsor) => Ok(Some(sponsor)),
        Err(err) => {
            if let Ok(literal) = raw.try_into_any_norito::<String>()
                && let Some(sponsor) = crate::block::parse_account_literal_with_world(
                    world,
                    dataspace_catalog,
                    &literal,
                )
            {
                return Ok(Some(sponsor));
            }
            Err(ValidationFail::NotPermitted(format!(
                "invalid fee_sponsor metadata: expected canonical I105 account id or on-chain alias ({err})"
            )))
        }
    }
}

fn execute_system_fee_instruction(
    instr: DMInstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let previous_tx_dataspace_id = state_transaction.current_dataspace_id;
    let previous_world_dataspace_id = state_transaction.world.current_dataspace_id;
    state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    let result = instr.execute(authority, state_transaction);
    state_transaction.current_dataspace_id = previous_tx_dataspace_id;
    state_transaction.world.current_dataspace_id = previous_world_dataspace_id;
    result
}

fn execute_gas_fee_transfer_instruction(
    definition: &AssetDefinition,
    instr: DMInstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if definition.balance_scope_policy() == AssetBalancePolicy::Global {
        execute_system_fee_instruction(instr, authority, state_transaction)
    } else {
        instr.execute(authority, state_transaction)
    }
}

fn resolve_effective_fee_sponsor(
    world: &impl WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    dataspace_fee_sponsors: &BTreeMap<DataSpaceId, String>,
    metadata: &Metadata,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<Option<AccountId>, ValidationFail> {
    if let Some(explicit_sponsor) = parse_fee_sponsor(world, dataspace_catalog, metadata)? {
        return Ok(Some(explicit_sponsor));
    }

    let Some(dataspace_id) = route_dataspace_id else {
        return Ok(None);
    };
    crate::state::dataspace_fee_sponsor_from_config(
        world,
        dataspace_catalog,
        dataspace_fee_sponsors,
        dataspace_id,
    )
}

fn metadata_string(metadata: &Metadata, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|raw| raw.try_into_any_norito::<String>().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn should_charge_pipeline_gas_asset(
    skip_nexus_fee: bool,
    nexus_enabled: bool,
    nexus_fees: &NexusFees,
    gas_asset_opt: &Option<String>,
) -> bool {
    !skip_nexus_fee
        && gas_asset_opt.is_some()
        && (!nexus_enabled || nexus_fees.per_gas_unit_fee.is_zero())
}

fn is_sora_v2_tx_hash_literal(value: &str) -> bool {
    let hex = value.strip_prefix("0x").unwrap_or(value);
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn account_literal_matches(
    world: &impl WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    literal: &str,
    expected: &AccountId,
) -> bool {
    if let Ok(canonical) = AccountId::canonicalize(literal)
        && expected
            .canonical_i105()
            .ok()
            .as_deref()
            .is_some_and(|expected| expected == canonical)
    {
        return true;
    }

    crate::block::parse_account_literal_with_world(world, dataspace_catalog, literal)
        .as_ref()
        .is_some_and(|account| account == expected)
}

fn successful_claim_fee_authority_allowed(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    authority: &AccountId,
) -> bool {
    nexus
        .fees
        .successful_claim_fee_exempt_authorities
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|literal| !literal.is_empty())
        .any(|literal| {
            account_literal_matches(world, world.dataspace_catalog(), literal, authority)
        })
}

fn successful_claim_fee_exempt_instructions(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    authority: &AccountId,
    metadata: &Metadata,
    instructions: &[InstructionBox],
    observation_time_ms: u64,
) -> bool {
    if !successful_claim_fee_authority_allowed(world, nexus, authority) {
        return false;
    }

    let Some(claim_tx_hash) = metadata_string(metadata, SORA_V2_CLAIM_TX_HASH_METADATA_KEY) else {
        return false;
    };
    if !is_sora_v2_tx_hash_literal(&claim_tx_hash) {
        return false;
    }

    let Some(recipient) = metadata_string(metadata, SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY)
        .and_then(|literal| parse_account_id_literal(world, world.dataspace_catalog(), &literal))
    else {
        return false;
    };

    let Some(asset_def) = crate::block::parse_asset_definition_literal_with_world(
        world,
        &nexus.fees.fee_asset_id,
        observation_time_ms,
    ) else {
        return false;
    };

    let [instruction] = instructions else {
        return false;
    };

    let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() else {
        return false;
    };

    match mint {
        MintBox::Asset(mint) => {
            mint.destination.account() == &recipient
                && mint.destination.definition() == &asset_def
                && !mint.object.is_zero()
        }
        MintBox::TriggerRepetitions(_) => false,
    }
}

fn successful_claim_fee_exempt_transaction(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
) -> bool {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return false;
    };
    successful_claim_fee_exempt_instructions(
        world,
        nexus,
        transaction.authority(),
        transaction.metadata(),
        instructions.as_ref(),
        observation_time_ms,
    )
}

fn nexus_protocol_fee_exempt_instruction(instruction: &InstructionBox) -> bool {
    let any = instruction.as_any();
    any.downcast_ref::<iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay>()
        .is_some()
        || any
            .downcast_ref::<iroha_data_model::isi::nexus::RegisterVerifiedNexusFeeBudget>()
            .is_some()
}

fn nexus_fee_exempt_instruction(instruction: &InstructionBox) -> bool {
    nexus_protocol_fee_exempt_instruction(instruction)
}

fn nexus_fee_exempt_instructions(instructions: &[InstructionBox]) -> bool {
    !instructions.is_empty() && instructions.iter().all(nexus_fee_exempt_instruction)
}

fn nexus_fee_exempt_transaction(transaction: &SignedTransaction) -> bool {
    if crate::tx::is_heartbeat_transaction(transaction) {
        return true;
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return false;
    };
    nexus_fee_exempt_instructions(instructions.as_ref())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RedeemFundedNexusFeeCapacity {
    payer: AccountId,
    capacity: Numeric,
}

fn redeem_funded_nexus_fee_capacity(
    world: &impl WorldReadOnly,
    cfg: &iroha_config::parameters::actual::NexusFees,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
    has_fee_sponsor: bool,
) -> Result<Option<RedeemFundedNexusFeeCapacity>, NexusFeeAdmissionError> {
    if has_fee_sponsor {
        return Ok(None);
    }

    let payer = transaction.authority();
    let instructions: &[InstructionBox] = match transaction.instructions() {
        Executable::Instructions(instructions) => instructions.as_ref(),
        Executable::IvmProved(proved) => proved.overlay.as_ref(),
        Executable::ContractCall(_) | Executable::Ivm(_) => return Ok(None),
    };

    let mut candidate_redeems: Vec<(AssetDefinitionId, Numeric)> = Vec::new();
    for instruction in instructions {
        let any = instruction.as_any();
        if let Some(redeem) =
            any.downcast_ref::<iroha_data_model::isi::offline::RedeemKagemushaRecursiveV2>()
        {
            // Do not admit a transaction against credit that execution cannot
            // produce.  The public V2 wire type remains decodable while its
            // recursive proof backend is unavailable, but Core rejects the
            // instruction before mutating balances.  Returning `None` also
            // denies mixed batches rather than letting another redeem mask the
            // unsupported instruction.
            // TODO: Remove this gate only when the V2 proof backend and complete
            // Core execution path ship atomically.
            if !iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE {
                return Ok(None);
            }
            if &redeem.request.recipient != payer {
                return Ok(None);
            }
            candidate_redeems.push((
                redeem.request.bundle.statement.asset.clone(),
                redeem.request.amount.public_numeric(),
            ));
            continue;
        }
        return Ok(None);
    }

    if candidate_redeems.is_empty() {
        return Ok(None);
    }

    let Some(fee_asset_def) = crate::block::parse_asset_definition_literal_with_world(
        world,
        &cfg.fee_asset_id,
        observation_time_ms,
    ) else {
        return Ok(None);
    };

    let mut redeemed_amount = Numeric::zero();
    for (asset_def, amount) in candidate_redeems {
        if asset_def != fee_asset_def {
            return Ok(None);
        }
        redeemed_amount = checked_nexus_fee_add(redeemed_amount, amount, "offline redeem amount")?;
    }

    if redeemed_amount <= Numeric::zero() {
        return Ok(None);
    }

    let payer_asset = AssetId::new(fee_asset_def, payer.clone());
    let existing_balance = world
        .assets()
        .get(&payer_asset)
        .map_or_else(Numeric::zero, |balance| {
            balance.as_ref().as_numeric().clone()
        });
    let capacity = checked_nexus_fee_add(
        existing_balance,
        redeemed_amount,
        "offline redeem-funded fee capacity",
    )?;

    Ok(Some(RedeemFundedNexusFeeCapacity {
        payer: payer.clone(),
        capacity,
    }))
}

fn redeem_funded_nexus_fee_covers(
    world: &impl WorldReadOnly,
    cfg: &iroha_config::parameters::actual::NexusFees,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
    next_block_height: u64,
    has_fee_sponsor: bool,
    fee: &Numeric,
    in_flight_fees: Numeric,
) -> Result<bool, NexusFeeAdmissionError> {
    let Some(capacity) = redeem_funded_nexus_fee_capacity(
        world,
        cfg,
        transaction,
        observation_time_ms,
        has_fee_sponsor,
    )?
    else {
        return Ok(false);
    };

    let mut required = fee.clone();
    if cfg.lane_relay_burn_receipts_active_at(next_block_height) {
        let unsettled =
            unsettled_verified_nexus_fee_amount(world, &capacity.payer, cfg.fee_asset_id.as_str())?;
        required = checked_nexus_fee_add(required, unsettled, "unsettled receipts")?;
        required = checked_nexus_fee_add(required, in_flight_fees, "in-flight receipts")?;
    }

    Ok(capacity.capacity >= required)
}

fn check_redeem_funded_lane_relay_fee_balance(
    world: &impl WorldReadOnly,
    cfg: &iroha_config::parameters::actual::NexusFees,
    payer: &AccountId,
    observation_time_ms: u64,
    fee: &Numeric,
    in_flight_fees: Numeric,
) -> Result<(), NexusFeeAdmissionError> {
    let fee_asset_def = crate::block::parse_asset_definition_literal_with_world(
        world,
        &cfg.fee_asset_id,
        observation_time_ms,
    )
    .ok_or_else(|| {
        NexusFeeAdmissionError::ConfigInvalid(
            "invalid nexus fee asset id; expected canonical Base58 asset definition id or active asset alias"
                .to_owned(),
        )
    })?;
    let payer_asset = AssetId::new(fee_asset_def, payer.clone());
    let available = world
        .assets()
        .get(&payer_asset)
        .map_or_else(Numeric::zero, |balance| {
            balance.as_ref().as_numeric().clone()
        });
    let unsettled = unsettled_verified_nexus_fee_amount(world, payer, cfg.fee_asset_id.as_str())?;
    let required = checked_nexus_fee_add(fee.clone(), unsettled, "unsettled receipts")?;
    let required = checked_nexus_fee_add(required, in_flight_fees, "in-flight receipts")?;
    if available < required {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "redeemed offline fee balance for payer `{payer}` is insufficient: requires {required}, available {available}"
        )));
    }
    Ok(())
}

fn parse_account_id_literal(
    world: &impl WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    literal: &str,
) -> Option<AccountId> {
    crate::block::parse_account_literal_with_world(world, dataspace_catalog, literal)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NexusFeeAdmissionError {
    Rejected(String),
    ConfigInvalid(String),
}

fn smart_contract_state_name(
    raw: String,
    context: &'static str,
) -> Result<Name, NexusFeeAdmissionError> {
    Name::from_str(&raw).map_err(|_| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "invalid smart-contract state key: {context}"
        ))
    })
}

fn verified_nexus_fee_budget_state_key(
    sponsor: &AccountId,
    fee_asset_id: &str,
) -> Result<Name, NexusFeeAdmissionError> {
    smart_contract_state_name(
        VerifiedNexusFeeBudgetRecord::state_key_for(sponsor, fee_asset_id),
        "verified Nexus fee budget",
    )
}

fn nexus_fee_receipt_marker_key(source_id: &[u8; 32]) -> Result<Name, NexusFeeAdmissionError> {
    smart_contract_state_name(
        format!("nexus_fee_receipt_settled_{}", hex::encode(source_id)),
        "settled Nexus fee receipt",
    )
}

fn decode_verified_nexus_fee_budget_record_state(
    payload: &[u8],
) -> Result<VerifiedNexusFeeBudgetRecord, NexusFeeAdmissionError> {
    let json: Json = norito::decode_from_bytes(payload).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified Nexus fee budget state decode failed: {err}"
        ))
    })?;
    norito::json::from_slice(json.get().as_bytes()).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified Nexus fee budget JSON decode failed: {err}"
        ))
    })
}

fn decode_verified_lane_relay_record_state(
    payload: &[u8],
) -> Result<VerifiedLaneRelayRecord, NexusFeeAdmissionError> {
    let json: Json = norito::decode_from_bytes(payload).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified lane relay state decode failed: {err}"
        ))
    })?;
    norito::json::from_slice(json.get().as_bytes()).map_err(|err| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified lane relay JSON decode failed: {err}"
        ))
    })
}

fn checked_nexus_fee_add(
    lhs: Numeric,
    rhs: Numeric,
    context: &'static str,
) -> Result<Numeric, NexusFeeAdmissionError> {
    lhs.checked_add(rhs).ok_or_else(|| {
        NexusFeeAdmissionError::ConfigInvalid(format!(
            "Nexus fee budget arithmetic overflow while adding {context}"
        ))
    })
}

fn unsettled_verified_nexus_fee_amount(
    world: &impl WorldReadOnly,
    payer: &AccountId,
    fee_asset_id: &str,
) -> Result<Numeric, NexusFeeAdmissionError> {
    let mut total = Numeric::zero();
    for (key, payload) in world.smart_contract_state().iter() {
        if !key
            .to_string()
            .starts_with(VERIFIED_LANE_RELAY_STATE_KEY_PREFIX)
        {
            continue;
        }
        let record = decode_verified_lane_relay_record_state(payload)?;
        for receipt in &record
            .relay_envelope
            .settlement_commitment
            .nexus_fee_receipts
        {
            if &receipt.payer_account_id != payer || receipt.fee_asset_id != fee_asset_id {
                continue;
            }
            let marker = nexus_fee_receipt_marker_key(&receipt.source_id)?;
            if world.smart_contract_state().get(&marker).is_some() {
                continue;
            }
            total = checked_nexus_fee_add(
                total,
                receipt.fee_amount.as_numeric().clone(),
                "unsettled receipts",
            )?;
        }
    }
    Ok(total)
}

fn check_lane_relay_burn_fee_budget(
    world: &impl WorldReadOnly,
    cfg: &iroha_config::parameters::actual::NexusFees,
    payer: &AccountId,
    fee: &Numeric,
    in_flight_fees: Numeric,
) -> Result<(), NexusFeeAdmissionError> {
    let key = verified_nexus_fee_budget_state_key(payer, cfg.fee_asset_id.as_str())?;
    let payload = world.smart_contract_state().get(&key).ok_or_else(|| {
        NexusFeeAdmissionError::Rejected(format!(
            "missing verified Nexus fee budget for payer `{payer}` and asset `{}`",
            cfg.fee_asset_id
        ))
    })?;
    let record = decode_verified_nexus_fee_budget_record_state(payload)?;
    if record.sponsor_account_id != *payer || record.fee_asset_id != cfg.fee_asset_id {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "verified Nexus fee budget record does not match payer `{payer}` and asset `{}`",
            cfg.fee_asset_id
        )));
    }
    if record.verified_balance.mantissa().is_negative() {
        return Err(NexusFeeAdmissionError::ConfigInvalid(format!(
            "verified Nexus fee budget for payer `{payer}` has a negative balance"
        )));
    }
    if record.manifest_root.iter().all(|byte| *byte == 0)
        || record.fastpq_binding.verified_effect_type != "nexus_fee_budget"
    {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "verified Nexus fee budget for payer `{payer}` is invalid"
        )));
    }

    let unsettled = unsettled_verified_nexus_fee_amount(world, payer, cfg.fee_asset_id.as_str())?;
    let required = checked_nexus_fee_add(fee.clone(), unsettled, "current fee")?;
    let required = checked_nexus_fee_add(required, in_flight_fees, "in-flight receipts")?;
    let required = checked_nexus_fee_add(
        required,
        cfg.sponsor_verified_balance_safety_floor
            .as_numeric()
            .clone(),
        "safety floor",
    )?;
    if record.verified_balance < required {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "verified Nexus fee budget for payer `{payer}` is insufficient: requires {required}, available {}",
            record.verified_balance
        )));
    }

    Ok(())
}

fn check_lane_relay_burn_canonical_sponsor(
    world: &impl WorldReadOnly,
    cfg: &iroha_config::parameters::actual::NexusFees,
    payer: &AccountId,
) -> Result<(), NexusFeeAdmissionError> {
    let raw = cfg.canonical_sponsor_account_id.as_deref().ok_or_else(|| {
        NexusFeeAdmissionError::ConfigInvalid(
            "nexus.fees.canonical_sponsor_account_id must be configured for activated lane-relay-burn fee settlement"
                .to_owned(),
        )
    })?;
    let canonical = parse_account_id_literal(world, world.dataspace_catalog(), raw).ok_or_else(
        || {
            NexusFeeAdmissionError::ConfigInvalid(
                "invalid nexus.fees.canonical_sponsor_account_id; expected canonical I105 account id or on-chain alias"
                    .to_owned(),
            )
        },
    )?;
    if &canonical != payer {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "activated lane-relay-burn fees require canonical sponsor `{canonical}`; got `{payer}`"
        )));
    }
    Ok(())
}

fn validation_fail_to_nexus_fee_admission_error(err: ValidationFail) -> NexusFeeAdmissionError {
    match err {
        ValidationFail::InternalError(reason) => NexusFeeAdmissionError::ConfigInvalid(reason),
        other => NexusFeeAdmissionError::Rejected(other.to_string()),
    }
}

fn nexus_fee_admission_error_to_validation_fail(err: NexusFeeAdmissionError) -> ValidationFail {
    match err {
        NexusFeeAdmissionError::Rejected(reason) => ValidationFail::NotPermitted(reason),
        NexusFeeAdmissionError::ConfigInvalid(reason) => ValidationFail::InternalError(reason),
    }
}

#[cfg(test)]
pub(crate) fn can_use_fee_sponsor_read_only(
    world: &impl WorldReadOnly,
    caller: &AccountId,
    sponsor: &AccountId,
    nexus: &iroha_config::parameters::actual::Nexus,
    route_dataspace_id: Option<DataSpaceId>,
) -> bool {
    !fee_sponsor_policy_ids_read_only(world, caller, sponsor, nexus, route_dataspace_id).is_empty()
}

pub(crate) fn fee_sponsor_policy_ids_read_only(
    world: &impl WorldReadOnly,
    caller: &AccountId,
    sponsor: &AccountId,
    nexus: &iroha_config::parameters::actual::Nexus,
    route_dataspace_id: Option<DataSpaceId>,
) -> BTreeSet<FeeSponsorPolicyId> {
    let dataspace_catalog = world.dataspace_catalog();
    let mut policy_ids = BTreeSet::new();
    let mut collect_policy = |permission: &Permission| {
        if let Some(policy_id) =
            crate::state::fee_sponsor_policy_from_permission(world, dataspace_catalog, permission)
            && policy_id.sponsor.subject_id() == sponsor.subject_id()
        {
            policy_ids.insert(policy_id);
        }
    };

    if let Some(permissions) = world.account_permissions().get(caller) {
        for permission in permissions {
            collect_policy(permission);
        }
    }

    for role in world
        .account_roles_iter(caller)
        .filter_map(|role_id| world.roles().get(role_id))
    {
        for permission in &role.permissions {
            collect_policy(permission);
        }
    }

    if let Some(dataspace_id) = route_dataspace_id
        && let Ok(Some(default_policy)) = crate::state::dataspace_fee_sponsor_policy_from_config(
            world,
            dataspace_catalog,
            &nexus.dataspace_fee_sponsors,
            &nexus.dataspace_fee_sponsor_policies,
            dataspace_id,
        )
        && default_policy.sponsor.subject_id() == sponsor.subject_id()
    {
        policy_ids.insert(default_policy);
    }

    policy_ids
}

#[derive(Clone, Debug)]
struct FeeSponsorOperation {
    kind: FeeSponsorExecutableKind,
    instruction_wire_id: Option<String>,
    contract_address: Option<iroha_data_model::smart_contract::ContractAddress>,
    contract_entrypoint: Option<String>,
}

fn fee_sponsor_executable_kind(executable: &Executable) -> FeeSponsorExecutableKind {
    match executable {
        Executable::Instructions(_) => FeeSponsorExecutableKind::Instructions,
        Executable::ContractCall(_) => FeeSponsorExecutableKind::ContractCall,
        Executable::Ivm(_) => FeeSponsorExecutableKind::Ivm,
        Executable::IvmProved(_) => FeeSponsorExecutableKind::IvmProved,
    }
}

fn fee_sponsor_operations(
    transaction: &SignedTransaction,
) -> Result<Vec<FeeSponsorOperation>, NexusFeeAdmissionError> {
    match transaction.instructions() {
        Executable::Instructions(instructions) => instructions
            .iter()
            .map(|instruction| {
                let wire_id = iroha_data_model::isi::instruction_wire_id(instruction)
                    .ok_or_else(|| {
                        NexusFeeAdmissionError::Rejected(
                            "fee sponsor policy could not resolve native instruction wire id"
                                .to_owned(),
                        )
                    })?
                    .to_owned();
                Ok(FeeSponsorOperation {
                    kind: FeeSponsorExecutableKind::Instructions,
                    instruction_wire_id: Some(wire_id),
                    contract_address: None,
                    contract_entrypoint: None,
                })
            })
            .collect(),
        Executable::ContractCall(invocation) => Ok(vec![FeeSponsorOperation {
            kind: FeeSponsorExecutableKind::ContractCall,
            instruction_wire_id: None,
            contract_address: Some(invocation.contract_address.clone()),
            contract_entrypoint: Some(invocation.entrypoint.clone()),
        }]),
        Executable::Ivm(_) => Ok(vec![FeeSponsorOperation {
            kind: FeeSponsorExecutableKind::Ivm,
            instruction_wire_id: None,
            contract_address: None,
            contract_entrypoint: None,
        }]),
        Executable::IvmProved(proved) => proved
            .overlay
            .iter()
            .map(|instruction| {
                let wire_id = iroha_data_model::isi::instruction_wire_id(instruction)
                    .ok_or_else(|| {
                        NexusFeeAdmissionError::Rejected(
                            "fee sponsor policy could not resolve proved overlay instruction wire id"
                                .to_owned(),
                        )
                    })?
                    .to_owned();
                Ok(FeeSponsorOperation {
                    kind: FeeSponsorExecutableKind::IvmProved,
                    instruction_wire_id: Some(wire_id),
                    contract_address: None,
                    contract_entrypoint: None,
                })
            })
            .collect(),
    }
}

fn contract_selector_matches(
    world: &impl WorldReadOnly,
    selector: &FeeSponsorContractSelector,
    operation: &FeeSponsorOperation,
) -> bool {
    let Some(address) = operation.contract_address.as_ref() else {
        return false;
    };
    if selector
        .contract_address
        .as_ref()
        .is_some_and(|selected| selected != address)
    {
        return false;
    }
    if let Some(alias) = selector.contract_alias.as_ref() {
        let alias_matches = world
            .contract_aliases()
            .get(alias)
            .is_some_and(|target| target == address)
            || world
                .contract_alias_bindings()
                .get(address)
                .is_some_and(|binding| &binding.alias == alias);
        if !alias_matches {
            return false;
        }
    }
    selector.entrypoints.is_empty()
        || operation
            .contract_entrypoint
            .as_ref()
            .is_some_and(|entrypoint| selector.entrypoints.contains(entrypoint))
}

fn fee_sponsor_rule_matches_operation(
    world: &impl WorldReadOnly,
    rule: &FeeSponsorRule,
    dataspace_id: DataSpaceId,
    operation: &FeeSponsorOperation,
) -> bool {
    if !rule.dataspaces.is_empty() && !rule.dataspaces.contains(&dataspace_id) {
        return false;
    }
    if !rule.executable_kinds.is_empty() && !rule.executable_kinds.contains(&operation.kind) {
        return false;
    }
    if !rule.instruction_wire_ids.is_empty()
        && !operation
            .instruction_wire_id
            .as_ref()
            .is_some_and(|wire_id| rule.instruction_wire_ids.contains(wire_id))
    {
        return false;
    }
    if !rule.contract_selectors.is_empty()
        && !rule
            .contract_selectors
            .iter()
            .any(|selector| contract_selector_matches(world, selector, operation))
    {
        return false;
    }
    true
}

fn fee_sponsor_policy_allows_transaction(
    world: &impl WorldReadOnly,
    policy: &FeeSponsorPolicy,
    transaction: &SignedTransaction,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<bool, NexusFeeAdmissionError> {
    if !policy.enabled {
        return Ok(false);
    }
    let dataspace_id = route_dataspace_id.unwrap_or(DataSpaceId::UNIVERSAL);
    let operations = fee_sponsor_operations(transaction)?;
    if operations.is_empty() {
        return Ok(false);
    }

    for operation in &operations {
        if policy.rules.iter().any(|rule| {
            rule.effect == FeeSponsorRuleEffect::Deny
                && fee_sponsor_rule_matches_operation(world, rule, dataspace_id, operation)
        }) {
            return Ok(false);
        }
        let allowed = policy.rules.iter().any(|rule| {
            rule.effect == FeeSponsorRuleEffect::Allow
                && fee_sponsor_rule_matches_operation(world, rule, dataspace_id, operation)
        });
        if !allowed {
            return Ok(false);
        }
    }

    let executable_kind = fee_sponsor_executable_kind(transaction.instructions());
    if policy.rules.iter().any(|rule| {
        rule.effect == FeeSponsorRuleEffect::Deny
            && rule.instruction_wire_ids.is_empty()
            && rule.contract_selectors.is_empty()
            && (rule.executable_kinds.is_empty()
                || rule.executable_kinds.contains(&executable_kind))
            && (rule.dataspaces.is_empty() || rule.dataspaces.contains(&dataspace_id))
    }) {
        return Ok(false);
    }

    Ok(true)
}

fn authorize_fee_sponsor_policy_from_ids(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    sponsor: &AccountId,
    policy_ids: impl IntoIterator<Item = FeeSponsorPolicyId>,
    transaction: &SignedTransaction,
    fee: &Numeric,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<FeeSponsorPolicyId, NexusFeeAdmissionError> {
    for policy_id in policy_ids {
        if policy_id.sponsor.subject_id() != sponsor.subject_id() {
            continue;
        }
        let configured_policy =
            configured_default_fee_sponsor_policy(world, nexus, &policy_id, route_dataspace_id);
        let Some(policy) = world
            .fee_sponsor_policies()
            .get(&policy_id)
            .or(configured_policy.as_ref())
        else {
            continue;
        };
        if policy.id.sponsor.subject_id() != sponsor.subject_id() {
            continue;
        }
        if let Some(max_fee) = &policy.max_fee
            && fee > max_fee.as_numeric()
        {
            continue;
        }
        if fee_sponsor_policy_allows_transaction(world, policy, transaction, route_dataspace_id)? {
            return Ok(policy_id);
        }
    }

    Err(NexusFeeAdmissionError::Rejected(
        "fee sponsor policy is not authorized".to_owned(),
    ))
}

fn configured_default_fee_sponsor_policy(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    policy_id: &FeeSponsorPolicyId,
    route_dataspace_id: Option<DataSpaceId>,
) -> Option<FeeSponsorPolicy> {
    let mut dataspace_ids = BTreeSet::new();
    if let Some(dataspace_id) = route_dataspace_id {
        dataspace_ids.insert(dataspace_id);
    }
    dataspace_ids.extend(nexus.dataspace_fee_sponsors.keys().copied());

    for dataspace_id in dataspace_ids {
        let Ok(Some(configured_id)) = crate::state::dataspace_fee_sponsor_policy_from_config(
            world,
            &nexus.dataspace_catalog,
            &nexus.dataspace_fee_sponsors,
            &nexus.dataspace_fee_sponsor_policies,
            dataspace_id,
        ) else {
            continue;
        };
        if configured_id.sponsor.subject_id() != policy_id.sponsor.subject_id()
            || configured_id.name != policy_id.name.clone()
        {
            continue;
        }
        let mut policy = FeeSponsorPolicy::new(policy_id.clone());
        policy.enabled = true;
        policy
            .rules
            .push(FeeSponsorRule::new(FeeSponsorRuleEffect::Allow));
        return Some(policy);
    }

    None
}

fn authorize_fee_sponsor_policy_for_state_transaction(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    sponsor: &AccountId,
    transaction: &SignedTransaction,
    fee: &Numeric,
) -> Result<FeeSponsorPolicyId, ValidationFail> {
    let policy_ids = state_transaction.fee_sponsor_policy_ids_for(authority, sponsor);
    authorize_fee_sponsor_policy_from_ids(
        &state_transaction.world,
        &state_transaction.nexus,
        sponsor,
        policy_ids,
        transaction,
        fee,
        state_transaction.current_dataspace_id,
    )
    .map_err(nexus_fee_admission_error_to_validation_fail)
}

/// Parse optional `gas_limit` from transaction metadata.
pub(crate) fn parse_gas_limit(metadata: &Metadata) -> Result<Option<u64>, ValidationFail> {
    iroha_data_model::transaction::parse_transaction_gas_limit(metadata)
        .map_err(|err| ValidationFail::NotPermitted(err.to_string()))
}

fn overlay_build_error_to_validation_fail(
    error: crate::pipeline::overlay::OverlayBuildError,
) -> ValidationFail {
    match error {
        crate::pipeline::overlay::OverlayBuildError::HeaderPolicy(error) => {
            ValidationFail::IvmAdmission(error)
        }
        crate::pipeline::overlay::OverlayBuildError::AxtReject(context) => {
            ValidationFail::AxtReject(context)
        }
        other => ValidationFail::NotPermitted(other.to_string()),
    }
}

/// Apply the canonical first-release IVM admission policy to an already prepared program.
///
/// Preparation authenticates and predecodes the image, while this check binds execution to the
/// live node/governance limits. Keeping it shared prevents direct, trigger, and proved dispatch
/// from assigning different meaning to the same ABI V1 header.
pub(crate) fn validate_prepared_ivm_execution_policy<R: StateReadOnly>(
    state: &R,
    metadata: &ivm::ProgramMetadata,
    code_offset: usize,
    bytecode: &[u8],
) -> Result<std::num::NonZeroU64, ValidationFail> {
    crate::pipeline::overlay::validate_header_policy(metadata)
        .map_err(ValidationFail::IvmAdmission)?;
    if metadata.mode & ivm::ivm_mode::ZK != 0
        && !(state.zk().halo2.enabled || state.zk().stark.enabled)
    {
        return Err(ValidationFail::IvmAdmission(
            iroha_data_model::executor::IvmAdmissionError::UnsupportedFeatureBits(
                ivm::ivm_mode::ZK,
            ),
        ));
    }
    let effective_cycles = crate::smartcontracts::ivm::validate_cycle_limits(
        metadata,
        state.pipeline().ivm_max_cycles_upper_bound,
        state.world().parameters().smart_contract().fuel(),
    )
    .map_err(ValidationFail::IvmAdmission)?;
    crate::pipeline::overlay::enforce_pre_execution_policy(
        state.pipeline().ivm_max_cycles_upper_bound,
        metadata,
        code_offset,
        bytecode,
    )
    .map_err(overlay_build_error_to_validation_fail)?;
    Ok(effective_cycles)
}

#[derive(Clone, Debug)]
pub(crate) struct ContractRuntimeExecutionContext {
    #[allow(dead_code)]
    pub(crate) contract_address: iroha_data_model::smart_contract::ContractAddress,
    pub(crate) contract_subject: AccountId,
    // Retained as canonical provenance for queued/nested calls. Authorization must never branch
    // on this value; caller metadata is canonicalized against WSV before this context is built.
    #[allow(dead_code)]
    pub(crate) contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    pub(crate) entrypoint: String,
}

/// Immutable authorization selected before a contract invocation is decoded or executed.
///
/// This snapshot deliberately carries the permission name chosen from the validated artifact.
/// Apply paths must validate this exact value and must not derive a replacement from mutable
/// world state after the VM has queued effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContractEntrypointAuthorizationSnapshot {
    pub(crate) authority: AccountId,
    pub(crate) entrypoint: String,
    pub(crate) permission: Option<String>,
    pub(crate) contract_address: iroha_data_model::smart_contract::ContractAddress,
    pub(crate) contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    pub(crate) contract_alias_binding: Option<crate::state::ContractAliasBindingRecord>,
    pub(crate) code_hash: iroha_crypto::Hash,
    parent: Option<Box<ContractEntrypointAuthorizationSnapshot>>,
}

impl ContractEntrypointAuthorizationSnapshot {
    /// Capture the exact live identity and selected artifact permission at dispatch time.
    pub(crate) fn new(
        authority: AccountId,
        entrypoint: String,
        permission: Option<String>,
        identity: &code::BoundContractIdentity,
    ) -> Self {
        Self {
            authority,
            entrypoint,
            permission,
            contract_address: identity.contract_address.clone(),
            contract_alias: identity.contract_alias.clone(),
            contract_alias_binding: identity.contract_alias_binding.clone(),
            code_hash: identity.code_hash,
            parent: None,
        }
    }

    /// Attach the complete caller authorization chain for a nested invocation.
    #[must_use]
    pub(crate) fn with_parent(
        mut self,
        parent: Option<ContractEntrypointAuthorizationSnapshot>,
    ) -> Self {
        self.parent = parent.map(Box::new);
        self
    }

    /// Return whether this snapshot is the root or retains it in its caller chain.
    pub(crate) fn descends_from(&self, root: &Self) -> bool {
        self == root
            || self
                .parent
                .as_deref()
                .is_some_and(|parent| parent.descends_from(root))
    }

    /// Return whether this snapshot represents a top-level invocation.
    pub(crate) fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Return whether `path` is owned by the exact contract instance captured by this snapshot.
    ///
    /// Durable contract state is namespaced by the immutable contract address rather than by a
    /// movable alias. Lifecycle markers use the same address digest in their reserved namespace.
    /// Keeping this check on the snapshot prevents a valid permission for one contract from being
    /// attached to a durable write targeting another contract's namespace.
    pub(crate) fn owns_durable_state_path(&self, path: &Name) -> bool {
        let address = self.contract_address.to_string();
        let digest = hex::encode(iroha_crypto::Hash::new(address.as_bytes()).as_ref());
        let path: &str = path.as_ref();
        path.strip_prefix("sc/")
            .and_then(|suffix| suffix.strip_prefix(&digest))
            .is_some_and(|suffix| suffix.starts_with('/'))
            || path == code::contract_lifecycle_state_key(&self.contract_address).as_ref()
    }

    /// Validate the immutable caller relationship between every adjacent invocation.
    ///
    /// A nested contract executes as the subject account derived from its immediate caller's
    /// address. Merely retaining an arbitrary ancestor is insufficient: without this adjacency
    /// check a forged leaf could borrow an unrelated caller's permission while still embedding a
    /// valid root snapshot somewhere in its chain.
    pub(crate) fn validate_chain_structure(
        &self,
        world: &impl WorldReadOnly,
    ) -> Result<(), ValidationFail> {
        let Some(parent) = self.parent.as_deref() else {
            return Ok(());
        };
        parent.validate_chain_structure(world)?;
        let parent_subject = world
            .contract_subject_bindings()
            .get(&parent.contract_address)
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "parent contract instance `{}` has no subject binding",
                    parent.contract_address
                ))
            })?;
        parent_subject
            .validate_for(&parent.contract_address)
            .map_err(ValidationFail::NotPermitted)?;
        if self.authority != parent_subject.subject {
            return Err(ValidationFail::NotPermitted(
                "nested contract authorization caller does not match its immediate parent contract"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Revalidate the captured caller permission and the exact forward/reverse live binding.
    pub(crate) fn validate(&self, world: &impl WorldReadOnly) -> Result<(), ValidationFail> {
        self.validate_chain_structure(world)?;
        self.validate_live(world)
    }

    fn validate_live(&self, world: &impl WorldReadOnly) -> Result<(), ValidationFail> {
        if let Some(parent) = self.parent.as_deref() {
            parent.validate_live(world)?;
        }
        let live_code_hash = world
            .contract_instances()
            .get(&self.contract_address)
            .copied()
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "contract instance `{}` is no longer active",
                    self.contract_address
                ))
            })?;
        if live_code_hash != self.code_hash {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` changed code binding while its call was prepared",
                self.contract_address
            )));
        }

        let live_alias_binding = world
            .contract_alias_bindings()
            .get(&self.contract_address)
            .cloned();
        if live_alias_binding != self.contract_alias_binding {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` changed alias binding while its call was prepared",
                self.contract_address
            )));
        }
        let reverse_alias = live_alias_binding
            .as_ref()
            .map(|binding| binding.alias.clone());
        if reverse_alias != self.contract_alias {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` has inconsistent captured alias binding metadata",
                self.contract_address
            )));
        }
        if let Some(alias) = self.contract_alias.as_ref()
            && world.contract_aliases().get(alias) != Some(&self.contract_address)
        {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` has an inconsistent live alias binding",
                self.contract_address
            )));
        }
        if world.contract_aliases().iter().any(|(alias, address)| {
            address == &self.contract_address && Some(alias) != self.contract_alias.as_ref()
        }) {
            return Err(ValidationFail::NotPermitted(format!(
                "contract instance `{}` has a non-canonical forward alias binding",
                self.contract_address
            )));
        }

        enforce_named_contract_entrypoint_permission(
            world,
            &self.authority,
            &self.contract_address,
            &self.entrypoint,
            self.permission.as_deref(),
        )
    }

    /// Validate the snapshot and require the apply-time caller to be the captured caller.
    pub(crate) fn validate_for_authority(
        &self,
        world: &impl WorldReadOnly,
        authority: &AccountId,
    ) -> Result<(), ValidationFail> {
        if authority != &self.authority {
            return Err(ValidationFail::NotPermitted(
                "prepared contract authorization caller changed before apply".to_owned(),
            ));
        }
        self.validate(world)
    }
}

/// Reject binding mutations emitted from a lifecycle hook before executor dispatch.
///
/// This guard runs ahead of both initial and user-provided executors and is shared by owned and
/// borrowed overlay paths. Without it, a hook could deactivate/reactivate its address and let the
/// completion tombstone erase the newly staged lifecycle record.
pub(crate) fn ensure_lifecycle_hook_cannot_mutate_contract_binding(
    context: Option<&ContractRuntimeExecutionContext>,
    instruction: &InstructionBox,
) -> Result<(), ValidationFail> {
    let Some(context) = context else {
        return Ok(());
    };
    if !matches!(
        context.entrypoint.as_str(),
        "hajimari" | "始まり" | "kaizen" | "改善"
    ) {
        return Ok(());
    }
    let instruction = instruction.as_any();
    if instruction
        .downcast_ref::<iroha_data_model::isi::smart_contract_code::ActivateContractInstance>()
        .is_none()
        && instruction
            .downcast_ref::<iroha_data_model::isi::smart_contract_code::DeactivateContractInstance>(
            )
            .is_none()
    {
        return Ok(());
    }

    Err(ValidationFail::NotPermitted(format!(
        "lifecycle entrypoint `{}` cannot activate or deactivate contract bindings",
        context.entrypoint
    )))
}

#[derive(Clone, Debug)]
/// Parsed contract dispatch metadata used to configure IVM execution.
pub struct ContractCallExecutionContext {
    pub(crate) contract_address: Option<iroha_data_model::smart_contract::ContractAddress>,
    pub(crate) contract_subject: Option<AccountId>,
    pub(crate) contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    pub(crate) entrypoint: Option<String>,
    pub(crate) entrypoint_pc: Option<u64>,
    pub(crate) entrypoint_permission: Option<String>,
    pub(crate) args: Json,
    pub(crate) argument_record: Option<ivm::PreparedArgumentRecord>,
}

impl ContractCallExecutionContext {
    pub(crate) fn runtime_context(&self) -> Option<ContractRuntimeExecutionContext> {
        let contract_address = self.contract_address.clone()?;
        let contract_subject = self.contract_subject.clone()?;
        Some(ContractRuntimeExecutionContext {
            contract_subject,
            contract_address,
            contract_alias: self.contract_alias.clone(),
            entrypoint: self.entrypoint.clone()?,
        })
    }

    pub(crate) fn bind_runtime_identity(
        &mut self,
        identity: code::BoundContractIdentity,
        contract_subject: AccountId,
    ) {
        self.contract_address = Some(identity.contract_address);
        self.contract_subject = Some(contract_subject);
        self.contract_alias = identity.contract_alias;
    }

    pub(crate) fn entrypoint_pc(&self) -> Option<u64> {
        self.entrypoint_pc
    }

    pub(crate) fn entrypoint_permission(&self) -> Option<&str> {
        self.entrypoint_permission.as_deref()
    }

    pub(crate) fn args(&self) -> &Json {
        &self.args
    }

    #[cfg(test)]
    pub(crate) fn argument_record(&self) -> Option<&[u8]> {
        self.argument_record
            .as_ref()
            .map(ivm::PreparedArgumentRecord::canonical_bytes)
    }

    pub(crate) fn prepared_argument_record(&self) -> Option<&ivm::PreparedArgumentRecord> {
        self.argument_record.as_ref()
    }
}

pub(crate) fn encode_contract_argument_record(
    schema: Option<&ivm::EntrypointArgumentSchemaV1>,
    payload: Option<&Json>,
) -> Result<Option<Vec<u8>>, ValidationFail> {
    match (schema, payload) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ValidationFail::NotPermitted(
            "zero-parameter entrypoint must not receive a payload".to_owned(),
        )),
        (Some(_), None) => Err(ValidationFail::NotPermitted(
            "parameterized entrypoint requires a payload".to_owned(),
        )),
        (Some(schema), Some(payload)) => ivm::encode_argument_record_from_json(schema, payload)
            .map(Some)
            .map_err(|error| {
                ValidationFail::NotPermitted(format!(
                    "contract payload does not match the entrypoint argument schema: {error}"
                ))
            }),
    }
}

fn prepare_contract_argument_record_from_json(
    schema: Option<&ivm::EntrypointArgumentSchemaV1>,
    payload: Option<&Json>,
    gas_limit: u64,
) -> Result<Option<ivm::PreparedArgumentRecord>, ValidationFail> {
    let canonical = encode_contract_argument_record(schema, payload)?;
    match (schema, canonical) {
        (None, None) => Ok(None),
        (Some(schema), Some(canonical)) => {
            ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), gas_limit)
                .map(Some)
                .map_err(|error| {
                    ValidationFail::NotPermitted(format!(
                        "failed to prepare canonical contract arguments: {error}"
                    ))
                })
        }
        _ => Err(ValidationFail::InternalError(
            "contract argument schema and canonical record diverged".to_owned(),
        )),
    }
}

fn prepare_validated_contract_argument_record(
    schema: Option<&ivm::EntrypointArgumentSchemaV1>,
    arguments: Option<&[u8]>,
    gas_limit: u64,
) -> Result<Option<ivm::PreparedArgumentRecord>, ValidationFail> {
    match (schema, arguments) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ValidationFail::NotPermitted(
            "zero-parameter entrypoint must not carry an argument record".to_owned(),
        )),
        (Some(_), None) => Err(ValidationFail::NotPermitted(
            "parameterized entrypoint requires an argument record".to_owned(),
        )),
        (Some(schema), Some(arguments)) => ivm::prepare_argument_record_with_gas_limit(
            schema,
            Arc::<[u8]>::from(arguments),
            gas_limit,
        )
        .map(Some)
        .map_err(|error| {
            ValidationFail::NotPermitted(format!("invalid contract argument record: {error}"))
        }),
    }
}

type ResolvedContractEntrypoint = (u64, Option<String>, Option<ivm::EntrypointArgumentSchemaV1>);

#[cfg(test)]
fn resolve_callable_contract_entrypoint(
    bytecode: &[u8],
    selector: &str,
    interface_required_message: &'static str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let parsed = ivm::ProgramMetadata::parse(bytecode).map_err(|err| {
        ValidationFail::NotPermitted(format!(
            "invalid contract artifact for contract call dispatch: {err}"
        ))
    })?;
    let prefix_len = parsed.prefix_len() as u64;
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .ok_or_else(|| ValidationFail::NotPermitted(interface_required_message.to_owned()))?;
    let descriptor = contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
        })?;
    let permission = callable_contract_entrypoint_permission(descriptor, selector)?;
    Ok((
        prefix_len + descriptor.entry_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}

fn resolve_raw_contract_entrypoint(
    bytecode: &[u8],
    selector: &str,
    interface_required_message: &'static str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let parsed = ivm::ProgramMetadata::parse(bytecode).map_err(|err| {
        ValidationFail::NotPermitted(format!(
            "invalid contract artifact for contract call dispatch: {err}"
        ))
    })?;
    let prefix_len = parsed.prefix_len() as u64;
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .ok_or_else(|| ValidationFail::NotPermitted(interface_required_message.to_owned()))?;
    let descriptor = contract_interface
        .entrypoints
        .iter()
        .find(|candidate| candidate.name == selector)
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
        })?;
    let permission = raw_contract_entrypoint_permission(descriptor, selector)?;
    Ok((
        prefix_len + descriptor.entry_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}

fn resolve_prepared_contract_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    let permission = callable_contract_entrypoint_permission(descriptor, selector)?;
    Ok((
        entrypoint_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}

fn resolve_prepared_nested_contract_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    let permission = nested_contract_entrypoint_permission(descriptor, selector)?;
    Ok((
        entrypoint_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}

fn resolve_prepared_contract_view_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;

    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    if descriptor.kind != EntryPointKind::View {
        return Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` is not a read-only view"
        )));
    }
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    Ok((
        entrypoint_pc,
        descriptor.permission.clone(),
        descriptor.argument_schema.clone(),
    ))
}

fn resolve_prepared_raw_contract_entrypoint(
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<ResolvedContractEntrypoint, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    let entrypoint_pc = contract.entrypoint_pc(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` has no validated program counter"
        ))
    })?;
    let permission = raw_contract_entrypoint_permission(descriptor, selector)?;
    Ok((
        entrypoint_pc,
        permission,
        descriptor.argument_schema.clone(),
    ))
}

/// Resolve authorization for a top-level deployed-contract transaction entrypoint.
pub(crate) fn callable_contract_entrypoint_permission(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    selector: &str,
) -> Result<Option<String>, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;

    match descriptor.kind {
        EntryPointKind::Kotoage => Ok(descriptor.permission.clone()),
        EntryPointKind::View => Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` is read-only and cannot be invoked as a transaction"
        ))),
        EntryPointKind::Hajimari => Ok(Some(
            iroha_data_model::smart_contract::CONTRACT_HAJIMARI_PERMISSION_NAME.to_owned(),
        )),
        EntryPointKind::Kaizen => Ok(Some(
            iroha_data_model::smart_contract::CONTRACT_KAIZEN_PERMISSION_NAME.to_owned(),
        )),
    }
}

/// Resolve authorization for raw-IVM source dispatch.
///
/// Lifecycle hooks require a consensus-bound deployed-instance transition and therefore can only
/// be selected through `Executable::ContractCall`.
pub(crate) fn raw_contract_entrypoint_permission(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    selector: &str,
) -> Result<Option<String>, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;

    match descriptor.kind {
        EntryPointKind::Kotoage => Ok(descriptor.permission.clone()),
        EntryPointKind::View => Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{selector}` is read-only and cannot be invoked as a transaction"
        ))),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => {
            Err(ValidationFail::NotPermitted(format!(
                "`{selector}` is a hajimari/始まり or kaizen/改善 entrypoint and requires a top-level deployed ContractCall"
            )))
        }
    }
}

/// Resolve authorization for an ordinary nested contract call.
///
/// Nested calls may invoke `kotoage`/`言挙げ` and `view` entrypoints, but lifecycle
/// hooks remain reserved for the deployment and `kaizen`/`改善` state machine.
pub(crate) fn nested_contract_entrypoint_permission(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    selector: &str,
) -> Result<Option<String>, ValidationFail> {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;

    match descriptor.kind {
        EntryPointKind::Kotoage | EntryPointKind::View => Ok(descriptor.permission.clone()),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => {
            Err(ValidationFail::NotPermitted(format!(
                "`{selector}` is a hajimari/始まり or kaizen/改善 entrypoint and cannot be invoked by a nested call"
            )))
        }
    }
}

fn is_self_describing_contract(bytecode: &[u8]) -> bool {
    ivm::ProgramMetadata::parse(bytecode)
        .ok()
        .and_then(|parsed| parsed.contract_interface)
        .is_some()
}

enum ContractDispatchSource<'a> {
    Bytecode(&'a [u8]),
    Prepared(&'a ivm::PreparedContract),
}

impl ContractDispatchSource<'_> {
    fn resolve(
        &self,
        selector: &str,
        interface_required_message: &'static str,
    ) -> Result<ResolvedContractEntrypoint, ValidationFail> {
        match self {
            Self::Bytecode(bytecode) => {
                resolve_raw_contract_entrypoint(bytecode, selector, interface_required_message)
            }
            Self::Prepared(contract) => {
                resolve_prepared_raw_contract_entrypoint(contract, selector)
            }
        }
    }

    fn is_self_describing(&self) -> bool {
        match self {
            Self::Bytecode(bytecode) => is_self_describing_contract(bytecode),
            Self::Prepared(_) => true,
        }
    }
}

#[cfg(test)]
pub(crate) fn parse_contract_call_execution_context(
    metadata: &Metadata,
    bytecode: &[u8],
) -> Result<Option<ContractCallExecutionContext>, ValidationFail> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Bytecode(bytecode),
        ContractArgumentSource::Metadata,
        u64::MAX,
    )
}

pub(crate) fn parse_prepared_contract_call_execution_context(
    metadata: &Metadata,
    contract: &ivm::PreparedContract,
    gas_limit: u64,
) -> Result<Option<ContractCallExecutionContext>, ValidationFail> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Prepared(contract),
        ContractArgumentSource::Metadata,
        gas_limit,
    )
}

/// Read and normalize the explicitly selected contract entrypoint.
///
/// Callers use this cheap metadata-only step to authorize a selector before
/// argument records are decoded or materialized.
pub(crate) fn requested_contract_entrypoint(
    metadata: &Metadata,
) -> Result<Option<String>, ValidationFail> {
    let entrypoint = metadata
        .get("contract_entrypoint")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                ValidationFail::NotPermitted(format!("invalid contract_entrypoint metadata: {err}"))
            })
        })
        .transpose()?
        .map(|value| value.trim().to_owned());
    if entrypoint.as_deref().is_some_and(str::is_empty) {
        return Err(ValidationFail::NotPermitted(
            "contract_entrypoint must not be empty".to_owned(),
        ));
    }
    Ok(entrypoint)
}

/// Require a by-reference invocation to match the exact live code binding
/// authorized by its signer.
pub(crate) fn ensure_contract_invocation_code_hash(
    invocation: &ContractInvocation,
    actual_code_hash: iroha_crypto::Hash,
) -> Result<(), ValidationFail> {
    if invocation.expected_code_hash != actual_code_hash {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{}` is bound to code `{actual_code_hash}`, not signed expected code `{}`",
            invocation.contract_address, invocation.expected_code_hash
        )));
    }
    Ok(())
}

fn requested_contract_address(
    metadata: &Metadata,
) -> Result<Option<iroha_data_model::smart_contract::ContractAddress>, ValidationFail> {
    metadata
        .get("contract_address")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                ValidationFail::NotPermitted(format!("invalid contract_address metadata: {err}"))
            })
        })
        .transpose()?
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(ValidationFail::NotPermitted(
                    "contract_address must not be empty".to_owned(),
                ));
            }
            trimmed.parse().map_err(|err| {
                ValidationFail::NotPermitted(format!(
                    "invalid contract_address metadata literal `{trimmed}`: {err}"
                ))
            })
        })
        .transpose()
}

fn requested_contract_alias(
    metadata: &Metadata,
) -> Result<Option<iroha_data_model::smart_contract::ContractAlias>, ValidationFail> {
    metadata
        .get("contract_alias")
        .map(|raw| {
            raw.try_into_any_norito::<String>().map_err(|err| {
                ValidationFail::NotPermitted(format!("invalid contract_alias metadata: {err}"))
            })
        })
        .transpose()?
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(ValidationFail::NotPermitted(
                    "contract_alias must not be empty".to_owned(),
                ));
            }
            trimmed.parse().map_err(|err| {
                ValidationFail::NotPermitted(format!(
                    "invalid contract_alias metadata literal `{trimmed}`: {err}"
                ))
            })
        })
        .transpose()
}

/// Resolve raw-IVM identity metadata exclusively through live world-state bindings.
///
/// User metadata selects an identity; it never supplies the trusted alias or
/// contract subject used by runtime authorization exceptions and state scope.
pub(crate) fn resolve_raw_contract_runtime_identity(
    world: &impl WorldReadOnly,
    code_hash: iroha_crypto::Hash,
    metadata: &Metadata,
) -> Result<Option<code::BoundContractIdentity>, ValidationFail> {
    let requested_address = requested_contract_address(metadata)?;
    let requested_alias = requested_contract_alias(metadata)?;
    let alias_address = requested_alias
        .as_ref()
        .map(|alias| {
            world.contract_aliases().get(alias).cloned().ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "contract alias `{alias}` is not bound in live state"
                ))
            })
        })
        .transpose()?;
    if let (Some(requested), Some(resolved)) = (&requested_address, &alias_address)
        && requested != resolved
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract alias metadata resolves to `{resolved}`, not requested address `{requested}`"
        )));
    }
    let Some(contract_address) = requested_address.or(alias_address) else {
        return Ok(None);
    };
    let bound_code_hash = world
        .contract_instances()
        .get(&contract_address)
        .copied()
        .ok_or_else(|| {
            ValidationFail::NotPermitted(format!(
                "contract instance `{contract_address}` not found in live state"
            ))
        })?;
    if bound_code_hash != code_hash {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{contract_address}` is bound to code `{bound_code_hash}`, not executing code `{code_hash}`"
        )));
    }
    let live_alias_binding = world
        .contract_alias_bindings()
        .get(&contract_address)
        .cloned();
    let live_alias = live_alias_binding
        .as_ref()
        .map(|binding| binding.alias.clone());
    if let Some(alias) = live_alias.as_ref()
        && world.contract_aliases().get(alias) != Some(&contract_address)
    {
        return Err(ValidationFail::NotPermitted(format!(
            "contract instance `{contract_address}` has an inconsistent live alias binding"
        )));
    }
    if requested_alias.as_ref().is_some_and(|requested| {
        live_alias.as_ref() != Some(requested)
            || world.contract_aliases().get(requested) != Some(&contract_address)
    }) {
        return Err(ValidationFail::NotPermitted(format!(
            "contract alias metadata does not match the live alias for `{contract_address}`"
        )));
    }
    Ok(Some(code::BoundContractIdentity {
        contract_address,
        contract_alias: live_alias,
        contract_alias_binding: live_alias_binding,
        code_hash,
    }))
}

/// Resolve the mandatory live identity for a selected raw-IVM contract entrypoint.
///
/// A selected entrypoint is contract dispatch, even when its descriptor has no named
/// permission. It therefore cannot execute with an anonymous/state-free runtime identity.
pub(crate) fn require_raw_contract_runtime_identity(
    world: &impl WorldReadOnly,
    code_hash: iroha_crypto::Hash,
    metadata: &Metadata,
) -> Result<code::BoundContractIdentity, ValidationFail> {
    resolve_raw_contract_runtime_identity(world, code_hash, metadata)?.ok_or_else(|| {
        ValidationFail::NotPermitted(
            "raw-IVM contract entrypoint dispatch requires a live contract_address or contract_alias binding"
                .to_owned(),
        )
    })
}

#[derive(Clone, Copy)]
enum ContractArgumentSource<'a> {
    Metadata,
    TriggerEvent(&'a Json),
    SchemaOnly,
}

/// Resolve a self-describing IVM trigger callback and bind the current event
/// arguments to its compiler-emitted schema.
///
/// Trigger actions select the callback with `contract_entrypoint` metadata, but
/// their payload is supplied by the event that fired the trigger. The payload
/// is converted here, once, into the same schema-bound canonical Norito record
/// used by ordinary contract calls. A fixed `contract_payload` in trigger
/// metadata is rejected so it cannot shadow the signed event arguments.
pub(crate) fn parse_prepared_trigger_call_execution_context(
    metadata: &Metadata,
    contract: &ivm::PreparedContract,
    event_args: &Json,
    gas_limit: u64,
) -> Result<ContractCallExecutionContext, ValidationFail> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Prepared(contract),
        ContractArgumentSource::TriggerEvent(event_args),
        gas_limit,
    )?
    .ok_or_else(|| {
        ValidationFail::NotPermitted(
            "self-describing IVM trigger action did not resolve a callback".to_owned(),
        )
    })
}

/// Validate trigger callback selection at registration without fabricating an
/// event payload for a parameterized callback.
pub(crate) fn validate_trigger_call_execution_context(
    metadata: &Metadata,
    bytecode: &[u8],
) -> Result<(), ValidationFail> {
    parse_contract_call_execution_context_from_source(
        metadata,
        ContractDispatchSource::Bytecode(bytecode),
        ContractArgumentSource::SchemaOnly,
        u64::MAX,
    )?
    .ok_or_else(|| {
        ValidationFail::NotPermitted(
            "self-describing IVM trigger action did not resolve a callback".to_owned(),
        )
    })?;
    Ok(())
}

fn parse_contract_call_execution_context_from_source(
    metadata: &Metadata,
    source: ContractDispatchSource<'_>,
    argument_source: ContractArgumentSource<'_>,
    gas_limit: u64,
) -> Result<Option<ContractCallExecutionContext>, ValidationFail> {
    let contract_address = requested_contract_address(metadata)?;
    let contract_alias = requested_contract_alias(metadata)?;

    let entrypoint = requested_contract_entrypoint(metadata)?;

    let metadata_payload = metadata.get("contract_payload").cloned();
    if !matches!(argument_source, ContractArgumentSource::Metadata) && metadata_payload.is_some() {
        return Err(ValidationFail::NotPermitted(
            "IVM trigger actions must take arguments from the triggering event, not contract_payload metadata"
                .to_owned(),
        ));
    }
    let (entrypoint, entrypoint_pc, entrypoint_permission, argument_schema) =
        if let Some(selector) = entrypoint.as_deref() {
            let (entrypoint_pc, entrypoint_permission, argument_schema) = source.resolve(
                selector,
                "contract call entrypoint metadata requires a self-describing contract artifact",
            )?;
            (
                Some(selector.to_owned()),
                Some(entrypoint_pc),
                entrypoint_permission,
                argument_schema,
            )
        } else if source.is_self_describing() {
            return Err(ValidationFail::NotPermitted(
                "self-describing contract calls require explicit contract_entrypoint metadata"
                    .to_owned(),
            ));
        } else if metadata_payload.is_none() {
            return Ok(None);
        } else {
            (None, None, None, None)
        };

    let payload = match argument_source {
        ContractArgumentSource::Metadata => metadata_payload,
        ContractArgumentSource::TriggerEvent(event_args) => {
            argument_schema.as_ref().map(|_| event_args.clone())
        }
        ContractArgumentSource::SchemaOnly => None,
    };
    let argument_record = if matches!(argument_source, ContractArgumentSource::SchemaOnly) {
        None
    } else {
        prepare_contract_argument_record_from_json(
            argument_schema.as_ref(),
            payload.as_ref(),
            gas_limit,
        )?
    };
    let args = match argument_source {
        ContractArgumentSource::TriggerEvent(event_args) => event_args.clone(),
        ContractArgumentSource::Metadata | ContractArgumentSource::SchemaOnly => {
            payload.unwrap_or_default()
        }
    };

    Ok(Some(ContractCallExecutionContext {
        contract_address,
        contract_subject: None,
        contract_alias,
        entrypoint,
        entrypoint_pc,
        entrypoint_permission,
        args,
        argument_record,
    }))
}

#[cfg(test)]
pub(crate) fn parse_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    bytecode: &[u8],
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
) -> Result<ContractCallExecutionContext, ValidationFail> {
    let selector = invocation.entrypoint.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        ));
    }

    let (entrypoint_pc, entrypoint_permission, argument_schema) =
        resolve_callable_contract_entrypoint(
            bytecode,
            selector,
            "contract call requires a self-describing contract artifact",
        )?;
    let args = Json::default();
    let argument_record = prepare_validated_contract_argument_record(
        argument_schema.as_ref(),
        invocation.arguments.as_deref(),
        u64::MAX,
    )?;

    Ok(ContractCallExecutionContext {
        contract_address: Some(invocation.contract_address.clone()),
        contract_subject: Some(contract_subject),
        contract_alias,
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        entrypoint_permission,
        args,
        argument_record,
    })
}

pub(crate) fn parse_prepared_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    contract: &ivm::PreparedContract,
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
    gas_limit: u64,
) -> Result<ContractCallExecutionContext, ValidationFail> {
    parse_prepared_contract_invocation_execution_context_with_resolver(
        invocation,
        contract,
        contract_alias,
        contract_subject,
        gas_limit,
        resolve_prepared_contract_entrypoint,
    )
}

/// Resolve a prepared ordinary nested call using the nested entrypoint policy.
///
/// Unlike top-level transaction dispatch, nested calls may enter read-only
/// views. Lifecycle entrypoints remain reserved for their dedicated state
/// transition machinery.
pub(crate) fn parse_prepared_nested_contract_invocation_execution_context(
    invocation: &ContractInvocation,
    contract: &ivm::PreparedContract,
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
    gas_limit: u64,
) -> Result<ContractCallExecutionContext, ValidationFail> {
    parse_prepared_contract_invocation_execution_context_with_resolver(
        invocation,
        contract,
        contract_alias,
        contract_subject,
        gas_limit,
        resolve_prepared_nested_contract_entrypoint,
    )
}

fn parse_prepared_contract_invocation_execution_context_with_resolver(
    invocation: &ContractInvocation,
    contract: &ivm::PreparedContract,
    contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    contract_subject: AccountId,
    gas_limit: u64,
    resolve_entrypoint: fn(
        &ivm::PreparedContract,
        &str,
    ) -> Result<ResolvedContractEntrypoint, ValidationFail>,
) -> Result<ContractCallExecutionContext, ValidationFail> {
    let selector = invocation.entrypoint.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        ));
    }

    let (entrypoint_pc, entrypoint_permission, argument_schema) =
        resolve_entrypoint(contract, selector)?;
    let args = Json::default();
    let argument_record = prepare_validated_contract_argument_record(
        argument_schema.as_ref(),
        invocation.arguments.as_deref(),
        gas_limit,
    )?;
    Ok(ContractCallExecutionContext {
        contract_address: Some(invocation.contract_address.clone()),
        contract_subject: Some(contract_subject),
        contract_alias,
        entrypoint: Some(selector.to_owned()),
        entrypoint_pc: Some(entrypoint_pc),
        entrypoint_permission,
        args,
        argument_record,
    })
}

/// Validate a top-level deployed entrypoint against the instance lifecycle state.
pub(crate) fn validate_prepared_contract_lifecycle_call(
    world: &impl WorldReadOnly,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    code_hash: iroha_crypto::Hash,
    contract: &ivm::PreparedContract,
    selector: &str,
) -> Result<Option<code::PendingContractLifecycle>, ValidationFail> {
    let descriptor = contract.entrypoint_descriptor(selector).ok_or_else(|| {
        ValidationFail::NotPermitted(format!("unknown contract entrypoint `{selector}`"))
    })?;
    code::validate_contract_lifecycle_call(world, contract_address, code_hash, descriptor.kind)
}

fn parse_executor_additional_fuel(metadata: &Metadata) -> Result<u64, ValidationFail> {
    let Some(raw) = metadata.get(EXECUTOR_ADDITIONAL_FUEL_KEY) else {
        return Ok(0);
    };
    raw.try_into_any_norito::<u64>().map_err(|err| {
        ValidationFail::NotPermitted(format!("invalid additional_fuel metadata: {err}"))
    })
}

pub(crate) fn compute_nexus_fee_amount(
    cfg: &iroha_config::parameters::actual::NexusFees,
    tx_bytes_len: usize,
    instruction_count: usize,
    gas_used: u64,
) -> Result<Quantity, ValidationFail> {
    let tx_bytes_u64 = u64::try_from(tx_bytes_len).map_err(|_| {
        ValidationFail::InternalError("transaction too large for fee accounting".to_owned())
    })?;
    let instr_u64 = u64::try_from(instruction_count).map_err(|_| {
        ValidationFail::InternalError("instruction count too large for fee accounting".to_owned())
    })?;
    let mut fee = cfg.base_fee.as_numeric().clone();
    fee = Executor::checked_numeric_add(
        fee,
        Executor::checked_numeric_mul_u64(
            cfg.per_byte_fee.as_numeric(),
            tx_bytes_u64,
            "fee amount",
        )?,
        "fee amount",
    )?;
    fee = Executor::checked_numeric_add(
        fee,
        Executor::checked_numeric_mul_u64(
            cfg.per_instruction_fee.as_numeric(),
            instr_u64,
            "fee amount",
        )?,
        "fee amount",
    )?;
    let fee = Executor::checked_numeric_add(
        fee,
        Executor::checked_numeric_mul_u64(
            cfg.per_gas_unit_fee.as_numeric(),
            gas_used,
            "fee amount",
        )?,
        "fee amount",
    )?
    .trim_trailing_zeros();
    Quantity::from_canonical_numeric(fee).map_err(|error| {
        ValidationFail::InternalError(format!(
            "computed nexus fee left the quantity domain: {error}"
        ))
    })
}

fn fee_bound_for_admission(
    transaction: &SignedTransaction,
) -> Result<(usize, usize, u64), NexusFeeAdmissionError> {
    let tx_bytes_len = to_bytes(transaction)
        .map(|bytes| bytes.len())
        .map_err(|err| {
            NexusFeeAdmissionError::ConfigInvalid(format!(
                "failed to encode transaction for fee metering: {err}"
            ))
        })?;

    let metadata = transaction.metadata();
    let (instruction_count, gas_used) = match transaction.instructions() {
        Executable::Instructions(instructions) => (
            instructions.len(),
            isi_gas::meter_instructions(instructions.as_ref()),
        ),
        Executable::ContractCall(_) | Executable::Ivm(_) => {
            let gas_limit = parse_gas_limit(metadata)
                .map_err(validation_fail_to_nexus_fee_admission_error)?
                .ok_or_else(|| {
                    NexusFeeAdmissionError::Rejected(
                        "missing gas_limit in transaction metadata".to_owned(),
                    )
                })?;
            (0, gas_limit)
        }
        Executable::IvmProved(proved) => {
            let gas_limit = parse_gas_limit(metadata)
                .map_err(validation_fail_to_nexus_fee_admission_error)?
                .ok_or_else(|| {
                    NexusFeeAdmissionError::Rejected(
                        "missing gas_limit in transaction metadata".to_owned(),
                    )
                })?;
            (proved.overlay.len(), gas_limit)
        }
    };

    Ok((tx_bytes_len, instruction_count, gas_used))
}

pub(crate) fn check_external_nexus_fee_admission(
    world: &impl WorldReadOnly,
    nexus: &iroha_config::parameters::actual::Nexus,
    transaction: &SignedTransaction,
    observation_time_ms: u64,
    next_block_height: u64,
    route_dataspace_id: Option<DataSpaceId>,
) -> Result<(), NexusFeeAdmissionError> {
    if !nexus.enabled {
        return Ok(());
    }
    if nexus_fee_exempt_transaction(transaction) {
        return Ok(());
    }
    if successful_claim_fee_exempt_transaction(world, nexus, transaction, observation_time_ms) {
        return Ok(());
    }

    let metadata = transaction.metadata();
    let fee_sponsor = resolve_effective_fee_sponsor(
        world,
        world.dataspace_catalog(),
        &nexus.dataspace_fee_sponsors,
        metadata,
        route_dataspace_id,
    )
    .map_err(validation_fail_to_nexus_fee_admission_error)?;
    let has_fee_sponsor = fee_sponsor.is_some();
    let externally_settled_sponsored_fee =
        fee_sponsor.is_some() && nexus.fees.external_settlement_enabled;
    let (tx_bytes_len, instruction_count, gas_used) = fee_bound_for_admission(transaction)?;
    let fee = compute_nexus_fee_amount(&nexus.fees, tx_bytes_len, instruction_count, gas_used)
        .map_err(validation_fail_to_nexus_fee_admission_error)?;

    if fee.is_zero() {
        return Ok(());
    }

    let payer = if let Some(sponsor) = fee_sponsor {
        if !nexus.fees.sponsorship_enabled {
            return Err(NexusFeeAdmissionError::Rejected(
                "fee sponsorship is disabled".to_owned(),
            ));
        }
        if !nexus.fees.sponsor_max_fee.is_zero() && fee > nexus.fees.sponsor_max_fee {
            return Err(NexusFeeAdmissionError::Rejected(
                "fee exceeds sponsor_max_fee".to_owned(),
            ));
        }
        let policy_ids = fee_sponsor_policy_ids_read_only(
            world,
            transaction.authority(),
            &sponsor,
            nexus,
            route_dataspace_id,
        );
        authorize_fee_sponsor_policy_from_ids(
            world,
            nexus,
            &sponsor,
            policy_ids,
            transaction,
            fee.as_numeric(),
            route_dataspace_id,
        )?;
        sponsor
    } else {
        transaction.authority().clone()
    };

    let redeem_funded_nexus_fee = redeem_funded_nexus_fee_covers(
        world,
        &nexus.fees,
        transaction,
        observation_time_ms,
        next_block_height,
        has_fee_sponsor,
        fee.as_numeric(),
        Numeric::zero(),
    )?;
    if redeem_funded_nexus_fee {
        return Ok(());
    }

    if nexus
        .fees
        .lane_relay_burn_receipts_active_at(next_block_height)
    {
        check_lane_relay_burn_canonical_sponsor(world, &nexus.fees, &payer)?;
        return check_lane_relay_burn_fee_budget(
            world,
            &nexus.fees,
            &payer,
            fee.as_numeric(),
            Numeric::zero(),
        );
    }

    if externally_settled_sponsored_fee {
        return Ok(());
    }

    let asset_def = crate::block::parse_asset_definition_literal_with_world(
        world,
        &nexus.fees.fee_asset_id,
        observation_time_ms,
    )
    .ok_or_else(|| {
        NexusFeeAdmissionError::ConfigInvalid(
            "invalid nexus fee asset id; expected canonical Base58 asset definition id or active asset alias"
                .to_owned(),
        )
    })?;

    let payer_asset = AssetId::new(asset_def, payer.clone());
    let Some(balance) = world.assets().get(&payer_asset) else {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "fee asset `{}` is missing for payer `{payer}`",
            payer_asset.definition()
        )));
    };

    let available = balance.as_ref().as_numeric().clone();
    if available < fee.as_numeric().clone() {
        return Err(NexusFeeAdmissionError::Rejected(format!(
            "fee balance for payer `{payer}` is insufficient: requires {fee}, available {available}"
        )));
    }

    Ok(())
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
#[allow(dead_code)]
pub(crate) fn charge_fees_for_applied_overlay(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    transaction: &SignedTransaction,
    overlay: &crate::pipeline::overlay::TxOverlay,
) -> Result<(), ValidationFail> {
    let tx_bytes_len = to_bytes(transaction)
        .map(|bytes| bytes.len())
        .map_err(|err| {
            ValidationFail::InternalError(format!(
                "failed to encode transaction for fee metering: {err}"
            ))
        })?;
    charge_fees_for_applied_overlay_with_encoded_len(
        state_transaction,
        authority,
        transaction,
        overlay,
        tx_bytes_len,
    )
}

/// Charge gas and Nexus fees for an overlay-applied transaction using trusted local metadata.
///
/// The `tx_bytes_len` value must come from locally prepared transaction metadata for the same
/// signed transaction. Network-provided byte lengths must not be forwarded here.
pub(crate) fn charge_fees_for_applied_overlay_with_encoded_len(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    transaction: &SignedTransaction,
    overlay: &crate::pipeline::overlay::TxOverlay,
    tx_bytes_len: usize,
) -> Result<(), ValidationFail> {
    // Genesis transactions are bootstrap operations and must remain fee-free.
    if state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty() {
        return Ok(());
    }

    let md = transaction.metadata();
    let fee_sponsor = resolve_effective_fee_sponsor(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.dataspace_fee_sponsors,
        md,
        state_transaction.current_dataspace_id,
    )?;
    let skip_nexus_fee = nexus_fee_exempt_transaction(transaction)
        || successful_claim_fee_exempt_instructions(
            &state_transaction.world,
            &state_transaction.nexus,
            authority,
            md,
            overlay.instruction_slice(),
            state_transaction.block_unix_timestamp_ms(),
        );

    // Keep gas policy snapshots aligned with governance/custom parameter updates.
    Executor::refresh_gas_from_parameters(state_transaction);

    let gas_asset_opt = md.get("gas_asset_id").map(|j| j.as_ref().to_string());
    let gas_limit_md = parse_gas_limit(md)?;
    let pipeline_gas = &state_transaction.pipeline.gas;
    if !skip_nexus_fee && !pipeline_gas.accepted_assets.is_empty() {
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
        Executable::ContractCall(_) | Executable::Ivm(_) => (
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
            overlay.ivm_gas_used().ok_or_else(|| {
                ValidationFail::InternalError(
                    "missing replayed IVM gas usage metadata for proved overlay transaction"
                        .to_owned(),
                )
            })?,
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
    Executor::enforce_transaction_gas_fits_block(state_transaction, gas_used)?;

    let tx_hash = transaction.hash();
    let settlement_source_id = {
        let mut bytes = [0u8; iroha_crypto::Hash::LENGTH];
        bytes.copy_from_slice(tx_hash.as_ref());
        bytes
    };

    if should_charge_pipeline_gas_asset(
        skip_nexus_fee,
        state_transaction.nexus.enabled,
        &state_transaction.nexus.fees,
        &gas_asset_opt,
    ) && let Some(gas_asset_id_str) = gas_asset_opt
    {
        Executor::charge_pipeline_gas_asset_fee(
            state_transaction,
            authority,
            transaction,
            tx_hash,
            settlement_source_id,
            &gas_asset_id_str,
            gas_used,
            fee_sponsor.as_ref(),
        )?;
    }

    if !skip_nexus_fee {
        let fee = compute_nexus_fee_amount(
            &state_transaction.nexus.fees,
            tx_bytes_len,
            instruction_count,
            gas_used,
        )?;
        let in_flight_fees = if state_transaction
            .nexus
            .fees
            .lane_relay_burn_receipts_active_at(state_transaction.block_height())
        {
            state_transaction
                .pending_nexus_fee_amount_for(
                    authority,
                    state_transaction.nexus.fees.fee_asset_id.as_str(),
                )
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "Nexus fee budget arithmetic overflow while summing in-flight receipts"
                            .to_owned(),
                    )
                })?
        } else {
            Numeric::zero()
        };
        let redeem_funded_nexus_fee = redeem_funded_nexus_fee_covers(
            &state_transaction.world,
            &state_transaction.nexus.fees,
            transaction,
            state_transaction.block_unix_timestamp_ms(),
            state_transaction.block_height(),
            fee_sponsor.is_some(),
            fee.as_numeric(),
            in_flight_fees,
        )
        .map_err(nexus_fee_admission_error_to_validation_fail)?;
        Executor::charge_nexus_fees(
            state_transaction,
            authority,
            transaction,
            tx_hash,
            fee_sponsor,
            tx_bytes_len,
            instruction_count,
            gas_used,
            redeem_funded_nexus_fee,
        )?;
    }

    Ok(())
}

impl Executor {
    fn resolve_pipeline_gas_asset_definition(
        state_transaction: &StateTransaction<'_, '_>,
        gas_asset_id_str: &str,
    ) -> Result<(AssetDefinitionId, AssetDefinition), ValidationFail> {
        let parsed = AssetDefinitionId::parse_address_literal(gas_asset_id_str).map_err(|_| {
            ValidationFail::NotPermitted(
                "invalid gas_asset_id; expected an unprefixed Base58 asset definition id"
                    .to_owned(),
            )
        })?;

        if let Ok(definition) = state_transaction.world.asset_definition(&parsed) {
            return Ok((definition.id().clone(), definition));
        }

        state_transaction
            .world
            .asset_definitions()
            .iter()
            .find(|(id, _)| id.canonical_address() == gas_asset_id_str)
            .map(|(id, definition)| (id.clone(), definition.clone()))
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!(
                    "gas asset `{gas_asset_id_str}` is not registered"
                ))
            })
    }

    fn enforce_transaction_gas_fits_block(
        state_transaction: &StateTransaction<'_, '_>,
        gas_used: u64,
    ) -> Result<(), ValidationFail> {
        if gas_used == 0 || state_transaction.gas_limit_per_block == 0 {
            return Ok(());
        }
        let total = state_transaction
            .gas_used_in_block_so_far
            .saturating_add(gas_used);
        if total > state_transaction.gas_limit_per_block {
            return Err(ValidationFail::NotPermitted(format!(
                "block gas limit exceeded: {total} > {}",
                state_transaction.gas_limit_per_block
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn record_pipeline_gas_settlement_receipt(
        state_transaction: &mut StateTransaction<'_, '_>,
        tx_hash: iroha_crypto::HashOf<SignedTransaction>,
        source_id: [u8; iroha_crypto::Hash::LENGTH],
        asset_definition_id: AssetDefinitionId,
        local_amount_micro: u128,
        twap_local_per_xor: Decimal,
        liquidity_profile: LiquidityProfile,
        volatility_bucket: VolatilityBucket,
    ) -> Result<(), ValidationFail> {
        let block_timestamp_ms_u128 = state_transaction._curr_block.creation_time().as_millis();
        let block_timestamp_ms = u64::try_from(block_timestamp_ms_u128).unwrap_or(u64::MAX);
        let quote = state_transaction
            .settlement_engine()
            .quote(
                source_id,
                local_amount_micro,
                twap_local_per_xor,
                liquidity_profile,
                volatility_bucket,
                block_timestamp_ms,
            )
            .map_err(|err| match err {
                QuoteError::LocalAmountOverflow(amount) => ValidationFail::NotPermitted(format!(
                    "local gas amount {amount} exceeds Decimal range"
                )),
                QuoteError::ZeroTwap => {
                    ValidationFail::NotPermitted("gas TWAP must be non-zero".to_owned())
                }
            })?;
        let config_snapshot = state_transaction.settlement_engine().config();
        let twap_window_seconds = config_snapshot.twap_window.whole_seconds().max(0);
        let twap_window_seconds = u32::try_from(twap_window_seconds).unwrap_or(u32::MAX);
        let xor_due_micro = Self::decimal_to_micro_u128(*quote.receipt.xor_due, "xor_due amount")?;
        let xor_after_haircut_micro = Self::decimal_to_micro_u128(
            *quote.receipt.xor_with_haircut,
            "xor_after_haircut amount",
        )?;
        let xor_variance_micro = xor_due_micro.saturating_sub(xor_after_haircut_micro);
        let pending = PendingSettlement {
            source_id,
            asset_definition_id,
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
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn charge_pipeline_gas_asset_fee(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        tx_hash: iroha_crypto::HashOf<SignedTransaction>,
        settlement_source_id: [u8; iroha_crypto::Hash::LENGTH],
        gas_asset_id_str: &str,
        gas_used: u64,
        fee_sponsor: Option<&AccountId>,
    ) -> Result<(), ValidationFail> {
        let gas_rate = state_transaction
            .pipeline
            .gas
            .units_per_gas
            .iter()
            .find(|rate| rate.asset == gas_asset_id_str)
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

        if gas_used == 0 || units_per_gas == 0 {
            return Ok(());
        }

        let tech_account: AccountId = parse_account_id_literal(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.pipeline.gas.tech_account_id,
        )
        .ok_or_else(|| {
            ValidationFail::InternalError(
                "invalid pipeline.gas.tech_account_id; expected canonical I105 account id or on-chain alias"
                    .to_owned(),
            )
        })?;
        let (asset_definition_id, definition) =
            Self::resolve_pipeline_gas_asset_definition(state_transaction, gas_asset_id_str)?;

        let fee_u128 = u128::from(gas_used).saturating_mul(u128::from(units_per_gas));
        if fee_u128 == 0 {
            return Ok(());
        }
        let payer = if let Some(sponsor) = fee_sponsor.filter(|sponsor| *sponsor != authority) {
            if !state_transaction.nexus.fees.sponsorship_enabled {
                return Err(ValidationFail::NotPermitted(
                    "fee sponsorship is disabled".to_owned(),
                ));
            }
            let sponsorship_fee = Numeric::try_new(fee_u128, 0).map_err(|_| {
                ValidationFail::NotPermitted(
                    "fee amount exceeds supported numeric bounds".to_owned(),
                )
            })?;
            authorize_fee_sponsor_policy_for_state_transaction(
                state_transaction,
                authority,
                sponsor,
                transaction,
                &sponsorship_fee,
            )?;
            sponsor.clone()
        } else {
            authority.clone()
        };
        let payer_scope = match definition.balance_scope_policy() {
            AssetBalancePolicy::Global => AssetBalanceScope::Global,
            AssetBalancePolicy::DataspaceRestricted => AssetBalanceScope::Dataspace(
                state_transaction
                    .current_dataspace_id
                    .unwrap_or(DataSpaceId::UNIVERSAL),
            ),
        };
        let payer_asset = AssetId::with_scope(asset_definition_id.clone(), payer, payer_scope);
        let qty = Quantity::from(fee_u128);
        let transfer = iroha_data_model::isi::Transfer::<
            Asset,
            Quantity,
            iroha_data_model::account::Account,
        >::asset_quantity(payer_asset, qty, tech_account);
        let instr: DMInstructionBox = transfer.into();
        execute_gas_fee_transfer_instruction(&definition, instr, authority, state_transaction)
            .map_err(|err| {
                iroha_logger::debug!(
                    ?err,
                    authority = %authority,
                    "gas fee transfer failed to apply"
                );
                ValidationFail::from(err)
            })?;
        #[cfg(feature = "telemetry")]
        {
            let delta = u64::try_from(fee_u128.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
            state_transaction.stage_block_fee_amount(Numeric::from(delta));
        }

        Self::record_pipeline_gas_settlement_receipt(
            state_transaction,
            tx_hash,
            settlement_source_id,
            asset_definition_id,
            fee_u128,
            twap_local_per_xor,
            liquidity_profile,
            volatility_bucket,
        )
    }

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

    fn checked_numeric_add(
        lhs: Numeric,
        rhs: Numeric,
        context: &'static str,
    ) -> Result<Numeric, ValidationFail> {
        lhs.checked_add(rhs).ok_or_else(|| {
            ValidationFail::NotPermitted(format!("{context} exceeds supported numeric bounds"))
        })
    }

    fn checked_numeric_mul_u64(
        value: &Numeric,
        multiplier: u64,
        context: &'static str,
    ) -> Result<Numeric, ValidationFail> {
        value
            .clone()
            .checked_mul(Numeric::from(multiplier), NumericSpec::unconstrained())
            .ok_or_else(|| {
                ValidationFail::NotPermitted(format!("{context} exceeds supported numeric bounds"))
            })
    }

    #[allow(clippy::too_many_lines)]
    fn charge_nexus_fees(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: &SignedTransaction,
        tx_hash: iroha_crypto::HashOf<SignedTransaction>,
        sponsor: Option<AccountId>,
        tx_bytes_len: usize,
        instruction_count: usize,
        gas_used: u64,
        redeem_funded_nexus_fee: bool,
    ) -> Result<(), ValidationFail> {
        if !state_transaction.nexus.enabled {
            return Ok(());
        }
        let cfg = state_transaction.nexus.fees.clone();
        let fee = compute_nexus_fee_amount(&cfg, tx_bytes_len, instruction_count, gas_used)?;

        if fee.is_zero() {
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
                    fee_amount = %fee,
                    "nexus fee sponsor rejected: sponsorship disabled"
                );
                return Err(ValidationFail::NotPermitted(
                    "fee sponsorship is disabled".to_owned(),
                ));
            }
            if !cfg.sponsor_max_fee.is_zero() && fee > cfg.sponsor_max_fee {
                let payer_id = sponsor.to_string();
                sumeragi_status::record_nexus_fee_event(NexusFeeEvent::SponsorCapExceeded {
                    payer_id: payer_id.clone(),
                    max_fee: cfg.sponsor_max_fee.clone(),
                    attempted_fee: fee.clone(),
                });
                warn!(
                    target: "economics",
                    payer = %payer_id,
                    fee_amount = %fee,
                    max_fee = %cfg.sponsor_max_fee,
                    "nexus fee sponsor rejected: exceeds sponsor_max_fee"
                );
                return Err(ValidationFail::NotPermitted(
                    "fee exceeds sponsor_max_fee".to_owned(),
                ));
            }
            if let Err(err) = authorize_fee_sponsor_policy_for_state_transaction(
                state_transaction,
                authority,
                &sponsor,
                transaction,
                fee.as_numeric(),
            ) {
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
                    fee_amount = %fee,
                    error = %err,
                    "nexus fee sponsor rejected: policy denied transaction"
                );
                return Err(err);
            }
            sponsor
        } else {
            authority.clone()
        };

        let payer_kind_label = match payer_kind {
            NexusFeePayer::Payer => "payer",
            NexusFeePayer::Sponsor => "sponsor",
        };
        let payer_id = payer.to_string();
        if cfg.lane_relay_burn_receipts_active_at(state_transaction.block_height()) {
            let asset_label = cfg.fee_asset_id.clone();
            let in_flight_fees = state_transaction
                .pending_nexus_fee_amount_for(&payer, cfg.fee_asset_id.as_str())
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "Nexus fee budget arithmetic overflow while summing in-flight receipts"
                            .to_owned(),
                    )
                })?;
            if redeem_funded_nexus_fee {
                check_redeem_funded_lane_relay_fee_balance(
                    &state_transaction.world,
                    &cfg,
                    &payer,
                    state_transaction.block_unix_timestamp_ms(),
                    fee.as_numeric(),
                    in_flight_fees,
                )
                .map_err(|err| match err {
                    NexusFeeAdmissionError::Rejected(reason)
                    | NexusFeeAdmissionError::ConfigInvalid(reason) => {
                        ValidationFail::NotPermitted(reason)
                    }
                })?;
            } else {
                check_lane_relay_burn_canonical_sponsor(&state_transaction.world, &cfg, &payer)
                    .map_err(|err| match err {
                        NexusFeeAdmissionError::Rejected(reason)
                        | NexusFeeAdmissionError::ConfigInvalid(reason) => {
                            ValidationFail::NotPermitted(reason)
                        }
                    })?;
                check_lane_relay_burn_fee_budget(
                    &state_transaction.world,
                    &cfg,
                    &payer,
                    fee.as_numeric(),
                    in_flight_fees,
                )
                .map_err(|err| match err {
                    NexusFeeAdmissionError::Rejected(reason)
                    | NexusFeeAdmissionError::ConfigInvalid(reason) => {
                        ValidationFail::NotPermitted(reason)
                    }
                })?;
            }
            let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
            source_id.copy_from_slice(tx_hash.as_ref());
            let tx_bytes_len = u64::try_from(tx_bytes_len).unwrap_or(u64::MAX);
            let instruction_count = u64::try_from(instruction_count).unwrap_or(u64::MAX);
            state_transaction.record_nexus_fee_receipt(
                tx_hash,
                PendingNexusFeeReceipt {
                    source_id,
                    payer_account_id: payer,
                    fee_asset_id: cfg.fee_asset_id.clone(),
                    fee_amount: fee.clone(),
                    schedule: NexusFeeScheduleInputs {
                        tx_bytes_len,
                        instruction_count,
                        gas_used,
                        base_fee: cfg.base_fee.clone(),
                        per_byte_fee: cfg.per_byte_fee.clone(),
                        per_instruction_fee: cfg.per_instruction_fee.clone(),
                        per_gas_unit_fee: cfg.per_gas_unit_fee.clone(),
                    },
                },
            );
            state_transaction.stage_nexus_fee_event(NexusFeeEvent::Charged {
                payer_kind,
                payer_id,
                amount: fee,
                asset_id: asset_label,
            });
            return Ok(());
        }
        let asset_def = crate::block::parse_asset_definition_literal_with_world(
            &state_transaction.world,
            &cfg.fee_asset_id,
            state_transaction.block_unix_timestamp_ms(),
        )
        .ok_or_else(|| {
            let reason =
                "invalid nexus fee asset id; expected canonical Base58 asset definition id or active asset alias"
                    .to_owned();
            sumeragi_status::record_nexus_fee_event(NexusFeeEvent::ConfigInvalid {
                reason: reason.clone(),
            });
            warn!(target: "economics", "nexus fee rejected: {reason}");
            ValidationFail::NotPermitted(reason)
        })?;

        let payer_asset = AssetId::new(asset_def, payer.clone());
        let asset_label = payer_asset.definition().to_string();
        if matches!(payer_kind, NexusFeePayer::Sponsor) && cfg.external_settlement_enabled {
            state_transaction.stage_nexus_fee_event(NexusFeeEvent::Charged {
                payer_kind,
                payer_id,
                amount: fee,
                asset_id: asset_label,
            });
            return Ok(());
        }

        let burn = Burn::asset_quantity(fee.clone(), payer_asset);
        let instr: DMInstructionBox = burn.into();
        let previous_tx_dataspace_id = state_transaction.current_dataspace_id;
        let previous_world_dataspace_id = state_transaction.world.current_dataspace_id;
        state_transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        state_transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let fee_burn_result = instr.execute(authority, state_transaction);
        state_transaction.current_dataspace_id = previous_tx_dataspace_id;
        state_transaction.world.current_dataspace_id = previous_world_dataspace_id;
        fee_burn_result.map_err(|err| {
            let reason = format!("nexus fee burn failed to apply: {err}");
            sumeragi_status::record_nexus_fee_event(NexusFeeEvent::TransferFailed {
                payer_kind,
                payer_id: payer_id.clone(),
                amount: fee.clone(),
                asset_id: asset_label.clone(),
                reason: reason.clone(),
            });
            warn!(
                target: "economics",
                ?err,
                payer = %payer_id,
                payer_kind = payer_kind_label,
                fee_amount = %fee,
                asset = %asset_label,
                "nexus fee burn failed"
            );
            ValidationFail::from(err)
        })?;

        // Stage the charged event so rejected transactions don't report successful debits.
        state_transaction.stage_nexus_fee_event(NexusFeeEvent::Charged {
            payer_kind,
            payer_id,
            amount: fee,
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
        transaction: &SignedTransaction,
        instructions: Vec<InstructionBox>,
        ivm_proved_replay: Option<crate::pipeline::overlay::IvmProvedReplay>,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
        entrypoint_authorization: Option<&ContractEntrypointAuthorizationSnapshot>,
        tx_bytes_len: usize,
        settlement_source_id: [u8; iroha_crypto::Hash::LENGTH],
        tx_hash: iroha_crypto::HashOf<SignedTransaction>,
        gas_limit_md: Option<u64>,
        require_gas_limit: bool,
        sccp_recording_proof_verified: bool,
        gas_asset_opt: Option<String>,
        fee_sponsor: Option<AccountId>,
        skip_nexus_fee: bool,
        redeem_funded_nexus_fee: bool,
    ) -> Result<(), ValidationFail> {
        if require_gas_limit && gas_limit_md.is_none() {
            return Err(ValidationFail::NotPermitted(
                "missing gas_limit in transaction metadata".to_owned(),
            ));
        }
        if let Some(replay) = ivm_proved_replay.as_ref() {
            crate::validation_fee::enforce_ivm_proved_completed_axt_admission(
                replay.completed_axt.len(),
                state_transaction,
            )?;
        }

        // 1) Deterministically meter the instruction batch. Proved IVM transactions retain the
        // verified replay gas because the plain overlay does not account for VM execution cost.
        let used = ivm_proved_replay.as_ref().map_or_else(
            || isi_gas::meter_instructions(&instructions),
            |replay| replay.gas_used,
        );

        // 2) Enforce optional payer-provided gas limit (caps fee exposure).
        if let Some(limit) = gas_limit_md
            && used > limit
        {
            return Err(ValidationFail::NotPermitted(format!(
                "out of gas: used {used} > limit {limit}"
            )));
        }
        Self::enforce_transaction_gas_fits_block(state_transaction, used)?;

        match (contract_runtime_context, entrypoint_authorization) {
            (Some(context), Some(authorization)) => {
                if !authorization.is_root() {
                    return Err(ValidationFail::NotPermitted(
                        "proved overlay root authorization contains a parent invocation".to_owned(),
                    ));
                }
                let live_subject = code::fetch_bound_contract_subject(
                    state_transaction,
                    &context.contract_address,
                )
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        context.contract_address
                    ))
                })?;
                if context.contract_subject != live_subject
                    || context.contract_address != authorization.contract_address
                    || context.contract_alias != authorization.contract_alias
                    || context.entrypoint != authorization.entrypoint
                {
                    return Err(ValidationFail::NotPermitted(
                        "proved overlay runtime context does not match its immutable authorization snapshot"
                            .to_owned(),
                    ));
                }
                authorization.validate_for_authority(&state_transaction.world, authority)?;
            }
            (Some(_), None) => {
                return Err(ValidationFail::NotPermitted(
                    "proved contract overlay is missing its immutable authorization snapshot"
                        .to_owned(),
                ));
            }
            (None, Some(_)) => {
                return Err(ValidationFail::InternalError(
                    "proved entrypoint authorization has no contract runtime context".to_owned(),
                ));
            }
            (None, None) => {}
        }
        if let Some(replay) = ivm_proved_replay.as_ref()
            && !replay.durable_state_overlay.is_empty()
        {
            let root = entrypoint_authorization.ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "proved durable-state replay is missing its root authorization snapshot"
                        .to_owned(),
                )
            })?;
            crate::pipeline::overlay::validate_ivm_proved_durable_authorizations(
                &state_transaction.world,
                &replay.durable_state_overlay,
                &replay.durable_state_authorizations,
                root,
            )?;
        }

        let instruction_count = instructions.len();
        let confidential_delta = instructions
            .iter()
            .map(crate::gas::confidential_gas_cost)
            .sum::<u64>();

        // 3) Execute ISIs in order.
        let prior_sccp_recording_proof_verified = state_transaction.sccp_recording_proof_verified;
        state_transaction.sccp_recording_proof_verified = sccp_recording_proof_verified;
        let execution_result = (|| -> Result<(), ValidationFail> {
            if let Some(replay) = ivm_proved_replay {
                for queued in replay.queued {
                    match (
                        queued.contract_runtime_context.as_ref(),
                        queued.entrypoint_authorization.as_ref(),
                    ) {
                        (Some(context), Some(authorization)) => {
                            if let Some(root) = entrypoint_authorization
                                && !authorization.descends_from(root)
                            {
                                return Err(ValidationFail::NotPermitted(
                                    "proved overlay effect authorization does not descend from its root invocation"
                                        .to_owned(),
                                ));
                            }
                            let live_subject = code::fetch_bound_contract_subject(
                                state_transaction,
                                &context.contract_address,
                            )
                            .ok_or_else(|| {
                                ValidationFail::NotPermitted(format!(
                                    "contract instance `{}` has no valid subject binding",
                                    context.contract_address
                                ))
                            })?;
                            if context.contract_subject != live_subject
                                || context.contract_address != authorization.contract_address
                                || context.contract_alias != authorization.contract_alias
                                || context.entrypoint != authorization.entrypoint
                                || queued.authority != context.contract_subject
                            {
                                return Err(ValidationFail::NotPermitted(
                                    "proved overlay effect runtime context does not match its immutable authorization snapshot"
                                        .to_owned(),
                                ));
                            }
                            authorization.validate(&state_transaction.world)?;
                        }
                        (Some(_), None) => {
                            return Err(ValidationFail::NotPermitted(
                                "proved contract effect is missing its immutable authorization snapshot"
                                    .to_owned(),
                            ));
                        }
                        (None, Some(_)) => {
                            return Err(ValidationFail::InternalError(
                                "proved effect authorization has no contract runtime context"
                                    .to_owned(),
                            ));
                        }
                        (None, None) => {}
                    }
                    self.execute_instruction_with_contract_runtime_context(
                        state_transaction,
                        &queued.authority,
                        queued.instruction,
                        queued.contract_runtime_context.as_ref(),
                    )?;
                    if let Some(authorization) = queued.entrypoint_authorization.as_ref() {
                        authorization.validate(&state_transaction.world)?;
                    }
                }
                if !replay.durable_state_overlay.is_empty() {
                    let root = entrypoint_authorization.ok_or_else(|| {
                        ValidationFail::NotPermitted(
                            "proved durable-state replay is missing its root authorization snapshot"
                                .to_owned(),
                        )
                    })?;
                    root.validate_for_authority(&state_transaction.world, authority)?;
                    // A queued instruction can revoke the selected permission or replace a live
                    // contract binding. Validate the complete set before recording any replay
                    // artifact or writing the first durable key, so rejection remains atomic.
                    crate::pipeline::overlay::validate_ivm_proved_durable_authorizations(
                        &state_transaction.world,
                        &replay.durable_state_overlay,
                        &replay.durable_state_authorizations,
                        root,
                    )?;
                }
                crate::smartcontracts::ivm::host::HostExecutionArtifacts::record_completed_axt_states(
                    state_transaction,
                    replay.completed_axt,
                );
                for (path, value) in replay.durable_state_overlay {
                    let authorization = replay
                        .durable_state_authorizations
                        .get(&path)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            ValidationFail::InternalError(format!(
                                "proved durable state path `{path}` lost its authorization snapshot before apply"
                            ))
                        })?;
                    authorization.validate(&state_transaction.world)?;
                    if !authorization.owns_durable_state_path(&path) {
                        return Err(ValidationFail::NotPermitted(format!(
                            "proved durable state path `{path}` does not belong to its contract authorization snapshot"
                        )));
                    }
                    if let Some(stored) = value {
                        state_transaction
                            .world
                            .smart_contract_state
                            .insert(path, stored);
                    } else {
                        state_transaction.world.smart_contract_state.remove(path);
                    }
                }
            } else {
                for isi in instructions {
                    if let Some(authorization) = entrypoint_authorization {
                        authorization
                            .validate_for_authority(&state_transaction.world, authority)?;
                    }
                    self.execute_instruction_with_contract_runtime_context(
                        state_transaction,
                        authority,
                        isi,
                        contract_runtime_context,
                    )?;
                    if let Some(authorization) = entrypoint_authorization {
                        authorization
                            .validate_for_authority(&state_transaction.world, authority)?;
                    }
                }
            }
            if let Some(authorization) = entrypoint_authorization {
                authorization.validate_for_authority(&state_transaction.world, authority)?;
            }
            Ok(())
        })();
        state_transaction.sccp_recording_proof_verified = prior_sccp_recording_proof_verified;
        execution_result?;

        // Track confidential gas after successful execution.
        if confidential_delta > 0 {
            state_transaction.record_confidential_gas_delta(confidential_delta);
        }

        // 4) Record gas used for block-level budget enforcement.
        state_transaction.last_tx_gas_used = used;

        // 5) Charge gas fees when configured and the transaction specified a gas asset.
        if should_charge_pipeline_gas_asset(
            skip_nexus_fee,
            state_transaction.nexus.enabled,
            &state_transaction.nexus.fees,
            &gas_asset_opt,
        ) && let Some(gas_asset_id_str) = gas_asset_opt
        {
            Self::charge_pipeline_gas_asset_fee(
                state_transaction,
                authority,
                transaction,
                tx_hash,
                settlement_source_id,
                &gas_asset_id_str,
                used,
                fee_sponsor.as_ref(),
            )?;
        }

        if !skip_nexus_fee {
            Self::charge_nexus_fees(
                state_transaction,
                authority,
                &transaction,
                tx_hash,
                fee_sponsor,
                tx_bytes_len,
                instruction_count,
                used,
                redeem_funded_nexus_fee,
            )?;
        }

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
        let fee_sponsor = resolve_effective_fee_sponsor(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.dataspace_fee_sponsors,
            transaction.metadata(),
            state_transaction.current_dataspace_id,
        )?;
        let skip_nexus_fee = nexus_fee_exempt_transaction(&transaction)
            || successful_claim_fee_exempt_transaction(
                &state_transaction.world,
                &state_transaction.nexus,
                &transaction,
                state_transaction.block_unix_timestamp_ms(),
            );
        if let Some(sponsor) = fee_sponsor.as_ref() {
            if !state_transaction.nexus.fees.sponsorship_enabled {
                sumeragi_status::record_nexus_fee_event(NexusFeeEvent::SponsorDisabled {
                    payer_id: sponsor.to_string(),
                });
                return Err(ValidationFail::NotPermitted(
                    "fee sponsorship is disabled".to_owned(),
                ));
            }
            let sponsorship_fee = if state_transaction.nexus.enabled && !skip_nexus_fee {
                let (_, instruction_count, gas_used) = fee_bound_for_admission(&transaction)
                    .map_err(nexus_fee_admission_error_to_validation_fail)?;
                compute_nexus_fee_amount(
                    &state_transaction.nexus.fees,
                    tx_bytes_len,
                    instruction_count,
                    gas_used,
                )?
            } else {
                Quantity::zero()
            };
            if !state_transaction.nexus.fees.sponsor_max_fee.is_zero()
                && sponsorship_fee > state_transaction.nexus.fees.sponsor_max_fee
            {
                return Err(ValidationFail::NotPermitted(
                    "fee exceeds sponsor_max_fee".to_owned(),
                ));
            }
            if let Err(err) = authorize_fee_sponsor_policy_for_state_transaction(
                state_transaction,
                authority,
                sponsor,
                &transaction,
                sponsorship_fee.as_numeric(),
            ) {
                sumeragi_status::record_nexus_fee_event(NexusFeeEvent::SponsorUnauthorized {
                    sponsor_id: sponsor.to_string(),
                    authority_id: authority.to_string(),
                });
                return Err(err);
            }
            if state_transaction.nexus.enabled && !skip_nexus_fee {
                // Sponsorship is available to any executable that can be fee-metered.
                // Keep the preflight so missing gas limits and fee arithmetic failures
                // are reported before execution.
                let (_, instruction_count, gas_used) = fee_bound_for_admission(&transaction)
                    .map_err(nexus_fee_admission_error_to_validation_fail)?;
                let _ = compute_nexus_fee_amount(
                    &state_transaction.nexus.fees,
                    tx_bytes_len,
                    instruction_count,
                    gas_used,
                )?;
            }
        }
        let redeem_funded_nexus_fee = if state_transaction.nexus.enabled && !skip_nexus_fee {
            let (_, instruction_count, gas_used) = fee_bound_for_admission(&transaction)
                .map_err(nexus_fee_admission_error_to_validation_fail)?;
            let nexus_fee = compute_nexus_fee_amount(
                &state_transaction.nexus.fees,
                tx_bytes_len,
                instruction_count,
                gas_used,
            )?;
            let in_flight_fees = if state_transaction
                .nexus
                .fees
                .lane_relay_burn_receipts_active_at(state_transaction.block_height())
            {
                state_transaction
                    .pending_nexus_fee_amount_for(
                        authority,
                        state_transaction.nexus.fees.fee_asset_id.as_str(),
                    )
                    .ok_or_else(|| {
                        ValidationFail::NotPermitted(
                            "Nexus fee budget arithmetic overflow while summing in-flight receipts"
                                .to_owned(),
                        )
                    })?
            } else {
                Numeric::zero()
            };
            redeem_funded_nexus_fee_covers(
                &state_transaction.world,
                &state_transaction.nexus.fees,
                &transaction,
                state_transaction.block_unix_timestamp_ms(),
                state_transaction.block_height(),
                fee_sponsor.is_some(),
                nexus_fee.as_numeric(),
                in_flight_fees,
            )
            .map_err(nexus_fee_admission_error_to_validation_fail)?
        } else {
            false
        };
        // Bind the transaction call_hash for ISI event emitters to use in audit fields
        let call_hash = transaction.hash_as_entrypoint();
        state_transaction.tx_call_hash = Some(iroha_crypto::Hash::from(call_hash));
        let tx_hash = transaction.hash();
        state_transaction.current_tx_hash = Some(tx_hash.clone());
        let settlement_source_id = {
            let mut bytes = [0u8; iroha_crypto::Hash::LENGTH];
            bytes.copy_from_slice(tx_hash.as_ref());
            bytes
        };
        // Disallow direct signing with multisig accounts; only explicit multisig
        // proposal/approval envelopes with bundled multisig signatures are allowed.
        {
            if let Ok(account) = state_transaction.world.account(authority) {
                if account.id().controller().multisig_policy().is_some() {
                    let only_custom_instruction_envelopes = matches!(
                        transaction.instructions(),
                        Executable::Instructions(items)
                            if !items.is_empty()
                                && items.iter().all(|instruction| {
                                    instruction
                                        .as_any()
                                        .downcast_ref::<CustomInstruction>()
                                        .is_some()
                                })
                    );
                    if only_custom_instruction_envelopes {
                        // Allowed: custom instruction envelopes are validated by their respective
                        // runtime handlers (including multisig propose/approve/register paths).
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
        }
        // Refresh pipeline gas settings from on-chain parameters (genesis/governance updates)
        Self::refresh_gas_from_parameters(state_transaction);
        // Gas asset admission: if an allowlist is configured, require the tx metadata to specify
        // a `gas_asset_id` present in the allowlist. The value must be a valid
        // unprefixed Base58 `AssetDefinitionId` string.
        let md = transaction.metadata().clone();
        let gas_asset_opt = md.get("gas_asset_id").map(|j| j.as_ref().to_string());
        // Payer-provided gas limit (optional for non-VM transactions); used to cap fee exposure
        let gas_limit_md = parse_gas_limit(&md)?;
        configure_executor_fuel_budget(self, state_transaction, &md)?;
        let pipeline_gas = &state_transaction.pipeline.gas;
        if !skip_nexus_fee && !pipeline_gas.accepted_assets.is_empty() {
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
        enforce_transaction_contract_permission_before_proof_verification(
            state_transaction,
            authority,
            &transaction,
            ivm_cache,
        )?;
        #[cfg(feature = "zk-preverify")]
        {
            use iroha_data_model::proof::{ProofAttachment, ProofAttachmentList};

            let namespace_hint = md
                .get("contract_alias")
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .and_then(|raw| {
                    raw.trim()
                        .parse::<iroha_data_model::smart_contract::ContractAlias>()
                        .ok()
                })
                .map(|alias| alias.dataspace_segment().to_owned())
                .or_else(|| {
                    md.get("contract_address")
                        .and_then(|value| value.try_into_any_norito::<String>().ok())
                        .and_then(|raw| {
                            raw.trim()
                                .parse::<iroha_data_model::smart_contract::ContractAddress>()
                                .ok()
                        })
                        .and_then(|contract_address| contract_address.dataspace_id().ok())
                        .and_then(|dataspace_id| {
                            state_transaction
                                .nexus
                                .dataspace_catalog
                                .by_id(dataspace_id)
                                .map(|entry| entry.alias.clone())
                        })
                });

            // Process ZK attachments embedded in V2 transactions.
            if let Some(ProofAttachmentList(list)) = transaction.attachments().cloned() {
                // Canonicalize verification order for determinism
                let mut list_sorted = list;
                if list_sorted.is_empty() {
                    return Err(ValidationFail::NotPermitted(
                        "proof attachment list must not be empty".to_owned(),
                    ));
                }
                list_sorted.sort_by(|a, b| {
                    let ah = crate::zk::hash_proof(&a.proof);
                    let bh = crate::zk::hash_proof(&b.proof);
                    (a.backend.as_str(), ah).cmp(&(b.backend.as_str(), bh))
                });
                for attachment in list_sorted.into_iter() {
                    if let Some((field, message)) = attachment.structural_error() {
                        return Err(ValidationFail::NotPermitted(format!(
                            "malformed proof attachment: {field} {message}"
                        )));
                    }
                    let ProofAttachment {
                        backend,
                        proof,
                        vk_ref,
                        vk_commitment,
                        ..
                    } = attachment;
                    // Sanity: proof.backend should match attachment backend
                    if proof.backend != backend {
                        return Err(ValidationFail::NotPermitted(
                            "proof backend mismatch".to_owned(),
                        ));
                    }
                    if vk_ref.backend != backend {
                        return Err(ValidationFail::NotPermitted(
                            "verifying key backend mismatch".to_owned(),
                        ));
                    }
                    if iroha_data_model::zk::BackendTag::is_pending_production_backend_label(
                        backend.as_str(),
                    ) {
                        return Err(ValidationFail::NotPermitted(
                            "pending-production proof backends are not supported".to_owned(),
                        ));
                    }
                    if crate::zk::is_production_claim_backend_label(backend.as_str()) {
                        return Err(ValidationFail::NotPermitted(
                            "production-claim proof backends are not supported".to_owned(),
                        ));
                    }
                    if crate::zk::is_trusted_setup_backend_label(backend.as_str()) {
                        return Err(ValidationFail::NotPermitted(
                            "trusted-setup proof backends are not supported".to_owned(),
                        ));
                    }
                    if crate::zk::is_developer_only_backend_label(backend.as_str()) {
                        return Err(ValidationFail::NotPermitted(
                            "developer-only proof backends are not supported".to_owned(),
                        ));
                    }
                    if !crate::zk::is_production_verify_backend_label(backend.as_str()) {
                        return Err(ValidationFail::NotPermitted(
                            "unsupported proof backends are not supported".to_owned(),
                        ));
                    }

                    // If a VK reference is provided without a commitment, check existence in
                    // WSV. If a commitment is provided, skip the lookup to keep pre-verify
                    // stateless and cheap.
                    if vk_commitment.is_none()
                        && state_transaction
                            .world
                            .verifying_keys
                            .get(&vk_ref)
                            .is_none()
                    {
                        return Err(ValidationFail::NotPermitted(format!(
                            "referenced verifying key missing: {}::{}",
                            vk_ref.backend, vk_ref.name
                        )));
                    }

                    // Perform lightweight pre-verify (dedup + tag sanity).
                    let block_height = state_transaction.block_height();
                    let (expected_commitment, vk_active) =
                        if let Some(rec) = state_transaction.world.verifying_keys.get(&vk_ref) {
                            if rec.backend.is_pending_production_backend() {
                                return Err(ValidationFail::NotPermitted(
                                    "pending-production verifying key backends are not supported"
                                        .to_owned(),
                                ));
                            }
                            if let Some(ns_hint) = namespace_hint.as_deref() {
                                if !rec.namespace.is_empty() && rec.namespace != ns_hint {
                                    return Err(ValidationFail::NotPermitted(
                                        "verifying key namespace/manifest mismatch".to_owned(),
                                    ));
                                }
                            }
                            (Some(rec.commitment), rec.is_active_at(block_height))
                        } else {
                            (vk_commitment, false)
                        };
                    let res = state_transaction.preverify_proof(
                        &proof,
                        None,
                        state_transaction.zk.preverify_budget_bytes,
                        vk_commitment,
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

        let mut proved_contract_runtime_context = None;
        let mut proved_entrypoint_authorization = None;

        // Full verification for proof-carrying IVM executables must run before we move the
        // transaction payload out of `SignedTransaction`.
        let ivm_proved_replay = if let Executable::IvmProved(proved) = transaction.instructions() {
            if gas_limit_md.is_none() {
                return Err(ValidationFail::NotPermitted(
                    "missing gas_limit in transaction metadata".to_owned(),
                ));
            }

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
            .map_err(overlay_build_error_to_validation_fail)?;

            crate::pipeline::overlay::validate_contract_binding(
                state_transaction,
                &transaction,
                &summary,
            )
            .map_err(overlay_build_error_to_validation_fail)?;

            let selector = requested_contract_entrypoint(transaction.metadata())?.ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "self-describing proved raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                        .to_owned(),
                )
            })?;
            let identity = require_raw_contract_runtime_identity(
                &state_transaction.world,
                summary.code_hash,
                transaction.metadata(),
            )?;
            let authorization = authorize_prepared_raw_contract_selector(
                &state_transaction.world,
                authority,
                summary.prepared_contract(),
                &selector,
                &identity,
            )?;
            let contract_subject =
                code::fetch_bound_contract_subject(state_transaction, &identity.contract_address)
                    .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        identity.contract_address
                    ))
                })?;
            proved_contract_runtime_context = Some(ContractRuntimeExecutionContext {
                contract_subject,
                contract_address: identity.contract_address,
                contract_alias: identity.contract_alias,
                entrypoint: selector,
            });
            proved_entrypoint_authorization = Some(authorization);

            crate::pipeline::overlay::enforce_manifest_is_pre_registered(
                state_transaction,
                &transaction,
                summary.code_hash,
            )
            .map_err(overlay_build_error_to_validation_fail)?;

            let replay = crate::pipeline::overlay::verify_ivm_proved_execution(
                state_transaction,
                &transaction,
                proved,
                &summary,
            )
            .map_err(overlay_build_error_to_validation_fail)?;
            Some(replay)
        } else {
            None
        };

        let tx_creation_time_ms =
            u64::try_from(transaction.creation_time().as_millis()).unwrap_or(u64::MAX);
        let transaction_for_fee = transaction.clone();
        let (tx_authority, executable) = transaction.into();
        debug_assert_eq!(&tx_authority, authority, "authority mismatch");

        match (self, executable) {
            (Self::Initial | Self::UserProvided(_), Executable::Instructions(instructions)) => self
                .execute_metered_instructions(
                    state_transaction,
                    authority,
                    &transaction_for_fee,
                    instructions.into_vec(),
                    None,
                    None,
                    None,
                    tx_bytes_len,
                    settlement_source_id,
                    tx_hash,
                    gas_limit_md,
                    false,
                    false,
                    gas_asset_opt,
                    fee_sponsor,
                    skip_nexus_fee,
                    redeem_funded_nexus_fee,
                ),
            (Self::Initial | Self::UserProvided(_), Executable::IvmProved(_)) => {
                let replay = ivm_proved_replay
                    .expect("proved execution must retain the deterministic replay verified above");
                let instructions = replay
                    .queued
                    .iter()
                    .map(|queued| queued.instruction.clone())
                    .collect();
                self.execute_metered_instructions(
                    state_transaction,
                    authority,
                    &transaction_for_fee,
                    instructions,
                    Some(replay),
                    proved_contract_runtime_context.as_ref(),
                    proved_entrypoint_authorization.as_ref(),
                    tx_bytes_len,
                    settlement_source_id,
                    tx_hash,
                    gas_limit_md,
                    true,
                    true,
                    gas_asset_opt,
                    fee_sponsor,
                    false,
                    redeem_funded_nexus_fee,
                )
            }
            (Self::Initial | Self::UserProvided(_), Executable::ContractCall(call)) => {
                use crate::smartcontracts::ivm::host::CoreHostImpl as CoreCoreHost;

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
                let identity =
                    code::fetch_bound_contract_identity(state_transaction, &call.contract_address)
                        .ok_or_else(|| {
                            ValidationFail::NotPermitted(format!(
                                "contract instance `{}` not found in WSV",
                                call.contract_address
                            ))
                        })?;
                ensure_contract_invocation_code_hash(&call, identity.code_hash)?;
                let contract_subject = code::fetch_bound_contract_subject(
                    state_transaction,
                    &identity.contract_address,
                )
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        identity.contract_address
                    ))
                })?;
                let code_bytes = state_transaction
                    .world
                    .contract_code()
                    .get(&identity.code_hash)
                    .ok_or_else(|| {
                        ValidationFail::NotPermitted(format!(
                            "contract bytecode `{}` not found in WSV",
                            identity.code_hash
                        ))
                    })?;
                let summary = if let Some(summary) = ivm_cache
                    .cached_program_summary(identity.code_hash)
                    .map_err(|error| ValidationFail::InternalError(error.to_string()))?
                {
                    summary
                } else {
                    ivm_cache
                        .summarize_program_with_hash(identity.code_hash, code_bytes.as_ref())
                        .map_err(|error| ValidationFail::InternalError(error.to_string()))?
                };
                if summary.prepared_contract().artifact() != code_bytes.as_slice() {
                    return Err(ValidationFail::NotPermitted(format!(
                        "cached contract bytecode `{}` does not match live WSV",
                        identity.code_hash
                    )));
                }
                let effective_cycles = validate_prepared_ivm_execution_policy(
                    state_transaction,
                    &summary.metadata,
                    summary.code_offset,
                    code_bytes.as_ref(),
                )?;
                let manifest = state_transaction
                    .world
                    .contract_manifests()
                    .get(&identity.code_hash)
                    .ok_or_else(|| {
                        ValidationFail::NotPermitted(format!(
                            "contract instance `{}` has no manifest",
                            identity.contract_address
                        ))
                    })?;
                crate::smartcontracts::ivm::validate_manifest_hashes(
                    manifest,
                    summary.code_hash,
                    summary.abi_hash,
                )
                .map_err(ValidationFail::IvmAdmission)?;
                let lifecycle_transition = validate_prepared_contract_lifecycle_call(
                    &state_transaction.world,
                    &call.contract_address,
                    identity.code_hash,
                    summary.prepared_contract(),
                    &call.entrypoint,
                )?;
                let entrypoint_authorization = authorize_prepared_contract_selector(
                    &state_transaction.world,
                    authority,
                    summary.prepared_contract(),
                    &call.entrypoint,
                    &identity,
                )?;
                let contract_call_context = parse_prepared_contract_invocation_execution_context(
                    &call,
                    summary.prepared_contract(),
                    identity.contract_alias.clone(),
                    contract_subject,
                    effective_limit,
                )?;
                let mut runtime = summary
                    .checkout_runtime(effective_limit)
                    .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
                runtime.set_max_cycles(effective_cycles.get());
                runtime.set_gas_limit(effective_limit);
                if let Some(argument_record) = contract_call_context.argument_record.as_ref() {
                    argument_record
                        .precharge_vm(&mut runtime)
                        .map_err(|error| ValidationFail::NotPermitted(error.to_string()))?;
                }
                if let Some(entrypoint_pc) = contract_call_context.entrypoint_pc {
                    let code_len = runtime.memory.code_len();
                    runtime.set_register(1, code_len);
                    runtime.set_program_counter(entrypoint_pc).map_err(|err| {
                        let selector = contract_call_context
                            .entrypoint
                            .as_deref()
                            .unwrap_or("main");
                        ValidationFail::NotPermitted(format!(
                            "contract entrypoint `{selector}` resolved to invalid pc: {err}"
                        ))
                    })?;
                }
                let contract_runtime_context = contract_call_context.runtime_context();
                let accounts = state_transaction.accounts_snapshot();
                let mut host = CoreCoreHost::with_accounts_and_argument_record(
                    authority.clone(),
                    Arc::clone(&accounts),
                    contract_call_context.argument_record,
                );
                host.set_prepared_contract_cache(summary.prepared_contract_cache());
                // User contract calls execute before the enclosing block has a finalized
                // creation timestamp, so expose the transaction creation time as the
                // logical "current time" seen by `current_time_ms()`.
                host.set_block_time_ms(tx_creation_time_ms);
                host.set_crypto_config(Arc::clone(&state_transaction.crypto));
                host.set_zk_config(&state_transaction.zk);
                host.set_public_inputs_from_parameters(state_transaction.world.parameters.get());
                host.set_vrf_epoch_seeds_from_world(&state_transaction.world);
                host.set_query_state(state_transaction);
                host.set_contract_runtime_context(contract_runtime_context.clone());
                host.set_contract_entrypoint_authorization(Some(entrypoint_authorization));
                if let Some(pending) = lifecycle_transition {
                    host.set_contract_lifecycle_transition(&call.contract_address, pending);
                }
                host.set_chain_id(&state_transaction.chain_id);
                #[cfg(feature = "telemetry")]
                host.set_telemetry(state_transaction.telemetry.clone());
                host.set_zk_snapshots_from_world(&state_transaction.world, &state_transaction.zk)
                    .map_err(|err| {
                        ValidationFail::InternalError(format!("invalid ZK snapshot state: {err}"))
                    })?;
                if let Err(err) = runtime.run_with_host(&mut host) {
                    return Err(
                        crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(
                            &runtime, &err,
                        ),
                    );
                }
                let gas_used = effective_limit.saturating_sub(runtime.remaining_gas());
                let artifacts = host.into_execution_artifacts(contract_runtime_context)?;
                if let Some(pending) = lifecycle_transition {
                    code::validate_contract_lifecycle_completion(
                        &state_transaction.world,
                        &call.contract_address,
                        pending,
                    )?;
                }
                let _executed = artifacts.apply_to_transaction_with_lifecycle(
                    state_transaction,
                    authority,
                    lifecycle_transition.map(|pending| (&call.contract_address, pending)),
                )?;
                state_transaction.last_tx_gas_used = gas_used;
                Self::enforce_transaction_gas_fits_block(state_transaction, gas_used)?;

                if should_charge_pipeline_gas_asset(
                    skip_nexus_fee,
                    state_transaction.nexus.enabled,
                    &state_transaction.nexus.fees,
                    &gas_asset_opt,
                ) && let Some(gas_asset_id_str) = gas_asset_opt
                {
                    Self::charge_pipeline_gas_asset_fee(
                        state_transaction,
                        authority,
                        &transaction_for_fee,
                        tx_hash,
                        settlement_source_id,
                        &gas_asset_id_str,
                        gas_used,
                        fee_sponsor.as_ref(),
                    )?;
                }

                Ok(())
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
                let admitted = ivm_cache
                    .summarize_executable(bytes.as_ref())
                    .map_err(crate::smartcontracts::ivm::program_admission_error)?;
                let summary = match admitted {
                    ExecutableProgramSummary::Contract(summary) => summary,
                    ExecutableProgramSummary::Generic(summary) => {
                        crate::smartcontracts::ivm::validate_generic_execution_context(
                            &state_transaction.world,
                            &md,
                            summary.code_hash,
                        )?;
                        let effective_cycles = validate_prepared_ivm_execution_policy(
                            state_transaction,
                            &summary.metadata,
                            summary.code_offset,
                            summary.program(),
                        )?;

                        let prepared_contract_cache = ivm_cache.prepared_contract_cache();
                        let amx_analysis =
                            ivm_cache
                                .analyze_generic_program(&summary)
                                .map_err(|error| {
                                    ValidationFail::InternalError(format!(
                                        "invalid admitted generic-program analysis: {error}"
                                    ))
                                })?;
                        let streaming_metadata =
                            crate::pipeline::overlay::resolve_streaming_metadata(
                                state_transaction,
                                authority,
                            );
                        let bound_contract_records =
                            code::snapshot_bound_contract_records_by_subject(state_transaction);
                        let axt_policy_snapshot = state_transaction.axt_policy_snapshot();
                        let mut runtime = ivm_cache
                            .checkout_generic_runtime(&summary, effective_limit)
                            .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
                        runtime.set_max_cycles(effective_cycles.get());
                        runtime.set_gas_limit(effective_limit);
                        let accounts = state_transaction.accounts_snapshot();
                        let mut host =
                            CoreCoreHost::with_accounts(authority.clone(), Arc::clone(&accounts));
                        host.set_generic_execution();
                        host.set_prepared_contract_cache(prepared_contract_cache);
                        host.set_amx_analysis(amx_analysis);
                        host.set_amx_limits(
                            crate::smartcontracts::ivm::host::CoreHost::amx_limits_from_config(
                                state_transaction.pipeline(),
                            ),
                        );
                        host.set_axt_timing(state_transaction.nexus().axt);
                        host.hydrate_axt_replay_ledger(state_transaction);
                        host.set_crypto_config(Arc::clone(&state_transaction.crypto));
                        host.set_zk_config(&state_transaction.zk);
                        host.set_public_inputs_from_parameters(
                            state_transaction.world.parameters.get(),
                        );
                        host.set_vrf_epoch_seeds_from_world(&state_transaction.world);
                        host.set_query_state(state_transaction);
                        host.set_bound_contract_records_by_subject_snapshot(bound_contract_records);
                        host = host.with_axt_policy_snapshot(&axt_policy_snapshot);
                        crate::pipeline::overlay::apply_streaming_metadata(
                            &mut host,
                            streaming_metadata,
                        );
                        host.set_chain_id(&state_transaction.chain_id);
                        #[cfg(feature = "telemetry")]
                        host.set_telemetry(state_transaction.telemetry.clone());
                        host.set_zk_snapshots_from_world(
                            &state_transaction.world,
                            &state_transaction.zk,
                        )
                        .map_err(|err| {
                            ValidationFail::InternalError(format!(
                                "invalid ZK snapshot state: {err}"
                            ))
                        })?;
                        if let Err(err) = runtime.run_with_host(&mut host) {
                            return Err(
                                crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(
                                    &runtime, &err,
                                ),
                            );
                        }
                        let gas_used = effective_limit.saturating_sub(runtime.remaining_gas());
                        let artifacts = host.into_execution_artifacts(None)?;
                        let _executed =
                            artifacts.apply_to_transaction(state_transaction, authority)?;
                        state_transaction.last_tx_gas_used = gas_used;
                        Self::enforce_transaction_gas_fits_block(state_transaction, gas_used)?;

                        if should_charge_pipeline_gas_asset(
                            skip_nexus_fee,
                            state_transaction.nexus.enabled,
                            &state_transaction.nexus.fees,
                            &gas_asset_opt,
                        ) && let Some(gas_asset_id_str) = gas_asset_opt
                        {
                            Self::charge_pipeline_gas_asset_fee(
                                state_transaction,
                                authority,
                                &transaction_for_fee,
                                tx_hash,
                                settlement_source_id,
                                &gas_asset_id_str,
                                gas_used,
                                fee_sponsor.as_ref(),
                            )?;
                        }
                        Self::charge_nexus_fees(
                            state_transaction,
                            authority,
                            &transaction_for_fee,
                            tx_hash,
                            fee_sponsor,
                            tx_bytes_len,
                            0,
                            gas_used,
                            false,
                        )?;
                        return Ok(());
                    }
                };
                let effective_cycles = validate_prepared_ivm_execution_policy(
                    state_transaction,
                    &summary.metadata,
                    summary.code_offset,
                    bytes.as_ref(),
                )?;
                crate::pipeline::overlay::validate_contract_binding(
                    state_transaction,
                    &transaction_for_fee,
                    &summary,
                )
                .map_err(overlay_build_error_to_validation_fail)?;
                let selector = requested_contract_entrypoint(&md)?.ok_or_else(|| {
                    ValidationFail::NotPermitted(
                        "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                            .to_owned(),
                    )
                })?;
                let runtime_identity = require_raw_contract_runtime_identity(
                    &state_transaction.world,
                    summary.code_hash,
                    &md,
                )?;
                let entrypoint_authorization = authorize_prepared_raw_contract_selector(
                    &state_transaction.world,
                    authority,
                    summary.prepared_contract(),
                    &selector,
                    &runtime_identity,
                )?;
                let contract_subject = code::fetch_bound_contract_subject(
                    state_transaction,
                    &runtime_identity.contract_address,
                )
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no valid subject binding",
                        runtime_identity.contract_address
                    ))
                })?;
                let transition = validate_prepared_contract_lifecycle_call(
                    &state_transaction.world,
                    &runtime_identity.contract_address,
                    runtime_identity.code_hash,
                    summary.prepared_contract(),
                    &selector,
                )?;
                debug_assert!(
                    transition.is_none(),
                    "raw lifecycle selectors are rejected before state validation"
                );
                let mut contract_call_context = parse_prepared_contract_call_execution_context(
                    &md,
                    summary.prepared_contract(),
                    effective_limit,
                )?;
                if let Some(context) = contract_call_context.as_mut() {
                    context.bind_runtime_identity(runtime_identity, contract_subject);
                }
                if let Some(context) = contract_call_context.as_ref() {
                    enforce_contract_entrypoint_permission(
                        &state_transaction.world,
                        authority,
                        context,
                    )?;
                }
                let mut runtime = summary
                    .checkout_runtime(effective_limit)
                    .map_err(|e| ValidationFail::InternalError(e.to_string()))?;
                runtime.set_max_cycles(effective_cycles.get());
                runtime.set_gas_limit(effective_limit);
                if let Some(argument_record) = contract_call_context
                    .as_ref()
                    .and_then(ContractCallExecutionContext::prepared_argument_record)
                {
                    argument_record
                        .precharge_vm(&mut runtime)
                        .map_err(|error| ValidationFail::NotPermitted(error.to_string()))?;
                }
                if let Some(context) = contract_call_context.as_ref() {
                    if let Some(entrypoint_pc) = context.entrypoint_pc {
                        let code_len = runtime.memory.code_len();
                        runtime.set_register(1, code_len);
                        runtime.set_program_counter(entrypoint_pc).map_err(|err| {
                            let selector = context.entrypoint.as_deref().unwrap_or("main");
                            ValidationFail::NotPermitted(format!(
                                "contract entrypoint `{selector}` resolved to invalid pc: {err}"
                            ))
                        })?;
                    }
                }
                let contract_runtime_context = contract_call_context
                    .as_ref()
                    .and_then(ContractCallExecutionContext::runtime_context);
                // Attach host with a snapshot of known accounts for vendor helpers when present.
                let accounts = state_transaction.accounts_snapshot();
                let mut host = if let Some(context) = contract_call_context {
                    CoreCoreHost::with_accounts_and_argument_record(
                        authority.clone(),
                        Arc::clone(&accounts),
                        context.argument_record,
                    )
                } else {
                    CoreCoreHost::with_accounts(authority.clone(), Arc::clone(&accounts))
                };
                host.set_prepared_contract_cache(summary.prepared_contract_cache());
                host.set_crypto_config(Arc::clone(&state_transaction.crypto));
                host.set_zk_config(&state_transaction.zk);
                host.set_public_inputs_from_parameters(state_transaction.world.parameters.get());
                host.set_vrf_epoch_seeds_from_world(&state_transaction.world);
                host.set_query_state(state_transaction);
                host.set_contract_runtime_context(contract_runtime_context.clone());
                host.set_contract_entrypoint_authorization(Some(entrypoint_authorization));
                // Thread chain_id from StateTransaction into the IVM host for VRF binding
                host.set_chain_id(&state_transaction.chain_id);
                #[cfg(feature = "telemetry")]
                host.set_telemetry(state_transaction.telemetry.clone());
                // Thread ZK snapshots (roots, elections, verifying keys) for read/verify syscalls.
                host.set_zk_snapshots_from_world(&state_transaction.world, &state_transaction.zk)
                    .map_err(|err| {
                        ValidationFail::InternalError(format!("invalid ZK snapshot state: {err}"))
                    })?;
                if let Err(err) = runtime.run_with_host(&mut host) {
                    return Err(
                        crate::smartcontracts::ivm::map_vm_error_with_context_to_validation(
                            &runtime, &err,
                        ),
                    );
                }
                let gas_used = effective_limit.saturating_sub(runtime.remaining_gas());

                // Drain and apply queued ISIs deterministically via executor.
                let artifacts = host.into_execution_artifacts(contract_runtime_context)?;
                let _executed = artifacts.apply_to_transaction(state_transaction, authority)?;
                state_transaction.last_tx_gas_used = gas_used;
                Self::enforce_transaction_gas_fits_block(state_transaction, gas_used)?;

                // Charge gas fees: if a gas asset was provided and accepted by policy.
                if should_charge_pipeline_gas_asset(
                    skip_nexus_fee,
                    state_transaction.nexus.enabled,
                    &state_transaction.nexus.fees,
                    &gas_asset_opt,
                ) && let Some(gas_asset_id_str) = gas_asset_opt
                {
                    Self::charge_pipeline_gas_asset_fee(
                        state_transaction,
                        authority,
                        &transaction_for_fee,
                        tx_hash,
                        settlement_source_id,
                        &gas_asset_id_str,
                        gas_used,
                        fee_sponsor.as_ref(),
                    )?;
                }
                Self::charge_nexus_fees(
                    state_transaction,
                    authority,
                    &transaction_for_fee,
                    tx_hash,
                    fee_sponsor,
                    tx_bytes_len,
                    0,
                    gas_used,
                    false,
                )?;
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
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            instruction,
            InstructionExecutionProfile::Runtime,
            None,
        )
    }

    /// Execute [`InstructionBox`] using the runtime profile and an optional
    /// contract execution context for nested contract-originated instructions.
    pub(crate) fn execute_instruction_with_contract_runtime_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            instruction,
            InstructionExecutionProfile::Runtime,
            contract_runtime_context,
        )
    }

    /// Execute a borrowed overlay instruction using the runtime profile.
    ///
    /// The public executor API remains owned-instruction based. Overlay apply
    /// calls this crate-private adapter so built-in executor borrowing can be
    /// extended without changing custom executor or wire/API behaviour.
    pub(crate) fn execute_borrowed_overlay_instruction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        match self {
            Self::Initial => self
                .execute_borrowed_instruction_with_profile_and_contract_runtime_context(
                    state_transaction,
                    authority,
                    instruction,
                    InstructionExecutionProfile::Runtime,
                    contract_runtime_context,
                ),
            Self::UserProvided(_) => {
                iroha_logger::trace!(
                    instr = %instruction.id(),
                    "using owned overlay instruction fallback for user-provided executor"
                );
                self.execute_instruction_with_profile_and_contract_runtime_context(
                    state_transaction,
                    authority,
                    instruction.clone(),
                    InstructionExecutionProfile::Runtime,
                    contract_runtime_context,
                )
            }
        }
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
        self.execute_instruction_with_profile_and_contract_runtime_context(
            state_transaction,
            authority,
            instruction,
            profile,
            None,
        )
    }

    fn execute_instruction_with_profile_and_contract_runtime_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        ensure_lifecycle_hook_cannot_mutate_contract_binding(
            contract_runtime_context,
            &instruction,
        )?;
        trace!("Running instruction execution");
        let instr_id = instruction.id();

        let result = match self {
            Self::Initial => Self::execute_initial_instruction(
                state_transaction,
                authority,
                &instruction,
                profile,
                contract_runtime_context,
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

    fn execute_borrowed_instruction_with_profile_and_contract_runtime_context(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        ensure_lifecycle_hook_cannot_mutate_contract_binding(
            contract_runtime_context,
            instruction,
        )?;
        trace!("Running borrowed instruction execution");
        let instr_id = instruction.id();

        let result = match self {
            Self::Initial => Self::execute_initial_instruction(
                state_transaction,
                authority,
                instruction,
                profile,
                contract_runtime_context,
            ),
            Self::UserProvided(_) => self
                .execute_instruction_with_profile_and_contract_runtime_context(
                    state_transaction,
                    authority,
                    instruction.clone(),
                    profile,
                    contract_runtime_context,
                ),
        };
        if let Err(err) = &result {
            iroha_logger::error!(
                ?profile,
                instr = %instr_id,
                ?err,
                "borrowed instruction execution failed"
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
        let domain = DomainId::parse_fully_qualified(domain_hint).map_err(|_| {
            ValidationFail::NotPermitted("violates multisig role name format".to_owned())
        })?;
        let prefix = iroha_data_model::account::address::chain_discriminant();
        let address = AccountAddress::parse_encoded(last, Some(prefix)).map_err(|_| {
            ValidationFail::NotPermitted("violates multisig role name format".to_owned())
        })?;
        address
            .ensure_domain_matches(&domain)
            .and_then(|_| address.to_account_id())
            .map(Some)
            .map_err(|_| {
                ValidationFail::NotPermitted("violates multisig role name format".to_owned())
            })
    }

    #[allow(clippy::too_many_lines, clippy::items_after_statements)]
    fn execute_initial_instruction(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        profile: InstructionExecutionProfile,
        contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        if matches!(profile, InstructionExecutionProfile::Runtime) {
            iroha_logger::trace!(
                instr = %instruction.id(),
                "executing instruction (Initial executor)"
            );
        }

        match MultisigInstructionBox::try_from(instruction) {
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

        if let Some(context) = contract_runtime_context {
            let contract_permission = instruction
                .as_any()
                .downcast_ref::<GrantBox>()
                .and_then(|grant| match grant {
                    GrantBox::Permission(grant) => Some(&grant.object),
                    GrantBox::Role(_) | GrantBox::RolePermission(_) => None,
                })
                .or_else(|| {
                    instruction
                        .as_any()
                        .downcast_ref::<RevokeBox>()
                        .and_then(|revoke| match revoke {
                            RevokeBox::Permission(revoke) => Some(&revoke.object),
                            RevokeBox::Role(_) | RevokeBox::RolePermission(_) => None,
                        })
                });
            if let Some(permission) = contract_permission {
                let scoped = iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint::try_from(permission)
                    .map_err(|_| ValidationFail::NotPermitted(
                        "deployed contracts may grant or revoke only exact CanInvokeContractEntrypoint tokens"
                            .to_owned(),
                    ))?;
                if authority != &context.contract_subject
                    || scoped.contract != context.contract_address
                    || scoped.entrypoint.is_empty()
                    || scoped.entrypoint.trim() != scoped.entrypoint
                {
                    return Err(ValidationFail::NotPermitted(
                        "deployed contract permission mutation must be bound to its immutable subject, address, and a canonical selector"
                            .to_owned(),
                    ));
                }
            }
        }

        if let Some(register_role) = extract_register_role(instruction) {
            if let Some(multisig_account) =
                Self::multisig_account_from(register_role.object().id())?
            {
                let _ = multisig_account;
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
            // Allow in genesis, or if tx authority owns any domain linked to trigger owner,
            // or if tx authority has explicit CanRegisterTrigger { authority: <owner> }.
            let trg_owner = reg_trg.object().action().authority().clone();
            let is_domain_owner =
                authority_owns_any_alias_domain(&state_transaction.world, authority, &trg_owner)?;

            // Prefer cached permission check; parse once per tx/account.
            let has_permission =
                (!is_genesis) && state_transaction.can_register_trigger_for(authority, &trg_owner);

            if !(is_genesis || is_domain_owner || has_permission) {
                return Err(ValidationFail::NotPermitted(
                    "Can't register trigger owned by another account".to_owned(),
                ));
            }
        }

        if let Some(reg_asset_definition) = extract_register_asset_definition(instruction) {
            ensure_asset_definition_registration_allowed(
                state_transaction,
                authority,
                &reg_asset_definition,
            )?;
        }

        if let Some(account_id) = extract_account_metadata_target(instruction) {
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

        if let Some(transfer_domain) = extract_transfer_domain(instruction)
            && !can_transfer_domain(&state_transaction.world, authority, &transfer_domain)?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer domain of another account".to_owned(),
            ));
        }
        if let Some(transfer_asset_definition) = extract_transfer_asset_definition(instruction)
            && !can_transfer_asset_definition(
                &state_transaction.world,
                authority,
                &transfer_asset_definition,
            )?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer asset definition of another account".to_owned(),
            ));
        }
        if let Some(transfer_nft) = extract_transfer_nft(instruction)
            && !can_transfer_nft(&state_transaction.world, authority, &transfer_nft)?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer NFT of another account".to_owned(),
            ));
        }

        if !is_genesis
            && let Some(transfer_asset) = extract_transfer_asset(instruction)
            && !can_transfer_asset(
                &state_transaction.world,
                authority,
                contract_runtime_context,
                &transfer_asset,
            )?
        {
            return Err(ValidationFail::NotPermitted(
                "Can't transfer asset: source asset owner must sign the transaction".to_owned(),
            ));
        }

        let instruction_id = instruction.id();
        crate::smartcontracts::isi::execute_borrowed_instruction(
            instruction,
            authority,
            state_transaction,
        )
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
                // The iterable query was validated when it started. Execution still
                // binds the cursor to this request's authority in LiveQueryStore
                // before advancing any stored state.
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
    // authenticated header + one HALT instruction.
    if bytecode.len() != ivm::HEADER_SIZE + core::mem::size_of::<u32>() {
        return None;
    }
    let parsed = ivm::ProgramMetadata::parse(bytecode).ok()?;
    if parsed.code_offset != ivm::HEADER_SIZE
        || parsed.metadata.version_major != 1
        || parsed.metadata.version_minor != 1
        || parsed.metadata.mode != 0
        || parsed.metadata.abi_version != 1
    {
        return None;
    }
    let vector_length = parsed.metadata.vector_length;
    let kind = FixtureExecutorKind::from_vector_length(vector_length)?;

    let halt = ivm::encoding::wide::encode_halt().to_le_bytes();
    if bytecode.get(ivm::HEADER_SIZE..) != Some(&halt) {
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
        .checkout_runtime_for_gas_limit(gas_limit)
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
        .checkout_runtime_for_gas_limit(gas_limit)
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
        Ok(()) => {
            if execute_multisig_custom_instruction_if_present(
                state_transaction,
                authority,
                &instruction,
            )? {
                return Ok(());
            }

            instruction
                .execute(authority, state_transaction)
                .map_err(|err| {
                    iroha_logger::debug!(
                        ?err,
                        %instruction_id,
                        authority = %authority,
                        "state application of executor-approved instruction failed"
                    );
                    ValidationFail::from(err)
                })
        }
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

fn execute_multisig_custom_instruction_if_present(
    state_transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: &InstructionBox,
) -> Result<bool, ValidationFail> {
    if instruction
        .as_any()
        .downcast_ref::<CustomInstruction>()
        .is_none()
    {
        return Ok(false);
    }

    let Ok(multisig) = MultisigInstructionBox::try_from(instruction) else {
        return Ok(false);
    };

    crate::smartcontracts::isi::multisig::execute_multisig_instruction(
        state_transaction,
        authority,
        multisig,
    )?;

    Ok(true)
}

#[derive(Debug, Clone, norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
struct FixtureMintAssetForAllAccounts {
    asset_definition: AssetDefinitionId,
    quantity: Quantity,
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

    if execute_multisig_custom_instruction_if_present(state_transaction, authority, &instruction)? {
        return Ok(());
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
        iroha_data_model::isi::Mint::asset_quantity(instruction.quantity.clone(), asset_id)
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
                .map(|value| value.as_ref().as_numeric().clone())
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
                .map(Quantity::into_numeric)
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
) -> Option<Transfer<Asset, Quantity, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) = instr_any.downcast_ref::<Transfer<Asset, Quantity, Account>>() {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Asset(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Asset, Quantity, Account>>(instruction) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    std::panic::catch_unwind(|| {
        let mut slice = &bytes[..];
        Transfer::<Asset, Quantity, Account>::decode(&mut slice).ok()
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
    if !instruction_has_concrete_type::<Transfer<Account, DomainId, Account>>(instruction) {
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

fn extract_transfer_asset_definition(
    instruction: &InstructionBox,
) -> Option<Transfer<Account, AssetDefinitionId, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) =
        instr_any.downcast_ref::<Transfer<Account, AssetDefinitionId, Account>>()
    {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::AssetDefinition(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Account, AssetDefinitionId, Account>>(instruction)
    {
        return None;
    }
    let bytes = instruction.dyn_encode();
    std::panic::catch_unwind(|| {
        let mut slice = &bytes[..];
        Transfer::<Account, AssetDefinitionId, Account>::decode(&mut slice).ok()
    })
    .ok()
    .flatten()
}

fn extract_transfer_nft(
    instruction: &InstructionBox,
) -> Option<Transfer<Account, iroha_data_model::NftId, Account>> {
    let instr_any = instruction.as_any();
    if let Some(transfer) =
        instr_any.downcast_ref::<Transfer<Account, iroha_data_model::NftId, Account>>()
    {
        return Some(transfer.clone());
    }
    if let Some(transfer_box) = instr_any.downcast_ref::<TransferBox>() {
        return match transfer_box {
            TransferBox::Nft(transfer) => Some(transfer.clone()),
            _ => None,
        };
    }
    if !instruction_has_concrete_type::<Transfer<Account, iroha_data_model::NftId, Account>>(
        instruction,
    ) {
        return None;
    }
    let bytes = instruction.dyn_encode();
    std::panic::catch_unwind(|| {
        let mut slice = &bytes[..];
        Transfer::<Account, iroha_data_model::NftId, Account>::decode(&mut slice).ok()
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

pub(crate) fn enforce_contract_entrypoint_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    context: &ContractCallExecutionContext,
) -> Result<(), ValidationFail> {
    let permission = context.entrypoint_permission();
    if permission.is_none() {
        return Ok(());
    }
    let contract_address = context.contract_address.as_ref().ok_or_else(|| {
        ValidationFail::NotPermitted(
            "permissioned contract entrypoint is missing its immutable contract address".to_owned(),
        )
    })?;
    enforce_named_contract_entrypoint_permission(
        world,
        authority,
        contract_address,
        context.entrypoint.as_deref().unwrap_or("main"),
        permission,
    )
}

/// Authorize a prepared deployed-contract selector and capture its immutable apply snapshot.
pub(crate) fn authorize_prepared_contract_selector(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    contract: &ivm::PreparedContract,
    selector: &str,
    identity: &code::BoundContractIdentity,
) -> Result<ContractEntrypointAuthorizationSnapshot, ValidationFail> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        ));
    }
    let (_, permission, _) = resolve_prepared_contract_entrypoint(contract, selector)?;
    let snapshot = ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        selector.to_owned(),
        permission,
        identity,
    );
    snapshot.validate(world)?;
    Ok(snapshot)
}

/// Authorize a prepared deployed-contract read-only selector and capture its immutable snapshot.
pub(crate) fn authorize_prepared_contract_view_selector(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    contract: &ivm::PreparedContract,
    selector: &str,
    identity: &code::BoundContractIdentity,
) -> Result<ContractEntrypointAuthorizationSnapshot, ValidationFail> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        ));
    }
    let (_, permission, _) = resolve_prepared_contract_view_entrypoint(contract, selector)?;
    let snapshot = ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        selector.to_owned(),
        permission,
        identity,
    );
    snapshot.validate(world)?;
    Ok(snapshot)
}

/// Authorize a prepared raw-IVM selector and capture its immutable apply snapshot.
pub(crate) fn authorize_prepared_raw_contract_selector(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    contract: &ivm::PreparedContract,
    selector: &str,
    identity: &code::BoundContractIdentity,
) -> Result<ContractEntrypointAuthorizationSnapshot, ValidationFail> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint must not be empty".to_owned(),
        ));
    }
    let (_, permission, _) = resolve_prepared_raw_contract_entrypoint(contract, selector)?;
    let snapshot = ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        selector.to_owned(),
        permission,
        identity,
    );
    snapshot.validate(world)?;
    Ok(snapshot)
}

/// Enforce the compiler-verified permission attached to a named public entrypoint.
///
/// Overlay preparation, live overlay application, direct execution, triggers,
/// and nested calls all use this helper so none of those paths can drift into a
/// weaker authorization policy.
pub(crate) fn enforce_named_contract_entrypoint_permission(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    entrypoint: &str,
    permission_name: Option<&str>,
) -> Result<(), ValidationFail> {
    let Some(permission_name) = permission_name else {
        return Ok(());
    };
    const SCOPED_PERMISSION_NAME: &str = "CanInvokeContractEntrypoint";
    if permission_name.is_empty()
        || permission_name.trim() != permission_name
        || entrypoint.is_empty()
        || entrypoint.trim() != entrypoint
    {
        return Err(ValidationFail::NotPermitted(
            "contract entrypoint and permission must use non-empty canonical spellings".to_owned(),
        ));
    }

    let target: Permission = if permission_name == SCOPED_PERMISSION_NAME {
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
            contract: contract_address.clone(),
            entrypoint: entrypoint.to_owned(),
        }
        .into()
    } else {
        // The artifact carries only a permission name for custom authorization
        // classes, so its one canonical token is that name with an empty
        // payload. Matching by name alone would let a differently scoped token
        // with the same name authorize this entrypoint.
        Permission::new(permission_name.to_owned(), Json::new(()))
    };
    if authority_has_permission(world, authority, &target)? {
        return Ok(());
    }

    if permission_name == SCOPED_PERMISSION_NAME {
        Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{entrypoint}` on `{contract_address}` requires an exact `{SCOPED_PERMISSION_NAME}` grant"
        )))
    } else {
        Err(ValidationFail::NotPermitted(format!(
            "contract entrypoint `{entrypoint}` requires permission `{permission_name}` with the canonical empty payload"
        )))
    }
}

fn enforce_transaction_contract_permission_before_proof_verification<R>(
    state: &R,
    authority: &AccountId,
    transaction: &SignedTransaction,
    ivm_cache: &mut IvmCache,
) -> Result<(), ValidationFail>
where
    R: StateReadOnly,
{
    match transaction.instructions() {
        Executable::Instructions(_) => Ok(()),
        Executable::ContractCall(call) => {
            let identity = code::fetch_bound_contract_identity(state, &call.contract_address)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` not found in WSV",
                        call.contract_address
                    ))
                })?;
            ensure_contract_invocation_code_hash(call, identity.code_hash)?;
            let code_bytes = state
                .world()
                .contract_code()
                .get(&identity.code_hash)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract bytecode `{}` not found in WSV",
                        identity.code_hash
                    ))
                })?;
            let summary = if let Some(summary) = ivm_cache
                .cached_program_summary(identity.code_hash)
                .map_err(|error| ValidationFail::InternalError(error.to_string()))?
            {
                summary
            } else {
                ivm_cache
                    .summarize_program_with_hash(identity.code_hash, code_bytes.as_ref())
                    .map_err(|error| ValidationFail::InternalError(error.to_string()))?
            };
            if summary.prepared_contract().artifact() != code_bytes.as_slice() {
                return Err(ValidationFail::NotPermitted(format!(
                    "cached contract bytecode `{}` does not match live WSV",
                    identity.code_hash
                )));
            }
            authorize_prepared_contract_selector(
                state.world(),
                authority,
                summary.prepared_contract(),
                &call.entrypoint,
                &identity,
            )
            .map(drop)?;
            validate_prepared_ivm_execution_policy(
                state,
                &summary.metadata,
                summary.code_offset,
                code_bytes.as_ref(),
            )?;
            let manifest = state
                .world()
                .contract_manifests()
                .get(&identity.code_hash)
                .ok_or_else(|| {
                    ValidationFail::NotPermitted(format!(
                        "contract instance `{}` has no manifest",
                        identity.contract_address
                    ))
                })?;
            crate::smartcontracts::ivm::validate_manifest_hashes(
                manifest,
                summary.code_hash,
                summary.abi_hash,
            )
            .map_err(ValidationFail::IvmAdmission)
        }
        Executable::Ivm(bytecode) => {
            let admitted = ivm_cache
                .summarize_executable(bytecode.as_ref())
                .map_err(crate::smartcontracts::ivm::program_admission_error)?;
            let summary = match admitted {
                ExecutableProgramSummary::Generic(summary) => {
                    crate::smartcontracts::ivm::validate_generic_execution_context(
                        state.world(),
                        transaction.metadata(),
                        summary.code_hash,
                    )?;
                    validate_prepared_ivm_execution_policy(
                        state,
                        &summary.metadata,
                        summary.code_offset,
                        summary.program(),
                    )?;
                    return Ok(());
                }
                ExecutableProgramSummary::Contract(summary) => summary,
            };
            let selector = requested_contract_entrypoint(transaction.metadata())?.ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "self-describing raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                        .to_owned(),
                )
            })?;
            let identity = require_raw_contract_runtime_identity(
                state.world(),
                summary.code_hash,
                transaction.metadata(),
            )?;
            authorize_prepared_raw_contract_selector(
                state.world(),
                authority,
                summary.prepared_contract(),
                &selector,
                &identity,
            )?;
            validate_prepared_ivm_execution_policy(
                state,
                &summary.metadata,
                summary.code_offset,
                bytecode.as_ref(),
            )?;
            crate::pipeline::overlay::validate_contract_binding(state, transaction, &summary)
                .map_err(overlay_build_error_to_validation_fail)?;
            Ok(())
        }
        Executable::IvmProved(proved) => {
            let summary = ivm_cache
                .summarize_program(proved.bytecode.as_ref())
                .map_err(|error| ValidationFail::InternalError(error.to_string()))?;
            let selector = requested_contract_entrypoint(transaction.metadata())?.ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "self-describing proved raw-IVM contract dispatch requires explicit contract_entrypoint metadata"
                        .to_owned(),
                )
            })?;
            let identity = require_raw_contract_runtime_identity(
                state.world(),
                summary.code_hash,
                transaction.metadata(),
            )?;
            authorize_prepared_raw_contract_selector(
                state.world(),
                authority,
                summary.prepared_contract(),
                &selector,
                &identity,
            )?;
            validate_prepared_ivm_execution_policy(
                state,
                &summary.metadata,
                summary.code_offset,
                proved.bytecode.as_ref(),
            )?;
            crate::pipeline::overlay::validate_contract_binding(state, transaction, &summary)
                .map_err(overlay_build_error_to_validation_fail)?;
            Ok(())
        }
    }
}

fn can_modify_account_metadata(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    account_id: &AccountId,
) -> Result<bool, ValidationFail> {
    if authority == account_id {
        return Ok(true);
    }

    let required: Permission = executor_permission::account::CanModifyAccountMetadata {
        account: account_id.clone(),
    }
    .into();
    authority_has_permission(world, authority, &required)
}

fn authority_owns_domain(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    domain_id: &DomainId,
) -> Result<bool, ValidationFail> {
    let owner = world
        .domain(domain_id)
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    Ok(&owner == authority)
}

fn authority_owns_any_alias_domain(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    subject: &AccountId,
) -> Result<bool, ValidationFail> {
    for alias in world.bound_account_aliases(subject) {
        let Some(domain_id) = alias.domain_id(world.dataspace_catalog()).map_err(|err| {
            ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(
                err.to_string().into(),
            ))
        })?
        else {
            continue;
        };
        if authority_owns_domain(world, authority, &domain_id)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn can_transfer_domain(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    transfer: &Transfer<Account, DomainId, Account>,
) -> Result<bool, ValidationFail> {
    if transfer.source() == authority {
        return Ok(true);
    }

    if authority_owns_any_alias_domain(world, authority, transfer.source())? {
        return Ok(true);
    }

    authority_owns_domain(world, authority, transfer.object())
}

fn can_transfer_asset_definition(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    transfer: &Transfer<Account, AssetDefinitionId, Account>,
) -> Result<bool, ValidationFail> {
    if transfer.source() == authority {
        return Ok(true);
    }

    let owner = world
        .asset_definition(transfer.object())
        .map(|definition| definition.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    Ok(&owner == authority)
}

fn can_transfer_nft(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    transfer: &Transfer<Account, iroha_data_model::NftId, Account>,
) -> Result<bool, ValidationFail> {
    if transfer.source() == authority {
        return Ok(true);
    }

    if authority_owns_domain(world, authority, transfer.object().domain())? {
        return Ok(true);
    }

    let required: Permission = executor_permission::nft::CanTransferNft {
        nft: transfer.object().clone(),
    }
    .into();
    authority_has_permission(world, authority, &required)
}

fn can_transfer_asset(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    contract_runtime_context: Option<&ContractRuntimeExecutionContext>,
    transfer: &Transfer<Asset, Quantity, Account>,
) -> Result<bool, ValidationFail> {
    if transfer.source().account() == authority {
        return Ok(true);
    }

    if contract_runtime_context
        .is_some_and(|context| transfer.source().account() == &context.contract_subject)
    {
        return Ok(true);
    }

    if let Some(domain_id) = transfer.source().definition().try_domain()
        && authority_owns_domain(world, authority, domain_id)?
    {
        return Ok(true);
    }

    if authority_owns_any_alias_domain(world, authority, transfer.source().account())? {
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

fn instruction_has_concrete_type<T: 'static>(instruction: &InstructionBox) -> bool {
    instruction.id() == core::any::type_name::<T>()
}

const INITIAL_EXECUTOR_PERMISSION_NAMES: &[&str] = &[
    "CanManagePeers",
    "CanRegisterDomain",
    "CanUnregisterDomain",
    "CanModifyDomainMetadata",
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
    "CanManageSorafsPopRegistry",
    "CanOperateSorafsPopIssuer",
    "CanUpsertSorafsProviderCredit",
    "CanOperateSorafsRepair",
    "CanRegisterSorafsProviderOwner",
    "CanUnregisterSorafsProviderOwner",
    "CanSetMusubiShortAlias",
    "CanIngestSoranetPrivacy",
    "CanResolveEscrowDispute",
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
    if !instruction_has_concrete_type::<Register<AssetDefinition>>(instruction) {
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
    let is_genesis_context = state_transaction._curr_block.is_genesis()
        && state_transaction.block_hashes.is_empty()
        && state_transaction
            .world
            .domain(&iroha_genesis::GENESIS_DOMAIN_ID)
            .is_ok();
    if is_genesis_context {
        return Ok(());
    }

    let Some(domain_id) = reg_asset_definition.object().id().try_domain() else {
        return Ok(());
    };

    let domain_owner = state_transaction
        .world
        .domain(domain_id)
        .map(|domain| domain.owned_by().clone())
        .map_err(|err| ValidationFail::InstructionFailed(InstructionExecutionError::Find(err)))?;
    if &domain_owner == authority {
        return Ok(());
    }

    Err(ValidationFail::NotPermitted(
        "Can't register asset definition".to_owned(),
    ))
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
    runtime_pool: Arc<Mutex<ExecutorRuntimePool>>,
    /// Arc is needed so cloning of executor will be fast.
    /// See [`crate::tx::TransactionExecutor::validate_with_runtime_executor`].
    raw_executor: Arc<data_model_executor::Executor>,
}

// Stack sizing is the only gas-derived property that changes the VM allocation.
// Keep a small bounded LRU so adversarial gas-limit variation cannot retain an
// unbounded number of complete memory images and Merkle baselines.
const EXECUTOR_RUNTIME_VARIANT_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ExecutorRuntimeKey {
    stack_limit: u64,
}

impl ExecutorRuntimeKey {
    fn for_gas_limit(gas_limit: u64) -> Self {
        // The exact gas limit is replenished on every checkout. Keying by it
        // would create distinct variants with identical memory layouts.
        Self {
            stack_limit: stack_limit_for_gas(gas_limit),
        }
    }
}

struct ExecutorRuntimeVariant {
    baseline: Arc<RuntimeTemplate>,
    available: Option<IVM>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ExecutorRuntimePoolStats {
    hits: u64,
    misses: u64,
    program_loads: u64,
    template_builds: u64,
    dirty_resets: u64,
    evictions: u64,
}

#[derive(Clone, Copy)]
enum ExecutorRuntimePoolEvent {
    Hit,
    Miss,
    ProgramLoad,
    TemplateBuild,
    DirtyReset,
    Eviction,
}

struct ExecutorRuntimePool {
    variants: BTreeMap<ExecutorRuntimeKey, ExecutorRuntimeVariant>,
    order: VecDeque<ExecutorRuntimeKey>,
    capacity: usize,
    #[cfg(test)]
    stats: ExecutorRuntimePoolStats,
}

impl ExecutorRuntimePool {
    fn new(
        key: ExecutorRuntimeKey,
        baseline: Arc<RuntimeTemplate>,
        vm: IVM,
        capacity: usize,
    ) -> Self {
        let capacity = capacity.max(1);
        let mut variants = BTreeMap::new();
        variants.insert(
            key,
            ExecutorRuntimeVariant {
                baseline,
                available: Some(vm),
            },
        );
        Self {
            variants,
            order: VecDeque::from([key]),
            capacity,
            #[cfg(test)]
            stats: ExecutorRuntimePoolStats {
                program_loads: 1,
                template_builds: 1,
                ..ExecutorRuntimePoolStats::default()
            },
        }
    }

    fn record(&mut self, event: ExecutorRuntimePoolEvent) {
        #[cfg(test)]
        match event {
            ExecutorRuntimePoolEvent::Hit => {
                self.stats.hits = self.stats.hits.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::Miss => {
                self.stats.misses = self.stats.misses.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::ProgramLoad => {
                self.stats.program_loads = self.stats.program_loads.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::TemplateBuild => {
                self.stats.template_builds = self.stats.template_builds.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::DirtyReset => {
                self.stats.dirty_resets = self.stats.dirty_resets.saturating_add(1);
            }
            ExecutorRuntimePoolEvent::Eviction => {
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
        #[cfg(not(test))]
        let _ = event;
    }

    fn touch(&mut self, key: ExecutorRuntimeKey) {
        if let Some(position) = self.order.iter().position(|candidate| *candidate == key) {
            self.order.remove(position);
        }
        self.order.push_back(key);
    }

    fn insert_variant(&mut self, key: ExecutorRuntimeKey, baseline: Arc<RuntimeTemplate>) {
        while self.variants.len() >= self.capacity {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            if self.variants.remove(&evicted).is_some() {
                self.record(ExecutorRuntimePoolEvent::Eviction);
            }
        }
        self.variants.insert(
            key,
            ExecutorRuntimeVariant {
                baseline,
                available: None,
            },
        );
        self.touch(key);
    }
}

struct ExecutorRuntimeLease {
    pool: Arc<Mutex<ExecutorRuntimePool>>,
    key: ExecutorRuntimeKey,
    baseline: Arc<RuntimeTemplate>,
    vm: Option<IVM>,
}

impl Deref for ExecutorRuntimeLease {
    type Target = IVM;

    fn deref(&self) -> &Self::Target {
        self.vm
            .as_ref()
            .expect("executor runtime lease always owns a VM")
    }
}

impl DerefMut for ExecutorRuntimeLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.vm
            .as_mut()
            .expect("executor runtime lease always owns a VM")
    }
}

impl Drop for ExecutorRuntimeLease {
    fn drop(&mut self) {
        let Some(mut vm) = self.vm.take() else {
            return;
        };

        let can_return = {
            let pool = self.pool.lock().unwrap_or_else(|error| error.into_inner());
            pool.variants.get(&self.key).is_some_and(|variant| {
                Arc::ptr_eq(&variant.baseline, &self.baseline) && variant.available.is_none()
            })
        };
        if !can_return {
            return;
        }

        vm.reset_from_runtime_template(&self.baseline);
        let mut pool = self.pool.lock().unwrap_or_else(|error| error.into_inner());
        let stored = pool.variants.get_mut(&self.key).is_some_and(|variant| {
            if !Arc::ptr_eq(&variant.baseline, &self.baseline) || variant.available.is_some() {
                return false;
            }
            variant.available = Some(vm);
            true
        });
        if stored {
            pool.record(ExecutorRuntimePoolEvent::DirtyReset);
            pool.touch(self.key);
        }
    }
}

fn stack_limit_for_gas(gas_limit: u64) -> u64 {
    IvmConfig::new(gas_limit).stack_limit_for_gas()
}

impl LoadedExecutor {
    pub(crate) fn load(raw_executor: data_model_executor::Executor) -> Result<Self, VMError> {
        let gas_limit = iroha_data_model::parameter::SmartContractParameters::default()
            .fuel
            .get();
        let key = ExecutorRuntimeKey::for_gas_limit(gas_limit);
        let raw_executor = Arc::new(raw_executor);
        let ivm = Self::load_runtime(raw_executor.as_ref(), gas_limit)?;
        let baseline = Arc::new(ivm.runtime_template());
        Ok(Self {
            runtime_pool: Arc::new(Mutex::new(ExecutorRuntimePool::new(
                key,
                baseline,
                ivm,
                EXECUTOR_RUNTIME_VARIANT_CAPACITY,
            ))),
            raw_executor,
        })
    }

    fn load_runtime(
        raw_executor: &data_model_executor::Executor,
        gas_limit: u64,
    ) -> Result<IVM, VMError> {
        let mut vm = IVM::new(gas_limit);
        vm.load_program(raw_executor.bytecode().as_ref())?;
        vm.set_gas_limit(gas_limit);
        Ok(vm)
    }

    fn checkout_runtime_for_gas_limit(
        &self,
        gas_limit: u64,
    ) -> Result<ExecutorRuntimeLease, VMError> {
        let key = ExecutorRuntimeKey::for_gas_limit(gas_limit);
        let (baseline, vm) = {
            let mut pool = self
                .runtime_pool
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if pool.variants.contains_key(&key) {
                let (baseline, vm) = {
                    let variant = pool
                        .variants
                        .get_mut(&key)
                        .expect("checked executor runtime variant exists");
                    (Arc::clone(&variant.baseline), variant.available.take())
                };
                if vm.is_some() {
                    pool.record(ExecutorRuntimePoolEvent::Hit);
                } else {
                    pool.record(ExecutorRuntimePoolEvent::Miss);
                }
                pool.touch(key);
                (baseline, vm)
            } else {
                pool.record(ExecutorRuntimePoolEvent::Miss);
                let vm = Self::load_runtime(self.raw_executor.as_ref(), gas_limit)?;
                let baseline = Arc::new(vm.runtime_template());
                pool.record(ExecutorRuntimePoolEvent::ProgramLoad);
                pool.record(ExecutorRuntimePoolEvent::TemplateBuild);
                pool.insert_variant(key, Arc::clone(&baseline));
                (baseline, Some(vm))
            }
        };

        let mut vm = if let Some(vm) = vm {
            vm
        } else {
            let vm = Self::load_runtime(self.raw_executor.as_ref(), gas_limit)?;
            let mut pool = self
                .runtime_pool
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            pool.record(ExecutorRuntimePoolEvent::ProgramLoad);
            vm
        };
        vm.set_gas_limit(gas_limit);
        Ok(ExecutorRuntimeLease {
            pool: Arc::clone(&self.runtime_pool),
            key,
            baseline,
            vm: Some(vm),
        })
    }

    #[cfg(test)]
    fn runtime_pool_snapshot(&self) -> (ExecutorRuntimePoolStats, usize) {
        let pool = self
            .runtime_pool
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (pool.stats, pool.variants.len())
    }

    #[cfg(test)]
    fn runtime_variant_capacity(&self) -> usize {
        self.runtime_pool
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .capacity
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
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        asset::AssetTransferControlWindow,
        executor::{self as data_model_executor, ExecutorDataModel},
        isi::{Grant, SetAssetTransferControl, SetAssetTransferFreeze},
        name::Name,
        nexus::{
            FeeSponsorContractSelector, FeeSponsorExecutableKind, FeeSponsorPolicy,
            FeeSponsorPolicyId, FeeSponsorRule, FeeSponsorRuleEffect,
        },
        parameter::{CustomParameter, CustomParameterId},
        prelude::*,
        query::{QueryRequest, SingularQueryBox, prelude::FindParameters},
        smart_contract::ContractAddress,
        transaction::executable::IvmBytecode,
    };
    use iroha_executor_data_model::{
        isi::multisig::{MultisigApprove, MultisigPropose, MultisigRegister, MultisigSpec},
        permission::nexus::CanUseFeeSponsor,
    };
    use iroha_primitives::{json::Json, time::TimeSource};
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

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("executor fixture key generation should succeed")
    }

    fn default_fee_sponsor_policy(sponsor: &AccountId) -> FeeSponsorPolicy {
        FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                sponsor.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![FeeSponsorRule::new(FeeSponsorRuleEffect::Allow)],
        }
    }

    fn seed_default_fee_sponsor_policy(world: &mut World, sponsor: &AccountId) {
        let policy = default_fee_sponsor_policy(sponsor);
        world.fee_sponsor_policies.insert(policy.id.clone(), policy);
    }

    fn checked_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("executor algorithm-specific fixture key generation should succeed")
    }

    fn checked_account_id() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }

    #[test]
    fn lifecycle_runtime_context_rejects_binding_mutations_for_every_executor_path() {
        let subject = checked_account_id();
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &subject,
            404,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let instructions = [
            InstructionBox::from(
                iroha_data_model::isi::smart_contract_code::ActivateContractInstance {
                    contract_address: contract_address.clone(),
                    code_hash: Hash::new(b"executor-lifecycle-activation"),
                },
            ),
            InstructionBox::from(
                iroha_data_model::isi::smart_contract_code::DeactivateContractInstance {
                    contract_address: contract_address.clone(),
                    reason: Some("executor lifecycle guard".to_owned()),
                },
            ),
        ];

        for entrypoint in ["hajimari", "始まり", "kaizen", "改善"] {
            let context = ContractRuntimeExecutionContext {
                contract_address: contract_address.clone(),
                contract_subject: contract_address.subject_id(),
                contract_alias: None,
                entrypoint: entrypoint.to_owned(),
            };
            for instruction in &instructions {
                assert!(matches!(
                    ensure_lifecycle_hook_cannot_mutate_contract_binding(
                        Some(&context),
                        instruction,
                    ),
                    Err(ValidationFail::NotPermitted(_))
                ));
            }
        }

        let ordinary_context = ContractRuntimeExecutionContext {
            contract_address,
            contract_subject: subject,
            contract_alias: None,
            entrypoint: "kotoage".to_owned(),
        };
        for instruction in &instructions {
            ensure_lifecycle_hook_cannot_mutate_contract_binding(
                Some(&ordinary_context),
                instruction,
            )
            .expect("ordinary kotoage dispatch is governed by the instruction permission layer");
        }
    }

    #[test]
    fn proved_empty_overlay_accounts_verified_replay_gas() {
        let keypair = checked_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query_handle);
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "gas fixture".to_owned())])
            .sign(keypair.private_key());
        let replay_gas = 40_000;
        let (axt_descriptor, axt_binding) = ivm::axt::AxtDescriptor::builder()
            .dataspace(DataSpaceId::UNIVERSAL)
            .build_with_binding()
            .expect("AXT descriptor");
        let mut completed_axt = ivm::axt::HostAxtState::new(axt_descriptor, axt_binding);
        completed_axt
            .record_proof(
                DataSpaceId::UNIVERSAL,
                Some(ivm::axt::ProofBlob {
                    payload: vec![1],
                    expiry_slot: None,
                }),
                None,
            )
            .expect("record AXT proof");
        completed_axt
            .validate_commit()
            .expect("completed AXT fixture");
        let replay = crate::pipeline::overlay::IvmProvedReplay {
            queued: Vec::new(),
            completed_axt: vec![completed_axt],
            durable_state_overlay: BTreeMap::new(),
            durable_state_authorizations: BTreeMap::new(),
            access_log: None,
            events_commitment: Hash::new(b"events"),
            gas_used: replay_gas,
            trace_hash: Hash::new(b"trace"),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        let tx_hash = tx.hash();
        super::Executor::Initial
            .execute_metered_instructions(
                &mut state_tx,
                &authority,
                &tx,
                Vec::new(),
                Some(replay),
                None,
                None,
                0,
                [0_u8; Hash::LENGTH],
                tx_hash,
                Some(50_000),
                true,
                true,
                None,
                None,
                true,
                false,
            )
            .expect("empty proved overlay should retain replay gas");
        assert_eq!(state_tx.last_tx_gas_used, replay_gas);
        state_tx.apply();
        assert_eq!(
            block.axt_envelopes().len(),
            1,
            "direct proved replay must persist completed AXT envelopes"
        );
        assert_eq!(block.axt_envelopes()[0].binding.as_bytes(), &axt_binding);
    }

    #[test]
    fn proved_replay_applies_durable_state_with_exact_per_path_authorization() {
        let keypair = checked_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").expect("valid test domain"))
                .build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            405,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let code_hash = Hash::new(b"proved durable-state contract");
        let mut world = World::with([domain], [account], []);
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            query::store::LiveQueryStore::start_test(),
        );
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "proved durable fixture".to_owned())])
            .sign(keypair.private_key());

        let authorization = ContractEntrypointAuthorizationSnapshot::new(
            authority.clone(),
            "write".to_owned(),
            None,
            &code::BoundContractIdentity {
                contract_address: contract_address.clone(),
                contract_alias: None,
                contract_alias_binding: None,
                code_hash,
            },
        );
        let runtime_context = ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address: contract_address.clone(),
            contract_alias: None,
            entrypoint: "write".to_owned(),
        };
        let digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
        let marker: Name = format!("sc/{digest}/Values/fixture")
            .parse()
            .expect("scoped durable state marker");
        let stored = vec![0xA5];
        let replay = crate::pipeline::overlay::IvmProvedReplay {
            queued: Vec::new(),
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::from([(marker.clone(), Some(stored.clone()))]),
            durable_state_authorizations: BTreeMap::from([(
                marker.clone(),
                Some(authorization.clone()),
            )]),
            access_log: None,
            events_commitment: Hash::new(b"events"),
            gas_used: 0,
            trace_hash: Hash::new(b"trace"),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        let tx_hash = tx.hash();
        super::Executor::Initial
            .execute_metered_instructions(
                &mut state_tx,
                &authority,
                &tx,
                Vec::new(),
                Some(replay),
                Some(&runtime_context),
                Some(&authorization),
                0,
                [0_u8; Hash::LENGTH],
                tx_hash,
                Some(50_000),
                true,
                true,
                None,
                None,
                true,
                false,
            )
            .expect("proved replay applies its authorized durable write");
        assert_eq!(
            state_tx.world.smart_contract_state.get(&marker),
            Some(&stored)
        );
        drop(state_tx);
        drop(block);

        let malformed_replay = crate::pipeline::overlay::IvmProvedReplay {
            queued: Vec::new(),
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::from([(marker.clone(), Some(stored))]),
            durable_state_authorizations: BTreeMap::new(),
            access_log: None,
            events_commitment: Hash::new(b"malformed-events"),
            gas_used: 0,
            trace_hash: Hash::new(b"malformed-trace"),
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut malformed_block = state.block(header);
        let mut malformed_tx = malformed_block.transaction();
        let error = super::Executor::Initial
            .execute_metered_instructions(
                &mut malformed_tx,
                &authority,
                &tx,
                Vec::new(),
                Some(malformed_replay),
                Some(&runtime_context),
                Some(&authorization),
                0,
                [0_u8; Hash::LENGTH],
                tx_hash,
                Some(50_000),
                true,
                true,
                None,
                None,
                true,
                false,
            )
            .expect_err("post-verification replay metadata must retain an exact authorization map");
        assert!(matches!(
            error,
            ValidationFail::InternalError(message)
                if message.contains("structurally inconsistent")
        ));
        assert!(
            malformed_tx
                .world
                .smart_contract_state
                .get(&marker)
                .is_none(),
            "malformed replay authorization metadata must apply zero durable writes"
        );
        drop(malformed_tx);
        drop(malformed_block);

        let foreign_digest = hex::encode(Hash::new(b"foreign contract namespace").as_ref());
        let foreign_path: Name = format!("sc/{foreign_digest}/Values/fixture")
            .parse()
            .expect("foreign scoped durable state marker");
        let foreign_replay = crate::pipeline::overlay::IvmProvedReplay {
            queued: Vec::new(),
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::from([(foreign_path.clone(), Some(vec![0x5A]))]),
            durable_state_authorizations: BTreeMap::from([(
                foreign_path.clone(),
                Some(authorization.clone()),
            )]),
            access_log: None,
            events_commitment: Hash::new(b"foreign-events"),
            gas_used: 0,
            trace_hash: Hash::new(b"foreign-trace"),
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut foreign_block = state.block(header);
        let mut foreign_tx = foreign_block.transaction();
        let error = super::Executor::Initial
            .execute_metered_instructions(
                &mut foreign_tx,
                &authority,
                &tx,
                Vec::new(),
                Some(foreign_replay),
                Some(&runtime_context),
                Some(&authorization),
                0,
                [0_u8; Hash::LENGTH],
                tx_hash,
                Some(50_000),
                true,
                true,
                None,
                None,
                true,
                false,
            )
            .expect_err("one contract's snapshot must not authorize another state namespace");
        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message)
                if message.contains("does not belong to its contract authorization snapshot")
        ));
        assert!(
            foreign_tx
                .world
                .smart_contract_state
                .get(&foreign_path)
                .is_none(),
            "a foreign per-path snapshot must apply zero durable writes"
        );
    }

    #[test]
    fn proved_replay_rejects_durable_state_without_root_authorization_before_effects() {
        let keypair = checked_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query_handle);
        let tx = TransactionBuilder::new(state.chain_id.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "gas fixture".to_owned())])
            .sign(keypair.private_key());
        let marker: Name = "proved_replay_forbidden_marker"
            .parse()
            .expect("durable state marker");
        let replay = crate::pipeline::overlay::IvmProvedReplay {
            queued: Vec::new(),
            completed_axt: Vec::new(),
            durable_state_overlay: BTreeMap::from([(marker.clone(), Some(vec![0xA5]))]),
            durable_state_authorizations: BTreeMap::from([(marker.clone(), None)]),
            access_log: None,
            events_commitment: Hash::new(b"events"),
            gas_used: 0,
            trace_hash: Hash::new(b"trace"),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        let tx_hash = tx.hash();
        let error = super::Executor::Initial
            .execute_metered_instructions(
                &mut state_tx,
                &authority,
                &tx,
                Vec::new(),
                Some(replay),
                None,
                None,
                0,
                [0_u8; Hash::LENGTH],
                tx_hash,
                Some(50_000),
                true,
                true,
                None,
                None,
                true,
                false,
            )
            .expect_err("proved replay durable state writes require root authorization");

        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message)
                if message.contains("missing its root authorization snapshot")
        ));
        assert!(
            state_tx.world.smart_contract_state.get(&marker).is_none(),
            "rejected proved replay must apply no durable state"
        );
    }

    fn make_peer_id() -> crate::PeerId {
        let kp = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        crate::PeerId::new(kp.public_key().clone())
    }

    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
        assert_eq!(
            checked_keypair_with_algorithm(Algorithm::Ed25519).algorithm(),
            Algorithm::Ed25519
        );
        assert_eq!(
            checked_keypair_with_algorithm(Algorithm::BlsNormal).algorithm(),
            Algorithm::BlsNormal
        );
    }

    fn alice() -> AccountId {
        iroha_test_samples::ALICE_ID.clone()
    }

    #[test]
    fn pipeline_gas_asset_charge_is_disabled_when_nexus_gas_fee_is_active() {
        let mut nexus_fees = NexusFees::default();
        let gas_asset = Some("xor#universal".to_owned());

        nexus_fees.per_gas_unit_fee = Quantity::zero();
        assert!(should_charge_pipeline_gas_asset(
            false,
            true,
            &nexus_fees,
            &gas_asset
        ));

        nexus_fees.per_gas_unit_fee = "0.001".parse().expect("valid gas fee");
        assert!(!should_charge_pipeline_gas_asset(
            false,
            true,
            &nexus_fees,
            &gas_asset
        ));
        assert!(should_charge_pipeline_gas_asset(
            false,
            false,
            &nexus_fees,
            &gas_asset
        ));

        assert!(!should_charge_pipeline_gas_asset(
            true,
            true,
            &nexus_fees,
            &gas_asset
        ));
        assert!(!should_charge_pipeline_gas_asset(
            false,
            false,
            &nexus_fees,
            &None
        ));
    }

    fn seed_verified_nexus_fee_budget(
        state: &State,
        sponsor: &AccountId,
        fee_asset_id: &str,
        verified_balance: Numeric,
    ) {
        let binding = iroha_data_model::nexus::AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: DataSpaceId::UNIVERSAL.as_u64(),
            source_dataspace: "universal".to_owned(),
            source_receipt_id: "executor-test-nexus-fee-budget".to_owned(),
            source_tx_commitment: hex::encode(Hash::new(b"executor-budget-source").as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(Hash::new(b"executor-budget-claim").as_ref()),
            witness_commitment: hex::encode(Hash::new(b"executor-budget-witness").as_ref()),
            policy_commitment: hex::encode(Hash::new(b"executor-budget-policy").as_ref()),
            verified_effect_type: "nexus_fee_budget".to_owned(),
            corridor: "executor-test".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
            effect_binding: None,
        };
        let record = VerifiedNexusFeeBudgetRecord::new(
            sponsor.clone(),
            fee_asset_id.to_owned(),
            verified_balance,
            Hash::new(b"executor-budget-proof-payload"),
            Hash::new(b"executor-budget-statement").into(),
            Hash::new(b"executor-budget-inner-proof"),
            1,
            [0x77; 32],
            binding,
        );
        let key = Name::from_str(&VerifiedNexusFeeBudgetRecord::state_key_for(
            sponsor,
            fee_asset_id,
        ))
        .expect("budget key");
        let json = Json::try_new(record).expect("budget JSON");
        let encoded = norito::to_bytes(&json).expect("budget state");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.world_mut_for_testing()
            .smart_contract_state_mut_for_testing()
            .insert(key, encoded);
        stx.apply();
        block.commit().expect("commit budget cache");
    }

    fn seed_verified_lane_relay_nexus_fee_receipt(
        state: &State,
        payer: &AccountId,
        fee_asset_id: &str,
        fee_amount: Quantity,
        source_id: [u8; 32],
    ) {
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let receipt = iroha_data_model::block::consensus::NexusFeeReceipt {
            version: 1,
            source_id,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_id: iroha_data_model::nexus::LaneId::new(0),
            block_height: 2,
            payer_account_id: payer.clone(),
            fee_asset_id: fee_asset_id.to_owned(),
            fee_amount: fee_amount.clone(),
            schedule: NexusFeeScheduleInputs {
                tx_bytes_len: 0,
                instruction_count: 0,
                gas_used: 0,
                base_fee: fee_amount,
                per_byte_fee: Quantity::zero(),
                per_instruction_fee: Quantity::zero(),
                per_gas_unit_fee: Quantity::zero(),
            },
        };
        let settlement = iroha_data_model::block::consensus::LaneBlockCommitment {
            block_height: 2,
            lane_id: iroha_data_model::nexus::LaneId::new(0),
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::UNIVERSAL,
            tx_count: 1,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: vec![receipt],
            native_amx_receipts: Vec::new(),
        };
        let envelope =
            iroha_data_model::nexus::LaneRelayEnvelope::new(header, None, None, settlement, 0)
                .expect("fee relay envelope")
                .with_manifest_root(Some([0x55; 32]))
                .with_fastpq_proof_material(Some(
                    iroha_data_model::nexus::LaneFastpqProofMaterial {
                        proof_digest: Hash::new(b"executor-lane-relay-proof"),
                        verified_at_height: 2,
                    },
                ));
        let binding = iroha_data_model::nexus::AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: DataSpaceId::UNIVERSAL.as_u64(),
            source_dataspace: "universal".to_owned(),
            source_receipt_id: "executor-test-lane-relay".to_owned(),
            source_tx_commitment: hex::encode(Hash::new(b"executor-relay-source").as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(Hash::new(b"executor-relay-claim").as_ref()),
            witness_commitment: hex::encode(Hash::new(b"executor-relay-witness").as_ref()),
            policy_commitment: hex::encode(Hash::new(b"executor-relay-policy").as_ref()),
            verified_effect_type: "lane_relay".to_owned(),
            corridor: "executor-test".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
            effect_binding: None,
        };
        let record = VerifiedLaneRelayRecord::new(
            envelope.clone(),
            Hash::new(b"executor-lane-relay-payload"),
            Hash::new(b"executor-lane-relay-statement").into(),
            Hash::new(b"executor-lane-relay-proof"),
            2,
            [0x55; 32],
            binding,
        );
        let key = Name::from_str(&envelope.relay_ref().relay_state_key()).expect("relay key");
        let json = Json::try_new(record).expect("relay JSON");
        let encoded = norito::to_bytes(&json).expect("relay state");
        let block_header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        stx.world_mut_for_testing()
            .smart_contract_state_mut_for_testing()
            .insert(key, encoded);
        stx.apply();
        block.commit().expect("commit relay cache");
    }

    fn generate_fixture_placeholder_program(vector_length: u8) -> Vec<u8> {
        let mut program = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length,
            max_cycles: 1_000_000,
            abi_version: 1,
        }
        .encode();
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
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(BOB_ID.clone()).build(&BOB_ID);
        let asset_definition_id: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
        let asset_definition = {
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);

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
            quantity: Quantity::from(1_u32),
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

        assert_eq!(alice_value, Quantity::from(1_u32));
        assert_eq!(bob_value, Quantity::from(1_u32));
    }

    #[test]
    fn fixture_executor_executes_multisig_register_custom_instruction() {
        let bytecode = generate_fixture_placeholder_program(1);
        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(bytecode));
        let executor =
            super::Executor::UserProvided(super::LoadedExecutor::load(raw).expect("load"));

        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let (existing_signer, _existing_signer_keypair) = gen_account_in("wonderland");
        let existing_account = Account::new(existing_signer.clone()).build(&existing_signer);
        let world = World::with([domain], [alice_account, existing_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let missing_signer = checked_account_id();
        let spec = MultisigSpec::new(
            BTreeMap::from([(existing_signer.clone(), 1), (missing_signer.clone(), 1)]),
            std::num::NonZeroU16::new(2).expect("quorum"),
            std::num::NonZeroU64::MAX,
        );
        let seed_account = checked_account_id();
        let instruction: InstructionBox =
            MultisigRegister::with_account(seed_account, domain_id, spec).into();

        executor
            .execute_instruction(&mut stx, &ALICE_ID, instruction)
            .expect("fixture user-provided executor should execute multisig register");

        let created_via_key: Name = "iroha:created_via".parse().expect("metadata key");
        let created = stx
            .world
            .accounts
            .get(&missing_signer)
            .expect("missing signatory should be materialized")
            .clone()
            .into_inner();
        assert_eq!(
            created.metadata().get(&created_via_key),
            Some(&Json::new("multisig"))
        );
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
    fn detached_asset_instructions_cannot_be_constructed_with_negative_quantities() {
        let negative = Numeric::new(-1_i32, 0);
        assert!(Quantity::try_from_numeric(negative).is_err());
    }

    #[test]
    fn detached_nft_metadata_records_delta() {
        let (bob_id, _bob_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(bob_id.clone()).build(&bob_id);
        let nft_id: NftId = "nft_detached$wonderland.universal".parse().expect("nft id");
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
            proof::{
                ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox, VerifyingKeyId,
                VerifyingKeyRecord,
            },
            transaction::{Executable, TransactionBuilder},
            zk::{BackendTag, OpenVerifyEnvelope},
        };
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

        let (_sink_id, _sink_kp) = gen_account_in("wonderland");
        let (_sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut world = World::with([domain], [alice_account], []);
        let backend: Ident = "halo2/ipa".parse().expect("backend ident");
        let vk = VerifyingKeyBox::new(backend.clone(), vec![4u8, 5, 6]);
        let vk_id = VerifyingKeyId::new(backend.clone(), "vk_preverify");
        let vk_commitment = crate::zk::hash_vk(&vk);
        let mut vk_record = VerifyingKeyRecord::new_with_owner(
            1,
            "preverify",
            None,
            "test",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            [0; 32],
            vk_commitment,
        );
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.vk_len = u32::try_from(vk.bytes.len()).expect("fixture vk length fits");
        vk_record.max_proof_bytes = 1024;
        vk_record.key = Some(vk);
        world.verifying_keys.insert(vk_id.clone(), vk_record);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);

        // Build attachments with canonical envelope metadata so preverify
        // exercises deduplication after production-shaped proof admission.
        let envelope = OpenVerifyEnvelope::new(
            BackendTag::Halo2IpaPasta,
            "halo2/ipa:preverify",
            vk_commitment,
            b"preverify-test-schema".to_vec(),
            vec![1u8, 2, 3],
        );
        let proof = ProofBox::new(
            backend.clone(),
            norito::to_bytes(&envelope).expect("encode preverify envelope"),
        );
        let mut attachment = ProofAttachment::new_ref(backend, proof, vk_id);
        attachment.vk_commitment = Some(vk_commitment);
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

    #[cfg(feature = "zk-preverify")]
    #[test]
    fn preverify_attachments_enforce_verifying_key_height_window() {
        use iroha_data_model::{
            proof::{
                ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox, VerifyingKeyId,
                VerifyingKeyRecord,
            },
            transaction::{Executable, TransactionBuilder},
            zk::{BackendTag, OpenVerifyEnvelope},
        };
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

        fn execute_with_window(
            activation_height: Option<u64>,
            withdraw_height: Option<u64>,
            block_height: u64,
        ) -> Result<(), ValidationFail> {
            let domain_id: DomainId =
                DomainId::try_new("wonderland", "universal").expect("domain id");
            let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
            let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let mut world = World::with([domain], [alice_account], []);

            let backend: Ident = "halo2/ipa".parse().expect("backend ident");
            let vk = VerifyingKeyBox::new(backend.clone(), vec![4u8, 5, 6]);
            let vk_id = VerifyingKeyId::new(backend.clone(), "vk_height_window");
            let vk_commitment = crate::zk::hash_vk(&vk);
            let mut vk_record = VerifyingKeyRecord::new_with_owner(
                1,
                "height-window",
                None,
                "test",
                BackendTag::Halo2IpaPasta,
                "pasta",
                [0xAA; 32],
                vk_commitment,
            );
            vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
            vk_record.activation_height = activation_height;
            vk_record.withdraw_height = withdraw_height;
            vk_record.vk_len = u32::try_from(vk.bytes.len()).expect("fixture vk length fits");
            vk_record.max_proof_bytes = 1024;
            vk_record.key = Some(vk);
            world.verifying_keys.insert(vk_id.clone(), vk_record);

            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                "halo2/ipa:height-window",
                vk_commitment,
                b"height-window-public-inputs".to_vec(),
                vec![1u8, 2, 3],
            );
            let proof = ProofBox::new(
                backend.clone(),
                norito::to_bytes(&envelope).expect("encode preverify envelope"),
            );
            let mut attachment = ProofAttachment::new_ref(backend, proof, vk_id);
            attachment.vk_commitment = Some(vk_commitment);
            let tx = TransactionBuilder::new("test-chain".parse().unwrap(), ALICE_ID.clone())
                .with_executable(Executable::Instructions(Vec::new().into()))
                .with_attachments(ProofAttachmentList(vec![attachment]))
                .sign(ALICE_KEYPAIR.private_key());

            let state = State::new_with_chain(
                world,
                Kura::blank_kura_for_testing(),
                query::store::LiveQueryStore::start_test(),
                ChainId::from("test-chain"),
            );
            let block_header = BlockHeader::new(
                std::num::NonZeroU64::new(block_height).expect("nonzero block height"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(block_header);
            let mut state_tx = block.transaction();
            let executor = super::Executor::Initial;
            let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
            executor.execute_transaction(&mut state_tx, &ALICE_ID.clone(), tx, &mut ivm_cache)
        }

        for (label, activation_height, withdraw_height, block_height) in [
            ("future", Some(2), None, 1),
            ("withdrawn", Some(1), Some(1), 1),
            ("expired", Some(1), Some(2), 2),
        ] {
            let err = execute_with_window(activation_height, withdraw_height, block_height)
                .expect_err("out-of-window verifying key must reject");
            match err {
                ValidationFail::NotPermitted(msg) => assert!(
                    msg.contains("verifying key inactive"),
                    "case {label}: unexpected error: {msg}"
                ),
                other => panic!("case {label}: unexpected error: {other:?}"),
            }
        }

        execute_with_window(Some(1), Some(2), 1)
            .expect("in-window active verifying key must preverify");
    }

    #[cfg(feature = "zk-preverify")]
    #[test]
    fn preverify_attachments_reject_non_production_backend_labels_before_vk_lookup() {
        use iroha_data_model::{
            proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
            transaction::{Executable, TransactionBuilder},
        };
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        for (idx, (backend, expected_msg)) in [
            (
                "halo2/ipa:production-ready",
                "production-claim proof backends",
            ),
            ("halo2/ipa:kzg", "trusted-setup proof backends"),
            ("halo2/ipa:dev-fixture", "developer-only proof backends"),
            ("halo2/unknown-native-v1", "unsupported proof backends"),
        ]
        .into_iter()
        .enumerate()
        {
            let backend_ident: Ident = backend.parse().expect("backend ident");
            let proof = ProofBox::new(
                backend_ident.clone(),
                vec![0xA0 | u8::try_from(idx).unwrap()],
            );
            let attachment = ProofAttachment::new_ref(
                backend_ident.clone(),
                proof,
                VerifyingKeyId::new(backend_ident, format!("missing_vk_{idx}")),
            );
            let tx = TransactionBuilder::new("test-chain".parse().unwrap(), ALICE_ID.clone())
                .with_executable(Executable::Instructions(Vec::new().into()))
                .with_attachments(ProofAttachmentList(vec![attachment]))
                .sign(ALICE_KEYPAIR.private_key());

            let mut state_tx = block.transaction();
            let err = executor
                .execute_transaction(&mut state_tx, &ALICE_ID.clone(), tx, &mut ivm_cache)
                .expect_err("non-production proof backend label must fail before vk lookup");
            match err {
                ValidationFail::NotPermitted(msg) => {
                    assert!(
                        msg.contains(expected_msg),
                        "unexpected msg for {backend}: {msg}"
                    );
                    assert!(
                        !msg.contains("referenced verifying key missing"),
                        "backend classification for {backend} must precede vk lookup: {msg}"
                    );
                }
                other => panic!("unexpected error for {backend}: {other:?}"),
            }
        }
    }

    #[cfg(feature = "zk-preverify")]
    #[test]
    fn preverify_attachments_reject_malformed_attachment_shapes_before_vk_lookup() {
        use iroha_data_model::{
            proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
            transaction::{Executable, TransactionBuilder},
        };
        use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        let mut zero_vk_commitment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
        );
        zero_vk_commitment.vk_commitment = Some([0u8; 32]);

        let mut zero_envelope_hash = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
        );
        zero_envelope_hash.envelope_hash = Some([0u8; 32]);

        let mut forged_envelope_hash = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
        );
        let mut forged_hash: [u8; 32] =
            iroha_crypto::Hash::new(&forged_envelope_hash.proof.bytes).into();
        forged_hash[0] ^= 0x80;
        forged_envelope_hash.envelope_hash = Some(forged_hash);

        let cases = [
            (
                "empty-list",
                ProofAttachmentList(Vec::new()),
                "must not be empty",
            ),
            (
                "proof-backend-mismatch",
                ProofAttachmentList(vec![ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("stark/fri".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
                )]),
                "proof.backend",
            ),
            (
                "nonportable-vk-ref-name",
                ProofAttachmentList(vec![ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "VkPreverify"),
                )]),
                "vk_ref",
            ),
            (
                "empty-proof-bytes",
                ProofAttachmentList(vec![ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), Vec::new()),
                    VerifyingKeyId::new("halo2/ipa", "vk_preverify"),
                )]),
                "proof.bytes",
            ),
            (
                "zero-vk-commitment",
                ProofAttachmentList(vec![zero_vk_commitment]),
                "vk_commitment",
            ),
            (
                "zero-envelope-hash",
                ProofAttachmentList(vec![zero_envelope_hash]),
                "envelope_hash",
            ),
            (
                "forged-envelope-hash",
                ProofAttachmentList(vec![forged_envelope_hash]),
                "envelope_hash",
            ),
        ];

        for (label, attachments, expected_msg) in cases {
            let tx = TransactionBuilder::new("test-chain".parse().unwrap(), ALICE_ID.clone())
                .with_executable(Executable::Instructions(Vec::new().into()))
                .with_attachments(attachments)
                .sign(ALICE_KEYPAIR.private_key());

            let mut state_tx = block.transaction();
            let err = executor
                .execute_transaction(&mut state_tx, &ALICE_ID.clone(), tx, &mut ivm_cache)
                .expect_err("malformed proof attachment must fail before vk lookup");
            match err {
                ValidationFail::NotPermitted(msg) => {
                    assert!(
                        msg.contains(expected_msg),
                        "case {label}: expected {expected_msg:?} in error message: {msg}"
                    );
                    assert!(
                        !msg.contains("referenced verifying key missing"),
                        "case {label}: malformed attachment shape must reject before vk lookup: {msg}"
                    );
                }
                other => panic!("case {label}: unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn initial_executor_denies_asset_definition_without_permission() {
        let alice_id = ALICE_ID.clone();
        let genesis_id = SAMPLE_GENESIS_ACCOUNT_ID.clone();

        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
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
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "invalid".parse().unwrap(),
            );
        let instruction = InstructionBox::from(Register::asset_definition({
            let __asset_definition_id = asset_definition_id;
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }));

        let mut stx = block.transaction();
        let res = executor.execute_instruction(&mut stx, &genesis_id, instruction);
        assert!(
            matches!(res, Err(ValidationFail::NotPermitted(_))),
            "initial executor should deny registering asset definition without permission"
        );
    }

    #[test]
    fn borrowed_overlay_apply_matches_owned_initial_executor_for_register_domain() {
        fn test_state() -> State {
            let wonderland_domain_id: DomainId =
                DomainId::try_new("wonderland", "universal").expect("domain id");
            let domain = Domain::new(wonderland_domain_id).build(&ALICE_ID);
            let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let world = World::with([domain], [alice_account], []);
            let kura = Kura::blank_kura_for_testing();
            let query_handle = query::store::LiveQueryStore::start_test();
            State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"))
        }

        let executor = super::Executor::Initial;
        let domain_id: DomainId =
            DomainId::try_new("borrowed-overlay", "universal").expect("domain id");

        let owned_state = test_state();
        let mut owned_block =
            owned_state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut owned_tx = owned_block.transaction();
        let owned_instruction = Register::domain(Domain::new(domain_id.clone())).into();
        executor
            .execute_instruction(&mut owned_tx, &ALICE_ID.clone(), owned_instruction)
            .expect("owned initial executor applies instruction");
        assert!(owned_tx.world.domains.get(&domain_id).is_some());

        let overlay_state = test_state();
        let mut overlay_block =
            overlay_state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut overlay_tx = overlay_block.transaction();
        let overlay_instruction = Register::domain(Domain::new(domain_id.clone())).into();
        let overlay =
            crate::pipeline::overlay::TxOverlay::from_instructions(vec![overlay_instruction]);
        overlay
            .apply_with_chunk(&mut overlay_tx, &ALICE_ID.clone(), 1)
            .expect("borrowed overlay applies instruction");
        assert!(overlay_tx.world.domains.get(&domain_id).is_some());
    }

    #[test]
    fn initial_executor_allows_native_escrow_open_without_transfer_permission() {
        let seller = ALICE_ID.clone();
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        let domain = Domain::new(asset_definition_id.domain().clone()).build(&seller);
        let seller_account = Account::new(seller.clone()).build(&seller);
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("XOR".to_owned())
            .build(&seller);
        let seller_asset_id = AssetId::of(asset_definition_id.clone(), seller.clone());
        let seller_asset = Asset::new(seller_asset_id.clone(), Quantity::from(100_u64));
        let world = World::with_assets(
            [domain],
            [seller_account],
            [asset_definition],
            [seller_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xE5; Hash::LENGTH]));

        let escrow_id = EscrowId::new(Hash::new("executor-native-escrow-open"));
        let instruction = iroha_data_model::isi::escrow::OpenAssetEscrow::new(
            escrow_id,
            asset_definition_id.clone(),
            Quantity::from(40_u64),
        );
        let res = super::Executor::Initial.execute_instruction(
            &mut stx,
            &seller,
            InstructionBox::from(instruction),
        );
        assert!(
            res.is_ok(),
            "native escrow opening should not require generic CanTransferAsset permission: {res:?}"
        );

        let record = stx
            .world
            .asset_escrows
            .get(&escrow_id)
            .expect("escrow record");
        let custody_asset_id = AssetId::of(asset_definition_id, record.custody.clone());
        let seller_balance = stx
            .world
            .assets
            .get(&seller_asset_id)
            .map(|value| value.as_ref().clone())
            .expect("seller balance");
        let custody_balance = stx
            .world
            .assets
            .get(&custody_asset_id)
            .map(|value| value.as_ref().clone())
            .expect("custody balance");
        assert_eq!(seller_balance, Quantity::from(60_u64));
        assert_eq!(custody_balance, Quantity::from(40_u64));
    }

    #[test]
    fn initial_executor_allows_registering_opaque_asset_definition_without_domain_projection() {
        let alice_id = ALICE_ID.clone();
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);

        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let executor = super::Executor::Initial;
        let asset_definition_id = AssetDefinitionId::from_uuid_bytes([
            0x2e, 0x3d, 0x34, 0xbe, 0xb8, 0xa8, 0x42, 0x39, 0xb3, 0xd9, 0x59, 0x07, 0x70, 0xf1,
            0x18, 0x9e,
        ])
        .expect("opaque asset definition id");
        let instruction = InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone()).with_name("cbdc".to_owned()),
        ));

        let mut stx = block.transaction();
        executor
            .execute_instruction(&mut stx, &alice_id, instruction)
            .expect("opaque asset definition should not require a domain projection");
        assert!(
            stx.world.asset_definition(&asset_definition_id).is_ok(),
            "opaque asset definition must be inserted into world state"
        );
    }

    #[test]
    fn extract_transfer_asset_definition_ignores_register_asset_definition_instruction() {
        let asset_definition_id: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("defs", "universal").expect("defs domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let instruction = InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id).with_name("bond".to_owned()),
        ));

        assert!(
            extract_transfer_asset_definition(&instruction).is_none(),
            "register asset-definition instruction must not decode as transfer"
        );
    }

    #[test]
    fn extract_register_asset_definition_accepts_register_asset_definition_instruction() {
        let asset_definition_id: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("defs", "universal").expect("defs domain id"),
            "bond".parse().expect("asset definition name"),
        );
        let instruction = InstructionBox::from(Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone()).with_name("bond".to_owned()),
        ));

        let reg = extract_register_asset_definition(&instruction)
            .expect("expected to extract register asset-definition instruction");
        assert_eq!(reg.object().id(), &asset_definition_id);
    }

    #[test]
    fn initial_executor_denies_transfer_domain_without_ownership() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let foo_domain_id: DomainId = DomainId::try_new("foo", "universal").expect("foo domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
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
                .domain(&users_domain_id)
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
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();

        let users_domain = Domain::new(users_domain_id.clone()).build(&alice_id);
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
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("users", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            user1.clone(),
        );
        let instruction = InstructionBox::from(Transfer::asset_quantity(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let transfer = extract_transfer_asset(&instruction)
            .expect("expected to extract asset transfer from instruction");

        let stx = block.transaction();
        let allowed = can_transfer_asset(&stx.world, &alice_id, None, &transfer)
            .expect("asset transfer permission check");
        assert!(
            allowed,
            "source domain owner should be allowed to transfer account assets"
        );
    }

    #[test]
    fn initial_executor_denies_transfer_asset_without_owner_signature() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
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
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("users", "universal").unwrap(),
                "coin".parse().unwrap(),
            ),
            user1.clone(),
        );
        let instruction = InstructionBox::from(Transfer::asset_quantity(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let transfer = extract_transfer_asset(&instruction)
            .expect("expected to extract asset transfer from instruction");

        let mut stx = block.transaction();
        let allowed = can_transfer_asset(&stx.world, &alice_id, None, &transfer)
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
    fn contract_runtime_context_alias_does_not_bypass_asset_transfer_authorization() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let defs_domain_id: DomainId =
            DomainId::try_new("defs", "universal").expect("defs domain id");
        let alice_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
        let defs_domain = Domain::new(defs_domain_id.clone()).build(&user1);
        let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);
        let asset_definition_id: AssetDefinitionId =
            AssetDefinitionId::new(defs_domain_id.clone(), "coin".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("coin".to_owned())
            .build(&user1);
        let transfer_asset_id = AssetId::new(asset_definition_id.clone(), user1.clone());
        let source_balance = Asset::new(transfer_asset_id.clone(), Quantity::from(10_u32));

        let world = World::with_assets(
            [alice_domain, users_domain, defs_domain],
            [alice_account, user1_account, user2_account],
            [asset_definition],
            [source_balance],
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
        let instruction = InstructionBox::from(Transfer::asset_quantity(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let contract_address = ContractAddress::derive(
            iroha_config::parameters::defaults::common::chain_discriminant(),
            &alice_id,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("benefit contract address");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address,
            contract_alias: Some("benefit::benefit".parse().expect("benefit alias")),
            entrypoint: "spend_to_merchant".to_owned(),
        };

        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xE6; Hash::LENGTH]));
        let result = executor.execute_instruction_with_contract_runtime_context(
            &mut stx,
            &alice_id,
            instruction,
            Some(&context),
        );
        assert!(
            matches!(
                result,
                Err(ValidationFail::NotPermitted(ref message))
                    if message.contains("source asset owner must sign the transaction")
            ),
            "contract alias must not bypass source-owner authorization: {result:?}"
        );
    }

    #[test]
    fn contract_runtime_context_does_not_bypass_non_benefit_spend_entrypoints() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let defs_domain_id: DomainId =
            DomainId::try_new("defs", "universal").expect("defs domain id");
        let alice_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
        let defs_domain = Domain::new(defs_domain_id.clone()).build(&user1);
        let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);
        let asset_definition_id: AssetDefinitionId =
            AssetDefinitionId::new(defs_domain_id.clone(), "coin".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("coin".to_owned())
            .build(&user1);
        let transfer_asset_id = AssetId::new(asset_definition_id.clone(), user1.clone());
        let source_balance = Asset::new(transfer_asset_id.clone(), Quantity::from(10_u32));

        let world = World::with_assets(
            [alice_domain, users_domain, defs_domain],
            [alice_account, user1_account, user2_account],
            [asset_definition],
            [source_balance],
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
        let instruction = InstructionBox::from(Transfer::asset_quantity(
            transfer_asset_id,
            1_u32,
            user2.clone(),
        ));
        let contract_address = ContractAddress::derive(
            iroha_config::parameters::defaults::common::chain_discriminant(),
            &alice_id,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("benefit contract address");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address,
            contract_alias: Some("benefit::benefit".parse().expect("benefit alias")),
            entrypoint: "create_tranche".to_owned(),
        };

        let mut stx = block.transaction();
        let res = executor.execute_instruction_with_contract_runtime_context(
            &mut stx,
            &alice_id,
            instruction,
            Some(&context),
        );
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("source asset owner must sign the transaction"),
                "unexpected rejection message: {msg}"
            ),
            other => panic!(
                "non-spend contract runtime context must not bypass asset transfer checks, got: {other:?}"
            ),
        }
    }

    #[test]
    fn contract_runtime_context_alias_does_not_bypass_permission_grant_authorization() {
        let alice_id = ALICE_ID.clone();
        let beneficiary = checked_account_id();
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let beneficiary_account = Account::new(beneficiary.clone()).build(&beneficiary);
        let world = World::with([domain], [alice_account, beneficiary_account], []);
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

        let raw = data_model_executor::Executor::new(IvmBytecode::from_compiled(
            generate_denied_program("executor denies permission grants"),
        ));
        let executor = super::Executor::UserProvided(
            super::LoadedExecutor::load(raw).expect("load denying executor"),
        );
        let instruction = InstructionBox::from(Grant::account_permission(
            Permission::new("BispSpend".to_owned(), Json::new(())),
            beneficiary.clone(),
        ));
        let contract_address = ContractAddress::derive(
            iroha_config::parameters::defaults::common::chain_discriminant(),
            &alice_id,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("bisp contract address");
        let context = ContractRuntimeExecutionContext {
            contract_subject: contract_address.subject_id(),
            contract_address,
            contract_alias: Some("bisp_bisp::sbp".parse().expect("bisp alias")),
            entrypoint: "create_tranche".to_owned(),
        };
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xE7; Hash::LENGTH]));
        let result = executor.execute_instruction_with_contract_runtime_context(
            &mut stx,
            &alice_id,
            instruction,
            Some(&context),
        );
        assert!(
            matches!(
                result,
                Err(ValidationFail::NotPermitted(ref message))
                    if message.contains("executor denies permission grants")
            ),
            "contract alias must not bypass the user-provided executor verdict: {result:?}"
        );
    }

    #[test]
    fn initial_executor_contract_alias_never_bypasses_permission_grant_validation() {
        fn execute_case(
            alias: &str,
            entrypoint: &str,
            permission_name: &str,
        ) -> Result<(), ValidationFail> {
            let alice_id = ALICE_ID.clone();
            let beneficiary = checked_account_id();
            let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
            let world = World::with(
                [Domain::new(domain_id).build(&alice_id)],
                [
                    Account::new(alice_id.clone()).build(&alice_id),
                    Account::new(beneficiary.clone()).build(&beneficiary),
                ],
                [],
            );
            let state = State::new(
                world,
                Kura::blank_kura_for_testing(),
                query::store::LiveQueryStore::start_test(),
            );
            state
                .block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0))
                .commit()
                .expect("commit bootstrap block");
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
            let contract_address = ContractAddress::derive(
                iroha_config::parameters::defaults::common::chain_discriminant(),
                &alice_id,
                0,
                DataSpaceId::UNIVERSAL,
            )
            .expect("contract address");
            let context = ContractRuntimeExecutionContext {
                contract_subject: contract_address.subject_id(),
                contract_address,
                contract_alias: Some(alias.parse().expect("contract alias")),
                entrypoint: entrypoint.to_owned(),
            };
            let instruction = InstructionBox::from(Grant::account_permission(
                Permission::new(permission_name.to_owned(), Json::new(())),
                beneficiary,
            ));
            super::Executor::Initial.execute_instruction_with_contract_runtime_context(
                &mut block.transaction(),
                &alice_id,
                instruction,
                Some(&context),
            )
        }

        for entrypoint in ["create_tranche", "set_beneficiary_spend_authority"] {
            assert!(matches!(
                execute_case("bisp_bisp::sbp", entrypoint, "BispSpend"),
                Err(ValidationFail::NotPermitted(_))
            ));
        }
        for (alias, entrypoint, permission) in [
            (
                "bisp_bisp::sbp",
                "grant_beneficiary_spend_permission",
                "BispSpend",
            ),
            ("bisp_bisp::sbp", "unrelated", "BispSpend"),
            ("unrelated::sbp", "create_tranche", "BispSpend"),
            ("bisp_bisp::sbp", "create_tranche", "CanSetParameters"),
        ] {
            assert!(
                matches!(
                    execute_case(alias, entrypoint, permission),
                    Err(ValidationFail::NotPermitted(_))
                ),
                "contract {alias}/{entrypoint} must not grant {permission}"
            );
        }
    }

    #[test]
    fn initial_executor_contract_alias_never_bypasses_transfer_control_validation() {
        fn execute_case(
            alias: &str,
            entrypoint: &str,
            instruction_kind: &str,
            window: AssetTransferControlWindow,
        ) -> Result<(), ValidationFail> {
            let caller = ALICE_ID.clone();
            let owner = checked_account_id();
            let target = checked_account_id();
            let domain_id = DomainId::try_new("cbdc", "sbp").expect("domain id");
            let asset_definition_id =
                AssetDefinitionId::new(domain_id.clone(), "pkr".parse().expect("asset name"));
            let world = World::with(
                [Domain::new(domain_id).build(&owner)],
                [
                    Account::new(caller.clone()).build(&caller),
                    Account::new(owner.clone()).build(&owner),
                    Account::new(target.clone()).build(&target),
                ],
                [AssetDefinition::numeric(asset_definition_id.clone())
                    .with_name("PKR".to_owned())
                    .build(&owner)],
            );
            let state = State::new(
                world,
                Kura::blank_kura_for_testing(),
                query::store::LiveQueryStore::start_test(),
            );
            state
                .block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0))
                .commit()
                .expect("commit bootstrap block");
            let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
            let instruction = match instruction_kind {
                "freeze" => InstructionBox::from(SetAssetTransferFreeze::new(
                    target,
                    asset_definition_id,
                    true,
                    Some("branded contract freeze fixture".to_owned()),
                )),
                "limit" => InstructionBox::from(SetAssetTransferControl::new(
                    target,
                    asset_definition_id,
                    vec![iroha_data_model::asset::AssetTransferLimit {
                        window,
                        cap_amount: Some(Numeric::from(100_u32)),
                    }],
                )),
                other => panic!("unsupported test instruction kind {other}"),
            };
            let contract_address = ContractAddress::derive(
                iroha_config::parameters::defaults::common::chain_discriminant(),
                &caller,
                0,
                DataSpaceId::UNIVERSAL,
            )
            .expect("contract address");
            let context = ContractRuntimeExecutionContext {
                contract_subject: contract_address.subject_id(),
                contract_address,
                contract_alias: Some(alias.parse().expect("contract alias")),
                entrypoint: entrypoint.to_owned(),
            };
            super::Executor::Initial.execute_instruction_with_contract_runtime_context(
                &mut block.transaction(),
                &caller,
                instruction,
                Some(&context),
            )
        }

        assert!(matches!(
            execute_case(
                "apps_freeze::sbp",
                "apply_freeze",
                "freeze",
                AssetTransferControlWindow::Day,
            ),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert!(matches!(
            execute_case(
                "apps_limits_update::sbp",
                "apply_limits",
                "limit",
                AssetTransferControlWindow::Day,
            ),
            Err(ValidationFail::NotPermitted(_))
        ));

        for (alias, entrypoint, kind, window) in [
            (
                "apps_freeze::sbp",
                "wrong",
                "freeze",
                AssetTransferControlWindow::Day,
            ),
            (
                "wrong::sbp",
                "apply_freeze",
                "freeze",
                AssetTransferControlWindow::Day,
            ),
            (
                "apps_freeze::sbp",
                "apply_freeze",
                "limit",
                AssetTransferControlWindow::Day,
            ),
            (
                "apps_limits_update::sbp",
                "apply_limits",
                "freeze",
                AssetTransferControlWindow::Day,
            ),
            (
                "apps_limits_update::sbp",
                "apply_limits",
                "limit",
                AssetTransferControlWindow::Week,
            ),
        ] {
            assert!(
                matches!(
                    execute_case(alias, entrypoint, kind, window),
                    Err(ValidationFail::NotPermitted(_))
                ),
                "contract {alias}/{entrypoint} must not emit {kind}/{window}"
            );
        }
    }

    #[test]
    fn initial_executor_denies_transfer_asset_definition_without_ownership() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let defs_domain_id: DomainId =
            DomainId::try_new("defs", "universal").expect("defs domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();
        let alice_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
        let defs_domain = Domain::new(defs_domain_id.clone()).build(&user1);
        let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);

        let asset_definition_id: AssetDefinitionId = AssetDefinitionId::new(
            defs_domain_id.clone(),
            "bond".parse().expect("asset definition name"),
        );
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("bond".to_owned())
            .build(&user1);

        let world = World::with(
            [alice_domain, users_domain, defs_domain],
            [alice_account, user1_account, user2_account],
            [asset_definition],
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
        let instruction = InstructionBox::from(Transfer::asset_definition(
            user1.clone(),
            asset_definition_id.clone(),
            user2.clone(),
        ));
        let transfer = extract_transfer_asset_definition(&instruction)
            .expect("expected to extract asset-definition transfer from instruction");

        let mut stx = block.transaction();
        let allowed = can_transfer_asset_definition(&stx.world, &alice_id, &transfer)
            .expect("asset-definition transfer permission check");
        assert!(
            !allowed,
            "alice should not be allowed to transfer user1-owned asset definition"
        );
        let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("Can't transfer asset definition"),
                "unexpected rejection message: {msg}"
            ),
            other => panic!(
                "initial executor should deny asset-definition transfer from another account, got: {other:?}"
            ),
        }
    }

    #[test]
    fn initial_executor_denies_transfer_asset_definition_by_definition_domain_owner() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let defs_domain_id: DomainId =
            DomainId::try_new("defs", "universal").expect("defs domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();
        let alice_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
        let defs_domain = Domain::new(defs_domain_id.clone()).build(&alice_id);
        let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);

        let asset_definition_id: AssetDefinitionId = AssetDefinitionId::new(
            defs_domain_id.clone(),
            "bond".parse().expect("asset definition name"),
        );
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("bond".to_owned())
            .build(&user1);

        let world = World::with(
            [alice_domain, users_domain, defs_domain],
            [alice_account, user1_account, user2_account],
            [asset_definition],
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

        let instruction = InstructionBox::from(Transfer::asset_definition(
            user1.clone(),
            asset_definition_id.clone(),
            user2.clone(),
        ));
        let transfer = extract_transfer_asset_definition(&instruction)
            .expect("expected to extract asset-definition transfer from instruction");

        let mut stx = block.transaction();
        let allowed = can_transfer_asset_definition(&stx.world, &alice_id, &transfer)
            .expect("asset-definition transfer permission check");
        assert!(
            !allowed,
            "definition-domain ownership must not authorize transfer without source ownership"
        );
        let res = super::Executor::Initial.execute_instruction(&mut stx, &alice_id, instruction);
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("Can't transfer asset definition"),
                "unexpected rejection message: {msg}"
            ),
            other => panic!(
                "initial executor should deny asset-definition transfer by non-source owner, got: {other:?}"
            ),
        }
    }

    #[test]
    fn initial_executor_denies_transfer_nft_without_ownership() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();
        let alice_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");

        let users_domain = Domain::new(users_domain_id.clone()).build(&user1);
        let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);
        let nft_id: NftId = "ticket$users.universal".parse().expect("nft id");
        let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&user1);

        let world = World::with_assets(
            [alice_domain, users_domain],
            [alice_account, user1_account, user2_account],
            [],
            [],
            [nft],
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
        let instruction =
            InstructionBox::from(Transfer::nft(user1.clone(), nft_id.clone(), user2.clone()));
        let transfer = extract_transfer_nft(&instruction)
            .expect("expected to extract nft transfer from instruction");

        let mut stx = block.transaction();
        let allowed = can_transfer_nft(&stx.world, &alice_id, &transfer)
            .expect("nft transfer permission check");
        assert!(
            !allowed,
            "alice should not be allowed to transfer user1-owned nft"
        );
        let res = executor.execute_instruction(&mut stx, &alice_id, instruction);
        match res {
            Err(ValidationFail::NotPermitted(msg)) => assert!(
                msg.contains("Can't transfer NFT"),
                "unexpected rejection message: {msg}"
            ),
            other => panic!(
                "initial executor should deny nft transfer from another account, got: {other:?}"
            ),
        }
    }

    #[test]
    fn initial_executor_allows_transfer_nft_by_nft_domain_owner() {
        let alice_id = ALICE_ID.clone();
        let users_domain_id: DomainId =
            DomainId::try_new("users", "universal").expect("users domain id");
        let user1 = checked_account_id();
        let user2 = checked_account_id();
        let alice_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");

        let users_domain = Domain::new(users_domain_id.clone()).build(&alice_id);
        let alice_domain = Domain::new(alice_domain_id.clone()).build(&alice_id);
        let alice_account = Account::new(alice_id.clone()).build(&alice_id);
        let user1_account = Account::new(user1.clone()).build(&user1);
        let user2_account = Account::new(user2.clone()).build(&user2);
        let nft_id: NftId = "ticket$users.universal".parse().expect("nft id");
        let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&user1);

        let world = World::with_assets(
            [alice_domain, users_domain],
            [alice_account, user1_account, user2_account],
            [],
            [],
            [nft],
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

        let instruction =
            InstructionBox::from(Transfer::nft(user1.clone(), nft_id.clone(), user2.clone()));
        let transfer = extract_transfer_nft(&instruction)
            .expect("expected to extract nft transfer from instruction");

        let mut stx = block.transaction();
        let allowed = can_transfer_nft(&stx.world, &alice_id, &transfer)
            .expect("nft transfer permission check");
        assert!(
            allowed,
            "nft-domain owner should be allowed to transfer ownership"
        );
        let res = super::Executor::Initial.execute_instruction(&mut stx, &alice_id, instruction);
        assert!(res.is_ok(), "expected transfer to succeed, got {res:?}");
    }

    #[test]
    fn initial_executor_denies_nft_metadata_edit_in_transaction() {
        let (bob_id, bob_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let bob_account = Account::new(bob_id.clone()).build(&bob_id);
        let nft_id: NftId = "nft_owner_modify$wonderland.universal"
            .parse()
            .expect("nft id");
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

    fn dpn_contract_call_executable_and_metadata(
        authority: &AccountId,
        entrypoint: &str,
        fee_sponsor: Option<&AccountId>,
    ) -> (Executable, Metadata) {
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            authority,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive DPN contract address");
        let call = iroha_data_model::transaction::executable::ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: iroha_crypto::Hash::new(b"dpn-contract-code"),
            entrypoint: entrypoint.to_owned(),
            arguments: None,
        };
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_address").expect("static name"),
            Json::new(contract_address.to_string()),
        );
        metadata.insert(
            Name::from_str("contract_alias").expect("static name"),
            Json::new("dpn_suite::dpn".to_owned()),
        );
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new(entrypoint.to_owned()),
        );
        metadata.insert(
            Name::from_str("gas_limit").expect("static name"),
            Json::new(1_u64),
        );
        if let Some(fee_sponsor) = fee_sponsor {
            metadata.insert(
                Name::from_str("fee_sponsor").expect("static name"),
                Json::new(fee_sponsor.to_string()),
            );
        }
        (Executable::ContractCall(call), metadata)
    }

    struct SponsoredFeeAdmissionFixture {
        state: State,
        authority_id: AccountId,
        authority_kp: KeyPair,
        sponsor_id: AccountId,
    }

    fn bind_dpn_contract_alias(world: &mut World, address: &ContractAddress) {
        world
            .bind_contract_alias(
                address,
                "dpn_suite::dpn".parse().expect("DPN contract alias"),
                None,
                None,
                0,
            )
            .expect("bind DPN contract alias");
    }

    fn sponsored_fee_admission_fixture(bind_dpn_alias: bool) -> SponsoredFeeAdmissionFixture {
        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let asset_def_id =
            AssetDefinitionId::new(domain_id.clone(), "xor".parse().expect("xor asset name"));
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&authority_id);
        let world = World::with_assets(
            [domain],
            [authority_account, sponsor_account],
            [asset_definition],
            [],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        seed_default_fee_sponsor_policy(&mut state.world, &sponsor_id);
        let dpn_contract_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &authority_id,
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive DPN contract address");
        if bind_dpn_alias {
            bind_dpn_contract_alias(&mut state.world, &dpn_contract_address);
        }
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.external_settlement_enabled = true;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sponsor_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        {
            let mut stx = block.transaction();
            Grant::account_permission(
                CanUseFeeSponsor {
                    sponsor: sponsor_id.clone(),
                    policy: "default".parse().expect("default fee sponsor policy"),
                },
                authority_id.clone(),
            )
            .execute(&sponsor_id, &mut stx)
            .expect("grant fee sponsor permission");
            stx.apply();
        }
        block.commit().expect("commit sponsor permission grant");

        SponsoredFeeAdmissionFixture {
            state,
            authority_id,
            authority_kp,
            sponsor_id,
        }
    }

    fn sign_sponsored_fixture_transaction(
        fixture: &SponsoredFeeAdmissionFixture,
        executable: Executable,
        metadata: Metadata,
    ) -> SignedTransaction {
        let chain: ChainId = "test-chain".parse().unwrap();
        TransactionBuilder::new(chain, fixture.authority_id.clone())
            .with_metadata(metadata)
            .with_executable(executable)
            .sign(fixture.authority_kp.private_key())
    }

    fn expect_sponsored_admission_rejection(
        fixture: &SponsoredFeeAdmissionFixture,
        executable: Executable,
        metadata: Metadata,
        expected_message: &str,
    ) {
        let tx = sign_sponsored_fixture_transaction(fixture, executable, metadata);
        let view = fixture.state.view();
        let err = check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect_err("sponsored transaction should be rejected");
        assert!(matches!(
            err,
            NexusFeeAdmissionError::Rejected(message) if message.contains(expected_message)
        ));
    }

    fn replace_metadata_string(metadata: &mut Metadata, key: &str, value: impl Into<String>) {
        metadata.insert(
            Name::from_str(key).expect("static metadata key"),
            Json::new(value.into()),
        );
    }

    fn sponsored_fee_metadata(fixture: &SponsoredFeeAdmissionFixture) -> Metadata {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static metadata key"),
            Json::new(fixture.sponsor_id.to_string()),
        );
        metadata
    }

    fn insert_gas_limit(metadata: &mut Metadata, gas_limit: u64) {
        metadata.insert(
            Name::from_str("gas_limit").expect("static metadata key"),
            Json::new(gas_limit),
        );
    }

    fn multisig_contract_trigger_instructions(
        fixture: &SponsoredFeeAdmissionFixture,
        entrypoint: &str,
        matching_execute_trigger: bool,
    ) -> Vec<InstructionBox> {
        let (executable, metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            entrypoint,
            Some(&fixture.sponsor_id),
        );
        let trigger_id: iroha_data_model::trigger::TriggerId =
            "sponsored_dpn_contract_call".parse().expect("trigger id");
        let execute_trigger_id = if matching_execute_trigger {
            trigger_id.clone()
        } else {
            "sponsored_dpn_contract_call_other"
                .parse()
                .expect("trigger id")
        };
        let action = iroha_data_model::trigger::action::Action::new(
            executable,
            iroha_data_model::trigger::action::Repeats::Exactly(1),
            fixture.authority_id.clone(),
            iroha_data_model::events::EventFilterBox::ExecuteTrigger(
                iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new(),
            ),
        )
        .with_metadata(metadata);
        let trigger = Trigger::new(trigger_id.clone(), action);
        vec![
            InstructionBox::from(Register::trigger(trigger)),
            InstructionBox::from(iroha_data_model::isi::ExecuteTrigger::new(
                execute_trigger_id,
            )),
        ]
    }

    fn multisig_instruction_tx(
        fixture: &SponsoredFeeAdmissionFixture,
        instruction: impl Into<MultisigInstructionBox>,
    ) -> SignedTransaction {
        multisig_instruction_batch_tx(fixture, vec![InstructionBox::from(instruction.into())])
    }

    fn multisig_instruction_batch_tx(
        fixture: &SponsoredFeeAdmissionFixture,
        instructions: Vec<InstructionBox>,
    ) -> SignedTransaction {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            Json::new(fixture.sponsor_id.to_string()),
        );
        sign_sponsored_fixture_transaction(fixture, Executable::from(instructions), metadata)
    }

    fn nexus_fee_lane_relay_burn_admission_fixture()
    -> (State, AccountId, KeyPair, AssetDefinitionId) {
        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority = Account::new(authority_id.clone()).build(&authority_id);
        let asset_def_id =
            AssetDefinitionId::new(domain_id, "shield".parse().expect("asset definition name"));
        let world = World::with([domain], [authority], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = "xor#universal".to_owned();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 1;
            nexus.fees.canonical_sponsor_account_id = Some(authority_id.to_string());
        }

        (state, authority_id, authority_kp, asset_def_id)
    }

    fn kagemusha_fee_test_proof_attachment(
        label: &str,
    ) -> iroha_data_model::proof::ProofAttachment {
        use iroha_data_model::proof::{ProofAttachment, ProofBox, VerifyingKeyId};

        let backend = "halo2/ipa".parse().expect("backend ident");
        let proof = ProofBox::new(backend, vec![0xA5; 32]);
        ProofAttachment::new_ref(
            proof.backend.clone(),
            proof,
            VerifyingKeyId::new("halo2/ipa", label),
        )
    }

    fn kagemusha_fee_test_recursive_redeem_v2(
        asset: AssetDefinitionId,
        recipient: AccountId,
        signer: &KeyPair,
    ) -> iroha_data_model::isi::offline::RedeemKagemushaRecursiveV2 {
        use iroha_data_model::{
            offline::{
                KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendArtifactBindingV3, KagemushaRecursiveSpendBranchClaimV2,
                KagemushaRecursiveSpendBundleV2, KagemushaRecursiveSpendProofV2,
                KagemushaRecursiveSpendPublicStatementV2, KagemushaRecursiveSpendRedeemRequestV2,
                KagemushaRecursiveSpendRedemptionIntentBuildRequestV2,
                KagemushaRecursiveSpendTopUpAnchorV2, KagemushaRequestAuthorizationV2,
                KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
                KagemushaUnshieldPublicInputsBindingV2, kagemusha_recursive_spend_lineage_root_v2,
            },
            proof::{ProofBox, VerifyingKeyId},
        };

        let chain_id = ChainId::from("fee-policy-chain");
        let amount = KagemushaScaledAmountV2 {
            atomic_units: 1,
            scale: 0,
        };
        let note = KagemushaSpendableNoteDescriptorV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            note_commitment: [0x41; 32],
            spend_nullifier: [0x42; 32],
            amount,
        };
        let artifact_binding = KagemushaRecursiveSpendArtifactBindingV3 {
            generation: "fee-policy-v3".to_owned(),
            manifest_sha256: [0x43; 32],
        };
        let topup_operation_id = [0x47; 32];
        let topup_anchor = KagemushaRecursiveSpendTopUpAnchorV2 {
            version: 2,
            chain_id: chain_id.clone(),
            payer: recipient.clone(),
            asset: AssetId::new(asset.clone(), recipient.clone()),
            asset_scale: amount.scale,
            amount,
            initial_root: [0x44; 32],
            finalized_root: [0x45; 32],
            shield_leaf_index: 0,
            current_note: note.clone(),
            topup_operation_id,
            shield_verifier_id: VerifyingKeyId::new(
                "halo2/ipa",
                "fee-policy-kagemusha-topup-shield-v2",
            ),
            shield_verifier_commitment: [0x53; 32],
            artifact_binding: artifact_binding.clone(),
            finalized_height: 1,
            finalized_tx_hash: [0x54; 32],
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("canonical fee-policy V2 top-up anchor");
        let topup_anchor_ref = topup_anchor
            .compact_ref()
            .expect("canonical fee-policy V2 anchor reference");
        let lineage_root =
            kagemusha_recursive_spend_lineage_root_v2(topup_anchor_ref.anchor_digest)
                .expect("canonical fee-policy V2 lineage root");
        let branch_claim = KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)
            .expect("canonical fee-policy V2 root claim");
        let verifier_key_id =
            VerifyingKeyId::new("halo2/ipa", KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1);
        let statement = KagemushaRecursiveSpendPublicStatementV2 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: amount.scale,
            final_root: topup_anchor.finalized_root,
            topup_anchor_refs: vec![topup_anchor_ref],
            proof_step_count: 1,
            peer_hop_count: 0,
            current_note: note.clone(),
            branch_claims: vec![branch_claim],
            transition: None,
            artifact_binding,
            verifier_key_id: verifier_key_id.clone(),
        };
        let public_statement_digest = statement
            .digest()
            .expect("canonical fee-policy V2 public statement");
        let bundle = KagemushaRecursiveSpendBundleV2 {
            statement,
            recursive_proof: KagemushaRecursiveSpendProofV2 {
                verifier_key_id: verifier_key_id.clone(),
                public_statement_digest,
                proof: ProofBox::new("halo2/ipa".parse().expect("backend ident"), vec![0x49; 32]),
            },
        };
        let operation_id = [0x4A; 32];
        let unshield_public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
            input_commitment_0: note.note_commitment,
            input_commitment_1: [0; 32],
            nullifier_0: note.spend_nullifier,
            nullifier_1: [0; 32],
            change_output_commitment: [0; 32],
            root: bundle.statement.final_root,
            public_amount: iroha_data_model::offline::kagemusha_confidential_amount_encoding_v2(
                amount.atomic_units,
            ),
            asset_tag: [0x4B; 32],
            chain_tag: [0x4C; 32],
        };
        let redemption = KagemushaRecursiveSpendRedemptionIntentBuildRequestV2 {
            previous_bundle: bundle.clone(),
            recipient: recipient.clone(),
            public_amount: amount,
            change_output: None,
            change_artifact_binding: None,
            unshield_public_inputs,
            unshield_public_inputs_digest: unshield_public_inputs
                .digest()
                .expect("canonical fee-policy V2 unshield digest"),
            operation_id,
        }
        .into_intent()
        .expect("canonical fee-policy V2 redemption intent");
        let authorization = KagemushaRequestAuthorizationV2 {
            authority: recipient.clone(),
            device_id: "fee-policy-v2-device".to_owned(),
            operation_id,
            issued_at_ms: 1,
            expires_at_ms: 2,
            nonce: [0x51; 32],
            payload_digest: [0x52; 32],
            app_attest_evidence_sha256: None,
            app_attest_evidence: None,
            signature: iroha_crypto::Signature::try_new(
                signer.private_key(),
                b"fee-policy-v2-unsupported",
            )
            .expect("fixture signature"),
        };
        let request = KagemushaRecursiveSpendRedeemRequestV2 {
            bundle,
            recipient,
            amount,
            redeem_proof: kagemusha_fee_test_proof_attachment("fee-policy-kagemusha-redeem-v2"),
            redemption,
            offline_change: None,
            block_height: 1,
            operation_id,
            authorization,
        };
        iroha_data_model::isi::offline::RedeemKagemushaRecursiveV2::new(request)
    }

    fn signed_fee_policy_transaction(
        authority_id: AccountId,
        authority_kp: &KeyPair,
        instruction: InstructionBox,
    ) -> SignedTransaction {
        signed_fee_policy_batch_transaction(authority_id, authority_kp, vec![instruction])
    }

    fn signed_fee_policy_batch_transaction(
        authority_id: AccountId,
        authority_kp: &KeyPair,
        instructions: Vec<InstructionBox>,
    ) -> SignedTransaction {
        let chain: ChainId = "fee-policy-chain".parse().unwrap();
        TransactionBuilder::new(chain, authority_id)
            .with_executable(Executable::from(instructions))
            .sign(authority_kp.private_key())
    }

    fn assert_lane_relay_burn_requires_fee_budget(state: &State, tx: &SignedTransaction) {
        let view = state.view();
        let err = check_external_nexus_fee_admission(&view.world, &view.nexus, tx, 0, 2, None)
            .expect_err("fee-paying transaction must require a verified Nexus budget");
        assert!(
            matches!(err, NexusFeeAdmissionError::Rejected(ref reason) if reason.contains("missing verified Nexus fee budget")),
            "unexpected Nexus fee admission error: {err:?}"
        );
    }

    #[test]
    fn nexus_fee_sponsor_accepts_configured_dpn_contract_call() {
        let fixture = sponsored_fee_admission_fixture(true);
        let (executable, metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            "transfer_dpn",
            Some(&fixture.sponsor_id),
        );
        let tx = sign_sponsored_fixture_transaction(&fixture, executable, metadata);
        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("fee-metered contract call should be sponsored");
    }

    #[test]
    fn nexus_fee_sponsor_accepts_contract_call_allowed_by_wildcard_policy() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let (mut executable, mut metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            "transfer_dpn",
            Some(&fixture.sponsor_id),
        );
        let other_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &fixture.authority_id,
            8,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive non-DPN contract address");
        fixture
            .state
            .world
            .bind_contract_alias(
                &other_address,
                "not_dpn::dpn".parse().expect("non-DPN contract alias"),
                None,
                None,
                0,
            )
            .expect("bind non-DPN contract alias");
        if let Executable::ContractCall(call) = &mut executable {
            call.contract_address = other_address.clone();
        }
        replace_metadata_string(&mut metadata, "contract_address", other_address.to_string());
        replace_metadata_string(&mut metadata, "contract_alias", "not_dpn::dpn");

        let tx = sign_sponsored_fixture_transaction(&fixture, executable, metadata);
        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("wildcard fee sponsor policy should allow the contract call");
    }

    #[test]
    fn nexus_fee_sponsor_policy_ignores_unbound_contract_alias_metadata() {
        let mut fixture = sponsored_fee_admission_fixture(false);
        let mut allow_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        allow_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::ContractCall);
        allow_rule
            .contract_selectors
            .push(FeeSponsorContractSelector {
                contract_alias: Some("dpn_suite::dpn".parse().expect("contract alias")),
                contract_address: None,
                entrypoints: ["transfer_dpn".to_owned()].into_iter().collect(),
            });
        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![allow_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);
        let (executable, metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            "transfer_dpn",
            Some(&fixture.sponsor_id),
        );

        expect_sponsored_admission_rejection(
            &fixture,
            executable,
            metadata,
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_native_batch_with_contract_metadata() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let mut allow_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        allow_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::ContractCall);
        allow_rule
            .contract_selectors
            .push(FeeSponsorContractSelector {
                contract_alias: Some("dpn_suite::dpn".parse().expect("contract alias")),
                contract_address: None,
                entrypoints: ["transfer_dpn".to_owned()].into_iter().collect(),
            });
        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![allow_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);
        let (_, metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            "transfer_dpn",
            Some(&fixture.sponsor_id),
        );
        let executable = Executable::Instructions(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "sponsored native batch".to_owned(),
            ))]
            .into(),
        );

        expect_sponsored_admission_rejection(
            &fixture,
            executable,
            metadata,
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_contract_call_with_wrong_selector() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let wrong_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &fixture.authority_id,
            99,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive wrong contract address");
        let mut allow_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        allow_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::ContractCall);
        allow_rule
            .contract_selectors
            .push(FeeSponsorContractSelector {
                contract_alias: None,
                contract_address: Some(wrong_address),
                entrypoints: ["transfer_dpn".to_owned()].into_iter().collect(),
            });
        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![allow_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);
        let (executable, metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            "transfer_dpn",
            Some(&fixture.sponsor_id),
        );

        expect_sponsored_admission_rejection(
            &fixture,
            executable,
            metadata,
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_deny_rule_overrides_wildcard_allow() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let blocked_instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            fixture.authority_id.clone(),
            "blocked".parse().expect("metadata key"),
            Json::new("value"),
        )
        .into();
        let blocked_wire_id = iroha_data_model::isi::instruction_wire_id(&blocked_instruction)
            .expect("SetKeyValue should have a stable wire id")
            .to_owned();

        let mut deny_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Deny);
        deny_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::Instructions);
        deny_rule.instruction_wire_ids.insert(blocked_wire_id);

        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![FeeSponsorRule::new(FeeSponsorRuleEffect::Allow), deny_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);

        expect_sponsored_admission_rejection(
            &fixture,
            Executable::Instructions(vec![blocked_instruction].into()),
            sponsored_fee_metadata(&fixture),
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_disabled_policy() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let mut policy = default_fee_sponsor_policy(&fixture.sponsor_id);
        policy.enabled = false;
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);

        expect_sponsored_admission_rejection(
            &fixture,
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "disabled policy".to_owned(),
                ))]
                .into(),
            ),
            sponsored_fee_metadata(&fixture),
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_missing_granted_policy() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        fixture.state.world.fee_sponsor_policies = Default::default();

        expect_sponsored_admission_rejection(
            &fixture,
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "missing policy".to_owned(),
                ))]
                .into(),
            ),
            sponsored_fee_metadata(&fixture),
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_accepts_configured_default_when_storage_missing() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        fixture.state.world.fee_sponsor_policies = Default::default();
        {
            let nexus = fixture.state.nexus.get_mut();
            nexus
                .dataspace_fee_sponsors
                .insert(DataSpaceId::UNIVERSAL, fixture.sponsor_id.to_string());
            nexus.dataspace_fee_sponsor_policies.insert(
                DataSpaceId::UNIVERSAL,
                "default".parse().expect("default fee sponsor policy"),
            );
        }

        let tx = sign_sponsored_fixture_transaction(
            &fixture,
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "configured missing policy".to_owned(),
                ))]
                .into(),
            ),
            sponsored_fee_metadata(&fixture),
        );
        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("configured default sponsor policy should authorize admission");
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_wrong_granted_policy_name() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        fixture.state.world.account_permissions.insert(
            fixture.authority_id.clone(),
            BTreeSet::from([Permission::from(CanUseFeeSponsor {
                sponsor: fixture.sponsor_id.clone(),
                policy: "transfers_only"
                    .parse()
                    .expect("alternate fee sponsor policy"),
            })]),
        );

        expect_sponsored_admission_rejection(
            &fixture,
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "wrong granted policy".to_owned(),
                ))]
                .into(),
            ),
            sponsored_fee_metadata(&fixture),
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_local_max_fee_rejects_even_when_global_cap_allows() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        fixture.state.nexus.get_mut().fees.sponsor_max_fee = Quantity::zero();
        let mut policy = default_fee_sponsor_policy(&fixture.sponsor_id);
        policy.max_fee = Some("0.1".parse().expect("valid sponsor fee cap"));
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);

        expect_sponsored_admission_rejection(
            &fixture,
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "too expensive for policy".to_owned(),
                ))]
                .into(),
            ),
            sponsored_fee_metadata(&fixture),
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_wrong_dataspace() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let allowed_dataspace = DataSpaceId::new(7);
        let routed_dataspace = DataSpaceId::new(8);
        let mut allow_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        allow_rule.dataspaces.insert(allowed_dataspace);
        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![allow_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);
        let executable = Executable::Instructions(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "wrong dataspace".to_owned(),
            ))]
            .into(),
        );
        let tx = sign_sponsored_fixture_transaction(
            &fixture,
            executable,
            sponsored_fee_metadata(&fixture),
        );
        let view = fixture.state.view();
        let err = check_external_nexus_fee_admission(
            &view.world,
            &view.nexus,
            &tx,
            0,
            1,
            Some(routed_dataspace),
        )
        .expect_err("wrong dataspace should reject sponsorship");
        assert!(matches!(
            err,
            NexusFeeAdmissionError::Rejected(message)
                if message.contains("fee sponsor policy is not authorized")
        ));
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_native_batch_with_unallowed_operation() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let allowed_instruction: InstructionBox =
            Log::new(Level::INFO, "allowed operation".to_owned()).into();
        let allowed_wire_id = iroha_data_model::isi::instruction_wire_id(&allowed_instruction)
            .expect("Log should have a stable wire id")
            .to_owned();
        let mut allow_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        allow_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::Instructions);
        allow_rule.instruction_wire_ids.insert(allowed_wire_id);
        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![allow_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);

        let blocked_instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            fixture.authority_id.clone(),
            "blocked".parse().expect("metadata key"),
            Json::new("value"),
        )
        .into();
        expect_sponsored_admission_rejection(
            &fixture,
            Executable::Instructions(vec![allowed_instruction, blocked_instruction].into()),
            sponsored_fee_metadata(&fixture),
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_policy_rejects_denied_ivm_proved_overlay_operation() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        let blocked_instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            fixture.authority_id.clone(),
            "blocked_overlay".parse().expect("metadata key"),
            Json::new("value"),
        )
        .into();
        let blocked_wire_id = iroha_data_model::isi::instruction_wire_id(&blocked_instruction)
            .expect("SetKeyValue should have a stable wire id")
            .to_owned();

        let mut allow_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        allow_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::IvmProved);
        let mut deny_rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Deny);
        deny_rule
            .executable_kinds
            .insert(FeeSponsorExecutableKind::IvmProved);
        deny_rule.instruction_wire_ids.insert(blocked_wire_id);
        let policy = FeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                fixture.sponsor_id.clone(),
                "default".parse().expect("default fee sponsor policy"),
            ),
            enabled: true,
            max_fee: None,
            rules: vec![allow_rule, deny_rule],
        };
        fixture
            .state
            .world
            .fee_sponsor_policies
            .insert(policy.id.clone(), policy);

        let mut metadata = sponsored_fee_metadata(&fixture);
        insert_gas_limit(&mut metadata, 1);
        let executable =
            Executable::IvmProved(iroha_data_model::transaction::executable::IvmProved {
                bytecode: IvmBytecode::from_compiled(vec![0x08, 0x08, 0x08]),
                overlay: vec![blocked_instruction].into(),
                events_commitment: Hash::new(b"events"),
                gas_policy_commitment: Hash::new(b"gas-policy"),
            });
        expect_sponsored_admission_rejection(
            &fixture,
            executable,
            metadata,
            "fee sponsor policy is not authorized",
        );
    }

    #[test]
    fn nexus_fee_sponsor_accepts_ivm_with_gas_limit() {
        let fixture = sponsored_fee_admission_fixture(true);
        let mut metadata = sponsored_fee_metadata(&fixture);
        insert_gas_limit(&mut metadata, 1);
        let executable = Executable::Ivm(IvmBytecode::from_compiled(vec![0x00, 0x01, 0x02]));

        let tx = sign_sponsored_fixture_transaction(&fixture, executable, metadata);
        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("IVM executable is sponsored when gas-limited");
    }

    #[test]
    fn nexus_fee_sponsor_accepts_ivm_proved_with_overlay() {
        let fixture = sponsored_fee_admission_fixture(true);
        let mut metadata = sponsored_fee_metadata(&fixture);
        insert_gas_limit(&mut metadata, 1);
        let executable =
            Executable::IvmProved(iroha_data_model::transaction::executable::IvmProved {
                bytecode: IvmBytecode::from_compiled(vec![0x07, 0x07, 0x07]),
                overlay: vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "sponsored proved IVM overlay".to_owned(),
                ))]
                .into(),
                events_commitment: Hash::new(b"events"),
                gas_policy_commitment: Hash::new(b"gas-policy"),
            });

        let tx = sign_sponsored_fixture_transaction(&fixture, executable, metadata);
        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("proved IVM executable is sponsored when fee-metered");
    }

    #[test]
    fn ivm_proved_admission_reserves_vm_gas_limit_even_with_empty_overlay() {
        let fixture = sponsored_fee_admission_fixture(true);
        let mut metadata = sponsored_fee_metadata(&fixture);
        let gas_limit = 987_654;
        insert_gas_limit(&mut metadata, gas_limit);
        let executable =
            Executable::IvmProved(iroha_data_model::transaction::executable::IvmProved {
                bytecode: IvmBytecode::from_compiled(vec![0x07, 0x07, 0x07]),
                overlay: Vec::<InstructionBox>::new().into(),
                events_commitment: Hash::new(b"events"),
                gas_policy_commitment: Hash::new(b"gas-policy"),
            });
        let tx = sign_sponsored_fixture_transaction(&fixture, executable, metadata);

        let (_, instruction_count, reserved_gas) =
            fee_bound_for_admission(&tx).expect("proved IVM fee bound");
        assert_eq!(instruction_count, 0);
        assert_eq!(reserved_gas, gas_limit);
    }

    #[test]
    fn nexus_fee_sponsor_rejects_ivm_without_gas_limit() {
        let fixture = sponsored_fee_admission_fixture(true);
        let metadata = sponsored_fee_metadata(&fixture);
        let executable = Executable::Ivm(IvmBytecode::from_compiled(vec![0x00, 0x01, 0x02]));

        expect_sponsored_admission_rejection(&fixture, executable, metadata, "missing gas_limit");
    }

    #[test]
    fn nexus_fee_sponsor_max_fee_applies_after_fee_metering() {
        let mut fixture = sponsored_fee_admission_fixture(true);
        fixture.state.nexus.get_mut().fees.sponsor_max_fee =
            "0.1".parse().expect("valid sponsor fee cap");
        let (executable, metadata) = dpn_contract_call_executable_and_metadata(
            &fixture.authority_id,
            "transfer_dpn",
            Some(&fixture.sponsor_id),
        );

        expect_sponsored_admission_rejection(
            &fixture,
            executable,
            metadata,
            "fee exceeds sponsor_max_fee",
        );
    }

    #[test]
    fn nexus_fee_sponsor_accepts_immediate_multisig_native_batch() {
        let fixture = sponsored_fee_admission_fixture(true);
        let proposal_instructions =
            multisig_contract_trigger_instructions(&fixture, "delete_everything", false);
        let instructions_hash = HashOf::new(&proposal_instructions);
        let tx = multisig_instruction_batch_tx(
            &fixture,
            vec![
                InstructionBox::from(MultisigPropose::new(
                    fixture.authority_id.clone(),
                    proposal_instructions,
                    None,
                )),
                InstructionBox::from(MultisigApprove::new(
                    fixture.authority_id.clone(),
                    instructions_hash,
                )),
            ],
        );

        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("multisig native batches are sponsored without contract-wrapper validation");
    }

    #[test]
    fn nexus_fee_sponsor_accepts_multisig_approval_without_proposal_lookup() {
        let fixture = sponsored_fee_admission_fixture(true);
        let proposal_instructions =
            multisig_contract_trigger_instructions(&fixture, "transfer_dpn", false);
        let instructions_hash = HashOf::new(&proposal_instructions);
        let tx = multisig_instruction_tx(
            &fixture,
            MultisigApprove::new(fixture.authority_id.clone(), instructions_hash),
        );

        let view = fixture.state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("multisig approvals are sponsored without proposal allowlist lookup");
    }

    #[test]
    fn nexus_fee_sponsor_rejected_when_disabled() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
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
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.sponsorship_enabled = false;
        nexus.fees.fee_asset_id = "4cuvDVPuLBKJyN6dPbRQhmLh68sU".to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
        nexus.fees.burn_from_unix_timestamp_ms = 0;

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
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let world = World::with([domain], [authority_account, sponsor_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        let nexus = state.nexus.get_mut();
        nexus.enabled = true;
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.sponsorship_enabled = true;
        nexus.fees.fee_asset_id = "4cuvDVPuLBKJyN6dPbRQhmLh68sU".to_string();
        nexus.fees.fee_sink_account_id = sink_id.to_string();
        nexus.fees.burn_from_unix_timestamp_ms = 0;

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            Json::new(sponsor_id.to_string()),
        );
        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata)
            .with_executable(Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "sponsored native batch".to_owned(),
                ))]
                .into(),
            ))
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
    fn nexus_fee_sponsor_accepts_native_batch_with_permission() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&authority_id);
        let sponsor_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sponsor_id.clone()),
            Quantity::from(10_000_u32),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sink_id.clone()),
            Quantity::zero(),
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
        seed_default_fee_sponsor_policy(&mut state.world, &sponsor_id);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let permission = CanUseFeeSponsor {
            sponsor: sponsor_id.clone(),
            policy: "default".parse().expect("default fee sponsor policy"),
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
            .with_executable(Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "sponsored native batch".to_owned(),
                ))]
                .into(),
            ))
            .sign(authority_kp.private_key());

        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let res = executor.execute_transaction(&mut stx, &authority_id, tx, &mut ivm_cache);
        res.expect("sponsored native batch should execute");

        let sponsor_balance_after = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sponsor_id.clone()))
            .expect("sponsor asset exists")
            .0
            .as_numeric()
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(sponsor_balance_after, 9_999);
        let sink_balance_after = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sink_id.clone()))
            .expect("sink asset exists")
            .0
            .as_numeric()
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(sink_balance_after, 0);

        stx.apply();

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 1);
    }

    #[test]

    fn nexus_fee_dataspace_default_sponsor_accepts_native_batch() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&authority_id);
        let sponsor_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sponsor_id.clone()),
            Quantity::from(10_000_u32),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sink_id.clone()),
            Quantity::zero(),
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
        seed_default_fee_sponsor_policy(&mut state.world, &sponsor_id);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus
                .dataspace_fee_sponsors
                .insert(DataSpaceId::UNIVERSAL, sponsor_id.to_string());
            nexus.dataspace_fee_sponsor_policies.insert(
                DataSpaceId::UNIVERSAL,
                "default".parse().expect("default fee sponsor policy"),
            );
        }

        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_executable(Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "dataspace sponsored native batch".to_owned(),
                ))]
                .into(),
            ))
            .sign(authority_kp.private_key());

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let mut stx = block.transaction();
        stx.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        stx.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);

        let res = executor.execute_transaction(&mut stx, &authority_id, tx, &mut ivm_cache);
        res.expect("dataspace-default sponsored native batch should execute");

        let sponsor_balance_after = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sponsor_id.clone()))
            .expect("sponsor asset exists")
            .0
            .as_numeric()
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(sponsor_balance_after, 9_999);
        let sink_balance_after = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sink_id.clone()))
            .expect("sink asset exists")
            .0
            .as_numeric()
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(sink_balance_after, 0);

        stx.apply();

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 1);
    }

    #[test]
    fn nexus_fee_external_settled_sponsor_does_not_require_local_fee_asset() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let asset_definition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&authority_id);
        let world = World::with_assets(
            [domain],
            [authority_account, sponsor_account],
            [asset_definition],
            [],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        seed_default_fee_sponsor_policy(&mut state.world, &sponsor_id);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.external_settlement_enabled = true;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sponsor_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        Grant::account_permission(
            CanUseFeeSponsor {
                sponsor: sponsor_id.clone(),
                policy: "default".parse().expect("default fee sponsor policy"),
            },
            authority_id.clone(),
        )
        .execute(&sponsor_id, &mut stx)
        .expect("grant fee sponsor permission");

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            Json::new(sponsor_id.to_string()),
        );
        let executable = Executable::Instructions(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "external-settled sponsored native batch".to_owned(),
            ))]
            .into(),
        );
        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_metadata(metadata)
            .with_executable(executable)
            .sign(authority_kp.private_key());

        check_external_nexus_fee_admission(&stx.world, &stx.nexus, &tx, 0, 1, None)
            .expect("external-settled native sponsor should not require local fee asset");

        assert!(
            stx.world
                .assets()
                .get(&AssetId::of(asset_def_id, sponsor_id))
                .is_none(),
            "external settlement must not create or debit a local sponsor asset"
        );
    }

    #[test]
    fn nexus_fee_sponsor_sink_accepts_native_batch() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor_account = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&authority_id);
        let sponsor_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sponsor_id.clone()),
            Quantity::from(10_000_u32),
        );
        let world = World::with_assets(
            [domain],
            [authority_account, sponsor_account],
            [ad],
            [sponsor_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);
        seed_default_fee_sponsor_policy(&mut state.world, &sponsor_id);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sponsor_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let permission = CanUseFeeSponsor {
            sponsor: sponsor_id.clone(),
            policy: "default".parse().expect("default fee sponsor policy"),
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
            .with_executable(Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "sink-sponsored native batch".to_owned(),
                ))]
                .into(),
            ))
            .sign(authority_kp.private_key());

        let executor = super::Executor::Initial;
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        let res = executor.execute_transaction(&mut stx, &authority_id, tx, &mut ivm_cache);
        res.expect("sponsored native batch should execute when sponsor is the fee sink");

        let sponsor_asset_id = AssetId::of(asset_def_id, sponsor_id);
        let sponsor_balance_after = stx
            .world
            .assets()
            .get(&sponsor_asset_id)
            .expect("sponsor asset exists")
            .0
            .as_numeric()
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(sponsor_balance_after, 9_999);

        stx.apply();

        let snap = crate::sumeragi::status::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 1);
    }

    #[test]
    fn nexus_fee_lane_relay_burn_records_receipt_without_local_xor_mutation() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let fee_asset_id = "xor#universal";
        let world = World::with([domain], [payer, sink], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = fee_asset_id.to_owned();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 2;
            nexus.fees.canonical_sponsor_account_id = Some(payer_id.to_string());
        }
        seed_verified_nexus_fee_budget(&state, &payer_id, fee_asset_id, Numeric::from(10_u32));

        let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            payer_id.clone(),
            "k".parse().unwrap(),
            Json::new("v"),
        )
        .into();
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_executable(Executable::from(core::iter::once(instruction)))
            .sign(payer_kp.private_key());
        let tx_hash = tx.hash();

        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &payer_id, tx, &mut ivm_cache)
            .expect("execution records asynchronous fee receipt");

        assert!(
            stx.world.assets().is_empty(),
            "lane-relay-burn mode must not require or mutate local XOR assets"
        );

        let pending = stx.drain_nexus_fee_records();
        let receipt = pending.get(&tx_hash).expect("receipt recorded for tx");
        assert_eq!(receipt.payer_account_id, payer_id);
        assert_eq!(receipt.fee_asset_id, fee_asset_id);
        assert_eq!(receipt.fee_amount, Quantity::from(1_u32));
        assert_eq!(receipt.schedule.base_fee, Quantity::from(1_u32));
    }

    #[test]
    fn nexus_fee_lane_relay_burn_admission_requires_canonical_sponsor() {
        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let world = World::with([domain], [payer, sponsor], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = "xor#universal".to_owned();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 2;
            nexus.fees.canonical_sponsor_account_id = Some(sponsor_id.to_string());
        }

        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(payer_kp.private_key());
        let view = state.view();
        let err = check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 2, None)
            .expect_err("non-canonical payer must be rejected");
        assert!(matches!(
            err,
            NexusFeeAdmissionError::Rejected(message)
                if message.contains("canonical sponsor")
        ));
    }

    #[test]
    fn nexus_fee_protocol_registration_transactions_are_fee_exempt() {
        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id).build(&authority_id);
        let authority = Account::new(authority_id.clone()).build(&authority_id);
        let world = World::with([domain], [authority], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = "xor#universal".to_owned();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 2;
            nexus.fees.canonical_sponsor_account_id = Some(authority_id.to_string());
        }

        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedNexusFeeBudget {
            sponsor_account_id: authority_id.clone(),
            fee_asset_id: "xor#universal".to_owned(),
            verified_balance: Numeric::from(1_u32),
            manifest_root: [0x42; 32],
            proof_blob: iroha_data_model::nexus::ProofBlob {
                payload: Vec::new(),
                expiry_slot: None,
            },
        };
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id)
            .with_executable(Executable::from(core::iter::once(InstructionBox::from(
                instruction,
            ))))
            .sign(authority_kp.private_key());
        let view = state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 2, None)
            .expect("protocol proof registration must not require a fee receipt");
    }

    #[test]
    fn nexus_fee_online_to_offline_shield_requires_fee_budget() {
        let (state, authority_id, authority_kp, asset_def_id) =
            nexus_fee_lane_relay_burn_admission_fixture();
        let shield = iroha_data_model::isi::zk::Shield::new(
            asset_def_id,
            authority_id.clone(),
            1,
            [0x44; 32],
            iroha_data_model::confidential::ConfidentialEncryptedPayload::default(),
        );
        let tx = signed_fee_policy_transaction(authority_id, &authority_kp, shield.into());
        assert_lane_relay_burn_requires_fee_budget(&state, &tx);
    }

    #[test]
    fn unavailable_kagemusha_v2_redeem_cannot_self_fund_nexus_fee() {
        assert!(
            !iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE,
            "this regression test must be replaced by end-to-end V2 execution coverage when the backend ships"
        );
        let (mut state, authority_id, authority_kp, asset_def_id) =
            nexus_fee_lane_relay_burn_admission_fixture();
        state.nexus.get_mut().fees.fee_asset_id = asset_def_id.to_string();
        let redeem = kagemusha_fee_test_recursive_redeem_v2(
            asset_def_id,
            authority_id.clone(),
            &authority_kp,
        );
        let tx = signed_fee_policy_transaction(authority_id, &authority_kp, redeem.into());

        let view = state.view();
        let capacity =
            redeem_funded_nexus_fee_capacity(&view.world, &view.nexus.fees, &tx, 0, false)
                .expect("unsupported V2 classification must not fail");
        assert_eq!(
            capacity, None,
            "an unavailable V2 proof backend cannot produce fee-paying credit"
        );
        drop(view);
        assert_lane_relay_burn_requires_fee_budget(&state, &tx);
    }

    #[test]
    fn nexus_fee_lane_relay_burn_redeem_funded_balance_records_receipt_without_budget() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let asset_def_id = AssetDefinitionId::new(domain_id, "xor".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&payer_id);
        let payer_asset = Asset::new(AssetId::of(asset_def_id.clone(), payer_id.clone()), 1_u32);
        let world = World::with_assets([domain], [payer], [asset_definition], [payer_asset], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 1;
        }

        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(payer_kp.private_key());
        let tx_hash = tx.hash();
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();

        super::Executor::charge_nexus_fees(&mut stx, &payer_id, &tx, tx_hash, None, 0, 0, 0, true)
            .expect("redeem-funded public balance should record a lane fee receipt");

        let pending = stx.drain_nexus_fee_records();
        let receipt = pending.get(&tx_hash).expect("fee receipt recorded");
        assert_eq!(receipt.payer_account_id, payer_id);
        assert_eq!(receipt.fee_asset_id, asset_def_id.to_string());
        assert_eq!(receipt.fee_amount, Quantity::from(1_u32));
    }

    #[test]
    fn heartbeat_execution_skips_required_gas_asset_metadata() {
        use std::time::Duration;

        let chain: ChainId = "heartbeat-gas-admission".parse().unwrap();
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new_with_chain(world, kura, query_handle, chain.clone());
        state.pipeline.gas.accepted_assets = vec!["xor#universal".to_owned()];

        let tx_params = state.view().world().parameters().transaction();
        let signer = checked_keypair_with_algorithm(Algorithm::Ed25519);
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let tx = crate::tx::build_heartbeat_transaction_with_time_source(
            chain,
            &signer,
            &tx_params,
            1,
            &time_source,
        );
        let authority = tx.authority().clone();

        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        executor
            .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
            .expect("heartbeat should not require gas_asset_id metadata");
        assert_eq!(stx.last_tx_gas_used, 0);
    }

    #[test]
    fn nexus_fee_lane_relay_burn_before_receipt_activation_uses_direct_fee_path() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id = AssetDefinitionId::new(domain_id, "xor".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&payer_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), payer_id.clone()),
            Quantity::from(10_u32),
        );
        let world = World::with_assets(
            [domain],
            [payer, sink],
            [asset_definition],
            [payer_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 10;
            nexus.fees.canonical_sponsor_account_id = Some(payer_id.to_string());
        }

        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain.clone(), payer_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(payer_kp.private_key());
        let view = state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 2, None)
            .expect("pre-activation lane-relay-burn mode should use direct fee admission");

        let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            payer_id.clone(),
            "k".parse().unwrap(),
            Json::new("v"),
        )
        .into();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_executable(Executable::from(core::iter::once(instruction)))
            .sign(payer_kp.private_key());
        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &payer_id, tx, &mut ivm_cache)
            .expect("pre-activation direct fee path should execute");

        assert!(
            stx.drain_nexus_fee_records().is_empty(),
            "pre-activation blocks must not stage Nexus fee receipts"
        );
        let payer_balance = stx
            .world
            .asset(&AssetId::of(asset_def_id, payer_id))
            .expect("payer asset")
            .value()
            .as_ref()
            .clone();
        assert_eq!(payer_balance, Quantity::from(9_u32));
    }

    #[test]
    fn nexus_fee_lane_relay_burn_admission_requires_verified_budget() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id = AssetDefinitionId::new(domain_id, "xor".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&payer_id);
        let world = World::with([domain], [payer, sink], [asset_definition]);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 1;
            nexus.fees.canonical_sponsor_account_id = Some(payer_id.to_string());
        }

        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(payer_kp.private_key());
        let view = state.view();
        let err = check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect_err("lane-relay-burn admission must fail closed without a verified budget");
        assert!(
            matches!(err, NexusFeeAdmissionError::Rejected(reason) if reason.contains("missing verified Nexus fee budget"))
        );
    }

    #[test]
    fn nexus_fee_lane_relay_burn_admission_subtracts_unsettled_receipts() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id = AssetDefinitionId::new(domain_id, "xor".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&payer_id);
        let world = World::with([domain], [payer, sink], [asset_definition]);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.settlement_mode =
                iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
            nexus.fees.fee_receipts_activation_height = 1;
            nexus.fees.canonical_sponsor_account_id = Some(payer_id.to_string());
        }
        seed_verified_nexus_fee_budget(
            &state,
            &payer_id,
            asset_def_id.to_string().as_str(),
            Numeric::from(10_u32),
        );
        seed_verified_lane_relay_nexus_fee_receipt(
            &state,
            &payer_id,
            asset_def_id.to_string().as_str(),
            Quantity::from(10_u32),
            [0xA5; 32],
        );

        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(payer_kp.private_key());
        let view = state.view();
        let err = check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect_err("unsettled receipt must reduce available verified budget");
        assert!(
            matches!(err, NexusFeeAdmissionError::Rejected(reason) if reason.contains("insufficient"))
        );
    }

    #[test]
    fn nexus_fee_admission_accepts_non_universal_route_with_global_balance() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&authority_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), authority_id.clone()),
            Quantity::from(10_u32),
        );
        let world = World::with_assets(
            [domain],
            [authority_account, sink_account],
            [ad],
            [payer_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(authority_kp.private_key());
        let view = state.view();
        check_external_nexus_fee_admission(
            view.world(),
            view.nexus(),
            &tx,
            0,
            1,
            Some(DataSpaceId::new(10)),
        )
        .expect("private routes should admit fees against the authoritative global bucket");
    }

    #[test]
    fn nexus_fee_non_universal_route_burns_global_fee_asset_bucket() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain: Domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&authority_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), authority_id.clone()),
            Quantity::from(10_u32),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sink_id.clone()),
            Quantity::zero(),
        );
        let world = World::with_assets(
            [domain],
            [authority_account, sink_account],
            [ad],
            [payer_asset, sink_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = Quantity::zero();
            nexus.fees.per_gas_unit_fee = Quantity::zero();
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let chain: iroha_data_model::ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(authority_kp.private_key());
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        stx.current_dataspace_id = Some(DataSpaceId::new(10));
        stx.world.current_dataspace_id = Some(DataSpaceId::new(10));
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        super::Executor::Initial
            .execute_transaction(&mut stx, &authority_id, tx, &mut ivm_cache)
            .expect("non-universal route should debit global fee bucket");

        let payer_asset_id = AssetId::of(asset_def_id.clone(), authority_id);
        let sink_asset_id = AssetId::of(asset_def_id, sink_id);
        let payer_balance = stx
            .world
            .asset(&payer_asset_id)
            .expect("payer asset")
            .value()
            .as_ref()
            .clone();
        let sink_balance = stx
            .world
            .asset(&sink_asset_id)
            .expect("sink asset")
            .value()
            .as_ref()
            .clone();
        assert_eq!(payer_balance, Quantity::from(9_u32));
        assert_eq!(sink_balance, Quantity::zero());
    }

    #[test]
    fn nexus_successful_claim_mint_bypasses_fee_admission_for_configured_authority() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (recipient_id, _recipient_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id = AssetDefinitionId::new(domain_id, "xor".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&authority_id);
        let world = World::with(
            [domain],
            [authority_account, sink_account],
            [asset_definition],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.per_instruction_fee = Quantity::from(1_u32);
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
            nexus
                .fees
                .successful_claim_fee_exempt_authorities
                .push(authority_id.to_string());
        }

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str(SORA_V2_CLAIM_TX_HASH_METADATA_KEY).expect("static name"),
            Json::new(format!("0x{}", "11".repeat(32))),
        );
        metadata.insert(
            Name::from_str(SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY).expect("static name"),
            Json::new(recipient_id.to_string()),
        );
        let instruction: InstructionBox =
            Mint::asset_quantity(3_u32, AssetId::of(asset_def_id, recipient_id)).into();
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id)
            .with_metadata(metadata)
            .with_executable(Executable::from(core::iter::once(instruction)))
            .sign(authority_kp.private_key());

        let view = state.view();
        check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect("configured successful claim mint should bypass fee admission");
    }

    #[test]
    fn nexus_successful_claim_mint_requires_configured_authority() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (authority_id, authority_kp) = gen_account_in("wonderland");
        let (recipient_id, _recipient_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority_account = Account::new(authority_id.clone()).build(&authority_id);
        let sink_account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id = AssetDefinitionId::new(domain_id, "xor".parse().unwrap());
        let asset_definition = AssetDefinition::numeric(asset_def_id.clone())
            .with_name(asset_def_id.name().to_string())
            .build(&authority_id);
        let world = World::with(
            [domain],
            [authority_account, sink_account],
            [asset_definition],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.per_instruction_fee = Quantity::from(1_u32);
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str(SORA_V2_CLAIM_TX_HASH_METADATA_KEY).expect("static name"),
            Json::new(format!("0x{}", "22".repeat(32))),
        );
        metadata.insert(
            Name::from_str(SORA_NEXUS_CLAIM_RECIPIENT_METADATA_KEY).expect("static name"),
            Json::new(recipient_id.to_string()),
        );
        let instruction: InstructionBox =
            Mint::asset_quantity(3_u32, AssetId::of(asset_def_id, recipient_id)).into();
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, authority_id)
            .with_metadata(metadata)
            .with_executable(Executable::from(core::iter::once(instruction)))
            .sign(authority_kp.private_key());

        let view = state.view();
        let err = check_external_nexus_fee_admission(&view.world, &view.nexus, &tx, 0, 1, None)
            .expect_err("unconfigured claim authority should still need fee balance");
        assert!(matches!(err, NexusFeeAdmissionError::Rejected(_)));
    }

    #[test]
    fn nexus_fee_charged_event_is_recorded_on_apply() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (alice_id, alice_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let dom: Domain = Domain::new(domain_id.clone()).build(&alice_id);
        let alice: Account = Account::new(alice_id.clone()).build(&alice_id);
        let sink: Account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&alice_id);
        let payer_asset = AssetId::of(asset_def_id.clone(), alice_id.clone());
        let payer_balance = Asset::new(payer_asset, Quantity::from(10_000_u32));
        let world = World::with_assets([dom], [alice, sink], [ad], [payer_balance], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::from(1_u32);
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
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

    #[test]
    fn nexus_fee_transfer_cost_is_exactly_one_cent_xor() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (recipient_id, _recipient_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let dom: Domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer: Account = Account::new(payer_id.clone()).build(&payer_id);
        let recipient: Account = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink: Account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition =
            AssetDefinition::new(asset_def_id.clone(), NumericSpec::default())
                .with_name("xor".to_owned())
                .build(&payer_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), payer_id.clone()),
            Quantity::from_str("10").unwrap(),
        );
        let recipient_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), recipient_id.clone()),
            Quantity::zero(),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sink_id.clone()),
            Quantity::zero(),
        );
        let world = World::with_assets(
            [dom],
            [payer, recipient, sink],
            [ad],
            [payer_asset, recipient_asset, sink_asset],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::zero();
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = "0.001".parse().expect("valid instruction fee");
            nexus.fees.per_gas_unit_fee = "0.00005".parse().expect("valid gas fee");
            nexus.fees.sponsorship_enabled = false;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let instruction: InstructionBox = Transfer::asset_quantity(
            AssetId::of(asset_def_id.clone(), payer_id.clone()),
            1_u32,
            recipient_id.clone(),
        )
        .into();
        assert_eq!(
            crate::gas::meter_instructions(std::slice::from_ref(&instruction)),
            180
        );
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_executable(Executable::from(core::iter::once(instruction)))
            .sign(payer_kp.private_key());

        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &payer_id, tx, &mut ivm_cache)
            .expect("execution");

        let sink_balance = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), sink_id.clone()))
            .expect("sink asset exists")
            .0
            .to_string();
        let payer_balance = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), payer_id.clone()))
            .expect("payer asset exists")
            .0
            .to_string();
        let recipient_balance = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id, recipient_id))
            .expect("recipient asset exists")
            .0
            .to_string();

        assert_eq!(sink_balance, "0");
        assert_eq!(payer_balance, "8.99");
        assert_eq!(recipient_balance, "1");
    }

    #[test]
    fn nexus_fee_set_account_kv_cost_is_scaled_from_transfer_anchor() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (sink_id, _sink_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let dom: Domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer: Account = Account::new(payer_id.clone()).build(&payer_id);
        let sink: Account = Account::new(sink_id.clone()).build(&sink_id);
        let asset_def_id: AssetDefinitionId = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let ad: AssetDefinition =
            AssetDefinition::new(asset_def_id.clone(), NumericSpec::default())
                .with_name("xor".to_owned())
                .build(&payer_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), payer_id.clone()),
            Quantity::from_str("10").unwrap(),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_def_id.clone(), sink_id.clone()),
            Quantity::zero(),
        );
        let world = World::with_assets([dom], [payer, sink], [ad], [payer_asset, sink_asset], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query_handle);

        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Quantity::zero();
            nexus.fees.per_byte_fee = Quantity::zero();
            nexus.fees.per_instruction_fee = "0.001".parse().expect("valid instruction fee");
            nexus.fees.per_gas_unit_fee = "0.00005".parse().expect("valid gas fee");
            nexus.fees.sponsorship_enabled = false;
            nexus.fees.fee_asset_id = asset_def_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let instruction: InstructionBox = iroha_data_model::isi::SetKeyValue::account(
            payer_id.clone(),
            "k".parse().unwrap(),
            Json::new("v"),
        )
        .into();
        assert_eq!(
            crate::gas::meter_instructions(std::slice::from_ref(&instruction)),
            67
        );
        let chain: ChainId = "test-chain".parse().unwrap();
        let tx = TransactionBuilder::new(chain, payer_id.clone())
            .with_executable(Executable::from(core::iter::once(instruction)))
            .sign(payer_kp.private_key());

        let executor = super::Executor::default();
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut stx = block.transaction();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        executor
            .execute_transaction(&mut stx, &payer_id, tx, &mut ivm_cache)
            .expect("execution");

        let payer_balance = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id.clone(), payer_id))
            .expect("payer asset exists")
            .0
            .to_string();
        let sink_balance = stx
            .world
            .assets()
            .get(&AssetId::of(asset_def_id, sink_id))
            .expect("sink asset exists")
            .0
            .to_string();
        assert_eq!(payer_balance, "9.99565");
        assert_eq!(sink_balance, "0");
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn block_fee_units_recorded_on_apply() {
        let metrics = Arc::new(Metrics::default());
        let telemetry = StateTelemetry::new(metrics.clone(), true);
        let (payer_id, payer_kp) = gen_account_in("wonderland");
        let (tech_id, _tech_kp) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let dom: Domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer: Account = Account::new(payer_id.clone()).build(&payer_id);
        let tech: Account = Account::new(tech_id.clone()).build(&tech_id);
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "gas".parse().unwrap(),
        );
        let ad: AssetDefinition = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&payer_id);
        let payer_asset = AssetId::of(asset_def_id.clone(), payer_id.clone());
        let payer_balance = Asset::new(payer_asset, Quantity::from(10_000_u32));
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
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let chain: iroha_data_model::ChainId = "multisig-direct-sign".parse().unwrap();
        let signer = checked_keypair();
        let member = iroha_data_model::account::MultisigMember::new(signer.public_key().clone(), 1)
            .expect("valid member");
        let policy =
            iroha_data_model::account::MultisigPolicy::new(1, vec![member]).expect("policy");
        let multisig_id = AccountId::new_multisig(policy);

        let domain: Domain = Domain::new(domain_id.clone()).build(&multisig_id);
        let multisig_account = Account::new(multisig_id.clone()).build(&multisig_id);

        let world = World::with([domain], [multisig_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);

        let tx = TransactionBuilder::new(chain, multisig_id.clone())
            .with_executable(Executable::Instructions(Vec::new().into()))
            .sign(signer.private_key());

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

    fn contract_program_with_entrypoint(
        entrypoint: &str,
        permission: Option<&str>,
    ) -> (Vec<u8>, u64) {
        use iroha_data_model::smart_contract::manifest::EntryPointKind;

        contract_program_with_entrypoint_kind(entrypoint, EntryPointKind::Kotoage, permission)
    }

    fn contract_program_with_entrypoint_kind(
        entrypoint: &str,
        kind: iroha_data_model::smart_contract::manifest::EntryPointKind,
        permission: Option<&str>,
    ) -> (Vec<u8>, u64) {
        use ivm::{EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, ProgramMetadata};

        let descriptor = EmbeddedEntrypointDescriptor {
            name: entrypoint.to_owned(),
            kind,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: permission.map(str::to_owned),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        };
        let interface = EmbeddedContractInterfaceV1 {
            seiyaku_name: "TestContract".to_owned(),
            compiler_fingerprint: "executor-test".to_owned(),
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![descriptor],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let interface_section = interface.encode_section();
        let expected_entrypoint_pc =
            u64::try_from(interface_section.len()).expect("section length fits u64");
        let metadata = ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1_000_000,
            abi_version: 1,
        };
        let mut program = metadata.encode();
        program.extend_from_slice(&interface_section);
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        (program, expected_entrypoint_pc)
    }

    fn prepared_parameterized_trigger_contract() -> ivm::PreparedContract {
        let source = r#"
seiyaku TriggerArguments {
  kotoage fn run(quantity val) authorize("Admin") {
    let _val = val;
  }
}
"#;
        let code = ivm::KotodamaCompiler::new()
            .compile_source(source)
            .expect("compile parameterized trigger callback");
        ivm::prepare_contract(Arc::<[u8]>::from(code))
            .expect("prepare parameterized trigger callback")
    }

    #[test]
    fn protected_contract_call_is_denied_before_argument_record_decode() {
        const REQUIRED_PERMISSION: &str = "CanInvokeContractEntrypoint";
        let (program, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(
                r#"
seiyaku GuardedValue {
  kotoage fn write(int value) authorize("CanInvokeContractEntrypoint") {
    ledger::account::set_detail(
      account: context::authority(),
      key: Name::parse("guarded_value"),
      value: Json::parse("{\"authorized\":true}")
    );
  }
}
"#,
            )
            .expect("compile parameterized protected contract");
        let parsed = ivm::ProgramMetadata::parse(&program).expect("parse protected contract");
        let schema = parsed
            .contract_interface
            .as_ref()
            .and_then(|interface| {
                interface
                    .entrypoints
                    .iter()
                    .find(|entry| entry.name == "write")
            })
            .and_then(|entry| entry.argument_schema.as_ref())
            .expect("write argument schema");
        let arguments = ivm::encode_argument_record_from_json(
            schema,
            &Json::from(norito::json!({ "value": "7" })),
        )
        .expect("encode valid protected arguments");
        let arguments =
            iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(arguments)
                .expect("bounded protected arguments");

        let chain_id = ChainId::from("protected-direct-call");
        let authority = ALICE_ID.clone();
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").expect("valid domain id"))
                .build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let code_hash = ivm::contract_code_hash(&program);
        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let metadata_marker: Name = "guarded_value"
            .parse()
            .expect("valid direct-call metadata marker");
        world.contract_code.insert(code_hash, program);
        world
            .contract_manifests
            .insert(code_hash, manifest.signed(&ALICE_KEYPAIR));
        world
            .contract_instances
            .insert(contract_address.clone(), code_hash);
        let state = State::new_with_chain(
            world,
            Kura::blank_kura_for_testing(),
            query::store::LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            "gas_limit".parse().expect("gas_limit key"),
            Json::new(50_000_000_u64),
        );
        let transaction = TransactionBuilder::new(chain_id, authority.clone())
            .with_metadata(metadata)
            .with_executable(Executable::ContractCall(ContractInvocation {
                contract_address: contract_address.clone(),
                expected_code_hash: code_hash,
                entrypoint: "write".to_owned(),
                arguments: Some(arguments),
            }))
            .sign(ALICE_KEYPAIR.private_key());
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut state_tx = block.transaction();
        let mut ivm_cache = IvmCache::new();

        ivm::reset_argument_record_decode_count();
        let error = super::Executor::Initial
            .execute_transaction(
                &mut state_tx,
                &authority,
                transaction.clone(),
                &mut ivm_cache,
            )
            .expect_err("missing entrypoint permission must deny the direct call");

        assert!(
            error.to_string().contains(REQUIRED_PERMISSION),
            "unexpected direct contract authorization error: {error}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "denied direct-call arguments must remain undecoded"
        );
        assert!(
            state_tx
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&metadata_marker)
                .is_none(),
            "denied direct contract call must apply no queued effect"
        );

        let entrypoint_permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: contract_address.clone(),
                entrypoint: "write".to_owned(),
            }
            .into();
        Grant::account_permission(entrypoint_permission.clone(), authority.clone())
            .execute(&authority, &mut state_tx)
            .expect("grant direct-call entrypoint permission");
        ivm::reset_argument_record_decode_count();
        super::Executor::Initial
            .execute_transaction(
                &mut state_tx,
                &authority,
                transaction.clone(),
                &mut ivm_cache,
            )
            .expect("granted direct contract call must execute");
        assert_eq!(
            ivm::argument_record_decode_count(),
            1,
            "granted direct-call arguments must be prepared exactly once"
        );
        let authorized_marker = state_tx
            .world
            .account(&authority)
            .expect("authority account")
            .metadata()
            .get(&metadata_marker)
            .cloned()
            .expect("authorized direct call writes its metadata marker");

        let live_code = state_tx
            .world
            .contract_code
            .remove(code_hash)
            .expect("remove live bytecode for warm-cache adversarial check");
        ivm::reset_argument_record_decode_count();
        let missing_code = super::Executor::Initial
            .execute_transaction(
                &mut state_tx,
                &authority,
                transaction.clone(),
                &mut ivm_cache,
            )
            .expect_err("a warm cache must not substitute for missing live bytecode");
        assert!(missing_code.to_string().contains("not found in WSV"));
        assert_eq!(ivm::argument_record_decode_count(), 0);
        assert_eq!(
            state_tx
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&metadata_marker),
            Some(&authorized_marker),
            "missing live bytecode must apply no queued effect"
        );
        state_tx.world.contract_code.insert(code_hash, live_code);

        let live_manifest = state_tx
            .world
            .contract_manifests
            .remove(code_hash)
            .expect("remove live manifest for warm-cache adversarial check");
        ivm::reset_argument_record_decode_count();
        let missing_manifest = super::Executor::Initial
            .execute_transaction(
                &mut state_tx,
                &authority,
                transaction.clone(),
                &mut ivm_cache,
            )
            .expect_err("a warm cache must not substitute for a missing live manifest");
        assert!(missing_manifest.to_string().contains("has no manifest"));
        assert_eq!(ivm::argument_record_decode_count(), 0);
        assert_eq!(
            state_tx
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&metadata_marker),
            Some(&authorized_marker),
            "missing live manifest must apply no queued effect"
        );
        state_tx
            .world
            .contract_manifests
            .insert(code_hash, live_manifest);

        Revoke::account_permission(entrypoint_permission.clone(), authority.clone())
            .execute(&authority, &mut state_tx)
            .expect("revoke direct-call entrypoint permission");
        ivm::reset_argument_record_decode_count();
        let revoked = super::Executor::Initial
            .execute_transaction(
                &mut state_tx,
                &authority,
                transaction.clone(),
                &mut ivm_cache,
            )
            .expect_err("revoked direct-call permission must deny execution");
        assert!(revoked.to_string().contains(REQUIRED_PERMISSION));
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "revoked direct-call arguments must remain undecoded"
        );
        assert_eq!(
            state_tx
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&metadata_marker),
            Some(&authorized_marker),
            "revoked direct contract call must preserve authorized state"
        );

        Grant::account_permission(entrypoint_permission, authority.clone())
            .execute(&authority, &mut state_tx)
            .expect("restore direct-call entrypoint permission");
        state_tx
            .world
            .contract_instances
            .remove(contract_address.clone());
        ivm::reset_argument_record_decode_count();
        let deactivated = super::Executor::Initial
            .execute_transaction(&mut state_tx, &authority, transaction, &mut ivm_cache)
            .expect_err("deactivated direct-call target must deny execution");
        assert!(
            deactivated.to_string().contains("not found"),
            "unexpected deactivated direct-call error: {deactivated}"
        );
        assert_eq!(
            ivm::argument_record_decode_count(),
            0,
            "deactivated direct-call arguments must remain undecoded"
        );
        assert_eq!(
            state_tx
                .world
                .account(&authority)
                .expect("authority account")
                .metadata()
                .get(&metadata_marker),
            Some(&authorized_marker),
            "deactivated direct contract call must apply no queued effect"
        );
    }

    #[test]
    fn identityless_raw_and_proved_dispatch_reject_before_argument_decode_or_proof_work() {
        let program =
            ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
                force_zk: true,
                max_cycles: 10_000,
                ..ivm::kotodama::compiler::CompilerOptions::default()
            })
            .compile_source(
                r#"
seiyaku IdentityRequired {
  view fn write(int value) -> int {
    return value;
  }
}
"#,
            )
            .expect("compile identity-required raw contract");
        let chain_id = ChainId::from("identity-required-direct");
        let authority = ALICE_ID.clone();
        let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
            .build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let state = State::new_with_chain(
            World::with([domain], [account], []),
            Kura::blank_kura_for_testing(),
            query::store::LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        let mut metadata = Metadata::default();
        metadata.insert(
            "gas_limit".parse().expect("gas_limit key"),
            Json::new(1_000_000_u64),
        );
        metadata.insert(
            "contract_entrypoint".parse().expect("entrypoint key"),
            Json::new("write"),
        );
        metadata.insert(
            "contract_payload".parse().expect("payload key"),
            Json::from(norito::json!({ "value": "7" })),
        );
        let bytecode = IvmBytecode::from_compiled(program);
        let raw = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_metadata(metadata.clone())
            .with_executable(Executable::Ivm(bytecode.clone()))
            .sign(ALICE_KEYPAIR.private_key());
        let proved = TransactionBuilder::new(chain_id, authority.clone())
            .with_metadata(metadata)
            .with_executable(Executable::IvmProved(
                iroha_data_model::transaction::IvmProved {
                    bytecode,
                    overlay: Vec::<InstructionBox>::new().into(),
                    events_commitment: Hash::new(b"identityless-events"),
                    gas_policy_commitment: Hash::new(b"identityless-gas"),
                },
            ))
            .sign(ALICE_KEYPAIR.private_key());

        for (label, transaction) in [("raw", raw), ("proved", proved)] {
            let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
            let mut state_tx = block.transaction();
            let mut ivm_cache = IvmCache::new();
            ivm::reset_argument_record_decode_count();
            let error = super::Executor::Initial
                .execute_transaction(&mut state_tx, &authority, transaction, &mut ivm_cache)
                .expect_err("identity-less raw contract dispatch must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("requires a live contract_address or contract_alias"),
                "unexpected identity-less {label} dispatch error: {error}"
            );
            assert_eq!(
                ivm::argument_record_decode_count(),
                0,
                "identity-less {label} dispatch must not decode its argument record"
            );
            assert!(
                state_tx.world.smart_contract_state.is_empty(),
                "identity-less {label} dispatch must apply no durable state"
            );
        }
    }

    fn contract_permission_context(
        contract_address: ContractAddress,
        entrypoint: &str,
    ) -> ContractCallExecutionContext {
        ContractCallExecutionContext {
            contract_subject: Some(contract_address.subject_id()),
            contract_address: Some(contract_address),
            contract_alias: None,
            entrypoint: Some(entrypoint.to_owned()),
            entrypoint_pc: Some(0),
            entrypoint_permission: Some("CanInvokeContractEntrypoint".to_owned()),
            args: Json::new(()),
            argument_record: None,
        }
    }

    #[test]
    fn contract_invocation_rejects_a_live_code_rebind() {
        let contract_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &ALICE_ID,
            77,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let signed_hash = iroha_crypto::Hash::new(b"signed-contract-code");
        let live_hash = iroha_crypto::Hash::new(b"rebound-contract-code");
        let invocation = ContractInvocation {
            contract_address,
            expected_code_hash: signed_hash,
            entrypoint: "run".to_owned(),
            arguments: None,
        };

        let error = ensure_contract_invocation_code_hash(&invocation, live_hash)
            .expect_err("a signed call must not cross a live code rebind");
        assert!(
            matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains(&signed_hash.to_string())
                    && message.contains(&live_hash.to_string())),
            "unexpected binding error: {error}"
        );
    }

    #[test]
    fn contract_dispatch_context_carries_entrypoint_permission() {
        let (program, expected_entrypoint_pc) =
            contract_program_with_entrypoint("admin", Some("ContractAdmin"));
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new("admin".to_owned()),
        );

        let metadata_context = parse_contract_call_execution_context(&metadata, &program)
            .expect("parse metadata dispatch")
            .expect("metadata dispatch context");
        assert_eq!(metadata_context.entrypoint.as_deref(), Some("admin"));
        assert_eq!(
            metadata_context.entrypoint_pc(),
            Some(expected_entrypoint_pc)
        );
        assert_eq!(
            metadata_context.entrypoint_permission(),
            Some("ContractAdmin")
        );

        let contract_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &ALICE_ID,
            1,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let invocation = ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: iroha_crypto::Hash::new(b"admin-contract-code"),
            entrypoint: "admin".to_owned(),
            arguments: None,
        };
        let invocation_context = parse_contract_invocation_execution_context(
            &invocation,
            &program,
            None,
            contract_address.subject_id(),
        )
        .expect("parse contract invocation");
        assert_eq!(
            invocation_context.entrypoint_pc(),
            Some(expected_entrypoint_pc)
        );
        assert_eq!(
            invocation_context.entrypoint_permission(),
            Some("ContractAdmin")
        );
    }

    #[test]
    fn nested_contract_dispatch_accepts_view_without_relaxing_top_level_calls() {
        use iroha_data_model::smart_contract::manifest::EntryPointKind;

        let (program, expected_entrypoint_pc) = contract_program_with_entrypoint_kind(
            "configuration",
            EntryPointKind::View,
            Some("CanInspectConfiguration"),
        );
        let prepared = ivm::prepare_contract(Arc::<[u8]>::from(program))
            .expect("prepare nested-view contract");
        let contract_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &ALICE_ID,
            2,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let invocation = ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: iroha_crypto::Hash::new(b"configuration-contract-code"),
            entrypoint: "configuration".to_owned(),
            arguments: None,
        };

        let top_level_err = parse_prepared_contract_invocation_execution_context(
            &invocation,
            &prepared,
            None,
            contract_address.subject_id(),
            u64::MAX,
        )
        .expect_err("top-level transaction dispatch must remain public-only");
        assert!(matches!(
            top_level_err,
            ValidationFail::NotPermitted(message) if message.contains("read-only")
        ));

        let nested = parse_prepared_nested_contract_invocation_execution_context(
            &invocation,
            &prepared,
            None,
            contract_address.subject_id(),
            u64::MAX,
        )
        .expect("nested dispatch should accept a declared view");
        assert_eq!(nested.entrypoint.as_deref(), Some("configuration"));
        assert_eq!(nested.entrypoint_pc(), Some(expected_entrypoint_pc));
        assert_eq!(
            nested.entrypoint_permission(),
            Some("CanInspectConfiguration")
        );

        for (selector, kind) in [
            ("hajimari", EntryPointKind::Hajimari),
            ("始まり", EntryPointKind::Hajimari),
            ("kaizen", EntryPointKind::Kaizen),
            ("改善", EntryPointKind::Kaizen),
        ] {
            let (program, _) = contract_program_with_entrypoint_kind(selector, kind, None);
            let prepared = ivm::prepare_contract(Arc::<[u8]>::from(program))
                .expect("prepare lifecycle contract");
            let invocation = ContractInvocation {
                contract_address: contract_address.clone(),
                expected_code_hash: iroha_crypto::Hash::new(selector.as_bytes()),
                entrypoint: selector.to_owned(),
                arguments: None,
            };
            let error = parse_prepared_nested_contract_invocation_execution_context(
                &invocation,
                &prepared,
                None,
                contract_address.subject_id(),
                u64::MAX,
            )
            .expect_err("nested dispatch must reject lifecycle entrypoints");
            assert!(
                matches!(error, ValidationFail::NotPermitted(ref message) if message.contains("cannot be invoked by a nested call")),
                "unexpected {kind:?} nested-dispatch error: {error}"
            );
        }
    }

    #[test]
    fn prepared_view_resolver_accepts_only_views_and_preserves_exact_permission() {
        use iroha_data_model::smart_contract::manifest::EntryPointKind;

        let (program, expected_pc) = contract_program_with_entrypoint_kind(
            "inspect",
            EntryPointKind::View,
            Some("CanInspectContract"),
        );
        let prepared =
            ivm::prepare_contract(Arc::<[u8]>::from(program)).expect("prepare view contract");
        let (pc, permission, arguments) =
            resolve_prepared_contract_view_entrypoint(&prepared, "inspect")
                .expect("declared view resolves");
        assert_eq!(pc, expected_pc);
        assert_eq!(permission.as_deref(), Some("CanInspectContract"));
        assert!(arguments.is_none());
        assert!(
            resolve_prepared_contract_entrypoint(&prepared, "inspect").is_err(),
            "transaction resolution must continue to reject read-only views"
        );

        for (selector, kind) in [
            ("write", EntryPointKind::Kotoage),
            ("hajimari", EntryPointKind::Hajimari),
            ("始まり", EntryPointKind::Hajimari),
            ("kaizen", EntryPointKind::Kaizen),
            ("改善", EntryPointKind::Kaizen),
        ] {
            let permission =
                (kind == EntryPointKind::Kotoage).then_some("CanInvokeContractEntrypoint");
            let (program, _) = contract_program_with_entrypoint_kind(selector, kind, permission);
            let prepared = ivm::prepare_contract(Arc::<[u8]>::from(program))
                .expect("prepare non-view contract");
            let error = resolve_prepared_contract_view_entrypoint(&prepared, selector)
                .expect_err("the view boundary must reject every non-view entrypoint kind");
            assert!(
                matches!(error, ValidationFail::NotPermitted(ref message) if message.contains("not a read-only view")),
                "unexpected {kind:?} view-resolution error: {error}"
            );
        }
    }

    #[test]
    fn raw_contract_dispatch_rejects_lifecycle_entrypoints() {
        use iroha_data_model::smart_contract::manifest::EntryPointKind;

        for (selector, kind) in [
            ("hajimari", EntryPointKind::Hajimari),
            ("kaizen", EntryPointKind::Kaizen),
        ] {
            let (program, _) = contract_program_with_entrypoint_kind(selector, kind, None);
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("contract_entrypoint").expect("static name"),
                Json::new(selector.to_owned()),
            );

            let error = parse_contract_call_execution_context(&metadata, &program)
                .expect_err("raw transaction dispatch must not invoke lifecycle hooks");
            assert!(
                matches!(error, ValidationFail::NotPermitted(message) if message.contains("top-level deployed ContractCall") && message.contains(selector))
            );
        }
    }

    #[test]
    fn top_level_contract_invocation_uses_branded_lifecycle_permissions() {
        use iroha_data_model::smart_contract::manifest::EntryPointKind;

        let contract_address = ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &ALICE_ID,
            44,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        for (selector, kind, expected_permission) in [
            (
                "hajimari",
                EntryPointKind::Hajimari,
                iroha_data_model::smart_contract::CONTRACT_HAJIMARI_PERMISSION_NAME,
            ),
            (
                "kaizen",
                EntryPointKind::Kaizen,
                iroha_data_model::smart_contract::CONTRACT_KAIZEN_PERMISSION_NAME,
            ),
        ] {
            let (program, _) = contract_program_with_entrypoint_kind(selector, kind, None);
            let invocation = ContractInvocation {
                contract_address: contract_address.clone(),
                expected_code_hash: iroha_crypto::Hash::new(selector.as_bytes()),
                entrypoint: selector.to_owned(),
                arguments: None,
            };
            let context = parse_contract_invocation_execution_context(
                &invocation,
                &program,
                None,
                contract_address.subject_id(),
            )
            .expect("top-level lifecycle invocation resolves");
            assert_eq!(
                context.entrypoint_permission(),
                Some(expected_permission),
                "{selector} must use its runtime-defined branded lifecycle permission"
            );
        }
    }

    #[test]
    fn contract_transaction_dispatch_rejects_view_entrypoints() {
        use iroha_data_model::smart_contract::manifest::EntryPointKind;

        let (program, _) =
            contract_program_with_entrypoint_kind("inspect", EntryPointKind::View, None);
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new("inspect".to_owned()),
        );

        let err = parse_contract_call_execution_context(&metadata, &program)
            .expect_err("view transaction dispatch must reject");
        assert!(matches!(
            err,
            ValidationFail::NotPermitted(message)
                if message.contains("read-only")
        ));
    }

    #[test]
    fn contract_dispatch_context_rejects_implicit_main_for_self_describing_artifact() {
        let (program, _) = contract_program_with_entrypoint("main", Some("ContractAdmin"));
        let metadata = Metadata::default();

        let err = parse_contract_call_execution_context(&metadata, &program)
            .expect_err("implicit main dispatch must reject");
        assert!(matches!(
            err,
            ValidationFail::NotPermitted(message)
                if message.contains("require explicit contract_entrypoint")
        ));
    }

    #[test]
    fn contract_dispatch_context_keeps_generic_raw_ivm_without_selector_unclassified() {
        let metadata = Metadata::default();
        let context = parse_contract_call_execution_context(&metadata, &generate_ok_program())
            .expect("parse generic raw ivm context");

        assert!(context.is_none());
    }

    #[test]
    fn direct_generic_ivm_remains_reachable_and_rejects_contract_metadata() {
        let chain_id = ChainId::from("generic-direct-ivm");
        let authority = ALICE_ID.clone();
        let domain = Domain::new(DomainId::try_new("wonderland", "universal").expect("domain id"))
            .build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let state = State::new_with_chain(
            World::with([domain], [account], []),
            Kura::blank_kura_for_testing(),
            query::store::LiveQueryStore::start_test(),
            chain_id.clone(),
        );
        let mut program = ivm::ProgramMetadata {
            max_cycles: 100,
            ..ivm::ProgramMetadata::default()
        }
        .encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let generic_code_hash = ivm::contract_code_hash(&program);
        let executable = Executable::Ivm(IvmBytecode::from_compiled(program));
        let transaction = |metadata: Metadata| {
            TransactionBuilder::new(chain_id.clone(), authority.clone())
                .with_metadata(metadata)
                .with_executable(executable.clone())
                .sign(ALICE_KEYPAIR.private_key())
        };
        let mut gas_metadata = Metadata::default();
        gas_metadata.insert(
            "gas_limit".parse().expect("gas-limit metadata key"),
            Json::new(1_000_000_u64),
        );
        let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
        let mut state_transaction = block.transaction();
        let mut ivm_cache = IvmCache::new();

        super::Executor::Initial
            .execute_transaction(
                &mut state_transaction,
                &authority,
                transaction(gas_metadata.clone()),
                &mut ivm_cache,
            )
            .expect("contract-less generic IVM must execute at pc zero");

        let mut reserved_metadata = gas_metadata;
        reserved_metadata.insert(
            "contract_manifest"
                .parse()
                .expect("contract-manifest metadata key"),
            Json::from_string_unchecked("malformed-reserved-value".to_owned()),
        );
        let error = super::Executor::Initial
            .execute_transaction(
                &mut state_transaction,
                &authority,
                transaction(reserved_metadata),
                &mut ivm_cache,
            )
            .expect_err("generic IVM must not accept contract metadata");
        assert!(
            error.to_string().contains("reserved `contract_manifest`"),
            "unexpected generic-metadata rejection: {error}"
        );

        state_transaction.world.contract_manifests.insert(
            generic_code_hash,
            iroha_data_model::smart_contract::manifest::ContractManifest {
                seiyaku_name: None,
                code_hash: Some(generic_code_hash),
                abi_hash: Some(Hash::prehashed(ivm::syscalls::compute_abi_hash(
                    ivm::SyscallPolicy::AbiV1,
                ))),
                compiler_fingerprint: None,
                features_bitmap: None,
                access_set_hints: None,
                entrypoints: None,
                states: None,
                kotoba: None,
                error_codes: None,
                provenance: None,
            },
        );
        let error = super::Executor::Initial
            .execute_transaction(
                &mut state_transaction,
                &authority,
                transaction({
                    let mut metadata = Metadata::default();
                    metadata.insert(
                        "gas_limit".parse().expect("gas-limit metadata key"),
                        Json::new(1_000_000_u64),
                    );
                    metadata
                }),
                &mut ivm_cache,
            )
            .expect_err("a manifest-bound hash must not execute as generic IVM");
        assert!(error.to_string().contains("contract manifest"));
        state_transaction
            .world
            .contract_manifests
            .remove(generic_code_hash);

        state_transaction.pipeline.ivm_max_cycles_upper_bound = nonzero!(50_u64);
        let error = super::Executor::Initial
            .execute_transaction(
                &mut state_transaction,
                &authority,
                transaction({
                    let mut metadata = Metadata::default();
                    metadata.insert(
                        "gas_limit".parse().expect("gas-limit metadata key"),
                        Json::new(1_000_000_u64),
                    );
                    metadata
                }),
                &mut ivm_cache,
            )
            .expect_err("direct generic IVM must honor the live cycle ceiling");
        assert!(matches!(
            error,
            ValidationFail::IvmAdmission(
                iroha_data_model::executor::IvmAdmissionError::MaxCyclesExceedsUpperBound(_)
            )
        ));
    }

    #[test]
    fn contract_dispatch_context_rejects_no_selector_self_describing_artifact_without_main() {
        let (program, expected_entrypoint_pc) =
            contract_program_with_entrypoint("run", Some("RunPermission"));
        let metadata = Metadata::default();

        let err = parse_contract_call_execution_context(&metadata, &program)
            .expect_err("self-describing default dispatch without main should reject");
        assert!(matches!(
            err,
            ValidationFail::NotPermitted(message)
                if message.contains("require explicit contract_entrypoint")
        ));

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new("run".to_owned()),
        );
        let context = parse_contract_call_execution_context(&metadata, &program)
            .expect("explicit run dispatch parses")
            .expect("explicit run context");
        assert_eq!(context.entrypoint.as_deref(), Some("run"));
        assert_eq!(context.entrypoint_pc(), Some(expected_entrypoint_pc));
        assert_eq!(context.entrypoint_permission(), Some("RunPermission"));
    }

    #[test]
    fn trigger_dispatch_encodes_event_args_as_one_canonical_record() {
        let contract = prepared_parameterized_trigger_contract();
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new("run".to_owned()),
        );
        let event_args = Json::from(norito::json!({"val": "1.25"}));

        validate_trigger_call_execution_context(&metadata, contract.artifact())
            .expect("registration validates the typed callback without a fabricated payload");

        let context = parse_prepared_trigger_call_execution_context(
            &metadata,
            &contract,
            &event_args,
            u64::MAX,
        )
        .expect("bind typed trigger arguments");
        let descriptor = contract
            .entrypoint_descriptor("run")
            .expect("run descriptor");
        let schema = descriptor
            .argument_schema
            .as_ref()
            .expect("run argument schema");
        let expected = ivm::encode_argument_record_from_json(schema, &event_args)
            .expect("encode expected canonical record");

        assert_eq!(context.argument_record(), Some(expected.as_slice()));
        ivm::validate_argument_record(
            schema,
            context.argument_record().expect("trigger argument record"),
        )
        .expect("roundtrip canonical trigger argument record");
    }

    #[test]
    #[cfg(debug_assertions)]
    fn malformed_invocation_arguments_fail_during_context_preparation() {
        let contract = prepared_parameterized_trigger_contract();
        let schema = contract
            .entrypoint_descriptor("run")
            .and_then(|descriptor| descriptor.argument_schema.as_ref())
            .expect("run argument schema");
        let mut malformed = ivm::encode_argument_record_from_json(
            schema,
            &Json::from(norito::json!({"val": "1.25"})),
        )
        .expect("encode valid argument fixture");
        *malformed.last_mut().expect("record hash byte") ^= 0x80;
        let contract_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &ALICE_ID,
            19,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let invocation = ContractInvocation {
            contract_address,
            expected_code_hash: iroha_crypto::Hash::new(b"malformed-argument-contract-code"),
            entrypoint: "run".to_owned(),
            arguments: Some(
                iroha_data_model::transaction::executable::ContractArgumentRecord::try_new(
                    malformed,
                )
                .expect("bounded malformed fixture"),
            ),
        };

        ivm::reset_argument_record_decode_count();
        let error = parse_prepared_contract_invocation_execution_context(
            &invocation,
            &contract,
            None,
            invocation.contract_address.subject_id(),
            u64::MAX,
        )
        .expect_err("malformed arguments must fail before a VM is constructed or entered");

        assert!(matches!(
            error,
            ValidationFail::NotPermitted(message)
                if message.contains("invalid contract argument record")
        ));
        assert_eq!(ivm::argument_record_decode_count(), 1);
    }

    #[test]
    fn trigger_dispatch_rejects_static_payload_and_implicit_entrypoint() {
        let contract = prepared_parameterized_trigger_contract();
        let event_args = Json::from(norito::json!({"val": "7"}));

        let err = parse_prepared_trigger_call_execution_context(
            &Metadata::default(),
            &contract,
            &event_args,
            u64::MAX,
        )
        .expect_err("trigger callback selection must be explicit");
        assert!(matches!(
            err,
            ValidationFail::NotPermitted(message)
                if message.contains("explicit contract_entrypoint")
        ));

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("contract_entrypoint").expect("static name"),
            Json::new("run".to_owned()),
        );
        metadata.insert(
            Name::from_str("contract_payload").expect("static name"),
            Json::from(norito::json!({"val": "99"})),
        );
        let err = parse_prepared_trigger_call_execution_context(
            &metadata,
            &contract,
            &event_args,
            u64::MAX,
        )
        .expect_err("fixed metadata payload must not shadow event arguments");
        assert!(matches!(
            err,
            ValidationFail::NotPermitted(message)
                if message.contains("triggering event")
        ));
    }

    #[test]
    fn contract_entrypoint_permission_accepts_direct_and_role_grants() {
        let authority = ALICE_ID.clone();
        let account = Account::new(authority.clone()).build(&authority);
        let world = World::with([], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let contract_address = ContractAddress::derive(
            iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
            &authority,
            91,
            DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let direct_context = contract_permission_context(contract_address.clone(), "admin");
        let err = enforce_contract_entrypoint_permission(&tx.world, &authority, &direct_context)
            .expect_err("missing permission should reject contract entrypoint");
        assert!(matches!(
            err,
            ValidationFail::NotPermitted(message)
                if message.contains("requires an exact `CanInvokeContractEntrypoint` grant")
        ));

        let direct_permission: Permission =
            iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: contract_address.clone(),
                entrypoint: "admin".to_owned(),
            }
            .into();
        Grant::account_permission(direct_permission, authority.clone())
            .execute(&authority, &mut tx)
            .expect("grant direct contract permission");
        enforce_contract_entrypoint_permission(&tx.world, &authority, &direct_context)
            .expect("direct permission should allow contract entrypoint");

        let role_context = contract_permission_context(contract_address.clone(), "role_admin");
        let role_id: RoleId = "contract_admin_role".parse().expect("role id");
        let role: iroha_data_model::role::NewRole = Role::new(role_id.clone(), authority.clone())
            .add_permission(Permission::from(
            iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: contract_address.clone(),
                entrypoint: "role_admin".to_owned(),
            },
        ));
        Register::role(role)
            .execute(&authority, &mut tx)
            .expect("register contract role");
        enforce_contract_entrypoint_permission(&tx.world, &authority, &role_context)
            .expect("role permission should allow contract entrypoint");

        for denied_context in [
            contract_permission_context(contract_address.clone(), "wrong_entrypoint"),
            contract_permission_context(
                ContractAddress::derive(
                    iroha_data_model::smart_contract::CHAIN_DISCRIMINANT_MAINNET,
                    &authority,
                    92,
                    DataSpaceId::UNIVERSAL,
                )
                .expect("derive distinct contract address"),
                "admin",
            ),
        ] {
            enforce_contract_entrypoint_permission(&tx.world, &authority, &denied_context)
                .expect_err("a grant for another contract or selector must fail closed");
        }

        Grant::account_permission(
            Permission::new("CanInvokeContractEntrypoint".to_owned(), Json::new(())),
            authority.clone(),
        )
        .execute(&authority, &mut tx)
        .expect("store malformed name-only compatibility fixture");
        let malformed_only =
            contract_permission_context(contract_address.clone(), "malformed_only");
        enforce_contract_entrypoint_permission(&tx.world, &authority, &malformed_only)
            .expect_err("a name-only permission must never bypass exact payload matching");

        let custom_name = "ContractOperations";
        let noncanonical_custom = Permission::new(
            custom_name.to_owned(),
            Json::from(norito::json!({ "scope": "different-contract" })),
        );
        Grant::account_permission(noncanonical_custom.clone(), authority.clone())
            .execute(&authority, &mut tx)
            .expect("store same-name custom permission with a noncanonical payload");
        enforce_named_contract_entrypoint_permission(
            &tx.world,
            &authority,
            &contract_address,
            "custom_admin",
            Some(custom_name),
        )
        .expect_err("a same-name custom payload must not authorize an entrypoint");
        Revoke::account_permission(noncanonical_custom, authority.clone())
            .execute(&authority, &mut tx)
            .expect("remove noncanonical custom permission");
        Grant::account_permission(
            Permission::new(custom_name.to_owned(), Json::new(())),
            authority.clone(),
        )
        .execute(&authority, &mut tx)
        .expect("grant canonical custom entrypoint permission");
        enforce_named_contract_entrypoint_permission(
            &tx.world,
            &authority,
            &contract_address,
            "custom_admin",
            Some(custom_name),
        )
        .expect("the exact empty-payload custom permission must authorize its marker");
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

        let wonderland_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(wonderland_domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [alice_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = query::store::LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, ChainId::from("test-chain"));
        let block_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(block_header);
        let mut state_tx = block.transaction();

        let domain_id: DomainId = DomainId::try_new("test", "universal").expect("domain id");
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

        {
            let vm_small = loaded
                .checkout_runtime_for_gas_limit(small_limit)
                .expect("checkout small");
            assert_eq!(
                vm_small.memory.stack_limit(),
                super::stack_limit_for_gas(small_limit)
            );
            assert_eq!(vm_small.remaining_gas(), small_limit);
        }

        {
            let vm_large = loaded
                .checkout_runtime_for_gas_limit(large_limit)
                .expect("checkout large");
            assert_eq!(
                vm_large.memory.stack_limit(),
                super::stack_limit_for_gas(large_limit)
            );
            assert_eq!(vm_large.remaining_gas(), large_limit);
        }
    }

    #[test]
    fn loaded_executor_reuses_and_resets_runtime_after_error_return() {
        const GAS_LIMIT: u64 = 10_000;

        fn dirty_then_fail(loaded: &super::LoadedExecutor) -> Result<(), *const u8> {
            let mut runtime = loaded
                .checkout_runtime_for_gas_limit(GAS_LIMIT)
                .expect("checkout runtime");
            let allocation = runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr();
            runtime.set_register(7, 99);
            runtime
                .memory
                .preload_input(0, &[0xA5])
                .expect("dirty input memory");
            Err(allocation)
        }

        let raw =
            data_model_executor::Executor::new(IvmBytecode::from_compiled(generate_ok_program()));
        let loaded = super::LoadedExecutor::load(raw).expect("load");
        let (before, _) = loaded.runtime_pool_snapshot();

        let allocation = dirty_then_fail(&loaded).expect_err("synthetic validation failure");
        let (after_error, _) = loaded.runtime_pool_snapshot();
        assert_eq!(after_error.dirty_resets, before.dirty_resets + 1);

        let runtime = loaded
            .checkout_runtime_for_gas_limit(GAS_LIMIT)
            .expect("warm checkout");
        assert_eq!(runtime.register(7), 0);
        assert_eq!(runtime.remaining_gas(), GAS_LIMIT);
        assert_eq!(
            runtime
                .memory
                .load_region(0, 1)
                .expect("code memory")
                .as_ptr(),
            allocation,
            "warm executor validation must reuse the same memory allocation"
        );
        assert_eq!(
            runtime
                .memory
                .load_region(Memory::INPUT_START, 1)
                .expect("input memory"),
            &[0],
            "dirty input must be restored before reuse"
        );
        let (after_reuse, _) = loaded.runtime_pool_snapshot();
        assert_eq!(after_reuse.hits, after_error.hits + 1);
        assert_eq!(after_reuse.program_loads, after_error.program_loads);
        assert_eq!(after_reuse.template_builds, after_error.template_builds);
    }

    #[test]
    fn loaded_executor_runtime_variants_are_bounded() {
        let raw =
            data_model_executor::Executor::new(IvmBytecode::from_compiled(generate_ok_program()));
        let loaded = super::LoadedExecutor::load(raw).expect("load");
        let capacity = loaded.runtime_variant_capacity();
        let (before, _) = loaded.runtime_pool_snapshot();
        let multiplier = ivm::gas_to_stack_multiplier().max(1);
        let mut observed_keys = BTreeSet::new();
        for index in 0..capacity.saturating_add(3) {
            let target_stack = 64_u64
                .saturating_mul(1024)
                .saturating_mul(u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1));
            let gas_limit = target_stack.saturating_add(multiplier.saturating_sub(1)) / multiplier;
            let key = super::ExecutorRuntimeKey::for_gas_limit(gas_limit);
            assert!(
                observed_keys.insert(key),
                "test gas limits must resolve to distinct stack variants"
            );
            let runtime = loaded
                .checkout_runtime_for_gas_limit(gas_limit)
                .expect("checkout gas/stack variant");
            assert_eq!(runtime.memory.stack_limit(), key.stack_limit);
        }

        let (after, variant_count) = loaded.runtime_pool_snapshot();
        assert_eq!(variant_count, capacity);
        assert!(after.evictions > before.evictions);
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

        let wonderland_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(wonderland_domain_id.clone()).build(&ALICE_ID);
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

        let wonderland_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(wonderland_domain_id.clone()).build(&ALICE_ID);
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

        let wonderland_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain: Domain = Domain::new(wonderland_domain_id.clone()).build(&ALICE_ID);
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
        // Start with a fully valid authenticated header so rejection exercises the
        // oversized code section rather than an earlier metadata failure.
        prog.extend_from_slice(&ivm::ProgramMetadata::default_for(1, 0, 1).encode());
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

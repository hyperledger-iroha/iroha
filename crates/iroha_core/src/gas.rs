//! Deterministic gas metering for native ISI execution.
//!
//! This module provides a minimal, stable cost model for non-VM transactions
//! (Executable::Instructions). It assigns a base cost per instruction family
//! and adds small dynamic components for payload sizes (e.g., JSON values).
//!
//! Goals
//! - Deterministic across peers and hardware.
//! - Independent of WSV contents (only instruction payloads are considered).
//! - Conservative but simple to reason about.
//!
//! Non-goals
//! - Perfect proportionality to runtime wall-clock. Costs are calibrated to be
//!   monotonic with payload sizes and relative complexity.

use std::sync::atomic::{AtomicU64, Ordering};

use iroha_config::parameters::actual::ConfidentialGas as ActualConfidentialGas;
use iroha_data_model::{
    isi as dm_isi,
    isi::{Instruction as _, InstructionBox},
    proof::ProofAttachment,
    zk::OpenVerifyEnvelope,
};
use norito::decode_from_bytes;

/// Per-instruction family base costs.
/// Chosen to be small compared to the default per-block gas limit.
// Tuned to target a simple fee envelope:
// - Typical SetKeyValue with small JSON: ~128 gas.
// - Register/Unregister: ~200/150 gas.
// - Transfer/Mint/Burn: ~180/150/150 gas.
const BASE_REGISTER: u64 = 200;
const BASE_UNREGISTER: u64 = 150;
const BASE_TRANSFER: u64 = 180;
const BASE_MINT: u64 = 150;
const BASE_BURN: u64 = 150;
const BASE_SET_KV: u64 = 64;
const BASE_REMOVE_KV: u64 = 48;
const BASE_GRANT: u64 = 96;
const BASE_REVOKE: u64 = 96;
const BASE_EXECUTE_TRIGGER: u64 = 220;
const BASE_UPGRADE: u64 = 2_000;
const BASE_LOG: u64 = 8;
const BASE_CUSTOM: u64 = 128;
const BASE_REGISTER_SMART_CONTRACT: u64 = 320;
const BASE_KAIGI_CREATE: u64 = 420;
const BASE_KAIGI_JOIN: u64 = 180;
const BASE_KAIGI_LEAVE: u64 = 150;
const BASE_KAIGI_END: u64 = 220;
const BASE_KAIGI_USAGE: u64 = 240;
const BASE_KAIGI_JOIN_ZK: u64 = 1_520;
const BASE_KAIGI_LEAVE_ZK: u64 = 1_520;
const BASE_KAIGI_USAGE_ZK: u64 = 1_180;
/// Default gas charged for a single confidential proof verification before any other factors.
pub const DEFAULT_ZK_GAS_BASE_VERIFY: u64 = 250_000;
/// Default gas multiplier per public input exposed by a confidential proof.
pub const DEFAULT_ZK_GAS_PER_PUBLIC_INPUT: u64 = 2_000;
/// Default gas multiplier per byte of the confidential proof payload.
pub const DEFAULT_ZK_GAS_PER_PROOF_BYTE: u64 = 5;
/// Default gas multiplier per nullifier consumed by the transaction.
pub const DEFAULT_ZK_GAS_PER_NULLIFIER: u64 = 300;
/// Default gas multiplier per commitment created by the transaction.
pub const DEFAULT_ZK_GAS_PER_COMMITMENT: u64 = 500;
const FIELD_ELEMENT_BYTES: usize = 32;

/// Dynamic factors (per-byte) applied to encoded payloads where sensible.
const PER_BYTE_JSON: u64 = 1; // charge per JSON byte
const PER_BYTE_GENERIC: u64 = 0; // currently unused; reserved for future
const PER_KAIGI_PROOF_BYTE: u64 = 5;

static ZK_GAS_BASE_VERIFY: AtomicU64 = AtomicU64::new(DEFAULT_ZK_GAS_BASE_VERIFY);
static ZK_GAS_PER_PUBLIC_INPUT: AtomicU64 = AtomicU64::new(DEFAULT_ZK_GAS_PER_PUBLIC_INPUT);
static ZK_GAS_PER_PROOF_BYTE: AtomicU64 = AtomicU64::new(DEFAULT_ZK_GAS_PER_PROOF_BYTE);
static ZK_GAS_PER_NULLIFIER: AtomicU64 = AtomicU64::new(DEFAULT_ZK_GAS_PER_NULLIFIER);
static ZK_GAS_PER_COMMITMENT: AtomicU64 = AtomicU64::new(DEFAULT_ZK_GAS_PER_COMMITMENT);

/// Tunable gas schedule for confidential verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfidentialGasSchedule {
    /// Base gas charged before applying any per-element multipliers.
    pub base_verify: u64,
    /// Gas multiplier applied per public input exposed by the proof.
    pub per_public_input: u64,
    /// Gas multiplier applied per byte of proof data.
    pub per_proof_byte: u64,
    /// Gas multiplier applied per nullifier referenced by the proof.
    pub per_nullifier: u64,
    /// Gas multiplier applied per commitment emitted by the proof.
    pub per_commitment: u64,
}

impl Default for ConfidentialGasSchedule {
    fn default() -> Self {
        Self {
            base_verify: DEFAULT_ZK_GAS_BASE_VERIFY,
            per_public_input: DEFAULT_ZK_GAS_PER_PUBLIC_INPUT,
            per_proof_byte: DEFAULT_ZK_GAS_PER_PROOF_BYTE,
            per_nullifier: DEFAULT_ZK_GAS_PER_NULLIFIER,
            per_commitment: DEFAULT_ZK_GAS_PER_COMMITMENT,
        }
    }
}

impl From<ActualConfidentialGas> for ConfidentialGasSchedule {
    fn from(value: ActualConfidentialGas) -> Self {
        Self {
            base_verify: value.proof_base,
            per_public_input: value.per_public_input,
            per_proof_byte: value.per_proof_byte,
            per_nullifier: value.per_nullifier,
            per_commitment: value.per_commitment,
        }
    }
}

/// Update the global confidential verification gas schedule.
pub fn configure_confidential_gas(schedule: ConfidentialGasSchedule) {
    ZK_GAS_BASE_VERIFY.store(schedule.base_verify, Ordering::Relaxed);
    ZK_GAS_PER_PUBLIC_INPUT.store(schedule.per_public_input, Ordering::Relaxed);
    ZK_GAS_PER_PROOF_BYTE.store(schedule.per_proof_byte, Ordering::Relaxed);
    ZK_GAS_PER_NULLIFIER.store(schedule.per_nullifier, Ordering::Relaxed);
    ZK_GAS_PER_COMMITMENT.store(schedule.per_commitment, Ordering::Relaxed);
}

fn zk_gas_base_verify() -> u64 {
    ZK_GAS_BASE_VERIFY.load(Ordering::Relaxed)
}

fn zk_gas_per_public_input() -> u64 {
    ZK_GAS_PER_PUBLIC_INPUT.load(Ordering::Relaxed)
}

fn zk_gas_per_proof_byte() -> u64 {
    ZK_GAS_PER_PROOF_BYTE.load(Ordering::Relaxed)
}

fn zk_gas_per_nullifier() -> u64 {
    ZK_GAS_PER_NULLIFIER.load(Ordering::Relaxed)
}

fn zk_gas_per_commitment() -> u64 {
    ZK_GAS_PER_COMMITMENT.load(Ordering::Relaxed)
}

fn halo2_public_input_count(attachment: &ProofAttachment) -> Option<u64> {
    if !attachment.backend.as_str().starts_with("halo2/") {
        return None;
    }
    let env: OpenVerifyEnvelope = decode_from_bytes(&attachment.proof.bytes).ok()?;
    let len = env.public_inputs.len();
    let stride = FIELD_ELEMENT_BYTES as u64;
    if len == 0 {
        return Some(0);
    }
    Some(((len as u64) + stride.saturating_sub(1)) / stride)
}

fn gas_for_proof_attachment(
    attachment: &ProofAttachment,
    nullifiers: usize,
    commitments: usize,
) -> u64 {
    let mut gas = zk_gas_base_verify();
    let proof_bytes = u64::try_from(attachment.proof.bytes.len()).unwrap_or(u64::MAX);
    gas = gas.saturating_add(zk_gas_per_proof_byte().saturating_mul(proof_bytes));
    if let Some(public_inputs) = halo2_public_input_count(attachment) {
        gas = gas.saturating_add(zk_gas_per_public_input().saturating_mul(public_inputs));
    }
    let nullifiers_u64 = u64::try_from(nullifiers).unwrap_or(u64::MAX);
    let commitments_u64 = u64::try_from(commitments).unwrap_or(u64::MAX);
    gas = gas.saturating_add(zk_gas_per_nullifier().saturating_mul(nullifiers_u64));
    gas = gas.saturating_add(zk_gas_per_commitment().saturating_mul(commitments_u64));
    gas
}

/// Compute gas for a single instruction using a simple schedule.
#[allow(clippy::too_many_lines)]
pub fn meter_instruction(instr: &InstructionBox) -> u64 {
    // Helper to compute JSON-like payload size when present.
    // Use the canonical JSON string length without re-encoding to avoid
    // extra allocations during metering. `Json` stores a normalized string,
    // so measuring its length yields a deterministic size signal.
    fn json_len(j: &iroha_primitives::json::Json) -> usize {
        j.get().len()
    }

    // Downcast by visiting known grouped enums first, then concrete types.
    // Fall back to Norito-encoded size to keep custom/unknown instructions
    // bounded deterministically.
    let any = instr.as_any();

    // Register
    if let Some(reg) = any.downcast_ref::<dm_isi::register::RegisterBox>() {
        return match reg {
            dm_isi::register::RegisterBox::Peer(_) => BASE_REGISTER + 20,
            dm_isi::register::RegisterBox::Domain(_)
            | dm_isi::register::RegisterBox::Account(_)
            | dm_isi::register::RegisterBox::AssetDefinition(_)
            | dm_isi::register::RegisterBox::Nft(_)
            | dm_isi::register::RegisterBox::Role(_) => BASE_REGISTER,
            dm_isi::register::RegisterBox::Trigger(_) => BASE_REGISTER + 50, // triggers slightly heavier
        };
    }

    // Unregister
    if let Some(unreg) = any.downcast_ref::<dm_isi::register::UnregisterBox>() {
        return match unreg {
            dm_isi::register::UnregisterBox::Peer(_)
            | dm_isi::register::UnregisterBox::Domain(_)
            | dm_isi::register::UnregisterBox::Account(_)
            | dm_isi::register::UnregisterBox::AssetDefinition(_)
            | dm_isi::register::UnregisterBox::Nft(_)
            | dm_isi::register::UnregisterBox::Role(_)
            | dm_isi::register::UnregisterBox::Trigger(_) => BASE_UNREGISTER,
        };
    }

    // Transfers
    if let Some(xfer) = any.downcast_ref::<dm_isi::transfer::TransferBox>() {
        return match xfer {
            dm_isi::transfer::TransferBox::AssetDefinition(_)
            | dm_isi::transfer::TransferBox::Domain(_) => BASE_TRANSFER + 20,
            dm_isi::transfer::TransferBox::Asset(_) | dm_isi::transfer::TransferBox::Nft(_) => {
                BASE_TRANSFER
            }
        };
    }
    if let Some(batch) = any.downcast_ref::<dm_isi::transfer::TransferAssetBatch>() {
        let count = u64::try_from(batch.entries().len()).unwrap_or(u64::MAX);
        return BASE_TRANSFER.saturating_mul(count);
    }

    // Mint / Burn
    if let Some(mint) = any.downcast_ref::<dm_isi::mint_burn::MintBox>() {
        return match mint {
            dm_isi::mint_burn::MintBox::Asset(_) => BASE_MINT,
            dm_isi::mint_burn::MintBox::TriggerRepetitions(_) => BASE_MINT / 2,
        };
    }
    if let Some(burn) = any.downcast_ref::<dm_isi::mint_burn::BurnBox>() {
        return match burn {
            dm_isi::mint_burn::BurnBox::Asset(_) => BASE_BURN,
            dm_isi::mint_burn::BurnBox::TriggerRepetitions(_) => BASE_BURN / 2,
        };
    }

    // Key-value
    if let Some(kv) = any.downcast_ref::<dm_isi::SetKeyValueBox>() {
        let sz = match kv {
            dm_isi::SetKeyValueBox::Domain(i) => json_len(&i.value),
            dm_isi::SetKeyValueBox::Account(i) => json_len(&i.value),
            dm_isi::SetKeyValueBox::AssetDefinition(i) => json_len(&i.value),
            dm_isi::SetKeyValueBox::Nft(i) => json_len(&i.value),
            dm_isi::SetKeyValueBox::Trigger(i) => json_len(&i.value),
        } as u64;
        return BASE_SET_KV + PER_BYTE_JSON.saturating_mul(sz);
    }
    if any.downcast_ref::<dm_isi::RemoveKeyValueBox>().is_some() {
        return BASE_REMOVE_KV;
    }

    // Permissions
    if any.downcast_ref::<dm_isi::GrantBox>().is_some() {
        return BASE_GRANT;
    }
    if any.downcast_ref::<dm_isi::RevokeBox>().is_some() {
        return BASE_REVOKE;
    }

    // Misc
    if let Some(et) = any.downcast_ref::<dm_isi::ExecuteTrigger>() {
        let args_len = json_len(&et.args) as u64;
        return BASE_EXECUTE_TRIGGER + PER_BYTE_JSON.saturating_mul(args_len);
    }
    if any.downcast_ref::<dm_isi::Upgrade>().is_some() {
        return BASE_UPGRADE;
    }
    if let Some(log) = any.downcast_ref::<dm_isi::Log>() {
        // Charge per message length (stored as String)
        return BASE_LOG + (log.msg.len() as u64);
    }
    if let Some(custom) = any.downcast_ref::<dm_isi::CustomInstruction>() {
        let sz = json_len(&custom.payload) as u64;
        return BASE_CUSTOM + PER_BYTE_JSON.saturating_mul(sz);
    }
    if any.downcast_ref::<dm_isi::kaigi::CreateKaigi>().is_some() {
        return BASE_KAIGI_CREATE;
    }
    if let Some(join) = any.downcast_ref::<dm_isi::kaigi::JoinKaigi>() {
        let is_privacy = join.commitment.is_some()
            || join.nullifier.is_some()
            || join.roster_root.is_some()
            || join.proof.is_some();
        if is_privacy {
            let proof_bytes = join.proof.as_ref().map_or(0, |p| p.len() as u64);
            return BASE_KAIGI_JOIN_ZK + PER_KAIGI_PROOF_BYTE.saturating_mul(proof_bytes);
        }
        return BASE_KAIGI_JOIN;
    }
    if let Some(leave) = any.downcast_ref::<dm_isi::kaigi::LeaveKaigi>() {
        let is_privacy = leave.commitment.is_some()
            || leave.nullifier.is_some()
            || leave.roster_root.is_some()
            || leave.proof.is_some();
        if is_privacy {
            let proof_bytes = leave.proof.as_ref().map_or(0, |p| p.len() as u64);
            return BASE_KAIGI_LEAVE_ZK + PER_KAIGI_PROOF_BYTE.saturating_mul(proof_bytes);
        }
        return BASE_KAIGI_LEAVE;
    }
    if any.downcast_ref::<dm_isi::kaigi::EndKaigi>().is_some() {
        return BASE_KAIGI_END;
    }
    if let Some(usage) = any.downcast_ref::<dm_isi::kaigi::RecordKaigiUsage>() {
        let is_privacy = usage.usage_commitment.is_some() || usage.proof.is_some();
        if is_privacy {
            let proof_bytes = usage.proof.as_ref().map_or(0, |p| p.len() as u64);
            return BASE_KAIGI_USAGE_ZK + PER_KAIGI_PROOF_BYTE.saturating_mul(proof_bytes);
        }
        return BASE_KAIGI_USAGE;
    }
    if let Some(verify) = any.downcast_ref::<dm_isi::zk::VerifyProof>() {
        return gas_for_proof_attachment(&verify.attachment, 0, 0);
    }
    if any.downcast_ref::<dm_isi::zk::Shield>().is_some() {
        return zk_gas_per_commitment();
    }
    if let Some(transfer) = any.downcast_ref::<dm_isi::zk::ZkTransfer>() {
        return gas_for_proof_attachment(
            &transfer.proof,
            transfer.inputs.len(),
            transfer.outputs.len(),
        );
    }
    if let Some(unshield) = any.downcast_ref::<dm_isi::zk::Unshield>() {
        return gas_for_proof_attachment(&unshield.proof, unshield.inputs.len(), 0);
    }
    if let Some(ballot) = any.downcast_ref::<dm_isi::zk::SubmitBallot>() {
        return gas_for_proof_attachment(&ballot.ballot_proof, 1, 0);
    }
    if let Some(finalize) = any.downcast_ref::<dm_isi::zk::FinalizeElection>() {
        return gas_for_proof_attachment(&finalize.tally_proof, 0, 0);
    }
    if any
        .downcast_ref::<dm_isi::smart_contract_code::RegisterSmartContractCode>()
        .is_some()
    {
        return BASE_REGISTER_SMART_CONTRACT;
    }

    // Fallback: charge based on Norito-encoded size of the instruction payload
    // This ensures determinism for unknown/custom instructions under custom executors.
    let bytes = instr.dyn_encode();
    BASE_CUSTOM + PER_BYTE_GENERIC.saturating_mul(bytes.len() as u64)
}

/// Compute gas for a sequence of instructions.
pub fn meter_instructions(is: &[InstructionBox]) -> u64 {
    is.iter().map(meter_instruction).sum()
}

/// Return the portion of the gas schedule attributed to confidential ISIs.
#[must_use]
pub fn confidential_gas_cost(instr: &InstructionBox) -> u64 {
    let any = instr.as_any();
    if let Some(verify) = any.downcast_ref::<dm_isi::zk::VerifyProof>() {
        return gas_for_proof_attachment(&verify.attachment, 0, 0);
    }
    if any.downcast_ref::<dm_isi::zk::Shield>().is_some() {
        return zk_gas_per_commitment();
    }
    if let Some(transfer) = any.downcast_ref::<dm_isi::zk::ZkTransfer>() {
        return gas_for_proof_attachment(
            &transfer.proof,
            transfer.inputs.len(),
            transfer.outputs.len(),
        );
    }
    if let Some(unshield) = any.downcast_ref::<dm_isi::zk::Unshield>() {
        return gas_for_proof_attachment(&unshield.proof, unshield.inputs.len(), 0);
    }
    if let Some(ballot) = any.downcast_ref::<dm_isi::zk::SubmitBallot>() {
        return gas_for_proof_attachment(&ballot.ballot_proof, 1, 0);
    }
    if let Some(finalize) = any.downcast_ref::<dm_isi::zk::FinalizeElection>() {
        return gas_for_proof_attachment(&finalize.tally_proof, 0, 0);
    }
    0
}

#[cfg(test)]
mod tests {
    use iroha_config::parameters::actual as cfg;
    use iroha_data_model::prelude::*;
    use iroha_primitives::json::Json;
    use iroha_test_samples::gen_account_in;

    use super::*;
    use crate::{
        kura::Kura, query::store::LiveQueryStore, state::State,
        zk::test_utils::halo2_fixture_envelope,
    };

    fn sample_account() -> AccountId {
        gen_account_in("wonderland").0
    }

    #[test]
    fn set_kv_scales_with_value_length() {
        let id = sample_account();
        let small = SetKeyValue::account(id.clone(), "k".parse().unwrap(), Json::new("v"));
        let big = SetKeyValue::account(id, "k".parse().unwrap(), Json::new("v".repeat(256)));
        let g_small = meter_instruction(&InstructionBox::from(SetKeyValueBox::from(small)));
        let g_big = meter_instruction(&InstructionBox::from(SetKeyValueBox::from(big)));
        assert!(g_big > g_small);
    }

    #[test]
    fn mint_and_transfer_have_nonzero_costs() {
        let a = sample_account();
        let def: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let mint =
            dm_isi::mint_burn::Mint::asset_numeric(1u64, AssetId::of(def.clone(), a.clone()));
        let xfer = dm_isi::transfer::Transfer::asset_numeric(AssetId::of(def, a.clone()), 1u64, a);
        let g_mint = meter_instruction(&InstructionBox::from(dm_isi::mint_burn::MintBox::from(
            mint,
        )));
        let g_xfer = meter_instruction(&InstructionBox::from(dm_isi::transfer::TransferBox::from(
            xfer,
        )));
        assert!(g_mint > 0 && g_xfer > 0);
    }

    #[test]
    fn batch_meter_sums_items() {
        let a = sample_account();
        let def: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let r = dm_isi::register::Register::asset_definition(AssetDefinition::numeric(def.clone()));
        let m = dm_isi::mint_burn::Mint::asset_numeric(10u64, AssetId::of(def, a));
        let v = vec![
            InstructionBox::from(dm_isi::register::RegisterBox::from(r)),
            InstructionBox::from(dm_isi::mint_burn::MintBox::from(m)),
        ];
        let sum_inline = v.iter().map(meter_instruction).sum::<u64>();
        assert_eq!(sum_inline, meter_instructions(&v));
    }

    #[test]
    fn transfer_batch_gas_matches_entry_sum() {
        let from = sample_account();
        let to = sample_account();
        let def: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let entry_a = dm_isi::transfer::TransferAssetBatchEntry::new(
            from.clone(),
            to.clone(),
            def.clone(),
            1u64,
        );
        let entry_b = dm_isi::transfer::TransferAssetBatchEntry::new(
            from.clone(),
            to.clone(),
            def.clone(),
            2u64,
        );
        let batch = dm_isi::transfer::TransferAssetBatch::new(vec![entry_a, entry_b]);
        let batch_gas = meter_instruction(&InstructionBox::from(batch));
        let t1 = dm_isi::transfer::Transfer::asset_numeric(
            AssetId::of(def.clone(), from.clone()),
            1u64,
            to.clone(),
        );
        let t2 = dm_isi::transfer::Transfer::asset_numeric(AssetId::of(def, from), 2u64, to);
        let expected = meter_instruction(&InstructionBox::from(
            dm_isi::transfer::TransferBox::from(t1),
        ))
        .saturating_add(meter_instruction(&InstructionBox::from(
            dm_isi::transfer::TransferBox::from(t2),
        )));
        assert_eq!(batch_gas, expected);
    }

    #[test]
    fn calibration_bench_gas_snapshot() {
        let (authority, _) = gen_account_in("wonderland");
        let role_id: RoleId = "bench_role".parse().unwrap();
        let trigger_id: TriggerId = "bench_trg".parse().unwrap();

        let register_domain: InstructionBox =
            dm_isi::register::Register::domain(Domain::new("bench".parse().unwrap())).into();

        let register_account: InstructionBox =
            dm_isi::register::Register::account(Account::new(authority.clone())).into();

        let asset_definition_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let register_asset_definition: InstructionBox =
            dm_isi::register::Register::asset_definition(AssetDefinition::numeric(
                asset_definition_id.clone(),
            ))
            .into();

        let set_account_kv: InstructionBox =
            dm_isi::SetKeyValue::account(authority.clone(), "k".parse().unwrap(), Json::new("v"))
                .into();

        let grant_account_role: InstructionBox =
            dm_isi::Grant::account_role(role_id.clone(), authority.clone()).into();

        let revoke_account_role: InstructionBox =
            dm_isi::Revoke::account_role(role_id.clone(), authority.clone()).into();

        let execute_trigger: InstructionBox =
            dm_isi::ExecuteTrigger::new(trigger_id.clone()).into();

        let asset_id = AssetId::of(asset_definition_id.clone(), authority.clone());
        let mint_asset: InstructionBox =
            dm_isi::mint_burn::Mint::asset_numeric(Numeric::new(1, 0), asset_id.clone()).into();

        let transfer_asset: InstructionBox = dm_isi::transfer::Transfer::asset_numeric(
            asset_id,
            Numeric::new(1, 0),
            authority.clone(),
        )
        .into();

        let cases = [
            ("RegisterDomain", register_domain, 200),
            ("RegisterAccount", register_account, 200),
            ("RegisterAssetDef", register_asset_definition, 200),
            ("SetAccountKV_small", set_account_kv, 67),
            ("GrantAccountRole", grant_account_role, 96),
            ("RevokeAccountRole", revoke_account_role, 96),
            ("ExecuteTrigger_empty_args", execute_trigger, 224),
            ("MintAsset", mint_asset, 150),
            ("TransferAsset", transfer_asset, 180),
        ];

        let mut total = 0_u64;
        for (label, instr, expected) in &cases {
            let expected = *expected;
            let gas = meter_instruction(instr);
            assert_eq!(
                gas, expected,
                "{label} gas mismatch (got {gas}, expected {expected})"
            );
            total += gas;
        }
        let expected_total: u64 = cases.iter().map(|(_, _, expected)| *expected).sum();
        assert_eq!(total, expected_total);
    }

    #[test]
    fn shield_gas_charges_commitment() {
        use iroha_data_model::{
            confidential::ConfidentialEncryptedPayload, isi::zk::Shield, prelude::AssetDefinitionId,
        };
        use iroha_test_samples::ALICE_ID;

        crate::test_alias::ensure();
        super::configure_confidential_gas(super::ConfidentialGasSchedule::default());
        let asset: AssetDefinitionId = "shield#domain".parse().unwrap();
        let account = ALICE_ID.clone();
        let shield = Shield::new(
            asset,
            account,
            42,
            [0x11; 32],
            ConfidentialEncryptedPayload::default(),
        );
        let shield_instr = InstructionBox::from(shield);
        let gas = meter_instruction(&shield_instr);
        assert_eq!(gas, super::DEFAULT_ZK_GAS_PER_COMMITMENT);
        assert_eq!(confidential_gas_cost(&shield_instr), gas);
    }

    #[test]
    fn verify_proof_gas_matches_schedule() {
        use iroha_data_model::{isi::zk::VerifyProof, proof::VerifyingKeyId};

        let schedule = super::ConfidentialGasSchedule::default();
        super::configure_confidential_gas(schedule);
        let fixture = halo2_fixture_envelope("halo2/ipa:gas-meter", [0u8; 32]);
        let proof_box = fixture.proof_box("halo2/ipa");
        let attachment = ProofAttachment::new_ref(
            proof_box.backend.clone(),
            proof_box,
            VerifyingKeyId::new("halo2/ipa", "vk-gas"),
        );
        let proof_bytes = attachment.proof.bytes.len() as u64;
        let public_inputs = (fixture.public_inputs.len() / super::FIELD_ELEMENT_BYTES) as u64;
        let instruction: InstructionBox = VerifyProof::new(attachment).into();
        let gas = meter_instruction(&instruction);
        assert_eq!(public_inputs, 5);
        let expected = schedule.base_verify
            + schedule.per_public_input.saturating_mul(public_inputs)
            + schedule.per_proof_byte.saturating_mul(proof_bytes);
        assert_eq!(gas, expected);
        assert_eq!(confidential_gas_cost(&instruction), expected);
    }

    #[test]
    fn zk_transfer_gas_accounts_for_nullifiers_and_commitments() {
        use iroha_data_model::{
            isi::zk::ZkTransfer, prelude::AssetDefinitionId, proof::VerifyingKeyId,
        };

        let schedule = super::ConfidentialGasSchedule::default();
        super::configure_confidential_gas(schedule);
        let fixture = halo2_fixture_envelope("halo2/ipa:transfer-gas", [0u8; 32]);
        let proof_box = fixture.proof_box("halo2/ipa");
        let attachment = ProofAttachment::new_ref(
            proof_box.backend.clone(),
            proof_box,
            VerifyingKeyId::new("halo2/ipa", "vk-transfer"),
        );
        let proof_bytes = attachment.proof.bytes.len() as u64;
        let public_inputs = (fixture.public_inputs.len() / super::FIELD_ELEMENT_BYTES) as u64;
        let asset: AssetDefinitionId = "shield#domain".parse().unwrap();
        let transfer = ZkTransfer::new(
            asset,
            vec![[0xAA; 32], [0xBB; 32]],
            vec![[0xCC; 32]],
            attachment,
            None,
        );
        let transfer_instr = InstructionBox::from(transfer);
        let gas = meter_instruction(&transfer_instr);
        let expected = schedule.base_verify
            + schedule.per_public_input.saturating_mul(public_inputs)
            + schedule.per_proof_byte.saturating_mul(proof_bytes)
            + schedule.per_nullifier.saturating_mul(2)
            + schedule.per_commitment;
        assert_eq!(gas, expected);
        assert_eq!(confidential_gas_cost(&transfer_instr), expected);
    }

    #[test]
    fn state_configured_gas_schedule_updates_metering() {
        use iroha_data_model::{
            isi::zk::{VerifyProof, ZkTransfer},
            proof::VerifyingKeyId,
        };

        configure_confidential_gas(ConfidentialGasSchedule::default());

        let world = crate::state::World::new();
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new(world, kura, query);

        let mut zk_cfg = state.zk.clone();
        zk_cfg.gas = cfg::ConfidentialGas {
            proof_base: 777_000,
            per_public_input: 3_131,
            per_proof_byte: 29,
            per_nullifier: 47,
            per_commitment: 59,
        };
        state.set_zk(zk_cfg.clone());

        let fixture = halo2_fixture_envelope("halo2/ipa:transfer-gas", [0u8; 32]);
        let proof_box = fixture.proof_box("halo2/ipa");
        let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
            proof_box.backend.clone(),
            proof_box,
            VerifyingKeyId::new("halo2/ipa", "vk-config-gas"),
        );
        let proof_bytes = attachment.proof.bytes.len() as u64;
        let public_inputs =
            halo2_public_input_count(&attachment).expect("fixture exposes halo2 public inputs");

        let verify_instr: InstructionBox = VerifyProof::new(attachment.clone()).into();
        let verify_gas = meter_instruction(&verify_instr);
        let expected_verify = zk_cfg.gas.proof_base
            + zk_cfg.gas.per_public_input.saturating_mul(public_inputs)
            + zk_cfg.gas.per_proof_byte.saturating_mul(proof_bytes);
        assert_eq!(verify_gas, expected_verify);

        let asset: AssetDefinitionId = "shield#domain".parse().unwrap();
        let nullifiers = vec![[0xAA; 32], [0xBB; 32]];
        let commitments = vec![[0xCC; 32], [0xDD; 32]];
        let transfer_instr: InstructionBox = ZkTransfer::new(
            asset,
            nullifiers.clone(),
            commitments.clone(),
            attachment,
            None,
        )
        .into();
        let transfer_gas = meter_instruction(&transfer_instr);
        let expected_transfer = expected_verify
            + zk_cfg
                .gas
                .per_nullifier
                .saturating_mul(nullifiers.len() as u64)
            + zk_cfg
                .gas
                .per_commitment
                .saturating_mul(commitments.len() as u64);
        assert_eq!(transfer_gas, expected_transfer);

        configure_confidential_gas(ConfidentialGasSchedule::default());
    }

    #[test]
    fn confidential_gas_cost_zero_for_non_confidential_instr() {
        let account = sample_account();
        let asset: AssetDefinitionId = "xor#wonderland".parse().unwrap();
        let mint =
            dm_isi::mint_burn::Mint::asset_numeric(1u64, AssetId::of(asset, account.clone()));
        let instr = InstructionBox::from(dm_isi::mint_burn::MintBox::from(mint));
        assert_eq!(confidential_gas_cost(&instr), 0);
    }
}

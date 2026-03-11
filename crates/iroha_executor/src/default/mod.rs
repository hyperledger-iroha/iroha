//! Definition of Iroha default executor and accompanying execute functions.

/// Re-export account visitor helpers used by the default executor.
pub use account::{
    visit_register_account, visit_remove_account_key_value, visit_set_account_key_value,
    visit_unregister_account,
};
/// Re-export asset visitor helpers used by the default executor.
pub use asset::{
    visit_burn_asset_numeric, visit_mint_asset_numeric, visit_remove_asset_key_value,
    visit_set_asset_key_value, visit_transfer_asset_numeric,
};
/// Re-export asset-definition visitor helpers used by the default executor.
pub use asset_definition::{
    visit_register_asset_definition, visit_remove_asset_definition_key_value,
    visit_set_asset_definition_key_value, visit_transfer_asset_definition,
    visit_unregister_asset_definition,
};
/// Re-export bridge receipt visitor helper.
pub use bridge::visit_record_bridge_receipt;
/// Re-export domain visitor helpers used by the default executor.
pub use domain::{
    visit_register_domain, visit_remove_domain_key_value, visit_set_domain_key_value,
    visit_transfer_domain, visit_unregister_domain,
};
/// Re-export upgrade visitor helper.
pub use executor::visit_upgrade;
use iroha_smart_contract::data_model::{
    isi::{
        ActivatePublicLaneValidator, ApprovePinManifest, BindManifestAlias,
        CompleteReplicationOrder, ExitPublicLaneValidator, IssueReplicationOrder,
        ReclaimExpiredOfflineAllowance, RecordCapacityTelemetry, RecordReplicationReceipt,
        RegisterCapacityDeclaration, RegisterCapacityDispute, RegisterOfflineAllowance,
        RegisterPeerWithPop, RegisterPinManifest, RegisterProviderOwner,
        RegisterPublicLaneValidator, RemoveAssetKeyValue, RetirePinManifest, SetAssetKeyValue,
        SetLaneRelayEmergencyValidators, SetPricingSchedule, SubmitOfflineToOnlineTransfer,
        UnregisterProviderOwner, UpsertProviderCredit,
        bridge::RecordBridgeReceipt,
        repo::{RepoInstructionBox, RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
    },
    prelude::*,
    visit::Visit,
};
/// Re-export dispatch for custom instructions.
pub use isi::visit_custom_instruction;
/// Re-export logging instruction visitor helper.
pub use log::visit_log;
/// Re-export Nexus visitor helper used by the default executor.
pub use nexus::visit_set_lane_relay_emergency_validators;
/// Re-export NFT visitor helpers used by the default executor.
pub use nft::{
    visit_register_nft, visit_remove_nft_key_value, visit_set_nft_key_value, visit_transfer_nft,
    visit_unregister_nft,
};
/// Re-export parameter visitor helpers used by the default executor.
pub use parameter::visit_set_parameter;
/// Re-export peer visitor helpers used by the default executor.
pub use peer::{visit_register_peer, visit_unregister_peer};
/// Re-export permission visitor helpers used by the default executor.
pub use permission::{visit_grant_account_permission, visit_revoke_account_permission};
/// Re-export role visitor helpers used by the default executor.
pub use role::{
    visit_grant_account_role, visit_grant_role_permission, visit_register_role,
    visit_revoke_account_role, visit_revoke_role_permission, visit_unregister_role,
};
/// Re-export staking visitor helpers used by the default executor.
pub use staking::{
    visit_activate_public_lane_validator, visit_exit_public_lane_validator,
    visit_register_public_lane_validator,
};
mod offline;

/// Re-export offline settlement visitor helper.
pub use offline::{
    visit_reclaim_expired_offline_allowance, visit_register_offline_allowance,
    visit_submit_offline_to_online_transfer,
};
/// Re-export trigger visitor helpers used by the default executor.
pub use trigger::{
    visit_burn_trigger_repetitions, visit_execute_trigger, visit_mint_trigger_repetitions,
    visit_register_trigger, visit_remove_trigger_key_value, visit_set_trigger_key_value,
    visit_unregister_trigger,
};

use crate::{
    Execute, deny, execute,
    permission::{AnyPermission, ExecutorPermission as _},
};

/// Helpers shared by custom instruction integrations.
pub mod isi;

// NOTE: If any new `visit_..` functions are introduced in this module, one should
// not forget to update the default executor boilerplate too, specifically the
// `iroha_executor::derive::default::impl_derive_visit` function
// signature list.

#[derive(norito::derive::JsonDeserialize)]
struct IvmProvedJsonView {
    bytecode: IvmBytecode,
    overlay: Vec<InstructionBox>,
    events_commitment: Hash,
    gas_policy_commitment: Hash,
}

fn decode_ivm_proved_view(proved: &IvmProved) -> Option<(IvmBytecode, Vec<InstructionBox>)> {
    let rendered = norito::json::to_json(proved).ok()?;
    let parsed: IvmProvedJsonView = norito::json::from_str(&rendered).ok()?;
    let _ = (parsed.events_commitment, parsed.gas_policy_commitment);
    Some((parsed.bytecode, parsed.overlay))
}

#[cfg(test)]
mod ivm_proved_decode_tests {
    use super::*;

    #[test]
    fn decode_ivm_proved_view_roundtrips_minimal_payload() {
        let expected_bytecode = IvmBytecode::from_compiled(vec![0xA1, 0xB2, 0xC3]);
        let expected_events_commitment = Hash::new(b"executor-events");
        let expected_gas_commitment = Hash::new(b"executor-gas");

        let bytecode_json = norito::json::to_json(&expected_bytecode).expect("bytecode json");
        let events_json =
            norito::json::to_json(&expected_events_commitment).expect("events commitment json");
        let gas_json = norito::json::to_json(&expected_gas_commitment).expect("gas json");
        let proved_json = format!(
            "{{\"bytecode\":{bytecode_json},\"overlay\":[],\"events_commitment\":{events_json},\"gas_policy_commitment\":{gas_json}}}"
        );
        let proved: IvmProved =
            norito::json::from_str(&proved_json).expect("IvmProved JSON payload should decode");

        let (bytecode, overlay) =
            decode_ivm_proved_view(&proved).expect("helper should decode proved payload");
        assert_eq!(bytecode.as_ref(), expected_bytecode.as_ref());
        assert!(overlay.is_empty(), "overlay should roundtrip as empty");
    }
}

/// Execute [`SignedTransaction`].
///
/// Transaction is executed following successful validation.
///
/// # Warning
///
/// [`Executable::Ivm`] is not executed because it is validated on the host side.
pub fn visit_transaction<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    transaction: &SignedTransaction,
) {
    match transaction.instructions() {
        Executable::Ivm(bytecode) => executor.visit_ivm(bytecode),
        Executable::IvmProved(proved) => {
            let (bytecode, overlay) =
                decode_ivm_proved_view(proved).expect("IvmProved payload must decode");
            executor.visit_ivm(&bytecode);
            for isi in &overlay {
                if executor.verdict().is_ok() {
                    executor.visit_instruction(isi);
                }
            }
        }
        Executable::Instructions(instructions) => {
            for isi in instructions {
                if executor.verdict().is_ok() {
                    executor.visit_instruction(isi);
                }
            }
        }
    }
}

/// Execute [`InstructionBox`] by delegating to the appropriate visitor
/// implementation.
pub fn visit_instruction<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &InstructionBox) {
    isi.dispatch(executor);
}

trait InstructionDispatch {
    fn dispatch<V: Execute + Visit + ?Sized>(&self, executor: &mut V);
}

impl InstructionDispatch for InstructionBox {
    #[allow(clippy::too_many_lines)]
    fn dispatch<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        // InstructionBox wraps a trait object. Downcast to known built-ins.
        let any = self.as_any();

        if let Some(isi) = any.downcast_ref::<SetParameter>() {
            executor.visit_set_parameter(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<Log>() {
            executor.visit_log(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ExecuteTrigger>() {
            executor.visit_execute_trigger(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<BurnBox>() {
            executor.visit_burn(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<GrantBox>() {
            executor.visit_grant(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterPeerWithPop>() {
            visit_register_peer(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<MintBox>() {
            executor.visit_mint(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterBox>() {
            if let RegisterBox::Peer(peer) = isi {
                visit_register_peer(executor, peer);
                return;
            }
            executor.visit_register(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RemoveKeyValueBox>() {
            executor.visit_remove_key_value(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RevokeBox>() {
            executor.visit_revoke(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetKeyValueBox>() {
            executor.visit_set_key_value(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetAssetKeyValue>() {
            visit_set_asset_key_value(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RemoveAssetKeyValue>() {
            visit_remove_asset_key_value(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterPublicLaneValidator>() {
            visit_register_public_lane_validator(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ActivatePublicLaneValidator>() {
            visit_activate_public_lane_validator(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ExitPublicLaneValidator>() {
            visit_exit_public_lane_validator(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetLaneRelayEmergencyValidators>() {
            nexus::visit_set_lane_relay_emergency_validators(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<TransferBox>() {
            executor.visit_transfer(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SubmitOfflineToOnlineTransfer>() {
            visit_submit_offline_to_online_transfer(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterOfflineAllowance>() {
            visit_register_offline_allowance(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ReclaimExpiredOfflineAllowance>() {
            visit_reclaim_expired_offline_allowance(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RepoInstructionBox>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RepoIsi>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<ReverseRepoIsi>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RepoMarginCallIsi>() {
            execute!(executor, isi);
        }
        if let Some(isi) = any.downcast_ref::<RegisterPinManifest>() {
            sorafs::visit_register_pin_manifest(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<ApprovePinManifest>() {
            sorafs::visit_approve_pin_manifest(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RetirePinManifest>() {
            sorafs::visit_retire_pin_manifest(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<BindManifestAlias>() {
            sorafs::visit_bind_manifest_alias(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterCapacityDeclaration>() {
            sorafs::visit_register_capacity_declaration(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RecordCapacityTelemetry>() {
            sorafs::visit_record_capacity_telemetry(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterCapacityDispute>() {
            sorafs::visit_register_capacity_dispute(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<IssueReplicationOrder>() {
            sorafs::visit_issue_replication_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CompleteReplicationOrder>() {
            sorafs::visit_complete_replication_order(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RecordReplicationReceipt>() {
            sorafs::visit_record_replication_receipt(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RecordBridgeReceipt>() {
            bridge::visit_record_bridge_receipt(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<RegisterProviderOwner>() {
            sorafs::visit_register_provider_owner(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UnregisterProviderOwner>() {
            sorafs::visit_unregister_provider_owner(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<SetPricingSchedule>() {
            sorafs::visit_set_pricing_schedule(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UpsertProviderCredit>() {
            sorafs::visit_upsert_provider_credit(executor, isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<UnregisterBox>() {
            executor.visit_unregister(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<Upgrade>() {
            executor.visit_upgrade(isi);
            return;
        }
        if let Some(isi) = any.downcast_ref::<CustomInstruction>() {
            executor.visit_custom_instruction(isi);
            return;
        }

        deny!(executor, "unexpected instruction type");
    }
}

/// Permission-checked visitors for peer management instructions.
pub mod peer {
    use iroha_executor_data_model::permission::peer::CanManagePeers;

    use super::*;

    /// Registers a peer when genesis or a peer manager submits the instruction.
    pub fn visit_register_peer<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterPeerWithPop,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register peer");
    }

    /// Unregisters a peer if the caller has peer management privileges.
    pub fn visit_unregister_peer<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Peer>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister peer");
    }
}

/// Permission-checked visitors for public-lane validator lifecycle instructions.
pub mod staking {
    use iroha_executor_data_model::permission::peer::CanManagePeers;

    use super::*;

    /// Register a public-lane validator when the caller is authorised or during genesis.
    pub fn visit_register_public_lane_validator<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterPublicLaneValidator,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't register public-lane validator");
    }

    /// Activate a pending public-lane validator when the caller is authorised or during genesis.
    pub fn visit_activate_public_lane_validator<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ActivatePublicLaneValidator,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't activate public-lane validator");
    }

    /// Mark a validator as exiting when the caller is authorised or during genesis.
    pub fn visit_exit_public_lane_validator<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ExitPublicLaneValidator,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't exit public-lane validator");
    }
}

/// Permission-checked visitors for Nexus lane relay recovery instructions.
pub mod nexus {
    use iroha_executor_data_model::permission::peer::CanManagePeers;

    use super::*;

    /// Set or clear emergency lane relay validators when the caller is authorised or during genesis.
    pub fn visit_set_lane_relay_emergency_validators<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetLaneRelayEmergencyValidators,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManagePeers.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        deny!(executor, "Can't set lane relay emergency validators");
    }
}

/// Permission-checked visitors for `SoraFS` registry and pricing instructions.
pub mod sorafs {
    use iroha_executor_data_model::permission::sorafs::{
        CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
        CanDeclareSorafsCapacity, CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder,
        CanRegisterSorafsPin, CanRegisterSorafsProviderOwner, CanRetireSorafsPin,
        CanSetSorafsPricing, CanSubmitSorafsTelemetry, CanUnregisterSorafsProviderOwner,
        CanUpsertSorafsProviderCredit,
    };

    use super::*;

    /// Register a `SoraFS` pin manifest when permitted (or during genesis).
    pub fn visit_register_pin_manifest<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterPinManifest,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRegisterSorafsPin.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register SoraFS pin manifest");
    }

    /// Approve a pending `SoraFS` pin manifest when permitted.
    pub fn visit_approve_pin_manifest<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ApprovePinManifest,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanApproveSorafsPin.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't approve SoraFS pin manifest");
    }

    /// Retire a `SoraFS` pin manifest when permitted.
    pub fn visit_retire_pin_manifest<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RetirePinManifest,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRetireSorafsPin.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't retire SoraFS pin manifest");
    }

    /// Bind or update a `SoraFS` manifest alias when permitted.
    pub fn visit_bind_manifest_alias<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &BindManifestAlias,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanBindSorafsAlias.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't bind SoraFS manifest alias");
    }

    /// Register a capacity declaration when permitted.
    pub fn visit_register_capacity_declaration<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterCapacityDeclaration,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanDeclareSorafsCapacity.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register SoraFS capacity declaration");
    }

    /// Record a capacity telemetry snapshot when permitted.
    pub fn visit_record_capacity_telemetry<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RecordCapacityTelemetry,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanSubmitSorafsTelemetry.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't record SoraFS capacity telemetry");
    }

    /// File a capacity dispute when permitted.
    pub fn visit_register_capacity_dispute<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterCapacityDispute,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanFileSorafsCapacityDispute.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't file SoraFS capacity dispute");
    }

    /// Issue a replication order when permitted.
    pub fn visit_issue_replication_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &IssueReplicationOrder,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanIssueSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't issue SoraFS replication order");
    }

    /// Complete a replication order when permitted.
    pub fn visit_complete_replication_order<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &CompleteReplicationOrder,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanCompleteSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't complete SoraFS replication order");
    }

    /// Record a replication receipt when permitted.
    pub fn visit_record_replication_receipt<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RecordReplicationReceipt,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanCompleteSorafsReplicationOrder
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't record SoraFS replication receipt");
    }

    /// Register or update the owner binding for a `SoraFS` provider when permitted.
    pub fn visit_register_provider_owner<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RegisterProviderOwner,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRegisterSorafsProviderOwner
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register SoraFS provider owner binding");
    }

    /// Remove the owner binding for a `SoraFS` provider when permitted.
    pub fn visit_unregister_provider_owner<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &UnregisterProviderOwner,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanUnregisterSorafsProviderOwner
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister SoraFS provider owner binding");
    }

    /// Update the `SoraFS` pricing schedule when permitted.
    pub fn visit_set_pricing_schedule<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetPricingSchedule,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanSetSorafsPricing.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't set SoraFS pricing schedule");
    }

    /// Upsert a `SoraFS` provider credit record when permitted.
    pub fn visit_upsert_provider_credit<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &UpsertProviderCredit,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanUpsertSorafsProviderCredit.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't upsert SoraFS provider credit");
    }
}

/// Permission-checked visitors for domain lifecycle instructions.
pub mod domain {
    use iroha_executor_data_model::permission::domain::{
        CanModifyDomainMetadata, CanRegisterDomain, CanUnregisterDomain,
    };
    use iroha_smart_contract::data_model::domain::DomainId;

    use super::*;
    use crate::permission::{
        account::is_account_owner, domain::is_domain_owner, revoke_permissions,
    };

    /// Registers a domain when genesis or a caller with the register-domain permission requests it.
    pub fn visit_register_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Domain>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanRegisterDomain.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't register domain");
    }

    /// Unregisters a domain after checking that the caller governs the domain or holds the revoke permission.
    pub fn visit_unregister_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Domain>,
    ) {
        let domain_id = isi.object();

        if executor.context().curr_block.is_genesis()
            || match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
                Err(err) => deny!(executor, err),
                Ok(is_domain_owner) => is_domain_owner,
            }
            || {
                let can_unregister_domain_token = CanUnregisterDomain {
                    domain: domain_id.clone(),
                };
                can_unregister_domain_token
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_domain_associated(permission, domain_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }

            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister domain");
    }

    /// Transfers domain ownership when the caller owns the source account or domain.
    pub fn visit_transfer_domain<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Account, DomainId, Account>,
    ) {
        let source_id = isi.source();
        let domain_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(source_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        deny!(executor, "Can't transfer domain of another account");
    }

    /// Sets domain metadata after verifying the caller's authority.
    pub fn visit_set_domain_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Domain>,
    ) {
        let domain_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_domain_token = CanModifyDomainMetadata {
            domain: domain_id.clone(),
        };
        if can_set_key_value_in_domain_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't set key value in domain metadata");
    }

    /// Removes domain metadata when the caller holds the relevant modify permission.
    pub fn visit_remove_domain_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Domain>,
    ) {
        let domain_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_domain_owner(domain_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_domain_token = CanModifyDomainMetadata {
            domain: domain_id.clone(),
        };
        if can_remove_key_value_in_domain_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't remove key value in domain metadata");
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn is_permission_domain_associated(
        permission: &Permission,
        domain_id: &DomainId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterDomain(permission) => &permission.domain == domain_id,
            AnyPermission::CanModifyDomainMetadata(permission) => &permission.domain == domain_id,
            AnyPermission::CanRegisterAccount(permission) => &permission.domain == domain_id,
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanModifyAssetDefinitionMetadata(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanMintAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanBurnAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanTransferAssetWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanModifyAssetMetadataWithDefinition(permission) => {
                permission.asset_definition.domain() == domain_id
            }
            AnyPermission::CanMintAsset(permission) => {
                permission.asset.definition().domain() == domain_id
            }
            AnyPermission::CanBurnAsset(permission) => {
                permission.asset.definition().domain() == domain_id
            }
            AnyPermission::CanTransferAsset(permission) => {
                permission.asset.definition().domain() == domain_id
            }
            AnyPermission::CanModifyAssetMetadata(permission) => {
                permission.asset.definition().domain() == domain_id
            }
            AnyPermission::CanRegisterNft(permission) => &permission.domain == domain_id,
            AnyPermission::CanUnregisterNft(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanTransferNft(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanModifyNftMetadata(permission) => permission.nft.domain() == domain_id,
            AnyPermission::CanUseFeeSponsor(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_) => false,
        }
    }
}

/// Permission-checked visitors for account management instructions.
pub mod account {
    use iroha_executor_data_model::permission::account::{
        CanModifyAccountMetadata, CanRegisterAccount, CanUnregisterAccount,
    };

    use super::*;
    use crate::permission::{account::is_account_owner, revoke_permissions};

    /// Registers an account when the caller owns the domain or holds the registration permission.
    pub fn visit_register_account<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Account>,
    ) {
        let domain_id = isi.object().domain();

        match crate::permission::domain::is_domain_owner(
            domain_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_register_account_in_domain = CanRegisterAccount {
            domain: domain_id.clone(),
        };
        if can_register_account_in_domain
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't register account in a domain owned by another account"
        );
    }

    /// Unregisters an account when the caller owns it or has the unregister permission.
    pub fn visit_unregister_account<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Account>,
    ) {
        let account_id = isi.object();

        if executor.context().curr_block.is_genesis()
            || is_account_owner(account_id, &executor.context().authority, executor.host())
            || {
                let can_unregister_user_account = CanUnregisterAccount {
                    account: account_id.clone(),
                };
                can_unregister_user_account
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_account_associated(permission, account_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }

            execute!(executor, isi);
        }
        deny!(executor, "Can't unregister another account");
    }

    /// Sets account metadata after verifying ownership or the metadata permission.
    pub fn visit_set_account_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Account>,
    ) {
        let account_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(account_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let can_set_key_value_in_user_account_token = CanModifyAccountMetadata {
            account: account_id.clone(),
        };
        if can_set_key_value_in_user_account_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the metadata of another account"
        );
    }

    /// Removes account metadata provided the caller is authorised.
    pub fn visit_remove_account_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Account>,
    ) {
        let account_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(account_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let can_remove_key_value_in_user_account_token = CanModifyAccountMetadata {
            account: account_id.clone(),
        };
        if can_remove_key_value_in_user_account_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the metadata of another account"
        );
    }

    pub(crate) fn is_permission_account_associated(
        permission: &Permission,
        account_id: &AccountId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterAccount(permission) => permission.account == *account_id,
            AnyPermission::CanModifyAccountMetadata(permission) => {
                permission.account == *account_id
            }
            AnyPermission::CanMintAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanBurnAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanTransferAsset(permission) => permission.asset.account() == account_id,
            AnyPermission::CanModifyAssetMetadata(permission) => {
                permission.asset.account() == account_id
            }
            AnyPermission::CanUseFeeSponsor(permission) => permission.sponsor == *account_id,
            AnyPermission::CanRegisterTrigger(permission) => permission.authority == *account_id,
            AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanModifyAssetDefinitionMetadata(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanModifyAssetMetadataWithDefinition(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_) => false,
        }
    }
}

/// Permission-checked visitors for asset definition instructions.
pub mod asset_definition {
    use iroha_executor_data_model::permission::asset_definition::{
        CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition,
    };
    use iroha_smart_contract::data_model::asset::AssetDefinitionId;

    use super::*;
    use crate::permission::{
        account::is_account_owner, asset_definition::is_asset_definition_owner, revoke_permissions,
    };

    /// Registers an asset definition.
    pub fn visit_register_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<AssetDefinition>,
    ) {
        execute!(executor, isi);
    }

    /// Unregisters an asset definition after confirming ownership or revoke permission.
    pub fn visit_unregister_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();

        if executor.context().curr_block.is_genesis()
            || match is_asset_definition_owner(
                asset_definition_id,
                &executor.context().authority,
                executor.host(),
            ) {
                Err(err) => deny!(executor, err),
                Ok(is_asset_definition_owner) => is_asset_definition_owner,
            }
            || {
                let can_unregister_asset_definition_token = CanUnregisterAssetDefinition {
                    asset_definition: asset_definition_id.clone(),
                };
                can_unregister_asset_definition_token
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_asset_definition_associated(permission, asset_definition_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }

            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister asset definition owned by another account"
        );
    }

    /// Transfers an asset definition when the caller owns the source account or definition.
    pub fn visit_transfer_asset_definition<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Account, AssetDefinitionId, Account>,
    ) {
        let source_id = isi.source();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(source_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't transfer asset definition of another account"
        );
    }

    /// Sets metadata on an asset definition provided the caller is authorised.
    pub fn visit_set_asset_definition_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_definition_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_asset_definition_token = CanModifyAssetDefinitionMetadata {
            asset_definition: asset_definition_id.clone(),
        };
        if can_set_key_value_in_asset_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the asset definition metadata created by another account"
        );
    }

    /// Removes metadata from an asset definition when the caller holds modify rights.
    pub fn visit_remove_asset_definition_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<AssetDefinition>,
    ) {
        let asset_definition_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_definition_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_asset_definition_token = CanModifyAssetDefinitionMetadata {
            asset_definition: asset_definition_id.clone(),
        };
        if can_remove_key_value_in_asset_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the asset definition metadata created by another account"
        );
    }

    pub(crate) fn is_permission_asset_definition_associated(
        permission: &Permission,
        asset_definition_id: &AssetDefinitionId,
    ) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterAssetDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanModifyAssetDefinitionMetadata(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanMintAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanBurnAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanTransferAssetWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanModifyAssetMetadataWithDefinition(permission) => {
                &permission.asset_definition == asset_definition_id
            }
            AnyPermission::CanMintAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanBurnAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanTransferAsset(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanModifyAssetMetadata(permission) => {
                permission.asset.definition() == asset_definition_id
            }
            AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanUnregisterTrigger(_)
            | AnyPermission::CanExecuteTrigger(_)
            | AnyPermission::CanModifyTrigger(_)
            | AnyPermission::CanModifyTriggerMetadata(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanUseFeeSponsor(_) => false,
        }
    }
}

/// Permission-checked visitors for asset operations.
pub mod asset {
    use iroha_executor_data_model::permission::asset::{
        CanBurnAsset, CanBurnAssetWithDefinition, CanMintAsset, CanMintAssetWithDefinition,
        CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition, CanTransferAsset,
        CanTransferAssetWithDefinition,
    };
    use iroha_smart_contract::data_model::isi::{
        BuiltInInstruction, RemoveAssetKeyValue, SetAssetKeyValue,
    };
    use norito::NoritoSerialize;

    use super::*;
    use crate::permission::{asset::is_asset_owner, asset_definition::is_asset_definition_owner};

    fn execute_mint_asset<V, Q>(executor: &mut V, isi: &Mint<Q, Asset>)
    where
        V: Execute + Visit + ?Sized,
        Q: Into<Numeric>,
        Mint<Q, Asset>: BuiltInInstruction + NoritoSerialize,
    {
        let asset_id = isi.destination();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_assets_with_definition_token = CanMintAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_mint_assets_with_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        let can_mint_user_asset_token = CanMintAsset {
            asset: asset_id.clone(),
        };
        if can_mint_user_asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't mint assets with definitions registered by other accounts"
        );
    }

    /// Mints numeric assets when the caller owns the definition or has explicit mint permission.
    pub fn visit_mint_asset_numeric<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Mint<Numeric, Asset>,
    ) {
        execute_mint_asset(executor, isi);
    }

    fn execute_burn_asset<V, Q>(executor: &mut V, isi: &Burn<Q, Asset>)
    where
        V: Execute + Visit + ?Sized,
        Q: Into<Numeric>,
        Burn<Q, Asset>: BuiltInInstruction + NoritoSerialize,
    {
        let asset_id = isi.destination();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_burn_assets_with_definition_token = CanBurnAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_burn_assets_with_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        let can_burn_user_asset_token = CanBurnAsset {
            asset: asset_id.clone(),
        };
        if can_burn_user_asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't burn assets from another account");
    }

    /// Burns numeric assets if the caller controls the asset or holds burn permission.
    pub fn visit_burn_asset_numeric<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Burn<Numeric, Asset>,
    ) {
        execute_burn_asset(executor, isi);
    }

    /// Placeholder visitor for asset metadata insertion.
    pub fn visit_set_asset_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetAssetKeyValue,
    ) {
        let asset_id = isi.asset();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let definition_token = CanModifyAssetMetadataWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if definition_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let asset_token = CanModifyAssetMetadata {
            asset: asset_id.clone(),
        };
        if asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set metadata for an asset without ownership or explicit permission"
        );
    }

    /// Modify asset metadata by removing a key if ownership or grants allow it.
    pub fn visit_remove_asset_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveAssetKeyValue,
    ) {
        let asset_id = isi.asset();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let definition_token = CanModifyAssetMetadataWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if definition_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        let asset_token = CanModifyAssetMetadata {
            asset: asset_id.clone(),
        };
        if asset_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove metadata for an asset without ownership or explicit permission"
        );
    }

    /// Transfers numeric assets after verifying ownership or transfer permission.
    pub fn visit_transfer_asset_numeric<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Asset, Numeric, Account>,
    ) {
        let asset_id = isi.source();
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_asset_owner(asset_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_asset_definition_owner(
            asset_id.definition(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_transfer_assets_with_definition_token = CanTransferAssetWithDefinition {
            asset_definition: asset_id.definition().clone(),
        };
        if can_transfer_assets_with_definition_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }
        let can_transfer_user_asset_token = CanTransferAsset {
            asset: asset_id.clone(),
        };
        if can_transfer_user_asset_token.is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(executor, "Can't transfer assets of another account");
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_crypto::{Algorithm, KeyPair};

        use super::*;
        use crate::{
            Execute, Iroha,
            data_model::{
                ValidationFail,
                account::AccountId,
                asset::{AssetDefinitionId, AssetId},
                block::BlockHeader,
                bridge::BridgeReceipt,
                domain::DomainId,
                executor::Result as ExecResult,
                isi::{
                    InstructionBox, RegisterPublicLaneValidator,
                    bridge::RecordBridgeReceipt,
                    repo::{RepoInstructionBox, RepoIsi},
                },
                metadata::Metadata,
                name::Name,
                nexus::LaneId,
                peer::PeerId,
                prelude::{Json, Numeric},
                repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
            },
            prelude::{Context, Visit},
        };

        struct StubExecutor {
            host: Iroha,
            context: Context,
            verdict: ExecResult,
        }

        impl StubExecutor {
            fn new(height: u64) -> (Self, AssetId) {
                let domain: DomainId = "test_domain".parse().expect("valid domain");
                let seed_byte = height.to_le_bytes()[0];
                let keypair = KeyPair::from_seed(vec![seed_byte; 32], Algorithm::Ed25519);
                let account = AccountId::new(keypair.public_key().clone());
                let asset_definition = AssetDefinitionId::new(
                    domain.clone(),
                    "sample_asset".parse::<Name>().expect("valid name"),
                );
                let asset = AssetId::new(asset_definition, account.clone());
                let header = BlockHeader::new(
                    NonZeroU64::new(height).expect("height > 0"),
                    None,
                    None,
                    None,
                    0,
                    0,
                );
                let context = Context {
                    authority: account,
                    curr_block: header,
                };
                (
                    Self {
                        host: Iroha,
                        context,
                        verdict: Ok(()),
                    },
                    asset,
                )
            }
        }

        impl Execute for StubExecutor {
            fn host(&self) -> &Iroha {
                &self.host
            }

            fn context(&self) -> &Context {
                &self.context
            }

            fn context_mut(&mut self) -> &mut Context {
                &mut self.context
            }

            fn verdict(&self) -> &ExecResult {
                &self.verdict
            }

            fn deny(&mut self, reason: ValidationFail) {
                self.verdict = Err(reason);
            }
        }

        impl Visit for StubExecutor {}

        #[test]
        fn set_asset_key_value_genesis_returns_ok() {
            let (mut executor, asset) = StubExecutor::new(1);
            let instruction = SetAssetKeyValue::new(
                asset,
                "tag".parse().expect("valid name"),
                Json::new("value"),
            );

            visit_set_asset_key_value(&mut executor, &instruction);

            assert!(executor.verdict().is_ok());
        }

        #[test]
        fn set_asset_key_value_non_genesis_owner_allows() {
            let (mut executor, asset) = StubExecutor::new(2);
            let instruction = SetAssetKeyValue::new(
                asset.clone(),
                "tag".parse().expect("valid name"),
                Json::new("value"),
            );

            visit_set_asset_key_value(&mut executor, &instruction);

            assert!(
                executor.verdict().is_ok(),
                "asset owner should be able to set metadata outside genesis"
            );
        }

        #[test]
        fn set_asset_key_value_non_owner_denied() {
            let (mut executor, asset) = StubExecutor::new(2);
            let intruder_key = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
            let intruder = AccountId::new(intruder_key.public_key().clone());
            executor.context_mut().authority = intruder;
            let instruction = SetAssetKeyValue::new(
                asset.clone(),
                "tag".parse().expect("valid name"),
                Json::new("value"),
            );

            visit_set_asset_key_value(&mut executor, &instruction);

            assert!(
                matches!(
                    executor.verdict(),
                    Err(ValidationFail::InstructionFailed(_) | ValidationFail::NotPermitted(_))
                ),
                "expected denial for non-owner, got {:?}",
                executor.verdict()
            );
        }

        #[test]
        fn visit_instruction_dispatches_register_peer_with_pop() {
            let (mut executor, _) = StubExecutor::new(1);
            let peer_keypair = KeyPair::from_seed(vec![42; 32], Algorithm::Ed25519);
            let peer_id = PeerId::from(peer_keypair.public_key().clone());
            let instruction = RegisterPeerWithPop::new(peer_id, vec![1, 2, 3]);
            let instruction_box: InstructionBox = instruction.into();

            visit_instruction(&mut executor, &instruction_box);

            assert!(
                executor.verdict().is_ok(),
                "register peer with pop should succeed during genesis"
            );
        }

        #[test]
        fn visit_instruction_dispatches_register_public_lane_validator() {
            let (mut executor, asset) = StubExecutor::new(1);
            let validator = asset.account().clone();
            let instruction = RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator.clone(),
                validator,
                Numeric::from(1u64),
                Metadata::default(),
            );
            let instruction_box: InstructionBox = instruction.into();

            visit_instruction(&mut executor, &instruction_box);

            assert!(
                executor.verdict().is_ok(),
                "register public-lane validator should succeed during genesis"
            );
        }

        #[test]
        fn visit_instruction_dispatches_record_bridge_receipt() {
            let (mut executor, _) = StubExecutor::new(1);
            let receipt = BridgeReceipt {
                lane: LaneId::new(2),
                direction: b"mint".to_vec(),
                source_tx: [0x11; 32],
                dest_tx: None,
                proof_hash: [0x22; 32],
                amount: 1,
                asset_id: b"wBTC#btc".to_vec(),
                recipient: b"alice@main".to_vec(),
            };
            let instruction = RecordBridgeReceipt::new(receipt);
            let instruction_box: InstructionBox = instruction.into();

            visit_instruction(&mut executor, &instruction_box);

            assert!(
                executor.verdict().is_ok(),
                "record bridge receipt should succeed during genesis"
            );
        }

        #[test]
        fn visit_instruction_dispatches_repo_instruction_box() {
            let (mut executor, _) = StubExecutor::new(1);
            let domain: DomainId = "wonderland".parse().expect("valid domain");
            let counterparty_keypair = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
            let counterparty = AccountId::new(counterparty_keypair.public_key().clone());
            let cash_def =
                AssetDefinitionId::new(domain.clone(), "cash".parse::<Name>().expect("valid name"));
            let collateral_def =
                AssetDefinitionId::new(domain, "collateral".parse::<Name>().expect("valid name"));
            let agreement_id: RepoAgreementId = "repo_dispatch".parse().expect("repo id");
            let repo_instruction = RepoIsi::new(
                agreement_id,
                executor.context().authority.clone(),
                counterparty,
                None,
                RepoCashLeg {
                    asset_definition_id: cash_def,
                    quantity: Numeric::from(1u32),
                },
                RepoCollateralLeg::new(collateral_def, Numeric::from(1u32)),
                0,
                1,
                RepoGovernance::with_defaults(0, 0),
            );
            let instruction_box: InstructionBox = RepoInstructionBox::from(repo_instruction).into();

            visit_instruction(&mut executor, &instruction_box);

            assert!(
                executor.verdict().is_ok(),
                "repo instruction box should dispatch through executor"
            );
        }

        #[test]
        fn remove_asset_key_value_non_genesis_owner_allows() {
            let (mut executor, asset) = StubExecutor::new(2);
            let instruction =
                RemoveAssetKeyValue::new(asset.clone(), "tag".parse().expect("valid name"));

            visit_remove_asset_key_value(&mut executor, &instruction);

            assert!(
                executor.verdict().is_ok(),
                "asset owner should be able to remove metadata outside genesis"
            );
        }

        #[test]
        fn remove_asset_key_value_non_owner_denied() {
            let (mut executor, asset) = StubExecutor::new(2);
            let intruder_key = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
            let intruder = AccountId::new(intruder_key.public_key().clone());
            executor.context_mut().authority = intruder;
            let instruction =
                RemoveAssetKeyValue::new(asset.clone(), "tag".parse().expect("valid name"));

            visit_remove_asset_key_value(&mut executor, &instruction);

            assert!(
                matches!(
                    executor.verdict(),
                    Err(ValidationFail::InstructionFailed(_) | ValidationFail::NotPermitted(_))
                ),
                "expected denial for non-owner, got {:?}",
                executor.verdict()
            );
        }
    }
}

/// Permission-checked visitors for non-fungible asset instructions.
pub mod nft {
    use iroha_executor_data_model::permission::nft::{
        CanModifyNftMetadata, CanRegisterNft, CanTransferNft, CanUnregisterNft,
    };
    use norito::NoritoSerialize;

    use super::*;
    use crate::{
        data_model::isi::BuiltInInstruction,
        permission::{
            account::is_account_owner,
            nft::{is_nft_full_owner, is_nft_weak_owner},
            revoke_permissions,
        },
    };

    /// Registers an NFT when the caller owns the domain or has the registration permission.
    pub fn visit_register_nft<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &Register<Nft>) {
        let domain_id = isi.object().id().domain();

        match crate::permission::domain::is_domain_owner(
            domain_id,
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_register_nft_in_domain_token = CanRegisterNft {
            domain: domain_id.clone(),
        };
        if can_register_nft_in_domain_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't register NFT in a domain owned by another account"
        );
    }

    /// Unregisters an NFT once the caller proves ownership or holds the revoke token.
    pub fn visit_unregister_nft<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Nft>,
    ) {
        let nft_id = isi.object();

        if executor.context().curr_block.is_genesis()
            || match is_nft_full_owner(nft_id, &executor.context().authority, executor.host()) {
                Err(err) => deny!(executor, err),
                Ok(is_owner) => is_owner,
            }
            || {
                let can_unregister_token = CanUnregisterNft {
                    nft: nft_id.clone(),
                };
                can_unregister_token.is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_nft_associated(permission, nft_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }

            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister NFT in a domain owned by another account"
        );
    }

    fn is_permission_nft_associated(permission: &Permission, nft_id: &NftId) -> bool {
        use AnyPermission::*;
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            CanUnregisterNft(permission) => &permission.nft == nft_id,
            CanTransferNft(permission) => &permission.nft == nft_id,
            CanModifyNftMetadata(permission) => &permission.nft == nft_id,
            _ => false,
        }
    }

    /// Transfers an NFT after verifying the caller's ownership or transfer permission.
    pub fn visit_transfer_nft<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Transfer<Account, NftId, Account>,
    ) {
        let source_id = isi.source();
        let nft_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if is_account_owner(source_id, &executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }
        match is_nft_weak_owner(nft_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_transfer_nft_token = CanTransferNft {
            nft: nft_id.clone(),
        };
        if can_transfer_nft_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't transfer NFT of another account");
    }

    /// Sets NFT metadata when the caller is authorised to mutate it.
    pub fn visit_set_nft_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Nft>,
    ) {
        execute_modify_nft_key_value(executor, isi.object(), isi);
    }

    /// Removes NFT metadata once appropriate permissions are confirmed.
    pub fn visit_remove_nft_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Nft>,
    ) {
        execute_modify_nft_key_value(executor, isi.object(), isi);
    }

    fn execute_modify_nft_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        nft_id: &NftId,
        isi: &(impl BuiltInInstruction + NoritoSerialize),
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match crate::permission::domain::is_domain_owner(
            nft_id.domain(),
            &executor.context().authority,
            executor.host(),
        ) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }

        let can_modify_nft_token = CanModifyNftMetadata {
            nft: nft_id.clone(),
        };
        if can_modify_nft_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't modify NFT from domain owned by another account"
        );
    }
}

/// Permission-checked visitors for network parameter updates.
pub mod parameter {
    use iroha_executor_data_model::permission::parameter::CanSetParameters;

    use super::*;

    /// Applies a network parameter change when genesis or a parameter manager invokes it.
    pub fn visit_set_parameter<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &SetParameter) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanSetParameters.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set executor configuration parameters without permission"
        );
    }
}

/// Permission-checked visitors for role registration and mutation.
pub mod role {
    use iroha_executor_data_model::permission::role::CanManageRoles;
    use iroha_smart_contract::{Iroha, data_model::role::Role};

    use super::*;

    macro_rules! impl_execute_grant_revoke_account_role {
        ($executor:ident, $isi:ident) => {
            let role_id = $isi.object();

            if $executor.context().curr_block.is_genesis()
                || find_account_roles($executor.context().authority.clone(), $executor.host())
                    .any(|authority_role_id| authority_role_id == *role_id)
            {
                execute!($executor, $isi)
            }

            deny!($executor, "Can't grant or revoke role to another account");
        };
    }

    macro_rules! impl_execute_grant_revoke_role_permission {
        ($executor:ident, $isi:ident, $method:ident, $isi_type:ty) => {
            let role_id = $isi.destination().clone();
            let permission = $isi.object();

            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !$executor.context().curr_block.is_genesis() {
                    if !find_account_roles($executor.context().authority.clone(), $executor.host())
                        .any(|authority_role_id| authority_role_id == role_id)
                    {
                        deny!($executor, "Can't modify role");
                    }

                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        &$executor.context().authority,
                        $executor.context(),
                        $executor.host(),
                    ) {
                        deny!($executor, error);
                    }
                }

                let isi = &<$isi_type>::role_permission(any_permission, role_id);
                execute!($executor, isi);
            }

            deny!(
                $executor,
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            );
        };
    }

    fn find_account_roles(account_id: AccountId, host: &Iroha) -> impl Iterator<Item = RoleId> {
        use iroha_smart_contract::DebugExpectExt as _;

        host.query(FindRolesByAccountId::new(account_id))
            .execute()
            .dbg_expect("INTERNAL BUG: `FindRolesByAccountId` must never fail")
            .map(|role| role.dbg_expect("Failed to get role from cursor"))
    }

    /// Registers a role and seeds its permissions when the caller controls role governance.
    pub fn visit_register_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Role>,
    ) {
        let role = isi.object();
        let grant_role = &Grant::account_role(role.id().clone(), role.grant_to().clone());
        let mut new_role = Role::new(role.id().clone(), role.grant_to().clone());

        // Exception for multisig roles
        {
            use crate::permission::domain::is_domain_owner;

            const DELIMITER: char = '/';
            const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";

            fn multisig_home_domain_from(role: &RoleId) -> Option<DomainId> {
                role.name()
                    .as_ref()
                    .strip_prefix(&format!("{MULTISIG_SIGNATORY}{DELIMITER}"))?
                    .split_once(DELIMITER)
                    .and_then(|(domain, _)| domain.parse().ok())
            }

            if role.id().name().as_ref().starts_with(MULTISIG_SIGNATORY) {
                let Some(home_domain) = multisig_home_domain_from(role.id()) else {
                    deny!(executor, "violates multisig role name format")
                };
                if is_domain_owner(&home_domain, &executor.context().authority, executor.host())
                    .unwrap_or_default()
                {
                    deny!(
                        executor,
                        "reserved multisig role names may not be registered"
                    );
                }
                deny!(
                    executor,
                    "only the domain owner can register multisig roles"
                )
            }
        }

        for permission in role.inner().permissions() {
            iroha_smart_contract::log::debug!(&format!("Checking `{permission:?}`"));

            let Ok(any_permission) = AnyPermission::try_from(permission) else {
                deny!(
                    executor,
                    ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
                );
            };
            if !executor.context().curr_block.is_genesis()
                && let Err(error) = crate::permission::ValidateGrantRevoke::validate_grant(
                    &any_permission,
                    role.grant_to(),
                    executor.context(),
                    executor.host(),
                )
            {
                deny!(executor, error);
            }
            new_role = new_role.add_permission(any_permission);
        }

        if executor.context().curr_block.is_genesis()
            || CanManageRoles.is_owned_by(&executor.context().authority, executor.host())
        {
            let isi = &Register::role(new_role);
            if let Err(err) = executor.host().submit(isi) {
                deny!(executor, err);
            }

            execute!(executor, grant_role);
        }

        deny!(executor, "Can't register role");
    }

    /// Unregisters a role if genesis or a role manager invokes the instruction.
    pub fn visit_unregister_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Role>,
    ) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanManageRoles.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't unregister role");
    }

    /// Grants a role to an account when the caller is authorised to manage roles.
    pub fn visit_grant_account_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<RoleId, Account>,
    ) {
        impl_execute_grant_revoke_account_role!(executor, isi);
    }

    /// Revokes a role from an account after verifying role management permissions.
    pub fn visit_revoke_account_role<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<RoleId, Account>,
    ) {
        impl_execute_grant_revoke_account_role!(executor, isi);
    }

    /// Grants a permission to a role after ensuring the caller may mutate role permissions.
    pub fn visit_grant_role_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<Permission, Role>,
    ) {
        impl_execute_grant_revoke_role_permission!(executor, isi, validate_grant, Grant<Permission, Role>);
    }

    /// Revokes a permission from a role once the caller passes the permission gate.
    pub fn visit_revoke_role_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<Permission, Role>,
    ) {
        impl_execute_grant_revoke_role_permission!(executor, isi, validate_revoke, Revoke<Permission, Role>);
    }
}

/// Permission-checked visitors for trigger lifecycle and metadata instructions.
pub mod trigger {
    use iroha_executor_data_model::permission::trigger::{
        CanExecuteTrigger, CanModifyTrigger, CanModifyTriggerMetadata, CanRegisterTrigger,
        CanUnregisterTrigger,
    };
    use iroha_smart_contract::data_model::trigger::Trigger;

    use super::*;
    use crate::permission::{revoke_permissions, trigger::is_trigger_owner};

    /// Registers a trigger when the caller is genesis, the trigger authority, or holds the grant token.
    pub fn visit_register_trigger<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Register<Trigger>,
    ) {
        let trigger = isi.object();
        let is_genesis = executor.context().curr_block.is_genesis();

        if is_genesis || trigger.action().authority() == &executor.context().authority || {
            let can_register_user_trigger_token = CanRegisterTrigger {
                authority: isi.object().action().authority().clone(),
            };
            can_register_user_trigger_token
                .is_owned_by(&executor.context().authority, executor.host())
        } {
            // Execute via core `Execute` implementation to ensure all invariants
            // and state mutations happen atomically after permission gating.
            execute!(executor, isi);
        }
        deny!(executor, "Can't register trigger owned by another account");
    }

    /// Unregisters a trigger once the caller is authorised and revokes related permissions.
    pub fn visit_unregister_trigger<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Unregister<Trigger>,
    ) {
        let trigger_id = isi.object();

        if executor.context().curr_block.is_genesis()
            || match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
                Err(err) => deny!(executor, err),
                Ok(is_trigger_owner) => is_trigger_owner,
            }
            || {
                let can_unregister_user_trigger_token = CanUnregisterTrigger {
                    trigger: trigger_id.clone(),
                };
                can_unregister_user_trigger_token
                    .is_owned_by(&executor.context().authority, executor.host())
            }
        {
            let err = revoke_permissions(executor, |permission| {
                is_permission_trigger_associated(permission, trigger_id)
            });
            if let Err(err) = err {
                deny!(executor, err);
            }

            execute!(executor, isi);
        }
        deny!(
            executor,
            "Can't unregister trigger owned by another account"
        );
    }

    /// Increments the trigger repetition counter when the caller controls the trigger.
    pub fn visit_mint_trigger_repetitions<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Mint<u32, Trigger>,
    ) {
        let trigger_id = isi.destination();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_user_trigger_token = CanModifyTrigger {
            trigger: trigger_id.clone(),
        };
        if can_mint_user_trigger_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't mint execution count for trigger owned by another account"
        );
    }

    /// Decrements the trigger repetition counter for authorised callers.
    pub fn visit_burn_trigger_repetitions<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Burn<u32, Trigger>,
    ) {
        let trigger_id = isi.destination();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_mint_user_trigger_token = CanModifyTrigger {
            trigger: trigger_id.clone(),
        };
        if can_mint_user_trigger_token.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't burn execution count for trigger owned by another account"
        );
    }

    /// Executes a trigger when the caller is the owner or holds the execute permission.
    pub fn visit_execute_trigger<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &ExecuteTrigger,
    ) {
        let trigger_id = isi.trigger();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        let authority = &executor.context().authority;
        match is_trigger_owner(trigger_id, authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_execute_trigger_token = CanExecuteTrigger {
            trigger: trigger_id.clone(),
        };
        if can_execute_trigger_token.is_owned_by(authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't execute trigger owned by another account");
    }

    /// Sets metadata for a trigger when the caller may mutate its key-value store.
    pub fn visit_set_trigger_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &SetKeyValue<Trigger>,
    ) {
        let trigger_id = isi.object();

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_set_key_value_in_user_trigger_token = CanModifyTriggerMetadata {
            trigger: trigger_id.clone(),
        };
        if can_set_key_value_in_user_trigger_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't set value to the metadata of another trigger"
        );
    }

    /// Removes trigger metadata after verifying the caller may modify it.
    pub fn visit_remove_trigger_key_value<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RemoveKeyValue<Trigger>,
    ) {
        let trigger_id = isi.object();
        let isi = RemoveKeyValueBox::from(isi.clone());
        let isi = &isi;

        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        match is_trigger_owner(trigger_id, &executor.context().authority, executor.host()) {
            Err(err) => deny!(executor, err),
            Ok(true) => execute!(executor, isi),
            Ok(false) => {}
        }
        let can_remove_key_value_in_trigger_token = CanModifyTriggerMetadata {
            trigger: trigger_id.clone(),
        };
        if can_remove_key_value_in_trigger_token
            .is_owned_by(&executor.context().authority, executor.host())
        {
            execute!(executor, isi);
        }

        deny!(
            executor,
            "Can't remove value from the metadata of another trigger"
        );
    }

    fn is_permission_trigger_associated(permission: &Permission, trigger_id: &TriggerId) -> bool {
        let Ok(permission) = AnyPermission::try_from(permission) else {
            return false;
        };
        match permission {
            AnyPermission::CanUnregisterTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanExecuteTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanModifyTrigger(permission) => &permission.trigger == trigger_id,
            AnyPermission::CanModifyTriggerMetadata(permission) => {
                &permission.trigger == trigger_id
            }
            AnyPermission::CanRegisterTrigger(_)
            | AnyPermission::CanManagePeers(_)
            | AnyPermission::CanRegisterDomain(_)
            | AnyPermission::CanUnregisterDomain(_)
            | AnyPermission::CanModifyDomainMetadata(_)
            | AnyPermission::CanRegisterAccount(_)
            | AnyPermission::CanUnregisterAccount(_)
            | AnyPermission::CanModifyAccountMetadata(_)
            | AnyPermission::CanUnregisterAssetDefinition(_)
            | AnyPermission::CanModifyAssetDefinitionMetadata(_)
            | AnyPermission::CanModifyAssetMetadataWithDefinition(_)
            | AnyPermission::CanMintAssetWithDefinition(_)
            | AnyPermission::CanBurnAssetWithDefinition(_)
            | AnyPermission::CanTransferAssetWithDefinition(_)
            | AnyPermission::CanMintAsset(_)
            | AnyPermission::CanBurnAsset(_)
            | AnyPermission::CanModifyAssetMetadata(_)
            | AnyPermission::CanTransferAsset(_)
            | AnyPermission::CanSetParameters(_)
            | AnyPermission::CanManageRoles(_)
            | AnyPermission::CanRegisterNft(_)
            | AnyPermission::CanUnregisterNft(_)
            | AnyPermission::CanTransferNft(_)
            | AnyPermission::CanModifyNftMetadata(_)
            | AnyPermission::CanUpgradeExecutor(_)
            | AnyPermission::CanRegisterSmartContractCode(_)
            | AnyPermission::CanRegisterSorafsPin(_)
            | AnyPermission::CanApproveSorafsPin(_)
            | AnyPermission::CanRetireSorafsPin(_)
            | AnyPermission::CanBindSorafsAlias(_)
            | AnyPermission::CanDeclareSorafsCapacity(_)
            | AnyPermission::CanSubmitSorafsTelemetry(_)
            | AnyPermission::CanFileSorafsCapacityDispute(_)
            | AnyPermission::CanIssueSorafsReplicationOrder(_)
            | AnyPermission::CanCompleteSorafsReplicationOrder(_)
            | AnyPermission::CanSetSorafsPricing(_)
            | AnyPermission::CanUpsertSorafsProviderCredit(_)
            | AnyPermission::CanRegisterSorafsProviderOwner(_)
            | AnyPermission::CanUnregisterSorafsProviderOwner(_)
            | AnyPermission::CanIngestSoranetPrivacy(_)
            | AnyPermission::CanPublishSpaceDirectoryManifest(_)
            | AnyPermission::CanUseFeeSponsor(_) => false,
        }
    }

    #[cfg(test)]
    mod tests {
        use core::str::FromStr as _;

        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_executor_data_model::permission::{
            asset::{CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition},
            nexus::CanUseFeeSponsor,
            sorafs::{
                CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
                CanDeclareSorafsCapacity, CanFileSorafsCapacityDispute,
                CanIssueSorafsReplicationOrder, CanRegisterSorafsPin,
                CanRegisterSorafsProviderOwner, CanRetireSorafsPin, CanSetSorafsPricing,
                CanSubmitSorafsTelemetry, CanUnregisterSorafsProviderOwner,
                CanUpsertSorafsProviderCredit,
            },
            soranet::CanIngestSoranetPrivacy,
        };

        use super::*;
        use crate::data_model::{
            account::AccountId,
            asset::{AssetDefinitionId, AssetId},
            domain::DomainId,
        };

        fn sample_account_id(seed: u8, _domain_id: &DomainId) -> AccountId {
            let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            AccountId::new(keypair.public_key().clone())
        }

        fn sora_permissions() -> Vec<AnyPermission> {
            vec![
                AnyPermission::CanRegisterSorafsPin(CanRegisterSorafsPin),
                AnyPermission::CanApproveSorafsPin(CanApproveSorafsPin),
                AnyPermission::CanRetireSorafsPin(CanRetireSorafsPin),
                AnyPermission::CanBindSorafsAlias(CanBindSorafsAlias),
                AnyPermission::CanDeclareSorafsCapacity(CanDeclareSorafsCapacity),
                AnyPermission::CanSubmitSorafsTelemetry(CanSubmitSorafsTelemetry),
                AnyPermission::CanFileSorafsCapacityDispute(CanFileSorafsCapacityDispute),
                AnyPermission::CanIssueSorafsReplicationOrder(CanIssueSorafsReplicationOrder),
                AnyPermission::CanCompleteSorafsReplicationOrder(CanCompleteSorafsReplicationOrder),
                AnyPermission::CanSetSorafsPricing(CanSetSorafsPricing),
                AnyPermission::CanUpsertSorafsProviderCredit(CanUpsertSorafsProviderCredit),
                AnyPermission::CanRegisterSorafsProviderOwner(CanRegisterSorafsProviderOwner),
                AnyPermission::CanUnregisterSorafsProviderOwner(CanUnregisterSorafsProviderOwner),
                AnyPermission::CanIngestSoranetPrivacy(CanIngestSoranetPrivacy),
            ]
        }

        #[test]
        fn asset_metadata_permissions_not_trigger_associated() {
            let trigger_id =
                TriggerId::from_str("metadata_cleanup").expect("trigger id must be valid");
            let domain_id = DomainId::from_str("test").expect("domain id must be valid");
            let asset_definition_id = AssetDefinitionId::from_str("token#test")
                .expect("asset definition id must be valid");
            let account_id = sample_account_id(0x11, &domain_id);
            let asset_id = AssetId::new(asset_definition_id.clone(), account_id);

            let permission = Permission::from(AnyPermission::CanModifyAssetMetadataWithDefinition(
                CanModifyAssetMetadataWithDefinition {
                    asset_definition: asset_definition_id,
                },
            ));
            assert!(
                !is_permission_trigger_associated(&permission, &trigger_id),
                "metadata-with-definition permission must not bind to triggers"
            );

            let permission = Permission::from(AnyPermission::CanModifyAssetMetadata(
                CanModifyAssetMetadata { asset: asset_id },
            ));
            assert!(
                !is_permission_trigger_associated(&permission, &trigger_id),
                "asset-metadata permission must not bind to triggers"
            );
        }

        #[test]
        fn sora_permissions_not_trigger_associated() {
            let trigger_id =
                TriggerId::from_str("metadata_cleanup").expect("trigger id must be valid");

            for permission in sora_permissions() {
                let permission = Permission::from(permission);
                assert!(
                    !is_permission_trigger_associated(&permission, &trigger_id),
                    "Sora-specific permissions must not bind to triggers"
                );
            }
        }

        #[test]
        fn sora_permissions_not_domain_account_or_definition_associated() {
            let domain_id = DomainId::from_str("test").expect("domain id must be valid");
            let account_id = sample_account_id(0x12, &domain_id);
            let asset_definition_id = AssetDefinitionId::from_str("token#test")
                .expect("asset definition id must be valid");

            for permission in sora_permissions() {
                let permission = Permission::from(permission);
                assert!(
                    !domain::is_permission_domain_associated(&permission, &domain_id),
                    "Sora-specific permissions must not bind to domains"
                );
                assert!(
                    !account::is_permission_account_associated(&permission, &account_id),
                    "Sora-specific permissions must not bind to accounts"
                );
                assert!(
                    !asset_definition::is_permission_asset_definition_associated(
                        &permission,
                        &asset_definition_id
                    ),
                    "Sora-specific permissions must not bind to asset definitions"
                );
            }
        }

        #[test]
        fn fee_sponsor_permission_associations() {
            let domain_id = DomainId::from_str("test").expect("domain id must be valid");
            let sponsor = sample_account_id(0x21, &domain_id);
            let other_account = sample_account_id(0x22, &domain_id);
            let other_domain = DomainId::from_str("other").expect("domain id must be valid");
            let asset_definition_id = AssetDefinitionId::from_str("token#test")
                .expect("asset definition id must be valid");
            let trigger_id =
                TriggerId::from_str("fee_sponsor_trigger").expect("trigger id must be valid");

            let permission = Permission::from(AnyPermission::CanUseFeeSponsor(CanUseFeeSponsor {
                sponsor: sponsor.clone(),
            }));

            assert!(
                !domain::is_permission_domain_associated(&permission, &domain_id),
                "fee sponsor permission should not bind to domains"
            );
            assert!(
                !domain::is_permission_domain_associated(&permission, &other_domain),
                "fee sponsor permission should not bind to unrelated domains"
            );
            assert!(
                account::is_permission_account_associated(&permission, &sponsor),
                "fee sponsor permission should bind to sponsor account"
            );
            assert!(
                !account::is_permission_account_associated(&permission, &other_account),
                "fee sponsor permission should not bind to unrelated accounts"
            );
            assert!(
                !asset_definition::is_permission_asset_definition_associated(
                    &permission,
                    &asset_definition_id
                ),
                "fee sponsor permission should not bind to asset definitions"
            );
            assert!(
                !is_permission_trigger_associated(&permission, &trigger_id),
                "fee sponsor permission should not bind to triggers"
            );
        }
    }
}

#[cfg(test)]
mod sorafs_permission_tests {
    use core::num::NonZeroU64;

    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        isi::sorafs::{
            ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder, IssueReplicationOrder,
            RecordCapacityTelemetry, RecordReplicationReceipt, RegisterCapacityDeclaration,
            RegisterCapacityDispute, RegisterPinManifest, RegisterProviderOwner, RetirePinManifest,
            SetPricingSchedule, UnregisterProviderOwner, UpsertProviderCredit,
        },
        metadata::Metadata,
        permission::Permission as PermissionObject,
        prelude::ValidationFail,
        sorafs::{
            capacity::{
                CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
                CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
            },
            pin_registry::{
                ChunkerProfileHandle, ManifestAliasBinding, ManifestDigest, PinPolicy,
                ReplicationOrderId, ReplicationReceiptStatus,
            },
            prelude::StorageClass,
            pricing::{PricingScheduleRecord, ProviderCreditRecord},
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
        CanDeclareSorafsCapacity, CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder,
        CanRegisterSorafsPin, CanRegisterSorafsProviderOwner, CanRetireSorafsPin,
        CanSetSorafsPricing, CanSubmitSorafsTelemetry, CanUnregisterSorafsProviderOwner,
        CanUpsertSorafsProviderCredit,
    };

    use super::*;
    use crate::{Iroha, prelude, tests::with_mock_permissions};

    const AUTHORITY_PUBLIC_KEY: &str =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
    const OWNER_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

    fn account_id_from_public_key_hex(hex_literal: &str) -> AccountId {
        let public_key: PublicKey = hex_literal
            .parse()
            .expect("test public key literal should parse");
        AccountId::new(public_key)
    }

    fn authority_account_id() -> AccountId {
        account_id_from_public_key_hex(AUTHORITY_PUBLIC_KEY)
    }

    fn owner_account_id() -> AccountId {
        account_id_from_public_key_hex(OWNER_PUBLIC_KEY)
    }

    #[derive(Debug)]
    struct MockExecutor {
        host: Iroha,
        ctx: prelude::Context,
        verdict: crate::data_model::executor::Result<(), ValidationFail>,
    }

    impl MockExecutor {
        fn new(genesis: bool) -> Self {
            let height = if genesis { 1 } else { 2 };
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("nonzero height"),
                None,
                None,
                None,
                0,
                0,
            );
            let authority = authority_account_id();
            Self {
                host: Iroha,
                ctx: prelude::Context {
                    authority,
                    curr_block: header,
                },
                verdict: Ok(()),
            }
        }
    }

    impl Execute for MockExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }

        fn context(&self) -> &prelude::Context {
            &self.ctx
        }

        fn context_mut(&mut self) -> &mut prelude::Context {
            &mut self.ctx
        }

        fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
            &self.verdict
        }

        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }

    impl Visit for MockExecutor {}

    fn assert_denied_without_permission<T: Clone>(
        instruction: T,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        let mut executor = MockExecutor::new(false);
        visit(&mut executor, &instruction);
        assert!(
            executor.verdict().is_err(),
            "expected denial without permission"
        );
    }

    fn assert_allowed_with_permission<T: Clone>(
        instruction: T,
        permission: PermissionObject,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![permission], || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "expected instruction to be permitted with permission"
            );
        });
    }

    fn sample_provider_id() -> ProviderId {
        ProviderId::new([0xAB; 32])
    }

    fn sample_manifest_digest() -> ManifestDigest {
        ManifestDigest::new([0xCD; 32])
    }

    fn sample_chunker_profile() -> ChunkerProfileHandle {
        ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x12,
        }
    }

    fn sample_pin_policy() -> PinPolicy {
        PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        }
    }

    fn register_pin_manifest() -> RegisterPinManifest {
        RegisterPinManifest::new(
            sample_manifest_digest(),
            sample_chunker_profile(),
            [0xEF; 32],
            sample_pin_policy(),
            1,
            None,
            None,
        )
    }

    fn approve_pin_manifest() -> ApprovePinManifest {
        ApprovePinManifest::new(sample_manifest_digest(), 2, None, None)
    }

    fn retire_pin_manifest() -> RetirePinManifest {
        RetirePinManifest::new(sample_manifest_digest(), 3, None)
    }

    fn bind_manifest_alias() -> BindManifestAlias {
        BindManifestAlias::new(
            sample_manifest_digest(),
            ManifestAliasBinding {
                name: "docs".to_owned(),
                namespace: "sora".to_owned(),
                proof: Vec::new(),
            },
            4,
            5,
        )
    }

    fn register_capacity_declaration() -> RegisterCapacityDeclaration {
        RegisterCapacityDeclaration::new(CapacityDeclarationRecord::new(
            sample_provider_id(),
            vec![0xAA],
            100,
            1,
            1,
            2,
            Metadata::default(),
        ))
    }

    fn record_capacity_telemetry() -> RecordCapacityTelemetry {
        RecordCapacityTelemetry::new(
            CapacityTelemetryRecord::new(
                sample_provider_id(),
                1,
                2,
                100,
                90,
                80,
                1,
                1,
                10_000,
                10_000,
                0,
                0,
                0,
                0,
                0,
            )
            .with_nonce(0),
        )
    }

    fn register_capacity_dispute() -> RegisterCapacityDispute {
        RegisterCapacityDispute::new(CapacityDisputeRecord::new_pending(
            CapacityDisputeId::new([0x01; 32]),
            sample_provider_id(),
            [0x02; 32],
            None,
            0,
            1,
            "desc".to_owned(),
            None,
            CapacityDisputeEvidence {
                digest: [0x03; 32],
                media_type: None,
                uri: None,
                size_bytes: None,
            },
            vec![0x04],
        ))
    }

    fn issue_replication_order() -> IssueReplicationOrder {
        IssueReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), vec![0x22], 1, 2)
    }

    fn complete_replication_order() -> CompleteReplicationOrder {
        CompleteReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), 3)
    }

    fn record_replication_receipt() -> RecordReplicationReceipt {
        RecordReplicationReceipt::new(
            ReplicationOrderId::new([0x11; 32]),
            sample_provider_id(),
            ReplicationReceiptStatus::Accepted,
            0,
            None,
        )
    }

    fn set_pricing_schedule() -> SetPricingSchedule {
        SetPricingSchedule::new(PricingScheduleRecord::launch_default())
    }

    fn upsert_provider_credit() -> UpsertProviderCredit {
        UpsertProviderCredit::new(ProviderCreditRecord::new(
            sample_provider_id(),
            1,
            0,
            0,
            0,
            0,
            0,
            Metadata::default(),
        ))
    }

    fn register_provider_owner() -> RegisterProviderOwner {
        RegisterProviderOwner::new(sample_provider_id(), owner_account_id())
    }

    fn unregister_provider_owner() -> UnregisterProviderOwner {
        UnregisterProviderOwner::new(sample_provider_id())
    }

    macro_rules! sorafs_permission_case {
        ($name:ident, $instruction:expr, $permission:expr, $visitor:path) => {
            #[test]
            fn $name() {
                let instruction = $instruction;
                assert_denied_without_permission(instruction.clone(), $visitor);
                assert_allowed_with_permission(
                    instruction,
                    PermissionObject::from($permission),
                    $visitor,
                );
            }
        };
    }

    sorafs_permission_case!(
        register_pin_manifest_requires_permission,
        register_pin_manifest(),
        CanRegisterSorafsPin,
        sorafs::visit_register_pin_manifest
    );

    sorafs_permission_case!(
        approve_pin_manifest_requires_permission,
        approve_pin_manifest(),
        CanApproveSorafsPin,
        sorafs::visit_approve_pin_manifest
    );

    sorafs_permission_case!(
        retire_pin_manifest_requires_permission,
        retire_pin_manifest(),
        CanRetireSorafsPin,
        sorafs::visit_retire_pin_manifest
    );

    sorafs_permission_case!(
        bind_manifest_alias_requires_permission,
        bind_manifest_alias(),
        CanBindSorafsAlias,
        sorafs::visit_bind_manifest_alias
    );

    sorafs_permission_case!(
        register_capacity_declaration_requires_permission,
        register_capacity_declaration(),
        CanDeclareSorafsCapacity,
        sorafs::visit_register_capacity_declaration
    );

    sorafs_permission_case!(
        record_capacity_telemetry_requires_permission,
        record_capacity_telemetry(),
        CanSubmitSorafsTelemetry,
        sorafs::visit_record_capacity_telemetry
    );

    sorafs_permission_case!(
        record_capacity_dispute_requires_permission,
        register_capacity_dispute(),
        CanFileSorafsCapacityDispute,
        sorafs::visit_register_capacity_dispute
    );

    sorafs_permission_case!(
        issue_replication_order_requires_permission,
        issue_replication_order(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_issue_replication_order
    );

    sorafs_permission_case!(
        complete_replication_order_requires_permission,
        complete_replication_order(),
        CanCompleteSorafsReplicationOrder,
        sorafs::visit_complete_replication_order
    );

    sorafs_permission_case!(
        record_replication_receipt_requires_permission,
        record_replication_receipt(),
        CanCompleteSorafsReplicationOrder,
        sorafs::visit_record_replication_receipt
    );

    sorafs_permission_case!(
        register_provider_owner_requires_permission,
        register_provider_owner(),
        CanRegisterSorafsProviderOwner,
        sorafs::visit_register_provider_owner
    );

    sorafs_permission_case!(
        unregister_provider_owner_requires_permission,
        unregister_provider_owner(),
        CanUnregisterSorafsProviderOwner,
        sorafs::visit_unregister_provider_owner
    );

    sorafs_permission_case!(
        set_pricing_schedule_requires_permission,
        set_pricing_schedule(),
        CanSetSorafsPricing,
        sorafs::visit_set_pricing_schedule
    );

    sorafs_permission_case!(
        upsert_provider_credit_requires_permission,
        upsert_provider_credit(),
        CanUpsertSorafsProviderCredit,
        sorafs::visit_upsert_provider_credit
    );
}

/// Permission-checked visitors for direct permission grants and revocations.
pub mod permission {
    use super::*;

    macro_rules! impl_execute {
        ($executor:ident, $isi:ident, $method:ident, $isi_type:ty) => {
            let account_id = $isi.destination().clone();
            let permission = $isi.object();

            if let Ok(any_permission) = AnyPermission::try_from(permission) {
                if !$executor.context().curr_block.is_genesis() {
                    if let Err(error) = crate::permission::ValidateGrantRevoke::$method(
                        &any_permission,
                        &$executor.context().authority,
                        $executor.context(),
                        $executor.host(),
                    ) {
                        deny!($executor, error);
                    }
                }

                let isi = &<$isi_type>::account_permission(any_permission, account_id);
                execute!($executor, isi);
            }

            deny!(
                $executor,
                ValidationFail::NotPermitted(format!("{permission:?}: Unknown permission"))
            );
        };
    }

    /// Grants an account-level permission after validating the caller's authority.
    pub fn visit_grant_account_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Grant<Permission, Account>,
    ) {
        impl_execute!(executor, isi, validate_grant, Grant<Permission, Account>);
    }

    /// Revokes an account-level permission once the caller passes permission checks.
    pub fn visit_revoke_account_permission<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &Revoke<Permission, Account>,
    ) {
        impl_execute!(executor, isi, validate_revoke, Revoke<Permission, Account>);
    }
}

/// Permission-checked visitor for executor upgrade instructions.
pub mod executor {
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;

    use super::*;

    /// Upgrades the executor when invoked during genesis or by an authorised authority.
    pub fn visit_upgrade<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &Upgrade) {
        if executor.context().curr_block.is_genesis() {
            execute!(executor, isi);
        }
        if CanUpgradeExecutor.is_owned_by(&executor.context().authority, executor.host()) {
            execute!(executor, isi);
        }

        deny!(executor, "Can't upgrade executor");
    }
}

/// Visitor for log instructions which are always permitted.
pub mod log {
    use super::*;

    /// Emits a log instruction directly because logging has no permission gates.
    pub fn visit_log<V: Execute + Visit + ?Sized>(executor: &mut V, isi: &Log) {
        execute!(executor, isi)
    }
}

/// Visitor for bridge receipt instructions which are always permitted.
pub mod bridge {
    use super::*;

    /// Records a bridge receipt without additional permission gates.
    pub fn visit_record_bridge_receipt<V: Execute + Visit + ?Sized>(
        executor: &mut V,
        isi: &RecordBridgeReceipt,
    ) {
        execute!(executor, isi)
    }
}

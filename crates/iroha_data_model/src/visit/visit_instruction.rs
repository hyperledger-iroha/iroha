//! Visitor helper functions for instructions.

use iroha_primitives::numeric::Numeric;

use super::Visit;
use crate::{
    isi::{
        ActivateIdentifierPolicy, ClaimIdentifier, Log, RegisterIdentifierPolicy,
        RegisterPeerWithPop, RevokeIdentifier,
        nexus::{RegisterVerifiedLaneRelay, SetLaneRelayEmergencyValidators},
        soracloud::{
            AcknowledgeSoracloudAgentMessage, AdmitSoracloudPrivateCompileProfile,
            AdvanceSoracloudRollout, AdvertiseSoracloudModelHost,
            AllowSoracloudAgentAutonomyArtifact, AllowSoracloudUploadedModel,
            AppendSoracloudUploadedModelChunk, ApproveSoracloudAgentWalletSpend,
            CheckpointSoracloudTrainingJob, DeleteSoracloudServiceConfig,
            DeleteSoracloudServiceSecret, DeploySoracloudAgentApartment, DeploySoracloudService,
            EnqueueSoracloudAgentMessage, FinalizeSoracloudUploadedModelBundle,
            HeartbeatSoracloudModelHost, JoinSoracloudHfSharedLease, LeaveSoracloudHfSharedLease,
            MutateSoracloudState, PromoteSoracloudModelWeight,
            RecordSoracloudAgentAutonomyExecution, RecordSoracloudDecryptionRequest,
            RecordSoracloudMailboxMessage, RecordSoracloudPrivateInferenceCheckpoint,
            RecordSoracloudRuntimeReceipt, RegisterSoracloudModelArtifact,
            RegisterSoracloudModelWeight, RegisterSoracloudUploadedModelBundle,
            RenewSoracloudAgentLease, RenewSoracloudHfSharedLease,
            RequestSoracloudAgentWalletSpend, RestartSoracloudAgentApartment,
            RetrySoracloudTrainingJob, RevokeSoracloudAgentPolicy, RollbackSoracloudModelWeight,
            RollbackSoracloudService, RunSoracloudAgentAutonomy, RunSoracloudFheJob,
            SetSoracloudRuntimeState, SetSoracloudServiceConfig, SetSoracloudServiceSecret,
            StartSoracloudPrivateInference, StartSoracloudTrainingJob, UpgradeSoracloudService,
            WithdrawSoracloudModelHost,
        },
        staking::{
            ActivatePublicLaneValidator, ExitPublicLaneValidator, RebindPublicLaneValidatorPeer,
            RegisterPublicLaneValidator,
        },
    },
    prelude::*,
};

/// Dispatch a boxed instruction to the corresponding visitor hook.
pub fn visit_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) {
    if !(visit_core_instruction(visitor, isi)
        || visit_staking_and_identifier_instruction(visitor, isi)
        || visit_soracloud_service_instruction(visitor, isi)
        || visit_soracloud_agent_instruction(visitor, isi)
        || visit_soracloud_training_instruction(visitor, isi))
    {
        unreachable!("Unknown instruction type");
    }
}

fn visit_core_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<SetParameter>() {
        visitor.visit_set_parameter(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ExecuteTrigger>() {
        visitor.visit_execute_trigger(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<Log>() {
        visitor.visit_log(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<BurnBox>() {
        visitor.visit_burn(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<GrantBox>() {
        visitor.visit_grant(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<MintBox>() {
        visitor.visit_mint(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterBox>() {
        visitor.visit_register(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RemoveKeyValueBox>() {
        visitor.visit_remove_key_value(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RevokeBox>() {
        visitor.visit_revoke(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetKeyValueBox>() {
        visitor.visit_set_key_value(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<TransferBox>() {
        visitor.visit_transfer(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<UnregisterBox>() {
        visitor.visit_unregister(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<Upgrade>() {
        visitor.visit_upgrade(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<CustomInstruction>() {
        visitor.visit_custom_instruction(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PublishPedersenParams>() {
        visitor.visit_publish_pedersen_params(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetPedersenParamsLifecycle>() {
        visitor.visit_set_pedersen_params_lifecycle(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PublishPoseidonParams>() {
        visitor.visit_publish_poseidon_params(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetPoseidonParamsLifecycle>() {
        visitor.visit_set_poseidon_params_lifecycle(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ClaimTwitterFollowReward>() {
        visitor.visit_claim_twitter_follow_reward(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SendToTwitter>() {
        visitor.visit_send_to_twitter(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<CancelTwitterEscrow>() {
        visitor.visit_cancel_twitter_escrow(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::rwa::RwaInstructionBox>()
    {
        visitor.visit_rwa_instruction_box(v);
    } else {
        return false;
    }
    true
}

fn visit_staking_and_identifier_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    isi: &InstructionBox,
) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<RegisterPublicLaneValidator>() {
        visitor.visit_register_public_lane_validator(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RebindPublicLaneValidatorPeer>() {
        visitor.visit_rebind_public_lane_validator_peer(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ActivatePublicLaneValidator>() {
        visitor.visit_activate_public_lane_validator(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ExitPublicLaneValidator>() {
        visitor.visit_exit_public_lane_validator(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<SetLaneRelayEmergencyValidators>()
    {
        visitor.visit_set_lane_relay_emergency_validators(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterVerifiedLaneRelay>() {
        visitor.visit_register_verified_lane_relay(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterIdentifierPolicy>() {
        visitor.visit_register_identifier_policy(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ActivateIdentifierPolicy>() {
        visitor.visit_activate_identifier_policy(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ClaimIdentifier>() {
        visitor.visit_claim_identifier(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RevokeIdentifier>() {
        visitor.visit_revoke_identifier(v);
    } else {
        return false;
    }
    true
}

fn visit_soracloud_service_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    isi: &InstructionBox,
) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<DeploySoracloudService>() {
        visitor.visit_deploy_soracloud_service(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<UpgradeSoracloudService>() {
        visitor.visit_upgrade_soracloud_service(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RollbackSoracloudService>() {
        visitor.visit_rollback_soracloud_service(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetSoracloudServiceConfig>() {
        visitor.visit_set_soracloud_service_config(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<DeleteSoracloudServiceConfig>() {
        visitor.visit_delete_soracloud_service_config(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetSoracloudServiceSecret>() {
        visitor.visit_set_soracloud_service_secret(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<DeleteSoracloudServiceSecret>() {
        visitor.visit_delete_soracloud_service_secret(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<MutateSoracloudState>() {
        visitor.visit_mutate_soracloud_state(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RunSoracloudFheJob>() {
        visitor.visit_run_soracloud_fhe_job(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RecordSoracloudDecryptionRequest>()
    {
        visitor.visit_record_soracloud_decryption_request(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<JoinSoracloudHfSharedLease>() {
        visitor.visit_join_soracloud_hf_shared_lease(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<LeaveSoracloudHfSharedLease>() {
        visitor.visit_leave_soracloud_hf_shared_lease(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RenewSoracloudHfSharedLease>() {
        visitor.visit_renew_soracloud_hf_shared_lease(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<AdvertiseSoracloudModelHost>() {
        visitor.visit_advertise_soracloud_model_host(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<HeartbeatSoracloudModelHost>() {
        visitor.visit_heartbeat_soracloud_model_host(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<WithdrawSoracloudModelHost>() {
        visitor.visit_withdraw_soracloud_model_host(v);
    } else {
        return false;
    }
    true
}

fn visit_soracloud_agent_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    isi: &InstructionBox,
) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<DeploySoracloudAgentApartment>() {
        visitor.visit_deploy_soracloud_agent_apartment(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RenewSoracloudAgentLease>() {
        visitor.visit_renew_soracloud_agent_lease(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RestartSoracloudAgentApartment>()
    {
        visitor.visit_restart_soracloud_agent_apartment(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RevokeSoracloudAgentPolicy>() {
        visitor.visit_revoke_soracloud_agent_policy(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RequestSoracloudAgentWalletSpend>()
    {
        visitor.visit_request_soracloud_agent_wallet_spend(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<ApproveSoracloudAgentWalletSpend>()
    {
        visitor.visit_approve_soracloud_agent_wallet_spend(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<EnqueueSoracloudAgentMessage>() {
        visitor.visit_enqueue_soracloud_agent_message(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<AcknowledgeSoracloudAgentMessage>()
    {
        visitor.visit_acknowledge_soracloud_agent_message(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<AllowSoracloudAgentAutonomyArtifact>()
    {
        visitor.visit_allow_soracloud_agent_autonomy_artifact(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RunSoracloudAgentAutonomy>() {
        visitor.visit_run_soracloud_agent_autonomy(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RecordSoracloudAgentAutonomyExecution>()
    {
        visitor.visit_record_soracloud_agent_autonomy_execution(v);
    } else {
        return false;
    }
    true
}

fn visit_soracloud_training_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    isi: &InstructionBox,
) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<StartSoracloudTrainingJob>() {
        visitor.visit_start_soracloud_training_job(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<CheckpointSoracloudTrainingJob>()
    {
        visitor.visit_checkpoint_soracloud_training_job(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RetrySoracloudTrainingJob>() {
        visitor.visit_retry_soracloud_training_job(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RegisterSoracloudModelArtifact>()
    {
        visitor.visit_register_soracloud_model_artifact(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterSoracloudModelWeight>() {
        visitor.visit_register_soracloud_model_weight(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PromoteSoracloudModelWeight>() {
        visitor.visit_promote_soracloud_model_weight(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RollbackSoracloudModelWeight>() {
        visitor.visit_rollback_soracloud_model_weight(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RegisterSoracloudUploadedModelBundle>()
    {
        visitor.visit_register_soracloud_uploaded_model_bundle(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<AppendSoracloudUploadedModelChunk>()
    {
        visitor.visit_append_soracloud_uploaded_model_chunk(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<FinalizeSoracloudUploadedModelBundle>()
    {
        visitor.visit_finalize_soracloud_uploaded_model_bundle(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<AdmitSoracloudPrivateCompileProfile>()
    {
        visitor.visit_admit_soracloud_private_compile_profile(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<AllowSoracloudUploadedModel>() {
        visitor.visit_allow_soracloud_uploaded_model(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<StartSoracloudPrivateInference>()
    {
        visitor.visit_start_soracloud_private_inference(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RecordSoracloudPrivateInferenceCheckpoint>()
    {
        visitor.visit_record_soracloud_private_inference_checkpoint(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<AdvanceSoracloudRollout>() {
        visitor.visit_advance_soracloud_rollout(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetSoracloudRuntimeState>() {
        visitor.visit_set_soracloud_runtime_state(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RecordSoracloudMailboxMessage>() {
        visitor.visit_record_soracloud_mailbox_message(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RecordSoracloudRuntimeReceipt>() {
        visitor.visit_record_soracloud_runtime_receipt(v);
    } else {
        return false;
    }
    true
}

/// Dispatch register variants like peers, domains, and triggers.
pub fn visit_register<V: Visit + ?Sized>(visitor: &mut V, isi: &RegisterBox) {
    match isi {
        RegisterBox::Peer(obj) => visitor.visit_register_peer(obj),
        RegisterBox::Domain(obj) => visitor.visit_register_domain(obj),
        RegisterBox::Account(obj) => visitor.visit_register_account(obj),
        RegisterBox::AssetDefinition(obj) => visitor.visit_register_asset_definition(obj),
        RegisterBox::Nft(obj) => visitor.visit_register_nft(obj),
        RegisterBox::Role(obj) => visitor.visit_register_role(obj),
        RegisterBox::Trigger(obj) => visitor.visit_register_trigger(obj),
    }
}

/// Dispatch unregister variants across all registerable entities.
pub fn visit_unregister<V: Visit + ?Sized>(visitor: &mut V, isi: &UnregisterBox) {
    match isi {
        UnregisterBox::Peer(obj) => visitor.visit_unregister_peer(obj),
        UnregisterBox::Domain(obj) => visitor.visit_unregister_domain(obj),
        UnregisterBox::Account(obj) => visitor.visit_unregister_account(obj),
        UnregisterBox::AssetDefinition(obj) => visitor.visit_unregister_asset_definition(obj),
        UnregisterBox::Nft(obj) => visitor.visit_unregister_nft(obj),
        UnregisterBox::Role(obj) => visitor.visit_unregister_role(obj),
        UnregisterBox::Trigger(obj) => visitor.visit_unregister_trigger(obj),
    }
}

/// Dispatch mint variants to the appropriate hook.
pub fn visit_mint<V: Visit + ?Sized>(visitor: &mut V, isi: &MintBox) {
    match isi {
        MintBox::Asset(obj) => visitor.visit_mint_asset_numeric(obj),
        MintBox::TriggerRepetitions(obj) => visitor.visit_mint_trigger_repetitions(obj),
    }
}

/// Dispatch burn variants to the appropriate hook.
pub fn visit_burn<V: Visit + ?Sized>(visitor: &mut V, isi: &BurnBox) {
    match isi {
        BurnBox::Asset(obj) => visitor.visit_burn_asset_numeric(obj),
        BurnBox::TriggerRepetitions(obj) => visitor.visit_burn_trigger_repetitions(obj),
    }
}

/// Dispatch transfer variants to the appropriate hook.
pub fn visit_transfer<V: Visit + ?Sized>(visitor: &mut V, isi: &TransferBox) {
    match isi {
        TransferBox::Domain(obj) => visitor.visit_transfer_domain(obj),
        TransferBox::AssetDefinition(obj) => visitor.visit_transfer_asset_definition(obj),
        TransferBox::Asset(obj) => visitor.visit_transfer_asset_numeric(obj),
        TransferBox::Nft(obj) => visitor.visit_transfer_nft(obj),
    }
}

/// Dispatch set-key-value variants to the appropriate hook.
pub fn visit_set_key_value<V: Visit + ?Sized>(visitor: &mut V, isi: &SetKeyValueBox) {
    match isi {
        SetKeyValueBox::Domain(obj) => visitor.visit_set_domain_key_value(obj),
        SetKeyValueBox::Account(obj) => visitor.visit_set_account_key_value(obj),
        SetKeyValueBox::AssetDefinition(obj) => visitor.visit_set_asset_definition_key_value(obj),
        SetKeyValueBox::Nft(obj) => visitor.visit_set_nft_key_value(obj),
        SetKeyValueBox::Trigger(obj) => visitor.visit_set_trigger_key_value(obj),
    }
}

/// Dispatch remove-key-value variants to the appropriate hook.
pub fn visit_remove_key_value<V: Visit + ?Sized>(visitor: &mut V, isi: &RemoveKeyValueBox) {
    match isi {
        RemoveKeyValueBox::Domain(obj) => visitor.visit_remove_domain_key_value(obj),
        RemoveKeyValueBox::Account(obj) => visitor.visit_remove_account_key_value(obj),
        RemoveKeyValueBox::AssetDefinition(obj) => {
            visitor.visit_remove_asset_definition_key_value(obj)
        }
        RemoveKeyValueBox::Nft(obj) => visitor.visit_remove_nft_key_value(obj),
        RemoveKeyValueBox::Trigger(obj) => visitor.visit_remove_trigger_key_value(obj),
    }
}

/// Dispatch grouped RWA instructions.
pub fn visit_rwa_instruction_box<V: Visit + ?Sized>(
    _visitor: &mut V,
    _isi: &crate::isi::rwa::RwaInstructionBox,
) {
}

/// Dispatch grant variants to the appropriate hook.
pub fn visit_grant<V: Visit + ?Sized>(visitor: &mut V, isi: &GrantBox) {
    match isi {
        GrantBox::Permission(obj) => visitor.visit_grant_account_permission(obj),
        GrantBox::Role(obj) => visitor.visit_grant_account_role(obj),
        GrantBox::RolePermission(obj) => visitor.visit_grant_role_permission(obj),
    }
}

/// Dispatch revoke variants to the appropriate hook.
pub fn visit_revoke<V: Visit + ?Sized>(visitor: &mut V, isi: &RevokeBox) {
    match isi {
        RevokeBox::Permission(obj) => visitor.visit_revoke_account_permission(obj),
        RevokeBox::Role(obj) => visitor.visit_revoke_account_role(obj),
        RevokeBox::RolePermission(obj) => visitor.visit_revoke_role_permission(obj),
    }
}

/// Macro generating visitor method signatures for every instruction variant.
#[macro_export]
macro_rules! instruction_visitors {
    ($macro:ident) => {
        $macro! {
            visit_register_account(&Register<Account>),
            visit_unregister_account(&Unregister<Account>),
            visit_set_account_key_value(&SetKeyValue<Account>),
            visit_remove_account_key_value(&RemoveKeyValue<Account>),
            visit_register_nft(&Register<Nft>),
            visit_unregister_nft(&Unregister<Nft>),
            visit_mint_asset_numeric(&Mint<Numeric, Asset>),
            visit_burn_asset_numeric(&Burn<Numeric, Asset>),
            visit_transfer_asset_numeric(&Transfer<Asset, Numeric, Account>),
            visit_transfer_nft(&Transfer<Account, NftId, Account>),
            visit_set_nft_key_value(&SetKeyValue<Nft>),
            visit_remove_nft_key_value(&RemoveKeyValue<Nft>),
            visit_set_trigger_key_value(&SetKeyValue<Trigger>),
            visit_remove_trigger_key_value(&RemoveKeyValue<Trigger>),
            visit_register_asset_definition(&Register<AssetDefinition>),
            visit_unregister_asset_definition(&Unregister<AssetDefinition>),
            visit_transfer_asset_definition(&Transfer<Account, AssetDefinitionId, Account>),
            visit_set_asset_definition_key_value(&SetKeyValue<AssetDefinition>),
            visit_remove_asset_definition_key_value(&RemoveKeyValue<AssetDefinition>),
            visit_register_domain(&Register<Domain>),
            visit_unregister_domain(&Unregister<Domain>),
            visit_transfer_domain(&Transfer<Account, DomainId, Account>),
            visit_set_domain_key_value(&SetKeyValue<Domain>),
            visit_remove_domain_key_value(&RemoveKeyValue<Domain>),
            visit_register_peer(&RegisterPeerWithPop),
            visit_unregister_peer(&Unregister<Peer>),
            visit_grant_account_permission(&Grant<Permission, Account>),
            visit_revoke_account_permission(&Revoke<Permission, Account>),
            visit_register_role(&Register<Role>),
            visit_unregister_role(&Unregister<Role>),
            visit_grant_account_role(&Grant<RoleId, Account>),
            visit_revoke_account_role(&Revoke<RoleId, Account>),
            visit_grant_role_permission(&Grant<Permission, Role>),
            visit_revoke_role_permission(&Revoke<Permission, Role>),
            visit_register_trigger(&Register<Trigger>),
            visit_unregister_trigger(&Unregister<Trigger>),
            visit_mint_trigger_repetitions(&Mint<u32, Trigger>),
            visit_burn_trigger_repetitions(&Burn<u32, Trigger>),
            visit_upgrade(&Upgrade),
            visit_set_parameter(&SetParameter),
            visit_execute_trigger(&ExecuteTrigger),
            visit_log(&Log),
            visit_custom_instruction(&CustomInstruction),
            visit_publish_pedersen_params(&PublishPedersenParams),
            visit_set_pedersen_params_lifecycle(&SetPedersenParamsLifecycle),
            visit_publish_poseidon_params(&PublishPoseidonParams),
            visit_set_poseidon_params_lifecycle(&SetPoseidonParamsLifecycle),
            visit_claim_twitter_follow_reward(&ClaimTwitterFollowReward),
            visit_send_to_twitter(&SendToTwitter),
            visit_cancel_twitter_escrow(&CancelTwitterEscrow),
            visit_register_public_lane_validator(&RegisterPublicLaneValidator),
            visit_rebind_public_lane_validator_peer(&RebindPublicLaneValidatorPeer),
            visit_activate_public_lane_validator(&ActivatePublicLaneValidator),
            visit_exit_public_lane_validator(&ExitPublicLaneValidator),
            visit_set_lane_relay_emergency_validators(&SetLaneRelayEmergencyValidators),
            visit_register_verified_lane_relay(&RegisterVerifiedLaneRelay),
            visit_register_identifier_policy(&RegisterIdentifierPolicy),
            visit_activate_identifier_policy(&ActivateIdentifierPolicy),
            visit_claim_identifier(&ClaimIdentifier),
            visit_revoke_identifier(&RevokeIdentifier),
            visit_deploy_soracloud_service(&DeploySoracloudService),
            visit_upgrade_soracloud_service(&UpgradeSoracloudService),
            visit_rollback_soracloud_service(&RollbackSoracloudService),
            visit_set_soracloud_service_config(&SetSoracloudServiceConfig),
            visit_delete_soracloud_service_config(&DeleteSoracloudServiceConfig),
            visit_set_soracloud_service_secret(&SetSoracloudServiceSecret),
            visit_delete_soracloud_service_secret(&DeleteSoracloudServiceSecret),
            visit_mutate_soracloud_state(&MutateSoracloudState),
            visit_run_soracloud_fhe_job(&RunSoracloudFheJob),
            visit_record_soracloud_decryption_request(&RecordSoracloudDecryptionRequest),
            visit_join_soracloud_hf_shared_lease(&JoinSoracloudHfSharedLease),
            visit_leave_soracloud_hf_shared_lease(&LeaveSoracloudHfSharedLease),
            visit_renew_soracloud_hf_shared_lease(&RenewSoracloudHfSharedLease),
            visit_advertise_soracloud_model_host(&AdvertiseSoracloudModelHost),
            visit_heartbeat_soracloud_model_host(&HeartbeatSoracloudModelHost),
            visit_withdraw_soracloud_model_host(&WithdrawSoracloudModelHost),
            visit_deploy_soracloud_agent_apartment(&DeploySoracloudAgentApartment),
            visit_renew_soracloud_agent_lease(&RenewSoracloudAgentLease),
            visit_restart_soracloud_agent_apartment(&RestartSoracloudAgentApartment),
            visit_revoke_soracloud_agent_policy(&RevokeSoracloudAgentPolicy),
            visit_request_soracloud_agent_wallet_spend(&RequestSoracloudAgentWalletSpend),
            visit_approve_soracloud_agent_wallet_spend(&ApproveSoracloudAgentWalletSpend),
            visit_enqueue_soracloud_agent_message(&EnqueueSoracloudAgentMessage),
            visit_acknowledge_soracloud_agent_message(&AcknowledgeSoracloudAgentMessage),
            visit_allow_soracloud_agent_autonomy_artifact(&AllowSoracloudAgentAutonomyArtifact),
            visit_run_soracloud_agent_autonomy(&RunSoracloudAgentAutonomy),
            visit_record_soracloud_agent_autonomy_execution(&RecordSoracloudAgentAutonomyExecution),
            visit_start_soracloud_training_job(&StartSoracloudTrainingJob),
            visit_checkpoint_soracloud_training_job(&CheckpointSoracloudTrainingJob),
            visit_retry_soracloud_training_job(&RetrySoracloudTrainingJob),
            visit_register_soracloud_model_artifact(&RegisterSoracloudModelArtifact),
            visit_register_soracloud_model_weight(&RegisterSoracloudModelWeight),
            visit_promote_soracloud_model_weight(&PromoteSoracloudModelWeight),
            visit_rollback_soracloud_model_weight(&RollbackSoracloudModelWeight),
            visit_register_soracloud_uploaded_model_bundle(&RegisterSoracloudUploadedModelBundle),
            visit_append_soracloud_uploaded_model_chunk(&AppendSoracloudUploadedModelChunk),
            visit_finalize_soracloud_uploaded_model_bundle(&FinalizeSoracloudUploadedModelBundle),
            visit_admit_soracloud_private_compile_profile(&AdmitSoracloudPrivateCompileProfile),
            visit_allow_soracloud_uploaded_model(&AllowSoracloudUploadedModel),
            visit_start_soracloud_private_inference(&StartSoracloudPrivateInference),
            visit_record_soracloud_private_inference_checkpoint(&RecordSoracloudPrivateInferenceCheckpoint),
            visit_advance_soracloud_rollout(&AdvanceSoracloudRollout),
            visit_set_soracloud_runtime_state(&SetSoracloudRuntimeState),
            visit_record_soracloud_mailbox_message(&RecordSoracloudMailboxMessage),
            visit_record_soracloud_runtime_receipt(&RecordSoracloudRuntimeReceipt),
        }
    };
}

macro_rules! define_instruction_visitors {
    ( $( $visitor:ident($operation:ty) ),+ $(,)? ) => { $(
        #[doc = concat!("Visit ", stringify!($operation), ".")]
        pub fn $visitor<V: Visit + ?Sized>(_visitor: &mut V, _operation: $operation) {}
    )+ };
}

instruction_visitors!(define_instruction_visitors);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use iroha_crypto::{Algorithm, KeyPair};

    struct CountingVisitor {
        logs: usize,
    }

    impl Visit for CountingVisitor {
        fn visit_log(&mut self, _: &Log) {
            self.logs += 1;
        }
    }

    #[test]
    fn visit_log_dispatches() {
        let mut visitor = CountingVisitor { logs: 0 };
        let isi = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "test".to_string(),
        });
        visit_instruction(&mut visitor, &isi);
        assert_eq!(visitor.logs, 1);
    }

    #[test]
    fn visit_register_public_lane_validator_dispatches() {
        struct RegisterVisitor {
            called: bool,
        }

        impl Visit for RegisterVisitor {
            fn visit_register_public_lane_validator(&mut self, _: &RegisterPublicLaneValidator) {
                self.called = true;
            }
        }

        let _domain: DomainId = "wonderland".parse().expect("domain id");
        let key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let validator = AccountId::new(key_pair.public_key().clone());
        let instruction = RegisterPublicLaneValidator::new(
            LaneId::SINGLE,
            validator.clone(),
            PeerId::from(validator.signatory().clone()),
            validator,
            Numeric::from(1u64),
            Metadata::default(),
        );
        let isi = InstructionBox::from(instruction);

        let mut visitor = RegisterVisitor { called: false };
        visit_instruction(&mut visitor, &isi);
        assert!(visitor.called);
    }

    #[test]
    fn visit_rebind_public_lane_validator_peer_dispatches() {
        struct RebindVisitor {
            called: bool,
        }

        impl Visit for RebindVisitor {
            fn visit_rebind_public_lane_validator_peer(
                &mut self,
                _: &RebindPublicLaneValidatorPeer,
            ) {
                self.called = true;
            }
        }

        let _domain: DomainId = "wonderland".parse().expect("domain id");
        let validator_key = KeyPair::from_seed(vec![0x12; 32], Algorithm::Ed25519);
        let peer_key = KeyPair::from_seed(vec![0x13; 32], Algorithm::Ed25519);
        let validator = AccountId::new(validator_key.public_key().clone());
        let peer_id = PeerId::from(peer_key.public_key().clone());
        let isi = InstructionBox::from(RebindPublicLaneValidatorPeer::new(
            LaneId::SINGLE,
            validator,
            peer_id,
        ));

        let mut visitor = RebindVisitor { called: false };
        visit_instruction(&mut visitor, &isi);
        assert!(visitor.called);
    }
}

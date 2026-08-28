//! Visitor helper functions for instructions.
use super::Visit;
use crate::{
    isi::{
        ActivateIdentifierPolicy, ClaimIdentifier, Log, RegisterIdentifierPolicy,
        RegisterPeerWithPop, RevokeIdentifier,
        nexus::{
            ActivateFeeSponsorProgramRevision, BeginCloseFeeSponsorProgram, CloseFeeSponsorProgram,
            CreateFeeSponsorProgram, EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram,
            PauseFeeSponsorProgram, RegisterVerifiedFeeSponsorVaultAllocation,
            RegisterVerifiedLaneRelay, SetLaneRelayEmergencyValidators,
            StageFeeSponsorProgramRevision, UnenrollFeeSponsorBeneficiary,
            WithdrawFeeSponsorProgram,
        },
        soracloud::{
            AcknowledgeSoracloudAgentMessage, AdvanceSoracloudRollout, AdvertiseSoracloudInrouHost,
            AllowSoracloudAgentAutonomyArtifact, ApplySoracloudOrderedMailboxResult,
            ApproveSoracloudAgentWalletSpend, CheckpointSoracloudTrainingJob,
            ClearSoracloudInrouReplicaRuntimeState, DeleteSoracloudServiceConfig,
            DeleteSoracloudServiceSecret, DeploySoracloudAgentApartment, DeploySoracloudService,
            EnqueueSoracloudAgentMessage, FinalizeSoracloudUploadedModelBundle,
            JoinSoracloudHfSharedLease, LeaveSoracloudHfSharedLease, MutateSoracloudState,
            PromoteSoracloudModelWeight, ReconcileSoracloudInrouPlacements,
            RecordSoracloudAgentAutonomyExecution, RecordSoracloudDecryptionRequest,
            RecordSoracloudMailboxMessage, RecordSoracloudRuntimeReceipt,
            RegisterSoracloudFhePolicy, RegisterSoracloudModelArtifact,
            RegisterSoracloudModelWeight, RegisterSoracloudUploadedModelBundle,
            RenewSoracloudAgentLease, RenewSoracloudHfSharedLease,
            ReportSoracloudServiceLeaseUsage, RequestSoracloudAgentWalletSpend,
            RestartSoracloudAgentApartment, RetrySoracloudTrainingJob, RevokeSoracloudAgentPolicy,
            RevokeSoracloudFhePolicy, RollbackSoracloudModelWeight, RollbackSoracloudService,
            RotateSoracloudFhePolicy, RunSoracloudAgentAutonomy, RunSoracloudFheJob,
            SetSoracloudInrouReplicaRuntimeState, SetSoracloudRuntimeState,
            SetSoracloudServiceConfig, SetSoracloudServiceSecret, StartSoracloudTrainingJob,
            UpgradeSoracloudService, WithdrawSoracloudInrouHost,
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
        visitor.visit_unclassified_instruction(isi);
    }
}
/// Visit an instruction that has no typed hook in the generic data-model walker.
///
/// Registered native extensions are valid [`InstructionBox`] values even when
/// this intentionally small walker does not expose their semantics. The default
/// is therefore a leaf visit instead of a process-terminating assertion.
pub fn visit_unclassified_instruction<V: Visit + ?Sized>(_visitor: &mut V, _isi: &InstructionBox) {}
fn visit_core_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) -> bool {
    visit_core_setup_instruction(visitor, isi)
        || visit_core_box_instruction(visitor, isi)
        || visit_privacy_instruction(visitor, isi)
        || visit_integrated_instruction(visitor, isi)
}
fn visit_core_setup_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<SetParameter>() {
        visitor.visit_set_parameter(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<ExecuteTrigger>() {
        visitor.visit_execute_trigger(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::alias_setup::EnsureAlias>()
    {
        visitor.visit_ensure_alias(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::alias_setup::RenewAliasLease>()
    {
        visitor.visit_renew_alias_lease(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::alias_setup::ConfigureAliasAutoRenew>()
    {
        visitor.visit_configure_alias_auto_renew(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::alias_setup::RebindAccountAlias>()
    {
        visitor.visit_rebind_account_alias(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::alias_setup::CompareAndSetPrimaryAccountAlias>()
    {
        visitor.visit_compare_and_set_primary_account_alias(v);
    } else {
        return false;
    }
    true
}
fn visit_core_box_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<Log>() {
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
    } else {
        return false;
    }
    true
}
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive privacy instruction inventory stays in one dispatch chain so new variants cannot silently become no-ops"
)]
fn visit_privacy_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) -> bool {
    visit_privacy_protocol_instruction(visitor, isi)
        || visit_privacy_issuer_and_proof_instruction(visitor, isi)
}
fn visit_privacy_protocol_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    isi: &InstructionBox,
) -> bool {
    if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyProtocolActivationV1>()
    {
        visitor.visit_register_privacy_protocol_activation_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyExact12QualificationV1>()
    {
        visitor.visit_register_privacy_exact12_qualification_v1(v);
    } else if let Some(v) =
        isi.as_any()
            .downcast_ref::<crate::isi::privacy::SchedulePrivacyConsensusPolicyTighteningV1>()
    {
        visitor.visit_schedule_privacy_consensus_policy_tightening_v1(v);
    } else if let Some(v) =
        isi.as_any()
            .downcast_ref::<crate::isi::privacy::SchedulePrivacyProtocolLimitsTighteningV1>()
    {
        visitor.visit_schedule_privacy_protocol_limits_tightening_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::TransitionPrivacyProtocolLifecycleV1>()
    {
        visitor.visit_transition_privacy_protocol_lifecycle_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::PublishPrivacyRootV1>()
    {
        visitor.visit_publish_privacy_root_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::BootstrapPrivacyOrchardPoolV1>()
    {
        visitor.visit_bootstrap_privacy_orchard_pool_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::BootstrapPrivacyProofManagedPoolV1>()
    {
        visitor.visit_bootstrap_privacy_proof_managed_pool_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::BootstrapPrivacyPgcAccountsV1>()
    {
        visitor.visit_bootstrap_privacy_pgc_accounts_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::BootstrapPrivacyZkAmsRegistryV1>()
    {
        visitor.visit_bootstrap_privacy_zk_ams_registry_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyZkAcePolicyV1>()
    {
        visitor.visit_register_privacy_zk_ace_policy_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RotatePrivacyZkAcePolicyV1>()
    {
        visitor.visit_rotate_privacy_zk_ace_policy_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RevokePrivacyZkAcePolicyV1>()
    {
        visitor.visit_revoke_privacy_zk_ace_policy_v1(v);
    } else {
        return false;
    }
    true
}
fn visit_privacy_issuer_and_proof_instruction<V: Visit + ?Sized>(
    visitor: &mut V,
    isi: &InstructionBox,
) -> bool {
    if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyBootleLanternIssuerPolicyV1>()
    {
        visitor.visit_register_privacy_bootle_lantern_issuer_policy_v1(v);
    } else if let Some(v) =
        isi.as_any()
            .downcast_ref::<crate::isi::privacy::RotatePrivacyBootleLanternIssuerPolicyV1>()
    {
        visitor.visit_rotate_privacy_bootle_lantern_issuer_policy_v1(v);
    } else if let Some(v) =
        isi.as_any()
            .downcast_ref::<crate::isi::privacy::RevokePrivacyBootleLanternIssuerPolicyV1>()
    {
        visitor.visit_revoke_privacy_bootle_lantern_issuer_policy_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyVegaIssuerV1>()
    {
        visitor.visit_register_privacy_vega_issuer_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RotatePrivacyVegaIssuerV1>()
    {
        visitor.visit_rotate_privacy_vega_issuer_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RevokePrivacyVegaIssuerV1>()
    {
        visitor.visit_revoke_privacy_vega_issuer_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyZkX509TrustAnchorV1>()
    {
        visitor.visit_register_privacy_zk_x509_trust_anchor_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RotatePrivacyZkX509TrustAnchorV1>()
    {
        visitor.visit_rotate_privacy_zk_x509_trust_anchor_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RevokePrivacyZkX509TrustAnchorV1>()
    {
        visitor.visit_revoke_privacy_zk_x509_trust_anchor_v1(v);
    } else if let Some(v) =
        isi.as_any()
            .downcast_ref::<crate::isi::privacy::RegisterPrivacyZkX509CertificatePolicyV1>()
    {
        visitor.visit_register_privacy_zk_x509_certificate_policy_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RotatePrivacyZkX509CertificatePolicyV1>()
    {
        visitor.visit_rotate_privacy_zk_x509_certificate_policy_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RevokePrivacyZkX509CertificatePolicyV1>()
    {
        visitor.visit_revoke_privacy_zk_x509_certificate_policy_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RegisterPrivacyZkX509CrlV1>()
    {
        visitor.visit_register_privacy_zk_x509_crl_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RotatePrivacyZkX509CrlV1>()
    {
        visitor.visit_rotate_privacy_zk_x509_crl_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::RevokePrivacyZkX509CrlV1>()
    {
        visitor.visit_revoke_privacy_zk_x509_crl_v1(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::privacy::SubmitPrivacyProofV1>()
    {
        visitor.visit_submit_privacy_proof_v1(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PublishPedersenParams>() {
        visitor.visit_publish_pedersen_params(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetPedersenParamsLifecycle>() {
        visitor.visit_set_pedersen_params_lifecycle(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PublishPoseidonParams>() {
        visitor.visit_publish_poseidon_params(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetPoseidonParamsLifecycle>() {
        visitor.visit_set_poseidon_params_lifecycle(v);
    } else {
        return false;
    }
    true
}
fn visit_integrated_instruction<V: Visit + ?Sized>(visitor: &mut V, isi: &InstructionBox) -> bool {
    if let Some(v) = isi.as_any().downcast_ref::<ClaimTwitterFollowReward>() {
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
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<crate::isi::defi::DeFiInstructionBox>()
    {
        visitor.visit_defi_instruction_box(v);
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
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<RegisterVerifiedFeeSponsorVaultAllocation>()
    {
        visitor.visit_register_verified_fee_sponsor_vault_allocation(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<CreateFeeSponsorProgram>() {
        visitor.visit_create_fee_sponsor_program(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<StageFeeSponsorProgramRevision>()
    {
        visitor.visit_stage_fee_sponsor_program_revision(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<ActivateFeeSponsorProgramRevision>()
    {
        visitor.visit_activate_fee_sponsor_program_revision(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<PauseFeeSponsorProgram>() {
        visitor.visit_pause_fee_sponsor_program(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<BeginCloseFeeSponsorProgram>() {
        visitor.visit_begin_close_fee_sponsor_program(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<CloseFeeSponsorProgram>() {
        visitor.visit_close_fee_sponsor_program(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<EnrollFeeSponsorBeneficiary>() {
        visitor.visit_enroll_fee_sponsor_beneficiary(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<UnenrollFeeSponsorBeneficiary>() {
        visitor.visit_unenroll_fee_sponsor_beneficiary(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<FundFeeSponsorProgram>() {
        visitor.visit_fund_fee_sponsor_program(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<WithdrawFeeSponsorProgram>() {
        visitor.visit_withdraw_fee_sponsor_program(v);
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
    } else if let Some(v) = isi.as_any().downcast_ref::<RegisterSoracloudFhePolicy>() {
        visitor.visit_register_soracloud_fhe_policy(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RotateSoracloudFhePolicy>() {
        visitor.visit_rotate_soracloud_fhe_policy(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RevokeSoracloudFhePolicy>() {
        visitor.visit_revoke_soracloud_fhe_policy(v);
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
    } else if let Some(v) = isi.as_any().downcast_ref::<AdvertiseSoracloudInrouHost>() {
        visitor.visit_advertise_soracloud_inrou_host(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<WithdrawSoracloudInrouHost>() {
        visitor.visit_withdraw_soracloud_inrou_host(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<ReconcileSoracloudInrouPlacements>()
    {
        visitor.visit_reconcile_soracloud_inrou_placements(v);
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
        .downcast_ref::<FinalizeSoracloudUploadedModelBundle>()
    {
        visitor.visit_finalize_soracloud_uploaded_model_bundle(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<AdvanceSoracloudRollout>() {
        visitor.visit_advance_soracloud_rollout(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<SetSoracloudRuntimeState>() {
        visitor.visit_set_soracloud_runtime_state(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<SetSoracloudInrouReplicaRuntimeState>()
    {
        visitor.visit_set_soracloud_inrou_replica_runtime_state(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<ClearSoracloudInrouReplicaRuntimeState>()
    {
        visitor.visit_clear_soracloud_inrou_replica_runtime_state(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<ReportSoracloudServiceLeaseUsage>()
    {
        visitor.visit_report_soracloud_service_lease_usage(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RecordSoracloudMailboxMessage>() {
        visitor.visit_record_soracloud_mailbox_message(v);
    } else if let Some(v) = isi.as_any().downcast_ref::<RecordSoracloudRuntimeReceipt>() {
        visitor.visit_record_soracloud_runtime_receipt(v);
    } else if let Some(v) = isi
        .as_any()
        .downcast_ref::<ApplySoracloudOrderedMailboxResult>()
    {
        visitor.visit_apply_soracloud_ordered_mailbox_result(v);
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
        MintBox::Asset(obj) => visitor.visit_mint_asset_quantity(obj),
        MintBox::TriggerRepetitions(obj) => visitor.visit_mint_trigger_repetitions(obj),
    }
}
/// Dispatch burn variants to the appropriate hook.
pub fn visit_burn<V: Visit + ?Sized>(visitor: &mut V, isi: &BurnBox) {
    match isi {
        BurnBox::Asset(obj) => visitor.visit_burn_asset_quantity(obj),
        BurnBox::TriggerRepetitions(obj) => visitor.visit_burn_trigger_repetitions(obj),
    }
}
/// Dispatch transfer variants to the appropriate hook.
pub fn visit_transfer<V: Visit + ?Sized>(visitor: &mut V, isi: &TransferBox) {
    match isi {
        TransferBox::Domain(obj) => visitor.visit_transfer_domain(obj),
        TransferBox::AssetDefinition(obj) => visitor.visit_transfer_asset_definition(obj),
        TransferBox::Asset(obj) => visitor.visit_transfer_asset_quantity(obj),
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
/// Dispatch grouped `DeFi` instructions.
pub fn visit_defi_instruction_box<V: Visit + ?Sized>(
    _visitor: &mut V,
    _isi: &crate::isi::defi::DeFiInstructionBox,
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
            visit_mint_asset_quantity(&Mint<Quantity, Asset>),
            visit_burn_asset_quantity(&Burn<Quantity, Asset>),
            visit_transfer_asset_quantity(&Transfer<Asset, Quantity, Account>),
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
            visit_ensure_alias(&$crate::isi::alias_setup::EnsureAlias),
            visit_renew_alias_lease(&$crate::isi::alias_setup::RenewAliasLease),
            visit_configure_alias_auto_renew(&$crate::isi::alias_setup::ConfigureAliasAutoRenew),
            visit_rebind_account_alias(&$crate::isi::alias_setup::RebindAccountAlias),
            visit_compare_and_set_primary_account_alias(&$crate::isi::alias_setup::CompareAndSetPrimaryAccountAlias),
            visit_log(&Log),
            visit_custom_instruction(&CustomInstruction),
            visit_register_privacy_protocol_activation_v1(
                &$crate::isi::privacy::RegisterPrivacyProtocolActivationV1
            ),
            visit_register_privacy_exact12_qualification_v1(
                &$crate::isi::privacy::RegisterPrivacyExact12QualificationV1
            ),
            visit_schedule_privacy_consensus_policy_tightening_v1(
                &$crate::isi::privacy::SchedulePrivacyConsensusPolicyTighteningV1
            ),
            visit_schedule_privacy_protocol_limits_tightening_v1(
                &$crate::isi::privacy::SchedulePrivacyProtocolLimitsTighteningV1
            ),
            visit_transition_privacy_protocol_lifecycle_v1(
                &$crate::isi::privacy::TransitionPrivacyProtocolLifecycleV1
            ),
            visit_publish_privacy_root_v1(&$crate::isi::privacy::PublishPrivacyRootV1),
            visit_bootstrap_privacy_orchard_pool_v1(
                &$crate::isi::privacy::BootstrapPrivacyOrchardPoolV1
            ),
            visit_bootstrap_privacy_proof_managed_pool_v1(
                &$crate::isi::privacy::BootstrapPrivacyProofManagedPoolV1
            ),
            visit_bootstrap_privacy_pgc_accounts_v1(
                &$crate::isi::privacy::BootstrapPrivacyPgcAccountsV1
            ),
            visit_bootstrap_privacy_zk_ams_registry_v1(
                &$crate::isi::privacy::BootstrapPrivacyZkAmsRegistryV1
            ),
            visit_register_privacy_zk_ace_policy_v1(
                &$crate::isi::privacy::RegisterPrivacyZkAcePolicyV1
            ),
            visit_rotate_privacy_zk_ace_policy_v1(
                &$crate::isi::privacy::RotatePrivacyZkAcePolicyV1
            ),
            visit_revoke_privacy_zk_ace_policy_v1(
                &$crate::isi::privacy::RevokePrivacyZkAcePolicyV1
            ),
            visit_register_privacy_bootle_lantern_issuer_policy_v1(
                &$crate::isi::privacy::RegisterPrivacyBootleLanternIssuerPolicyV1
            ),
            visit_rotate_privacy_bootle_lantern_issuer_policy_v1(
                &$crate::isi::privacy::RotatePrivacyBootleLanternIssuerPolicyV1
            ),
            visit_revoke_privacy_bootle_lantern_issuer_policy_v1(
                &$crate::isi::privacy::RevokePrivacyBootleLanternIssuerPolicyV1
            ),
            visit_register_privacy_vega_issuer_v1(
                &$crate::isi::privacy::RegisterPrivacyVegaIssuerV1
            ),
            visit_rotate_privacy_vega_issuer_v1(
                &$crate::isi::privacy::RotatePrivacyVegaIssuerV1
            ),
            visit_revoke_privacy_vega_issuer_v1(
                &$crate::isi::privacy::RevokePrivacyVegaIssuerV1
            ),
            visit_register_privacy_zk_x509_trust_anchor_v1(
                &$crate::isi::privacy::RegisterPrivacyZkX509TrustAnchorV1
            ),
            visit_rotate_privacy_zk_x509_trust_anchor_v1(
                &$crate::isi::privacy::RotatePrivacyZkX509TrustAnchorV1
            ),
            visit_revoke_privacy_zk_x509_trust_anchor_v1(
                &$crate::isi::privacy::RevokePrivacyZkX509TrustAnchorV1
            ),
            visit_register_privacy_zk_x509_certificate_policy_v1(
                &$crate::isi::privacy::RegisterPrivacyZkX509CertificatePolicyV1
            ),
            visit_rotate_privacy_zk_x509_certificate_policy_v1(
                &$crate::isi::privacy::RotatePrivacyZkX509CertificatePolicyV1
            ),
            visit_revoke_privacy_zk_x509_certificate_policy_v1(
                &$crate::isi::privacy::RevokePrivacyZkX509CertificatePolicyV1
            ),
            visit_register_privacy_zk_x509_crl_v1(
                &$crate::isi::privacy::RegisterPrivacyZkX509CrlV1
            ),
            visit_rotate_privacy_zk_x509_crl_v1(
                &$crate::isi::privacy::RotatePrivacyZkX509CrlV1
            ),
            visit_revoke_privacy_zk_x509_crl_v1(
                &$crate::isi::privacy::RevokePrivacyZkX509CrlV1
            ),
            visit_submit_privacy_proof_v1(&$crate::isi::privacy::SubmitPrivacyProofV1),
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
            visit_register_verified_fee_sponsor_vault_allocation(&$crate::isi::nexus::RegisterVerifiedFeeSponsorVaultAllocation),
            visit_create_fee_sponsor_program(&$crate::isi::nexus::CreateFeeSponsorProgram),
            visit_stage_fee_sponsor_program_revision(&$crate::isi::nexus::StageFeeSponsorProgramRevision),
            visit_activate_fee_sponsor_program_revision(&$crate::isi::nexus::ActivateFeeSponsorProgramRevision),
            visit_pause_fee_sponsor_program(&$crate::isi::nexus::PauseFeeSponsorProgram),
            visit_begin_close_fee_sponsor_program(&$crate::isi::nexus::BeginCloseFeeSponsorProgram),
            visit_close_fee_sponsor_program(&$crate::isi::nexus::CloseFeeSponsorProgram),
            visit_enroll_fee_sponsor_beneficiary(&$crate::isi::nexus::EnrollFeeSponsorBeneficiary),
            visit_unenroll_fee_sponsor_beneficiary(&$crate::isi::nexus::UnenrollFeeSponsorBeneficiary),
            visit_fund_fee_sponsor_program(&$crate::isi::nexus::FundFeeSponsorProgram),
            visit_withdraw_fee_sponsor_program(&$crate::isi::nexus::WithdrawFeeSponsorProgram),
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
            visit_register_soracloud_fhe_policy(&RegisterSoracloudFhePolicy),
            visit_rotate_soracloud_fhe_policy(&RotateSoracloudFhePolicy),
            visit_revoke_soracloud_fhe_policy(&RevokeSoracloudFhePolicy),
            visit_run_soracloud_fhe_job(&RunSoracloudFheJob),
            visit_record_soracloud_decryption_request(&RecordSoracloudDecryptionRequest),
            visit_join_soracloud_hf_shared_lease(&JoinSoracloudHfSharedLease),
            visit_leave_soracloud_hf_shared_lease(&LeaveSoracloudHfSharedLease),
            visit_renew_soracloud_hf_shared_lease(&RenewSoracloudHfSharedLease),
            visit_advertise_soracloud_inrou_host(&AdvertiseSoracloudInrouHost),
            visit_withdraw_soracloud_inrou_host(&WithdrawSoracloudInrouHost),
            visit_reconcile_soracloud_inrou_placements(&ReconcileSoracloudInrouPlacements),
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
            visit_finalize_soracloud_uploaded_model_bundle(&FinalizeSoracloudUploadedModelBundle),
            visit_advance_soracloud_rollout(&AdvanceSoracloudRollout),
            visit_set_soracloud_runtime_state(&SetSoracloudRuntimeState),
            visit_set_soracloud_inrou_replica_runtime_state(&SetSoracloudInrouReplicaRuntimeState),
            visit_clear_soracloud_inrou_replica_runtime_state(&ClearSoracloudInrouReplicaRuntimeState),
            visit_report_soracloud_service_lease_usage(&ReportSoracloudServiceLeaseUsage),
            visit_record_soracloud_mailbox_message(&RecordSoracloudMailboxMessage),
            visit_record_soracloud_runtime_receipt(&RecordSoracloudRuntimeReceipt),
            visit_apply_soracloud_ordered_mailbox_result(&ApplySoracloudOrderedMailboxResult),
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
    use crate::{
        isi::privacy::{
            RegisterPrivacyBootleLanternIssuerPolicyV1, RegisterPrivacyVegaIssuerV1,
            RegisterPrivacyZkX509CertificatePolicyV1, RegisterPrivacyZkX509CrlV1,
            RegisterPrivacyZkX509TrustAnchorV1, RevokePrivacyBootleLanternIssuerPolicyV1,
            RevokePrivacyVegaIssuerV1, RevokePrivacyZkX509CertificatePolicyV1,
            RevokePrivacyZkX509CrlV1, RevokePrivacyZkX509TrustAnchorV1,
            RotatePrivacyBootleLanternIssuerPolicyV1, RotatePrivacyVegaIssuerV1,
            RotatePrivacyZkX509CertificatePolicyV1, RotatePrivacyZkX509CrlV1,
            RotatePrivacyZkX509TrustAnchorV1,
        },
        prelude::*,
        privacy::{
            BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BOOTLE_LANTERN_RING_DEGREE_V1,
            BootleLanternAllowedAttributeValuesV1, BootleLanternIssuerPolicyLifecycleV1,
            BootleLanternIssuerPolicyV1, BootleLanternIssuerPublicMatrixV1,
            BootleLanternPolynomialV1, PrivacyBootleLanternIssuerPolicyDigestV1,
            PrivacyCredentialDocumentTypeV1, PrivacyIssuerIdV1, PrivacyP256PointV1,
            PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyDigestV1,
            PrivacyPolicyIdV1, PrivacyRootV1, PrivacyVegaIssuerRecordDigestV1,
            PrivacyVegaIssuerRecordLifecycleV1, PrivacyVegaIssuerRecordV1,
            PrivacyVegaMdlDigestAlgorithmV1, PrivacyVegaMdlNamespaceV1,
            PrivacyVegaMdlSignatureAlgorithmV1, PrivacyX509CrlDerDigestV1,
            PrivacyX509CrlIssuerSpkiDigestV1, PrivacyX509ExtendedKeyUsageV1,
            PrivacyX509KeyUsageRequirementV1, PrivacyX509KeyUsageV1, PrivacyX509TrustStoreDigestV1,
            PrivacyZkX509CertificatePolicyRecordV1, PrivacyZkX509CrlRecordV1,
            PrivacyZkX509RecordLifecycleV1, PrivacyZkX509TrustAnchorRecordV1,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair};
    struct CountingVisitor {
        logs: usize,
    }
    impl Visit for CountingVisitor {
        fn visit_log(&mut self, _: &Log) {
            self.logs += 1;
        }
    }
    fn redigest_bootle_visitor_policy(policy: &mut BootleLanternIssuerPolicyV1) {
        policy.issuer_parameter_digest = policy
            .computed_issuer_parameter_digest()
            .expect("visitor issuer parameter encodes");
        policy.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        policy.record_digest = policy
            .computed_record_digest()
            .expect("visitor issuer policy encodes");
    }
    fn bootle_visitor_public_matrix(seed: u16) -> BootleLanternIssuerPublicMatrixV1 {
        let first_column = core::array::from_fn(|block| BootleLanternPolynomialV1 {
            coefficients: (0..BOOTLE_LANTERN_RING_DEGREE_V1)
                .map(|coefficient| {
                    seed + u16::try_from(block * BOOTLE_LANTERN_RING_DEGREE_V1 + coefficient)
                        .expect("degree-512 coefficient index fits u16")
                })
                .collect(),
        });
        BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(&first_column)
            .expect("canonical visitor degree-512 multiplication matrix")
    }
    fn bootle_visitor_policy() -> BootleLanternIssuerPolicyV1 {
        let mut policy = BootleLanternIssuerPolicyV1 {
            issuer_id: PrivacyIssuerIdV1::new([0x21; 32]),
            policy_id: PrivacyPolicyIdV1::new([0x22; 32]),
            epoch: 1,
            lifecycle: BootleLanternIssuerPolicyLifecycleV1::Active,
            issuer_parameter_id: PrivacyParameterIdV1::new([0x23; 32]),
            issuer_parameter_digest: PrivacyParameterDigestV1::new([0; 32]),
            issuer_public_matrix: bootle_visitor_public_matrix(1),
            required_disclosure_bitmap: 0,
            allowed_values: vec![
                BootleLanternAllowedAttributeValuesV1 { values: Vec::new() };
                BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1
            ],
            record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
        };
        redigest_bootle_visitor_policy(&mut policy);
        policy
            .validate_initial()
            .expect("canonical Bootle/Lantern visitor fixture");
        policy
    }
    fn vega_visitor_record(
        epoch: u64,
        key_seed: u8,
        previous_record_digest: Option<PrivacyVegaIssuerRecordDigestV1>,
        lifecycle: PrivacyVegaIssuerRecordLifecycleV1,
    ) -> PrivacyVegaIssuerRecordV1 {
        let mut key = [key_seed; 33];
        key[0] = 0x02;
        PrivacyVegaIssuerRecordV1::new(
            PrivacyIssuerIdV1::new([0x31; 32]),
            epoch,
            PrivacyP256PointV1::new(key),
            PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl,
            PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1,
            PrivacyVegaMdlDigestAlgorithmV1::Sha256,
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical structural Vega visitor fixture")
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
    fn unclassified_native_instruction_uses_fallback_hook() {
        struct FallbackVisitor {
            calls: usize,
        }
        impl Visit for FallbackVisitor {
            fn visit_unclassified_instruction(&mut self, _: &InstructionBox) {
                self.calls += 1;
            }
        }
        let isi = Box::new(crate::isi::ram_lfe::ActivateRamLfeProgramPolicy {
            program_id: "visitor_fallback"
                .parse()
                .expect("valid RAM-LFE program id"),
        })
        .into_instruction_box();
        let mut visitor = FallbackVisitor { calls: 0 };
        visit_instruction(&mut visitor, &isi);
        assert_eq!(visitor.calls, 1);
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
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let key_pair = KeyPair::try_from_seed(vec![0x11; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        let validator = AccountId::new(key_pair.public_key().clone());
        let instruction = RegisterPublicLaneValidator::new(
            LaneId::SINGLE,
            validator.clone(),
            PeerId::from(validator.expect_single_signatory().clone()),
            validator,
            Quantity::from(1_u64),
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
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::try_from_seed(vec![0x12; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        let peer_key = KeyPair::try_from_seed(vec![0x13; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
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
    #[test]
    fn visit_all_bootle_lantern_governance_instructions_dispatches_from_boxes() {
        struct BootleVisitor {
            calls: Vec<&'static str>,
        }
        impl Visit for BootleVisitor {
            fn visit_register_privacy_bootle_lantern_issuer_policy_v1(
                &mut self,
                _: &RegisterPrivacyBootleLanternIssuerPolicyV1,
            ) {
                self.calls.push("register");
            }
            fn visit_rotate_privacy_bootle_lantern_issuer_policy_v1(
                &mut self,
                _: &RotatePrivacyBootleLanternIssuerPolicyV1,
            ) {
                self.calls.push("rotate");
            }
            fn visit_revoke_privacy_bootle_lantern_issuer_policy_v1(
                &mut self,
                _: &RevokePrivacyBootleLanternIssuerPolicyV1,
            ) {
                self.calls.push("revoke");
            }
        }
        let current = bootle_visitor_policy();
        let mut rotated = current.clone();
        rotated.epoch += 1;
        rotated.issuer_parameter_id.0[0] ^= 1;
        rotated.issuer_public_matrix = bootle_visitor_public_matrix(701);
        redigest_bootle_visitor_policy(&mut rotated);
        rotated
            .validate_rotation_successor(&current)
            .expect("canonical Bootle/Lantern rotation visitor fixture");
        let mut revoked = current.clone();
        revoked.epoch += 1;
        revoked.lifecycle = BootleLanternIssuerPolicyLifecycleV1::Revoked;
        redigest_bootle_visitor_policy(&mut revoked);
        revoked
            .validate_revocation_successor(&current)
            .expect("canonical Bootle/Lantern revocation visitor fixture");
        let instructions: Vec<InstructionBox> = vec![
            RegisterPrivacyBootleLanternIssuerPolicyV1::new(current.clone()).into(),
            RotatePrivacyBootleLanternIssuerPolicyV1::new(current.record_digest, rotated).into(),
            RevokePrivacyBootleLanternIssuerPolicyV1::new(current.record_digest, revoked).into(),
        ];
        let mut visitor = BootleVisitor { calls: Vec::new() };
        for instruction in &instructions {
            visit_instruction(&mut visitor, instruction);
        }
        assert_eq!(visitor.calls, ["register", "rotate", "revoke"]);
    }
    #[test]
    fn visit_all_vega_governance_instructions_dispatches_from_boxes() {
        struct VegaVisitor {
            calls: Vec<&'static str>,
        }
        impl Visit for VegaVisitor {
            fn visit_register_privacy_vega_issuer_v1(&mut self, _: &RegisterPrivacyVegaIssuerV1) {
                self.calls.push("register");
            }
            fn visit_rotate_privacy_vega_issuer_v1(&mut self, _: &RotatePrivacyVegaIssuerV1) {
                self.calls.push("rotate");
            }
            fn visit_revoke_privacy_vega_issuer_v1(&mut self, _: &RevokePrivacyVegaIssuerV1) {
                self.calls.push("revoke");
            }
        }
        let current =
            vega_visitor_record(1, 0x41, None, PrivacyVegaIssuerRecordLifecycleV1::Active);
        let rotated = vega_visitor_record(
            2,
            0x42,
            Some(current.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        );
        let revoked = vega_visitor_record(
            2,
            0x41,
            Some(current.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Revoked,
        );
        let instructions: Vec<InstructionBox> = vec![
            RegisterPrivacyVegaIssuerV1::new(current).into(),
            RotatePrivacyVegaIssuerV1::new(current.record_digest, rotated).into(),
            RevokePrivacyVegaIssuerV1::new(current.record_digest, revoked).into(),
        ];
        let mut visitor = VegaVisitor { calls: Vec::new() };
        for instruction in &instructions {
            visit_instruction(&mut visitor, instruction);
        }
        assert_eq!(visitor.calls, ["register", "rotate", "revoke"]);
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive X.509 governance inventory verifies every register, rotate, and revoke visitor hook"
    )]
    fn visit_all_x509_governance_instructions_dispatches_from_boxes() {
        struct X509Visitor {
            calls: Vec<&'static str>,
        }
        impl Visit for X509Visitor {
            fn visit_register_privacy_zk_x509_trust_anchor_v1(
                &mut self,
                _: &RegisterPrivacyZkX509TrustAnchorV1,
            ) {
                self.calls.push("register-trust-anchor");
            }
            fn visit_rotate_privacy_zk_x509_trust_anchor_v1(
                &mut self,
                _: &RotatePrivacyZkX509TrustAnchorV1,
            ) {
                self.calls.push("rotate-trust-anchor");
            }
            fn visit_revoke_privacy_zk_x509_trust_anchor_v1(
                &mut self,
                _: &RevokePrivacyZkX509TrustAnchorV1,
            ) {
                self.calls.push("revoke-trust-anchor");
            }
            fn visit_register_privacy_zk_x509_certificate_policy_v1(
                &mut self,
                _: &RegisterPrivacyZkX509CertificatePolicyV1,
            ) {
                self.calls.push("register-certificate-policy");
            }
            fn visit_rotate_privacy_zk_x509_certificate_policy_v1(
                &mut self,
                _: &RotatePrivacyZkX509CertificatePolicyV1,
            ) {
                self.calls.push("rotate-certificate-policy");
            }
            fn visit_revoke_privacy_zk_x509_certificate_policy_v1(
                &mut self,
                _: &RevokePrivacyZkX509CertificatePolicyV1,
            ) {
                self.calls.push("revoke-certificate-policy");
            }
            fn visit_register_privacy_zk_x509_crl_v1(&mut self, _: &RegisterPrivacyZkX509CrlV1) {
                self.calls.push("register-crl");
            }
            fn visit_rotate_privacy_zk_x509_crl_v1(&mut self, _: &RotatePrivacyZkX509CrlV1) {
                self.calls.push("rotate-crl");
            }
            fn visit_revoke_privacy_zk_x509_crl_v1(&mut self, _: &RevokePrivacyZkX509CrlV1) {
                self.calls.push("revoke-crl");
            }
        }
        let trust_anchor_id = PrivacyIssuerIdV1::new([0x11; 32]);
        let policy_id = PrivacyPolicyIdV1::new([0x12; 32]);
        let trust_anchor = PrivacyZkX509TrustAnchorRecordV1::new(
            trust_anchor_id,
            1,
            PrivacyX509TrustStoreDigestV1::new([0x13; 32]),
            PrivacyRootV1::new([0x14; 32]),
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("valid X.509 trust-anchor visitor fixture");
        let policy = PrivacyZkX509CertificatePolicyRecordV1::new(
            trust_anchor_id,
            policy_id,
            1,
            PrivacyPolicyDigestV1::new([0x15; 32]),
            PrivacyX509KeyUsageV1 {
                digital_signature: PrivacyX509KeyUsageRequirementV1::new(true),
                content_commitment: PrivacyX509KeyUsageRequirementV1::new(false),
                key_encipherment: PrivacyX509KeyUsageRequirementV1::new(false),
                key_agreement: PrivacyX509KeyUsageRequirementV1::new(false),
            },
            vec![PrivacyX509ExtendedKeyUsageV1::ClientAuthentication],
            vec![0],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("valid X.509 certificate-policy visitor fixture");
        let crl = PrivacyZkX509CrlRecordV1::new(
            trust_anchor_id,
            policy_id,
            1,
            1,
            PrivacyX509CrlDerDigestV1::digest_exact_der(b"visitor CRL DER fixture"),
            PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(b"visitor SPKI DER fixture"),
            1_000,
            1_300,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("valid X.509 CRL visitor fixture");
        let instructions: Vec<InstructionBox> = vec![
            RegisterPrivacyZkX509TrustAnchorV1::new(trust_anchor).into(),
            RotatePrivacyZkX509TrustAnchorV1::new(trust_anchor.record_digest, trust_anchor).into(),
            RevokePrivacyZkX509TrustAnchorV1::new(trust_anchor.record_digest, trust_anchor).into(),
            RegisterPrivacyZkX509CertificatePolicyV1::new(policy.clone()).into(),
            RotatePrivacyZkX509CertificatePolicyV1::new(policy.record_digest, policy.clone())
                .into(),
            RevokePrivacyZkX509CertificatePolicyV1::new(policy.record_digest, policy).into(),
            RegisterPrivacyZkX509CrlV1::new(crl).into(),
            RotatePrivacyZkX509CrlV1::new(crl.record_digest, crl).into(),
            RevokePrivacyZkX509CrlV1::new(crl.record_digest, crl).into(),
        ];
        let mut visitor = X509Visitor { calls: Vec::new() };
        for instruction in &instructions {
            visit_instruction(&mut visitor, instruction);
        }
        assert_eq!(
            visitor.calls,
            [
                "register-trust-anchor",
                "rotate-trust-anchor",
                "revoke-trust-anchor",
                "register-certificate-policy",
                "rotate-certificate-policy",
                "revoke-certificate-policy",
                "register-crl",
                "rotate-crl",
                "revoke-crl",
            ]
        );
    }
}

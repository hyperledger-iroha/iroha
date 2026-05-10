#[cfg(feature = "governance")]
use crate::isi::governance;
use crate::{
    isi::{
        InstructionRegistry, account_alias_lease, account_recovery, asset_alias,
        asset_transfer_control, bridge, confidential, consensus_keys, content, contract_alias,
        domain_link, endorsement, escrow, identifier, kaigi, ministry, musubi, nexus, offline,
        oracle, ram_lfe, repo, runtime_upgrade, rwa, settlement, smart_contract_code, sns, social,
        soracloud, soradns, sorafs, space_directory,
        transparent::{
            AddSignatory, InvalidInstruction, RemoveAssetKeyValue, RemoveSignatory,
            SetAccountQuorum, SetAssetKeyValue,
        },
        verifying_keys, vpn, zk,
    },
    prelude::*,
};

/// Signature of helper functions that register instructions into [`InstructionRegistry`].
type Registrar = fn(InstructionRegistry) -> InstructionRegistry;

/// Built-in instruction registrations that make up the default registry used by Iroha.
const ALL_REGISTRARS: &[Registrar] = &[
    InstructionRegistry::register_slice::<RegisterBox>,
    InstructionRegistry::register_slice::<UnregisterBox>,
    InstructionRegistry::register_slice::<MintBox>,
    InstructionRegistry::register_slice::<BurnBox>,
    InstructionRegistry::register_slice::<TransferAssetBatch>,
    InstructionRegistry::register_slice::<TransferBox>,
    InstructionRegistry::register_slice::<asset_transfer_control::SetAssetTransferFreeze>,
    InstructionRegistry::register_slice::<asset_transfer_control::SetAssetTransferBlacklist>,
    InstructionRegistry::register_slice::<asset_transfer_control::SetAssetTransferControl>,
    InstructionRegistry::register_slice::<rwa::RwaInstructionBox>,
    InstructionRegistry::register_slice::<repo::RepoInstructionBox>,
    InstructionRegistry::register_slice::<settlement::SettlementInstructionBox>,
    InstructionRegistry::register_slice::<SetParameter>,
    InstructionRegistry::register_slice::<SetKeyValueBox>,
    InstructionRegistry::register_slice::<AddSignatory>,
    InstructionRegistry::register_slice::<RemoveSignatory>,
    InstructionRegistry::register_slice::<SetAccountQuorum>,
    InstructionRegistry::register_slice::<SetAssetKeyValue>,
    InstructionRegistry::register_slice::<RemoveKeyValueBox>,
    InstructionRegistry::register_slice::<RemoveAssetKeyValue>,
    InstructionRegistry::register_slice::<GrantBox>,
    InstructionRegistry::register_slice::<RevokeBox>,
    InstructionRegistry::register_slice::<offline::IssueOfflineNoteV2>,
    InstructionRegistry::register_slice::<offline::RedeemOfflineNoteV2>,
    InstructionRegistry::register_slice::<offline::AuditOfflineNoteV2>,
    InstructionRegistry::register_slice::<asset_alias::SetAssetDefinitionBalancePolicy>,
    InstructionRegistry::register_slice::<crate::isi::staking::RegisterPublicLaneValidator>,
    InstructionRegistry::register_slice::<crate::isi::staking::RebindPublicLaneValidatorPeer>,
    InstructionRegistry::register_slice::<crate::isi::staking::ActivatePublicLaneValidator>,
    InstructionRegistry::register_slice::<crate::isi::staking::ExitPublicLaneValidator>,
    InstructionRegistry::register::<crate::isi::staking::BondPublicLaneStake>,
    InstructionRegistry::register::<crate::isi::staking::SchedulePublicLaneUnbond>,
    InstructionRegistry::register::<crate::isi::staking::FinalizePublicLaneUnbond>,
    InstructionRegistry::register::<crate::isi::staking::SlashPublicLaneValidator>,
    InstructionRegistry::register_slice::<crate::isi::staking::CancelConsensusEvidencePenalty>,
    InstructionRegistry::register::<crate::isi::staking::RecordPublicLaneRewards>,
    InstructionRegistry::register::<crate::isi::staking::ClaimPublicLaneRewards>,
    InstructionRegistry::register_slice::<nexus::SetLaneRelayEmergencyValidators>,
    InstructionRegistry::register_slice::<nexus::RegisterVerifiedLaneRelay>,
    InstructionRegistry::register_slice::<nexus::RegisterVerifiedNexusFeeBudget>,
    InstructionRegistry::register_slice::<oracle::RegisterOracleFeed>,
    InstructionRegistry::register_slice::<oracle::SubmitOracleObservation>,
    InstructionRegistry::register_slice::<oracle::AggregateOracleFeed>,
    InstructionRegistry::register_slice::<oracle::OpenOracleDispute>,
    InstructionRegistry::register_slice::<oracle::ResolveOracleDispute>,
    InstructionRegistry::register_slice::<oracle::ProposeOracleChange>,
    InstructionRegistry::register_slice::<oracle::VoteOracleChangeStage>,
    InstructionRegistry::register_slice::<oracle::RollbackOracleChange>,
    InstructionRegistry::register_slice::<oracle::RecordTwitterBinding>,
    InstructionRegistry::register_slice::<oracle::RevokeTwitterBinding>,
    InstructionRegistry::register_slice::<social::ClaimTwitterFollowReward>,
    InstructionRegistry::register_slice::<social::SendToTwitter>,
    InstructionRegistry::register_slice::<social::CancelTwitterEscrow>,
    InstructionRegistry::register_slice::<escrow::OpenAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::AcceptAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::MarkEscrowPaymentSent>,
    InstructionRegistry::register_slice::<escrow::ReleaseAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::CancelAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::OpenEscrowDispute>,
    InstructionRegistry::register_slice::<escrow::ResolveEscrowDispute>,
    InstructionRegistry::register_slice::<escrow::OpenAnonymousAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::AcceptAnonymousAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::MarkAnonymousEscrowPaymentSent>,
    InstructionRegistry::register_slice::<escrow::ReleaseAnonymousAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::CancelAnonymousAssetEscrow>,
    InstructionRegistry::register_slice::<escrow::OpenAnonymousEscrowDispute>,
    InstructionRegistry::register_slice::<escrow::ResolveAnonymousEscrowDispute>,
    InstructionRegistry::register_slice::<vpn::OpenVpnLeaseEscrow>,
    InstructionRegistry::register_slice::<vpn::SettleVpnLease>,
    InstructionRegistry::register_slice::<vpn::RefundExpiredVpnLease>,
    InstructionRegistry::register_slice::<soracloud::DeploySoracloudService>,
    InstructionRegistry::register_slice::<soracloud::UpgradeSoracloudService>,
    InstructionRegistry::register_slice::<soracloud::RollbackSoracloudService>,
    InstructionRegistry::register_slice::<soracloud::SetSoracloudServiceConfig>,
    InstructionRegistry::register_slice::<soracloud::DeleteSoracloudServiceConfig>,
    InstructionRegistry::register_slice::<soracloud::SetSoracloudServiceSecret>,
    InstructionRegistry::register_slice::<soracloud::DeleteSoracloudServiceSecret>,
    InstructionRegistry::register_slice::<soracloud::MutateSoracloudState>,
    InstructionRegistry::register_slice::<soracloud::RunSoracloudFheJob>,
    InstructionRegistry::register_slice::<soracloud::RecordSoracloudDecryptionRequest>,
    InstructionRegistry::register_slice::<soracloud::JoinSoracloudHfSharedLease>,
    InstructionRegistry::register_slice::<soracloud::LeaveSoracloudHfSharedLease>,
    InstructionRegistry::register_slice::<soracloud::RenewSoracloudHfSharedLease>,
    InstructionRegistry::register_slice::<soracloud::AdvertiseSoracloudModelHost>,
    InstructionRegistry::register_slice::<soracloud::HeartbeatSoracloudModelHost>,
    InstructionRegistry::register_slice::<soracloud::WithdrawSoracloudModelHost>,
    InstructionRegistry::register_slice::<soracloud::ReconcileSoracloudModelHosts>,
    InstructionRegistry::register_slice::<soracloud::AdvertiseSoracloudInrouHost>,
    InstructionRegistry::register_slice::<soracloud::WithdrawSoracloudInrouHost>,
    InstructionRegistry::register_slice::<soracloud::ReconcileSoracloudInrouPlacements>,
    InstructionRegistry::register_slice::<soracloud::ReportSoracloudModelHostViolation>,
    InstructionRegistry::register_slice::<soracloud::DeploySoracloudAgentApartment>,
    InstructionRegistry::register_slice::<soracloud::RenewSoracloudAgentLease>,
    InstructionRegistry::register_slice::<soracloud::RestartSoracloudAgentApartment>,
    InstructionRegistry::register_slice::<soracloud::RevokeSoracloudAgentPolicy>,
    InstructionRegistry::register_slice::<soracloud::RequestSoracloudAgentWalletSpend>,
    InstructionRegistry::register_slice::<soracloud::ApproveSoracloudAgentWalletSpend>,
    InstructionRegistry::register_slice::<soracloud::EnqueueSoracloudAgentMessage>,
    InstructionRegistry::register_slice::<soracloud::AcknowledgeSoracloudAgentMessage>,
    InstructionRegistry::register_slice::<soracloud::AllowSoracloudAgentAutonomyArtifact>,
    InstructionRegistry::register_slice::<soracloud::RunSoracloudAgentAutonomy>,
    InstructionRegistry::register_slice::<soracloud::RecordSoracloudAgentAutonomyExecution>,
    InstructionRegistry::register_slice::<soracloud::StartSoracloudTrainingJob>,
    InstructionRegistry::register_slice::<soracloud::CheckpointSoracloudTrainingJob>,
    InstructionRegistry::register_slice::<soracloud::RetrySoracloudTrainingJob>,
    InstructionRegistry::register_slice::<soracloud::RegisterSoracloudModelArtifact>,
    InstructionRegistry::register_slice::<soracloud::RegisterSoracloudModelWeight>,
    InstructionRegistry::register_slice::<soracloud::PromoteSoracloudModelWeight>,
    InstructionRegistry::register_slice::<soracloud::RollbackSoracloudModelWeight>,
    InstructionRegistry::register_slice::<soracloud::RegisterSoracloudUploadedModelBundle>,
    InstructionRegistry::register_slice::<soracloud::AppendSoracloudUploadedModelChunk>,
    InstructionRegistry::register_slice::<soracloud::FinalizeSoracloudUploadedModelBundle>,
    InstructionRegistry::register_slice::<soracloud::AdmitSoracloudPrivateCompileProfile>,
    InstructionRegistry::register_slice::<soracloud::AllowSoracloudUploadedModel>,
    InstructionRegistry::register_slice::<soracloud::StartSoracloudPrivateInference>,
    InstructionRegistry::register_slice::<soracloud::RecordSoracloudPrivateInferenceCheckpoint>,
    InstructionRegistry::register_slice::<soracloud::AdvanceSoracloudRollout>,
    InstructionRegistry::register_slice::<soracloud::SetSoracloudRuntimeState>,
    InstructionRegistry::register_slice::<soracloud::SetSoracloudInrouReplicaRuntimeState>,
    InstructionRegistry::register_slice::<soracloud::ClearSoracloudInrouReplicaRuntimeState>,
    InstructionRegistry::register_slice::<soracloud::ReportSoracloudServiceLeaseUsage>,
    InstructionRegistry::register_slice::<soracloud::RecordSoracloudMailboxMessage>,
    InstructionRegistry::register_slice::<soracloud::RecordSoracloudRuntimeReceipt>,
    InstructionRegistry::register_slice::<ExecuteTrigger>,
    InstructionRegistry::register_slice::<Upgrade>,
    InstructionRegistry::register_slice::<Log>,
    InstructionRegistry::register_slice::<CustomInstruction>,
    InstructionRegistry::register_slice::<InvalidInstruction>,
    InstructionRegistry::register_slice::<verifying_keys::RegisterVerifyingKey>,
    InstructionRegistry::register_slice::<verifying_keys::UpdateVerifyingKey>,
    InstructionRegistry::register_slice::<consensus_keys::RegisterConsensusKey>,
    InstructionRegistry::register_slice::<consensus_keys::RotateConsensusKey>,
    InstructionRegistry::register_slice::<consensus_keys::DisableConsensusKey>,
    InstructionRegistry::register_slice::<endorsement::RegisterDomainCommittee>,
    InstructionRegistry::register_slice::<endorsement::SetDomainEndorsementPolicy>,
    InstructionRegistry::register_slice::<endorsement::SubmitDomainEndorsement>,
    InstructionRegistry::register_slice::<domain_link::SetAccountAliasBinding>,
    InstructionRegistry::register_slice::<domain_link::SetPrimaryAccountAlias>,
    InstructionRegistry::register_slice::<account_alias_lease::AcquireAccountAliasLease>,
    InstructionRegistry::register_slice::<account_alias_lease::RenewAccountAliasLease>,
    InstructionRegistry::register_slice::<sns::RegisterSnsName>,
    InstructionRegistry::register_slice::<sns::RenewSnsName>,
    InstructionRegistry::register_slice::<sns::TransferSnsName>,
    InstructionRegistry::register_slice::<sns::UpdateSnsNameControllers>,
    InstructionRegistry::register_slice::<sns::FreezeSnsName>,
    InstructionRegistry::register_slice::<sns::UnfreezeSnsName>,
    InstructionRegistry::register_slice::<account_recovery::ReplaceAccountController>,
    InstructionRegistry::register_slice::<account_recovery::SetAccountRecoveryPolicy>,
    InstructionRegistry::register_slice::<account_recovery::ClearAccountRecoveryPolicy>,
    InstructionRegistry::register_slice::<account_recovery::ProposeAccountRecovery>,
    InstructionRegistry::register_slice::<account_recovery::ApproveAccountRecovery>,
    InstructionRegistry::register_slice::<account_recovery::CancelAccountRecovery>,
    InstructionRegistry::register_slice::<account_recovery::FinalizeAccountRecovery>,
    InstructionRegistry::register_slice::<contract_alias::SetContractAlias>,
    InstructionRegistry::register_slice::<musubi::PublishMusubiRelease>,
    InstructionRegistry::register_slice::<musubi::YankMusubiRelease>,
    InstructionRegistry::register_slice::<musubi::SetMusubiShortAlias>,
    InstructionRegistry::register_slice::<musubi::AssertMusubiReleaseExists>,
    InstructionRegistry::register_slice::<ram_lfe::RegisterRamLfeProgramPolicy>,
    InstructionRegistry::register_slice::<ram_lfe::ActivateRamLfeProgramPolicy>,
    InstructionRegistry::register_slice::<ram_lfe::DeactivateRamLfeProgramPolicy>,
    InstructionRegistry::register_slice::<identifier::RegisterIdentifierPolicy>,
    InstructionRegistry::register_slice::<identifier::ActivateIdentifierPolicy>,
    InstructionRegistry::register_slice::<identifier::ClaimIdentifier>,
    InstructionRegistry::register_slice::<identifier::RevokeIdentifier>,
    InstructionRegistry::register_slice::<asset_alias::SetAssetDefinitionAlias>,
    InstructionRegistry::register_slice::<sorafs::RegisterPinManifest>,
    InstructionRegistry::register_slice::<sorafs::ApprovePinManifest>,
    InstructionRegistry::register_slice::<sorafs::RetirePinManifest>,
    InstructionRegistry::register_slice::<sorafs::BindManifestAlias>,
    InstructionRegistry::register_slice::<sorafs::RegisterCapacityDeclaration>,
    InstructionRegistry::register_slice::<sorafs::RecordCapacityTelemetry>,
    InstructionRegistry::register_slice::<sorafs::RegisterCapacityDispute>,
    InstructionRegistry::register_slice::<sorafs::IssueReplicationOrder>,
    InstructionRegistry::register_slice::<sorafs::CompleteReplicationOrder>,
    InstructionRegistry::register_slice::<sorafs::RegisterProviderOwner>,
    InstructionRegistry::register_slice::<sorafs::UnregisterProviderOwner>,
    InstructionRegistry::register_slice::<sorafs::SetPricingSchedule>,
    InstructionRegistry::register_slice::<sorafs::UpsertProviderCredit>,
    InstructionRegistry::register::<content::PublishContentBundle>,
    InstructionRegistry::register::<content::RetireContentBundle>,
    InstructionRegistry::register::<soradns::SubmitDirectoryDraft>,
    InstructionRegistry::register::<soradns::PublishDirectory>,
    InstructionRegistry::register::<soradns::RevokeResolver>,
    InstructionRegistry::register::<soradns::UnrevokeResolver>,
    InstructionRegistry::register::<soradns::AddReleaseSigner>,
    InstructionRegistry::register::<soradns::RemoveReleaseSigner>,
    InstructionRegistry::register::<soradns::SetDirectoryRotationPolicy>,
    InstructionRegistry::register_slice::<space_directory::PublishSpaceDirectoryManifest>,
    InstructionRegistry::register_slice::<space_directory::RevokeSpaceDirectoryManifest>,
    InstructionRegistry::register_slice::<space_directory::ExpireSpaceDirectoryManifest>,
    InstructionRegistry::register_slice::<smart_contract_code::RegisterSmartContractCode>,
    InstructionRegistry::register_slice::<smart_contract_code::DeactivateContractInstance>,
    InstructionRegistry::register_slice::<smart_contract_code::ActivateContractInstance>,
    InstructionRegistry::register_slice::<smart_contract_code::RegisterSmartContractBytes>,
    InstructionRegistry::register_slice::<smart_contract_code::RemoveSmartContractBytes>,
    InstructionRegistry::register_slice::<zk::VerifyProof>,
    InstructionRegistry::register_slice::<zk::PruneProofs>,
    InstructionRegistry::register_slice::<kaigi::CreateKaigi>,
    InstructionRegistry::register_slice::<kaigi::JoinKaigi>,
    InstructionRegistry::register_slice::<kaigi::LeaveKaigi>,
    InstructionRegistry::register_slice::<kaigi::EndKaigi>,
    InstructionRegistry::register_slice::<kaigi::RecordKaigiUsage>,
    InstructionRegistry::register_slice::<kaigi::SetKaigiRelayManifest>,
    InstructionRegistry::register_slice::<kaigi::RegisterKaigiRelay>,
    InstructionRegistry::register_slice::<kaigi::ReportKaigiRelayHealth>,
    InstructionRegistry::register_slice::<zk::RegisterZkAsset>,
    InstructionRegistry::register_slice::<zk::ScheduleConfidentialPolicyTransition>,
    InstructionRegistry::register_slice::<zk::CancelConfidentialPolicyTransition>,
    InstructionRegistry::register_slice::<zk::Shield>,
    InstructionRegistry::register_slice::<zk::ZkTransfer>,
    InstructionRegistry::register_slice::<zk::Unshield>,
    InstructionRegistry::register_slice::<zk::CreateElection>,
    InstructionRegistry::register_slice::<zk::SubmitBallot>,
    InstructionRegistry::register_slice::<zk::FinalizeElection>,
    InstructionRegistry::register_slice::<bridge::SubmitBridgeProof>,
    InstructionRegistry::register_slice::<bridge::RecordBridgeReceipt>,
    InstructionRegistry::register_slice::<bridge::RecordSccpMessage>,
    InstructionRegistry::register::<confidential::PublishPedersenParams>,
    InstructionRegistry::register::<confidential::SetPedersenParamsLifecycle>,
    InstructionRegistry::register::<confidential::PublishPoseidonParams>,
    InstructionRegistry::register::<confidential::SetPoseidonParamsLifecycle>,
    InstructionRegistry::register_slice::<ministry::SubmitAgendaProposal>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::ProposeDeployContract>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::ProposeRuntimeUpgradeProposal>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::CastZkBallot>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::CastPlainBallot>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::SlashGovernanceLock>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::RestituteGovernanceLock>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::EnactReferendum>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::FinalizeReferendum>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::ApproveGovernanceProposal>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::PersistCouncilForEpoch>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::RecordCitizenServiceOutcome>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::RegisterCitizen>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register_slice::<governance::UnregisterCitizen>,
    InstructionRegistry::register_slice::<runtime_upgrade::ProposeRuntimeUpgrade>,
    InstructionRegistry::register_slice::<runtime_upgrade::ActivateRuntimeUpgrade>,
    InstructionRegistry::register_slice::<runtime_upgrade::CancelRuntimeUpgrade>,
];

/// Create an [`InstructionRegistry`] populated with all instructions supported
/// by Iroha out of the box.
pub fn default() -> InstructionRegistry {
    let registry = apply_registrars(ALL_REGISTRARS.iter().copied());
    with_stable_ids(registry)
}

/// Apply every [`Registrar`] from the provided iterator to build an [`InstructionRegistry`].
fn apply_registrars(registrars: impl IntoIterator<Item = Registrar>) -> InstructionRegistry {
    registrars
        .into_iter()
        .fold(InstructionRegistry::new(), |registry, register| {
            register(registry)
        })
}

/// Attach stable wire identifiers for instructions that expose one explicitly.
fn with_stable_ids(registry: InstructionRegistry) -> InstructionRegistry {
    let registry = with_core_stable_ids(registry);
    let registry = with_soracloud_stable_ids(registry);
    let registry = with_consensus_stable_ids(registry);
    let registry = with_identity_stable_ids(registry);
    with_runtime_upgrade_stable_ids(registry)
}

fn with_core_stable_ids(mut registry: InstructionRegistry) -> InstructionRegistry {
    // Provide a stable wire id for a commonly used instruction as a starting point.
    // Others continue to use their Rust `type_name` as the wire id.
    registry = registry.register_with_id_slice::<Log>(Log::WIRE_ID);
    registry = registry.register_with_id_slice::<SetParameter>(SetParameter::WIRE_ID);
    registry = registry.register_with_id_slice::<ExecuteTrigger>(ExecuteTrigger::WIRE_ID);
    registry = registry.register_with_id_slice::<RegisterBox>(RegisterBox::WIRE_ID);
    registry = registry.register_with_id_slice::<UnregisterBox>(UnregisterBox::WIRE_ID);
    registry = registry.register_with_id_slice::<MintBox>(MintBox::WIRE_ID);
    registry = registry.register_with_id_slice::<BurnBox>(BurnBox::WIRE_ID);
    registry = registry.register_with_id_slice::<TransferBox>(TransferBox::WIRE_ID);
    registry = registry.register_with_id_slice::<TransferAssetBatch>(TransferAssetBatch::WIRE_ID);
    registry =
        registry.register_with_id_slice::<rwa::RwaInstructionBox>(rwa::RwaInstructionBox::WIRE_ID);
    registry = registry
        .register_with_id_slice::<repo::RepoInstructionBox>(repo::RepoInstructionBox::WIRE_ID);
    registry = registry.register_with_id_slice::<settlement::SettlementInstructionBox>(
        settlement::SettlementInstructionBox::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<zk::ScheduleConfidentialPolicyTransition>(
        "zk::ScheduleConfidentialPolicyTransition",
    );
    registry = registry.register_with_id_slice::<zk::CancelConfidentialPolicyTransition>(
        "zk::CancelConfidentialPolicyTransition",
    );
    registry = registry.register_with_id_slice::<SetKeyValueBox>(SetKeyValueBox::WIRE_ID);
    registry = registry.register_with_id_slice::<RemoveKeyValueBox>(RemoveKeyValueBox::WIRE_ID);
    registry = registry.register_with_id_slice::<GrantBox>(GrantBox::WIRE_ID);
    registry = registry.register_with_id_slice::<RevokeBox>(RevokeBox::WIRE_ID);
    registry = registry.register_with_id_slice::<musubi::PublishMusubiRelease>(
        musubi::PublishMusubiRelease::WIRE_ID,
    );
    registry = registry
        .register_with_id_slice::<musubi::YankMusubiRelease>(musubi::YankMusubiRelease::WIRE_ID);
    registry = registry.register_with_id_slice::<musubi::SetMusubiShortAlias>(
        musubi::SetMusubiShortAlias::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<musubi::AssertMusubiReleaseExists>(
        musubi::AssertMusubiReleaseExists::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<crate::isi::staking::ActivatePublicLaneValidator>(
        "iroha.staking.activate_public_lane_validator",
    );
    registry = registry
        .register_with_id_slice::<crate::isi::staking::RebindPublicLaneValidatorPeer>(
            "iroha.staking.rebind_public_lane_validator_peer",
        );
    registry = registry.register_with_id_slice::<crate::isi::staking::ExitPublicLaneValidator>(
        "iroha.staking.exit_public_lane_validator",
    );
    registry = registry.register_with_id_slice::<Upgrade>(Upgrade::WIRE_ID);
    registry = registry.register_with_id_slice::<CustomInstruction>(CustomInstruction::WIRE_ID);
    registry = registry.register_with_id_slice::<InvalidInstruction>(InvalidInstruction::WIRE_ID);
    registry
}

#[allow(clippy::too_many_lines)]
fn with_soracloud_stable_ids(mut registry: InstructionRegistry) -> InstructionRegistry {
    registry = registry.register_with_id_slice::<soracloud::DeploySoracloudService>(
        "soracloud::DeploySoracloudService",
    );
    registry = registry.register_with_id_slice::<soracloud::UpgradeSoracloudService>(
        "soracloud::UpgradeSoracloudService",
    );
    registry = registry.register_with_id_slice::<soracloud::RollbackSoracloudService>(
        "soracloud::RollbackSoracloudService",
    );
    registry = registry.register_with_id_slice::<soracloud::MutateSoracloudState>(
        "soracloud::MutateSoracloudState",
    );
    registry = registry
        .register_with_id_slice::<soracloud::RunSoracloudFheJob>("soracloud::RunSoracloudFheJob");
    registry = registry.register_with_id_slice::<soracloud::RecordSoracloudDecryptionRequest>(
        "soracloud::RecordSoracloudDecryptionRequest",
    );
    registry = registry.register_with_id_slice::<soracloud::JoinSoracloudHfSharedLease>(
        "soracloud::JoinSoracloudHfSharedLease",
    );
    registry = registry.register_with_id_slice::<soracloud::LeaveSoracloudHfSharedLease>(
        "soracloud::LeaveSoracloudHfSharedLease",
    );
    registry = registry.register_with_id_slice::<soracloud::RenewSoracloudHfSharedLease>(
        "soracloud::RenewSoracloudHfSharedLease",
    );
    registry = registry.register_with_id_slice::<soracloud::AdvertiseSoracloudModelHost>(
        "soracloud::AdvertiseSoracloudModelHost",
    );
    registry = registry.register_with_id_slice::<soracloud::HeartbeatSoracloudModelHost>(
        "soracloud::HeartbeatSoracloudModelHost",
    );
    registry = registry.register_with_id_slice::<soracloud::WithdrawSoracloudModelHost>(
        "soracloud::WithdrawSoracloudModelHost",
    );
    registry = registry.register_with_id_slice::<soracloud::AdvertiseSoracloudInrouHost>(
        "soracloud::AdvertiseSoracloudInrouHost",
    );
    registry = registry.register_with_id_slice::<soracloud::WithdrawSoracloudInrouHost>(
        "soracloud::WithdrawSoracloudInrouHost",
    );
    registry = registry.register_with_id_slice::<soracloud::ReconcileSoracloudInrouPlacements>(
        "soracloud::ReconcileSoracloudInrouPlacements",
    );
    registry = registry.register_with_id_slice::<soracloud::DeploySoracloudAgentApartment>(
        "soracloud::DeploySoracloudAgentApartment",
    );
    registry = registry.register_with_id_slice::<soracloud::RenewSoracloudAgentLease>(
        "soracloud::RenewSoracloudAgentLease",
    );
    registry = registry.register_with_id_slice::<soracloud::RestartSoracloudAgentApartment>(
        "soracloud::RestartSoracloudAgentApartment",
    );
    registry = registry.register_with_id_slice::<soracloud::RevokeSoracloudAgentPolicy>(
        "soracloud::RevokeSoracloudAgentPolicy",
    );
    registry = registry.register_with_id_slice::<soracloud::RequestSoracloudAgentWalletSpend>(
        "soracloud::RequestSoracloudAgentWalletSpend",
    );
    registry = registry.register_with_id_slice::<soracloud::ApproveSoracloudAgentWalletSpend>(
        "soracloud::ApproveSoracloudAgentWalletSpend",
    );
    registry = registry.register_with_id_slice::<soracloud::EnqueueSoracloudAgentMessage>(
        "soracloud::EnqueueSoracloudAgentMessage",
    );
    registry = registry.register_with_id_slice::<soracloud::AcknowledgeSoracloudAgentMessage>(
        "soracloud::AcknowledgeSoracloudAgentMessage",
    );
    registry = registry.register_with_id_slice::<soracloud::AllowSoracloudAgentAutonomyArtifact>(
        "soracloud::AllowSoracloudAgentAutonomyArtifact",
    );
    registry = registry.register_with_id_slice::<soracloud::RunSoracloudAgentAutonomy>(
        "soracloud::RunSoracloudAgentAutonomy",
    );
    registry = registry.register_with_id_slice::<soracloud::RecordSoracloudAgentAutonomyExecution>(
        "soracloud::RecordSoracloudAgentAutonomyExecution",
    );
    registry = registry.register_with_id_slice::<soracloud::StartSoracloudTrainingJob>(
        "soracloud::StartSoracloudTrainingJob",
    );
    registry = registry.register_with_id_slice::<soracloud::CheckpointSoracloudTrainingJob>(
        "soracloud::CheckpointSoracloudTrainingJob",
    );
    registry = registry.register_with_id_slice::<soracloud::RetrySoracloudTrainingJob>(
        "soracloud::RetrySoracloudTrainingJob",
    );
    registry = registry.register_with_id_slice::<soracloud::RegisterSoracloudModelArtifact>(
        "soracloud::RegisterSoracloudModelArtifact",
    );
    registry = registry.register_with_id_slice::<soracloud::RegisterSoracloudModelWeight>(
        "soracloud::RegisterSoracloudModelWeight",
    );
    registry = registry.register_with_id_slice::<soracloud::PromoteSoracloudModelWeight>(
        "soracloud::PromoteSoracloudModelWeight",
    );
    registry = registry.register_with_id_slice::<soracloud::RollbackSoracloudModelWeight>(
        "soracloud::RollbackSoracloudModelWeight",
    );
    registry = registry.register_with_id_slice::<soracloud::RegisterSoracloudUploadedModelBundle>(
        "soracloud::RegisterSoracloudUploadedModelBundle",
    );
    registry = registry.register_with_id_slice::<soracloud::AppendSoracloudUploadedModelChunk>(
        "soracloud::AppendSoracloudUploadedModelChunk",
    );
    registry = registry.register_with_id_slice::<soracloud::FinalizeSoracloudUploadedModelBundle>(
        "soracloud::FinalizeSoracloudUploadedModelBundle",
    );
    registry = registry.register_with_id_slice::<soracloud::AdmitSoracloudPrivateCompileProfile>(
        "soracloud::AdmitSoracloudPrivateCompileProfile",
    );
    registry = registry.register_with_id_slice::<soracloud::AllowSoracloudUploadedModel>(
        "soracloud::AllowSoracloudUploadedModel",
    );
    registry = registry.register_with_id_slice::<soracloud::StartSoracloudPrivateInference>(
        "soracloud::StartSoracloudPrivateInference",
    );
    registry = registry
        .register_with_id_slice::<soracloud::RecordSoracloudPrivateInferenceCheckpoint>(
            "soracloud::RecordSoracloudPrivateInferenceCheckpoint",
        );
    registry = registry.register_with_id_slice::<soracloud::AdvanceSoracloudRollout>(
        "soracloud::AdvanceSoracloudRollout",
    );
    registry = registry.register_with_id_slice::<soracloud::SetSoracloudRuntimeState>(
        "soracloud::SetSoracloudRuntimeState",
    );
    registry = registry.register_with_id_slice::<soracloud::SetSoracloudInrouReplicaRuntimeState>(
        "soracloud::SetSoracloudInrouReplicaRuntimeState",
    );
    registry = registry
        .register_with_id_slice::<soracloud::ClearSoracloudInrouReplicaRuntimeState>(
            "soracloud::ClearSoracloudInrouReplicaRuntimeState",
        );
    registry = registry.register_with_id_slice::<soracloud::ReportSoracloudServiceLeaseUsage>(
        "soracloud::ReportSoracloudServiceLeaseUsage",
    );
    registry = registry.register_with_id_slice::<soracloud::RecordSoracloudMailboxMessage>(
        "soracloud::RecordSoracloudMailboxMessage",
    );
    registry = registry.register_with_id_slice::<soracloud::RecordSoracloudRuntimeReceipt>(
        "soracloud::RecordSoracloudRuntimeReceipt",
    );
    registry
}

fn with_consensus_stable_ids(mut registry: InstructionRegistry) -> InstructionRegistry {
    registry = registry.register_with_id_slice::<consensus_keys::RegisterConsensusKey>(
        "consensus::RegisterConsensusKey",
    );
    registry = registry.register_with_id_slice::<consensus_keys::RotateConsensusKey>(
        "consensus::RotateConsensusKey",
    );
    registry = registry.register_with_id_slice::<consensus_keys::DisableConsensusKey>(
        "consensus::DisableConsensusKey",
    );
    registry = registry.register_with_id_slice::<endorsement::RegisterDomainCommittee>(
        "nexus::RegisterDomainCommittee",
    );
    registry = registry.register_with_id_slice::<endorsement::SetDomainEndorsementPolicy>(
        "nexus::SetDomainEndorsementPolicy",
    );
    registry = registry.register_with_id_slice::<endorsement::SubmitDomainEndorsement>(
        "nexus::SubmitDomainEndorsement",
    );
    registry = registry.register_with_id_slice::<domain_link::SetAccountAliasBinding>(
        "identity::SetAccountAliasBinding",
    );
    registry = registry.register_with_id_slice::<domain_link::SetPrimaryAccountAlias>(
        "identity::SetPrimaryAccountAlias",
    );
    registry = registry.register_with_id_slice::<account_recovery::ReplaceAccountController>(
        account_recovery::ReplaceAccountController::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<account_recovery::SetAccountRecoveryPolicy>(
        account_recovery::SetAccountRecoveryPolicy::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<account_recovery::ClearAccountRecoveryPolicy>(
        account_recovery::ClearAccountRecoveryPolicy::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<account_recovery::ProposeAccountRecovery>(
        account_recovery::ProposeAccountRecovery::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<account_recovery::ApproveAccountRecovery>(
        account_recovery::ApproveAccountRecovery::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<account_recovery::CancelAccountRecovery>(
        account_recovery::CancelAccountRecovery::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<account_recovery::FinalizeAccountRecovery>(
        account_recovery::FinalizeAccountRecovery::WIRE_ID,
    );
    registry
}

fn with_identity_stable_ids(mut registry: InstructionRegistry) -> InstructionRegistry {
    registry = registry.register_with_id_slice::<ram_lfe::RegisterRamLfeProgramPolicy>(
        "identity::RegisterRamLfeProgramPolicy",
    );
    registry = registry.register_with_id_slice::<ram_lfe::ActivateRamLfeProgramPolicy>(
        "identity::ActivateRamLfeProgramPolicy",
    );
    registry = registry.register_with_id_slice::<ram_lfe::DeactivateRamLfeProgramPolicy>(
        "identity::DeactivateRamLfeProgramPolicy",
    );
    registry = registry.register_with_id_slice::<identifier::RegisterIdentifierPolicy>(
        "identity::RegisterIdentifierPolicy",
    );
    registry = registry.register_with_id_slice::<identifier::ActivateIdentifierPolicy>(
        "identity::ActivateIdentifierPolicy",
    );
    registry =
        registry.register_with_id_slice::<identifier::ClaimIdentifier>("identity::ClaimIdentifier");
    registry = registry
        .register_with_id_slice::<identifier::RevokeIdentifier>("identity::RevokeIdentifier");
    registry = registry.register_with_id_slice::<asset_alias::SetAssetDefinitionAlias>(
        asset_alias::SetAssetDefinitionAlias::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<asset_alias::SetAssetDefinitionBalancePolicy>(
        asset_alias::SetAssetDefinitionBalancePolicy::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<asset_transfer_control::SetAssetTransferFreeze>(
        asset_transfer_control::SetAssetTransferFreeze::WIRE_ID,
    );
    registry = registry
        .register_with_id_slice::<asset_transfer_control::SetAssetTransferBlacklist>(
            asset_transfer_control::SetAssetTransferBlacklist::WIRE_ID,
        );
    registry = registry.register_with_id_slice::<asset_transfer_control::SetAssetTransferControl>(
        asset_transfer_control::SetAssetTransferControl::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<contract_alias::SetContractAlias>(
        contract_alias::SetContractAlias::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<nexus::SetLaneRelayEmergencyValidators>(
        "nexus::SetLaneRelayEmergencyValidators",
    );
    registry = registry.register_with_id_slice::<nexus::RegisterVerifiedLaneRelay>(
        "nexus::RegisterVerifiedLaneRelay",
    );
    registry = registry.register_with_id_slice::<nexus::RegisterVerifiedNexusFeeBudget>(
        "nexus::RegisterVerifiedNexusFeeBudget",
    );
    registry
}

fn with_runtime_upgrade_stable_ids(mut registry: InstructionRegistry) -> InstructionRegistry {
    registry = registry.register_with_id_slice::<runtime_upgrade::ProposeRuntimeUpgrade>(
        runtime_upgrade::ProposeRuntimeUpgrade::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<runtime_upgrade::ActivateRuntimeUpgrade>(
        runtime_upgrade::ActivateRuntimeUpgrade::WIRE_ID,
    );
    registry = registry.register_with_id_slice::<runtime_upgrade::CancelRuntimeUpgrade>(
        runtime_upgrade::CancelRuntimeUpgrade::WIRE_ID,
    );
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn domain_id() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("domain id")
    }

    fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(domain_id(), "rose".parse().expect("asset name"))
    }

    fn asset_id() -> AssetId {
        AssetId::of(asset_definition_id(), account(0xA1))
    }

    fn trigger_id() -> TriggerId {
        "registry_tick".parse().expect("trigger id")
    }

    fn role_id() -> RoleId {
        "registry_auditor".parse().expect("role id")
    }

    fn assert_default_registry_decodes<T>(value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let registry = default();
        let wire_id = registry
            .wire_id(std::any::type_name::<T>())
            .unwrap_or_else(|| std::any::type_name::<T>());
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed = norito::core::frame_bare_with_header_flags::<T>(&payload, flags)
            .expect("frame instruction payload");
        let decoded = registry
            .decode(wire_id, &framed)
            .expect("registered instruction")
            .expect("decode instruction");

        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn default_registry_registers_public_lane_validator() {
        let registry = default();
        assert!(registry.contains(std::any::type_name::<
            crate::isi::staking::RegisterPublicLaneValidator,
        >()));
    }

    #[test]
    fn default_registry_registers_public_lane_validator_rebind() {
        let registry = default();
        assert!(registry.contains(std::any::type_name::<
            crate::isi::staking::RebindPublicLaneValidatorPeer,
        >()));
    }

    #[test]
    fn default_registry_registers_kaigi_relay_health_report() {
        let registry = default();
        assert!(registry.contains(std::any::type_name::<
            crate::isi::kaigi::ReportKaigiRelayHealth,
        >()));
    }

    #[test]
    fn instruction_registry_excludes_direct_grouped_variants() {
        let registry = default();
        let removed_type_names = [
            std::any::type_name::<crate::isi::register::RegisterPeerWithPop>(),
            std::any::type_name::<Register<Domain>>(),
            std::any::type_name::<Register<Account>>(),
            std::any::type_name::<Register<AssetDefinition>>(),
            std::any::type_name::<Register<Nft>>(),
            std::any::type_name::<Register<Role>>(),
            std::any::type_name::<Register<Trigger>>(),
            std::any::type_name::<Unregister<Peer>>(),
            std::any::type_name::<Unregister<Domain>>(),
            std::any::type_name::<Unregister<Account>>(),
            std::any::type_name::<Unregister<AssetDefinition>>(),
            std::any::type_name::<Unregister<Nft>>(),
            std::any::type_name::<Unregister<Role>>(),
            std::any::type_name::<Unregister<Trigger>>(),
            std::any::type_name::<Mint<Numeric, Asset>>(),
            std::any::type_name::<Mint<u32, Trigger>>(),
            std::any::type_name::<Burn<Numeric, Asset>>(),
            std::any::type_name::<Burn<u32, Trigger>>(),
            std::any::type_name::<Transfer<Account, DomainId, Account>>(),
            std::any::type_name::<Transfer<Account, AssetDefinitionId, Account>>(),
            std::any::type_name::<Transfer<Asset, Numeric, Account>>(),
            std::any::type_name::<Transfer<Account, NftId, Account>>(),
            std::any::type_name::<SetKeyValue<Domain>>(),
            std::any::type_name::<SetKeyValue<Account>>(),
            std::any::type_name::<SetKeyValue<AssetDefinition>>(),
            std::any::type_name::<SetKeyValue<Nft>>(),
            std::any::type_name::<SetKeyValue<Trigger>>(),
            std::any::type_name::<RemoveKeyValue<Domain>>(),
            std::any::type_name::<RemoveKeyValue<Account>>(),
            std::any::type_name::<RemoveKeyValue<AssetDefinition>>(),
            std::any::type_name::<RemoveKeyValue<Nft>>(),
            std::any::type_name::<RemoveKeyValue<Trigger>>(),
            std::any::type_name::<Grant<Permission, Account>>(),
            std::any::type_name::<Grant<RoleId, Account>>(),
            std::any::type_name::<Grant<Permission, Role>>(),
            std::any::type_name::<Revoke<Permission, Account>>(),
            std::any::type_name::<Revoke<RoleId, Account>>(),
            std::any::type_name::<Revoke<Permission, Role>>(),
            std::any::type_name::<repo::RepoIsi>(),
            std::any::type_name::<repo::ReverseRepoIsi>(),
            std::any::type_name::<repo::RepoMarginCallIsi>(),
            std::any::type_name::<settlement::DvpIsi>(),
            std::any::type_name::<settlement::PvpIsi>(),
        ];

        for name in removed_type_names {
            assert!(
                !registry.contains(name),
                "{name} must not be in default registry"
            );
        }

        let removed_wire_ids = [
            repo::RepoIsi::WIRE_ID,
            repo::ReverseRepoIsi::WIRE_ID,
            repo::RepoMarginCallIsi::WIRE_ID,
            settlement::DvpIsi::WIRE_ID,
            settlement::PvpIsi::WIRE_ID,
        ];
        for wire_id in removed_wire_ids {
            assert!(
                !registry.contains(wire_id),
                "{wire_id} must not be in default registry"
            );
        }
    }

    #[test]
    fn instruction_registry_decodes_boxed_stable_ids() {
        assert_default_registry_decodes(RegisterBox::Domain(Register::domain(Domain::new(
            domain_id(),
        ))));
        assert_default_registry_decodes(UnregisterBox::Domain(Unregister::domain(domain_id())));
        assert_default_registry_decodes(MintBox::Asset(Mint::asset_numeric(7_u32, asset_id())));
        assert_default_registry_decodes(BurnBox::TriggerRepetitions(Burn::trigger_repetitions(
            2,
            trigger_id(),
        )));
        assert_default_registry_decodes(TransferBox::Asset(Transfer::asset_numeric(
            asset_id(),
            3_u32,
            account(0xA2),
        )));
        assert_default_registry_decodes(SetKeyValueBox::Domain(SetKeyValue::domain(
            domain_id(),
            "color".parse().expect("metadata key"),
            iroha_primitives::json::Json::new("blue"),
        )));
        assert_default_registry_decodes(RemoveKeyValueBox::Domain(RemoveKeyValue::domain(
            domain_id(),
            "color".parse().expect("metadata key"),
        )));
        assert_default_registry_decodes(GrantBox::Role(Grant::account_role(
            role_id(),
            account(0xA3),
        )));
        assert_default_registry_decodes(RevokeBox::Role(Revoke::account_role(
            role_id(),
            account(0xA3),
        )));

        let registry = default();
        assert!(registry.contains(rwa::RwaInstructionBox::WIRE_ID));
        assert!(registry.contains(repo::RepoInstructionBox::WIRE_ID));
        assert!(registry.contains(settlement::SettlementInstructionBox::WIRE_ID));
    }

    #[test]
    fn instruction_registry_registers_and_decodes_standalone_surface() {
        let registry = default();
        let expected = [
            std::any::type_name::<content::PublishContentBundle>(),
            std::any::type_name::<content::RetireContentBundle>(),
            std::any::type_name::<soradns::SubmitDirectoryDraft>(),
            std::any::type_name::<soradns::PublishDirectory>(),
            std::any::type_name::<soradns::RevokeResolver>(),
            std::any::type_name::<soradns::UnrevokeResolver>(),
            std::any::type_name::<soradns::AddReleaseSigner>(),
            std::any::type_name::<soradns::RemoveReleaseSigner>(),
            std::any::type_name::<soradns::SetDirectoryRotationPolicy>(),
            std::any::type_name::<confidential::PublishPedersenParams>(),
            std::any::type_name::<confidential::SetPedersenParamsLifecycle>(),
            std::any::type_name::<confidential::PublishPoseidonParams>(),
            std::any::type_name::<confidential::SetPoseidonParamsLifecycle>(),
            std::any::type_name::<sorafs::SetPricingSchedule>(),
            std::any::type_name::<sorafs::UpsertProviderCredit>(),
            std::any::type_name::<crate::isi::staking::BondPublicLaneStake>(),
            std::any::type_name::<crate::isi::staking::SchedulePublicLaneUnbond>(),
            std::any::type_name::<crate::isi::staking::FinalizePublicLaneUnbond>(),
            std::any::type_name::<crate::isi::staking::SlashPublicLaneValidator>(),
            std::any::type_name::<crate::isi::staking::RecordPublicLaneRewards>(),
            std::any::type_name::<crate::isi::staking::ClaimPublicLaneRewards>(),
        ];
        for name in expected {
            assert!(
                registry.contains(name),
                "{name} missing from default registry"
            );
        }

        assert_default_registry_decodes(content::PublishContentBundle {
            bundle_id: Hash::new(b"content-bundle"),
            tarball: b"tar".to_vec(),
            expires_at_height: None,
            manifest: None,
        });
        assert_default_registry_decodes(soradns::PublishDirectory {
            directory_id: [0xD1; 32],
            expected_prev: None,
        });
        assert_default_registry_decodes(confidential::PublishPedersenParams {
            params: crate::confidential::PedersenParams {
                params_id: crate::confidential::ConfidentialParamsId::new(7),
                generators_hash: [0x11; 32],
                constants_hash: [0x22; 32],
                metadata_uri_cid: None,
                params_cid: None,
                activation_height: Some(1),
                withdraw_height: None,
                status: crate::confidential::ConfidentialStatus::Active,
            },
        });
        assert_default_registry_decodes(sorafs::SetPricingSchedule::new(
            crate::sorafs::pricing::PricingScheduleRecord::launch_default(),
        ));
        assert_default_registry_decodes(sorafs::UpsertProviderCredit::new(
            crate::sorafs::pricing::ProviderCreditRecord::new(
                crate::sorafs::capacity::ProviderId::new([0xC1; 32]),
                1,
                0,
                0,
                0,
                0,
                0,
                Metadata::default(),
            ),
        ));
        assert_default_registry_decodes(crate::isi::staking::ClaimPublicLaneRewards {
            lane_id: crate::nexus::LaneId::SINGLE,
            account: account(0xA4),
            upto_epoch: Some(9),
        });
    }

    #[cfg(feature = "governance")]
    #[test]
    fn default_registry_registers_citizenship_instructions() {
        let registry = default();
        assert!(
            registry.contains(std::any::type_name::<crate::isi::governance::RegisterCitizen>())
        );
        assert!(registry.contains(std::any::type_name::<
            crate::isi::governance::UnregisterCitizen,
        >()));
        assert!(registry.contains(std::any::type_name::<
            crate::isi::governance::RecordCitizenServiceOutcome,
        >()));
    }
}

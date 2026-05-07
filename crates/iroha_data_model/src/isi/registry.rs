#[cfg(feature = "governance")]
use crate::isi::governance;
use crate::{
    isi::{
        InstructionRegistry, RegisterPeerWithPop, account_alias_lease, account_recovery,
        asset_alias, asset_transfer_control, bridge, consensus_keys, contract_alias, domain_link,
        endorsement, escrow, identifier, kaigi, ministry, musubi, nexus, offline, oracle, ram_lfe,
        repo, runtime_upgrade, rwa, settlement, smart_contract_code, sns, social, soracloud,
        sorafs, space_directory,
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
    InstructionRegistry::register_slice::<RegisterPeerWithPop>,
    InstructionRegistry::register_slice::<Register<Domain>>,
    InstructionRegistry::register_slice::<Register<Account>>,
    InstructionRegistry::register_slice::<Register<AssetDefinition>>,
    InstructionRegistry::register_slice::<Register<Nft>>,
    InstructionRegistry::register_slice::<Register<Role>>,
    InstructionRegistry::register_slice::<Register<Trigger>>,
    InstructionRegistry::register_slice::<RegisterBox>,
    InstructionRegistry::register_slice::<Unregister<Peer>>,
    InstructionRegistry::register_slice::<Unregister<Domain>>,
    InstructionRegistry::register_slice::<Unregister<Account>>,
    InstructionRegistry::register_slice::<Unregister<AssetDefinition>>,
    InstructionRegistry::register_slice::<Unregister<Nft>>,
    InstructionRegistry::register_slice::<Unregister<Role>>,
    InstructionRegistry::register_slice::<Unregister<Trigger>>,
    InstructionRegistry::register_slice::<UnregisterBox>,
    InstructionRegistry::register_slice::<Mint<Numeric, Asset>>,
    InstructionRegistry::register_slice::<Mint<u32, Trigger>>,
    InstructionRegistry::register_slice::<MintBox>,
    InstructionRegistry::register_slice::<Burn<Numeric, Asset>>,
    InstructionRegistry::register_slice::<Burn<u32, Trigger>>,
    InstructionRegistry::register_slice::<BurnBox>,
    InstructionRegistry::register_slice::<Transfer<Account, DomainId, Account>>,
    InstructionRegistry::register_slice::<Transfer<Account, AssetDefinitionId, Account>>,
    InstructionRegistry::register_slice::<Transfer<Asset, Numeric, Account>>,
    InstructionRegistry::register_slice::<Transfer<Account, NftId, Account>>,
    InstructionRegistry::register_slice::<TransferAssetBatch>,
    InstructionRegistry::register_slice::<TransferBox>,
    InstructionRegistry::register_slice::<asset_transfer_control::SetAssetTransferFreeze>,
    InstructionRegistry::register_slice::<asset_transfer_control::SetAssetTransferBlacklist>,
    InstructionRegistry::register_slice::<asset_transfer_control::SetAssetTransferControl>,
    InstructionRegistry::register_slice::<rwa::RwaInstructionBox>,
    InstructionRegistry::register_slice::<repo::RepoInstructionBox>,
    InstructionRegistry::register_slice::<repo::RepoIsi>,
    InstructionRegistry::register_slice::<repo::ReverseRepoIsi>,
    InstructionRegistry::register_slice::<settlement::SettlementInstructionBox>,
    InstructionRegistry::register_slice::<settlement::DvpIsi>,
    InstructionRegistry::register_slice::<settlement::PvpIsi>,
    InstructionRegistry::register_slice::<SetParameter>,
    InstructionRegistry::register_slice::<SetKeyValue<Domain>>,
    InstructionRegistry::register_slice::<SetKeyValue<Account>>,
    InstructionRegistry::register_slice::<SetKeyValue<AssetDefinition>>,
    InstructionRegistry::register_slice::<SetKeyValue<Nft>>,
    InstructionRegistry::register_slice::<SetKeyValue<Trigger>>,
    InstructionRegistry::register_slice::<SetKeyValueBox>,
    InstructionRegistry::register_slice::<AddSignatory>,
    InstructionRegistry::register_slice::<RemoveSignatory>,
    InstructionRegistry::register_slice::<SetAccountQuorum>,
    InstructionRegistry::register_slice::<SetAssetKeyValue>,
    InstructionRegistry::register_slice::<RemoveKeyValue<Domain>>,
    InstructionRegistry::register_slice::<RemoveKeyValue<Account>>,
    InstructionRegistry::register_slice::<RemoveKeyValue<AssetDefinition>>,
    InstructionRegistry::register_slice::<RemoveKeyValue<Nft>>,
    InstructionRegistry::register_slice::<RemoveKeyValue<Trigger>>,
    InstructionRegistry::register_slice::<RemoveKeyValueBox>,
    InstructionRegistry::register_slice::<RemoveAssetKeyValue>,
    InstructionRegistry::register_slice::<Grant<Permission, Account>>,
    InstructionRegistry::register_slice::<Grant<RoleId, Account>>,
    InstructionRegistry::register_slice::<Grant<Permission, Role>>,
    InstructionRegistry::register_slice::<GrantBox>,
    InstructionRegistry::register_slice::<Revoke<Permission, Account>>,
    InstructionRegistry::register_slice::<Revoke<RoleId, Account>>,
    InstructionRegistry::register_slice::<Revoke<Permission, Role>>,
    InstructionRegistry::register_slice::<RevokeBox>,
    InstructionRegistry::register_slice::<offline::IssueOfflineNoteV2>,
    InstructionRegistry::register_slice::<offline::RedeemOfflineNoteV2>,
    InstructionRegistry::register_slice::<offline::AuditOfflineNoteV2>,
    InstructionRegistry::register_slice::<asset_alias::SetAssetDefinitionBalancePolicy>,
    InstructionRegistry::register_slice::<crate::isi::staking::RegisterPublicLaneValidator>,
    InstructionRegistry::register_slice::<crate::isi::staking::RebindPublicLaneValidatorPeer>,
    InstructionRegistry::register_slice::<crate::isi::staking::ActivatePublicLaneValidator>,
    InstructionRegistry::register_slice::<crate::isi::staking::ExitPublicLaneValidator>,
    InstructionRegistry::register_slice::<crate::isi::staking::CancelConsensusEvidencePenalty>,
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
    registry = registry.register_with_id_slice::<repo::RepoIsi>(repo::RepoIsi::WIRE_ID);
    registry =
        registry.register_with_id_slice::<repo::ReverseRepoIsi>(repo::ReverseRepoIsi::WIRE_ID);
    registry = registry.register_with_id_slice::<settlement::DvpIsi>(settlement::DvpIsi::WIRE_ID);
    registry = registry.register_with_id_slice::<settlement::PvpIsi>(settlement::PvpIsi::WIRE_ID);
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

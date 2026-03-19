#[cfg(feature = "governance")]
use crate::isi::governance;
use crate::{
    isi::{
        InstructionRegistry, RegisterPeerWithPop, asset_alias, bridge, consensus_keys, domain_link,
        endorsement, identifier, kaigi, nexus, offline, oracle, ram_lfe, repo, runtime_upgrade,
        settlement, smart_contract_code, social, sorafs, space_directory,
        transparent::{
            AddSignatory, InvalidInstruction, RemoveAssetKeyValue, RemoveSignatory,
            SetAccountQuorum, SetAssetKeyValue,
        },
        verifying_keys, zk,
    },
    prelude::*,
};

/// Signature of helper functions that register instructions into [`InstructionRegistry`].
type Registrar = fn(InstructionRegistry) -> InstructionRegistry;

/// Built-in instruction registrations that make up the default registry used by Iroha.
const ALL_REGISTRARS: &[Registrar] = &[
    InstructionRegistry::register::<RegisterPeerWithPop>,
    InstructionRegistry::register::<Register<Domain>>,
    InstructionRegistry::register::<Register<Account>>,
    InstructionRegistry::register::<Register<AssetDefinition>>,
    InstructionRegistry::register::<Register<Nft>>,
    InstructionRegistry::register::<Register<Role>>,
    InstructionRegistry::register::<Register<Trigger>>,
    InstructionRegistry::register::<RegisterBox>,
    InstructionRegistry::register::<Unregister<Peer>>,
    InstructionRegistry::register::<Unregister<Domain>>,
    InstructionRegistry::register::<Unregister<Account>>,
    InstructionRegistry::register::<Unregister<AssetDefinition>>,
    InstructionRegistry::register::<Unregister<Nft>>,
    InstructionRegistry::register::<Unregister<Role>>,
    InstructionRegistry::register::<Unregister<Trigger>>,
    InstructionRegistry::register::<UnregisterBox>,
    InstructionRegistry::register::<Mint<Numeric, Asset>>,
    InstructionRegistry::register::<Mint<u32, Trigger>>,
    InstructionRegistry::register::<MintBox>,
    InstructionRegistry::register::<Burn<Numeric, Asset>>,
    InstructionRegistry::register::<Burn<u32, Trigger>>,
    InstructionRegistry::register::<BurnBox>,
    InstructionRegistry::register::<Transfer<Account, DomainId, Account>>,
    InstructionRegistry::register::<Transfer<Account, AssetDefinitionId, Account>>,
    InstructionRegistry::register::<Transfer<Asset, Numeric, Account>>,
    InstructionRegistry::register::<Transfer<Account, NftId, Account>>,
    InstructionRegistry::register::<TransferAssetBatch>,
    InstructionRegistry::register::<TransferBox>,
    InstructionRegistry::register::<repo::RepoInstructionBox>,
    InstructionRegistry::register::<repo::RepoIsi>,
    InstructionRegistry::register::<repo::ReverseRepoIsi>,
    InstructionRegistry::register::<settlement::SettlementInstructionBox>,
    InstructionRegistry::register::<settlement::DvpIsi>,
    InstructionRegistry::register::<settlement::PvpIsi>,
    InstructionRegistry::register::<SetParameter>,
    InstructionRegistry::register::<SetKeyValue<Domain>>,
    InstructionRegistry::register::<SetKeyValue<Account>>,
    InstructionRegistry::register::<SetKeyValue<AssetDefinition>>,
    InstructionRegistry::register::<SetKeyValue<Nft>>,
    InstructionRegistry::register::<SetKeyValue<Trigger>>,
    InstructionRegistry::register::<SetKeyValueBox>,
    InstructionRegistry::register::<AddSignatory>,
    InstructionRegistry::register::<RemoveSignatory>,
    InstructionRegistry::register::<SetAccountQuorum>,
    InstructionRegistry::register::<SetAssetKeyValue>,
    InstructionRegistry::register::<RemoveKeyValue<Domain>>,
    InstructionRegistry::register::<RemoveKeyValue<Account>>,
    InstructionRegistry::register::<RemoveKeyValue<AssetDefinition>>,
    InstructionRegistry::register::<RemoveKeyValue<Nft>>,
    InstructionRegistry::register::<RemoveKeyValue<Trigger>>,
    InstructionRegistry::register::<RemoveKeyValueBox>,
    InstructionRegistry::register::<RemoveAssetKeyValue>,
    InstructionRegistry::register::<Grant<Permission, Account>>,
    InstructionRegistry::register::<Grant<RoleId, Account>>,
    InstructionRegistry::register::<Grant<Permission, Role>>,
    InstructionRegistry::register::<GrantBox>,
    InstructionRegistry::register::<Revoke<Permission, Account>>,
    InstructionRegistry::register::<Revoke<RoleId, Account>>,
    InstructionRegistry::register::<Revoke<Permission, Role>>,
    InstructionRegistry::register::<RevokeBox>,
    InstructionRegistry::register::<offline::RegisterOfflineAllowance>,
    InstructionRegistry::register::<offline::SubmitOfflineToOnlineTransfer>,
    InstructionRegistry::register::<offline::RegisterOfflineVerdictRevocation>,
    InstructionRegistry::register::<offline::ReclaimExpiredOfflineAllowance>,
    InstructionRegistry::register::<crate::isi::staking::RegisterPublicLaneValidator>,
    InstructionRegistry::register::<crate::isi::staking::ActivatePublicLaneValidator>,
    InstructionRegistry::register::<crate::isi::staking::ExitPublicLaneValidator>,
    InstructionRegistry::register::<crate::isi::staking::CancelConsensusEvidencePenalty>,
    InstructionRegistry::register::<nexus::SetLaneRelayEmergencyValidators>,
    InstructionRegistry::register::<oracle::RegisterOracleFeed>,
    InstructionRegistry::register::<oracle::SubmitOracleObservation>,
    InstructionRegistry::register::<oracle::AggregateOracleFeed>,
    InstructionRegistry::register::<oracle::OpenOracleDispute>,
    InstructionRegistry::register::<oracle::ResolveOracleDispute>,
    InstructionRegistry::register::<oracle::ProposeOracleChange>,
    InstructionRegistry::register::<oracle::VoteOracleChangeStage>,
    InstructionRegistry::register::<oracle::RollbackOracleChange>,
    InstructionRegistry::register::<oracle::RecordTwitterBinding>,
    InstructionRegistry::register::<oracle::RevokeTwitterBinding>,
    InstructionRegistry::register::<social::ClaimTwitterFollowReward>,
    InstructionRegistry::register::<social::SendToTwitter>,
    InstructionRegistry::register::<social::CancelTwitterEscrow>,
    InstructionRegistry::register::<ExecuteTrigger>,
    InstructionRegistry::register::<Upgrade>,
    InstructionRegistry::register::<Log>,
    InstructionRegistry::register::<CustomInstruction>,
    InstructionRegistry::register::<InvalidInstruction>,
    InstructionRegistry::register::<verifying_keys::RegisterVerifyingKey>,
    InstructionRegistry::register::<verifying_keys::UpdateVerifyingKey>,
    InstructionRegistry::register::<consensus_keys::RegisterConsensusKey>,
    InstructionRegistry::register::<consensus_keys::RotateConsensusKey>,
    InstructionRegistry::register::<consensus_keys::DisableConsensusKey>,
    InstructionRegistry::register::<endorsement::RegisterDomainCommittee>,
    InstructionRegistry::register::<endorsement::SetDomainEndorsementPolicy>,
    InstructionRegistry::register::<endorsement::SubmitDomainEndorsement>,
    InstructionRegistry::register::<domain_link::LinkAccountDomain>,
    InstructionRegistry::register::<domain_link::BindAccountAlias>,
    InstructionRegistry::register::<domain_link::SetAccountLabel>,
    InstructionRegistry::register::<domain_link::UnlinkAccountDomain>,
    InstructionRegistry::register::<ram_lfe::RegisterRamLfeProgramPolicy>,
    InstructionRegistry::register::<ram_lfe::ActivateRamLfeProgramPolicy>,
    InstructionRegistry::register::<ram_lfe::DeactivateRamLfeProgramPolicy>,
    InstructionRegistry::register::<identifier::RegisterIdentifierPolicy>,
    InstructionRegistry::register::<identifier::ActivateIdentifierPolicy>,
    InstructionRegistry::register::<identifier::ClaimIdentifier>,
    InstructionRegistry::register::<identifier::RevokeIdentifier>,
    InstructionRegistry::register::<asset_alias::SetAssetDefinitionAlias>,
    InstructionRegistry::register::<sorafs::RegisterPinManifest>,
    InstructionRegistry::register::<sorafs::ApprovePinManifest>,
    InstructionRegistry::register::<sorafs::RetirePinManifest>,
    InstructionRegistry::register::<sorafs::BindManifestAlias>,
    InstructionRegistry::register::<sorafs::RegisterCapacityDeclaration>,
    InstructionRegistry::register::<sorafs::RecordCapacityTelemetry>,
    InstructionRegistry::register::<sorafs::RegisterCapacityDispute>,
    InstructionRegistry::register::<sorafs::IssueReplicationOrder>,
    InstructionRegistry::register::<sorafs::CompleteReplicationOrder>,
    InstructionRegistry::register::<sorafs::RegisterProviderOwner>,
    InstructionRegistry::register::<sorafs::UnregisterProviderOwner>,
    InstructionRegistry::register::<space_directory::PublishSpaceDirectoryManifest>,
    InstructionRegistry::register::<space_directory::RevokeSpaceDirectoryManifest>,
    InstructionRegistry::register::<space_directory::ExpireSpaceDirectoryManifest>,
    InstructionRegistry::register::<smart_contract_code::RegisterSmartContractCode>,
    InstructionRegistry::register::<smart_contract_code::DeactivateContractInstance>,
    InstructionRegistry::register::<smart_contract_code::ActivateContractInstance>,
    InstructionRegistry::register::<smart_contract_code::RegisterSmartContractBytes>,
    InstructionRegistry::register::<smart_contract_code::RemoveSmartContractBytes>,
    InstructionRegistry::register::<zk::VerifyProof>,
    InstructionRegistry::register::<kaigi::CreateKaigi>,
    InstructionRegistry::register::<kaigi::JoinKaigi>,
    InstructionRegistry::register::<kaigi::LeaveKaigi>,
    InstructionRegistry::register::<kaigi::EndKaigi>,
    InstructionRegistry::register::<kaigi::RecordKaigiUsage>,
    InstructionRegistry::register::<kaigi::SetKaigiRelayManifest>,
    InstructionRegistry::register::<kaigi::RegisterKaigiRelay>,
    InstructionRegistry::register::<kaigi::ReportKaigiRelayHealth>,
    InstructionRegistry::register::<zk::RegisterZkAsset>,
    InstructionRegistry::register::<zk::ScheduleConfidentialPolicyTransition>,
    InstructionRegistry::register::<zk::CancelConfidentialPolicyTransition>,
    InstructionRegistry::register::<zk::Shield>,
    InstructionRegistry::register::<zk::ZkTransfer>,
    InstructionRegistry::register::<zk::Unshield>,
    InstructionRegistry::register::<zk::CreateElection>,
    InstructionRegistry::register::<zk::SubmitBallot>,
    InstructionRegistry::register::<zk::FinalizeElection>,
    InstructionRegistry::register::<bridge::SubmitBridgeProof>,
    InstructionRegistry::register::<bridge::RecordBridgeReceipt>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::ProposeDeployContract>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::ProposeRuntimeUpgradeProposal>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::CastZkBallot>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::CastPlainBallot>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::SlashGovernanceLock>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::RestituteGovernanceLock>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::EnactReferendum>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::FinalizeReferendum>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::ApproveGovernanceProposal>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::PersistCouncilForEpoch>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::RecordCitizenServiceOutcome>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::RegisterCitizen>,
    #[cfg(feature = "governance")]
    InstructionRegistry::register::<governance::UnregisterCitizen>,
    InstructionRegistry::register::<runtime_upgrade::ProposeRuntimeUpgrade>,
    InstructionRegistry::register::<runtime_upgrade::ActivateRuntimeUpgrade>,
    InstructionRegistry::register::<runtime_upgrade::CancelRuntimeUpgrade>,
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
fn with_stable_ids(mut registry: InstructionRegistry) -> InstructionRegistry {
    // Provide a stable wire id for a commonly used instruction as a starting point.
    // Others continue to use their Rust `type_name` as the wire id.
    registry = registry.register_with_id::<Log>(Log::WIRE_ID);
    registry = registry.register_with_id::<SetParameter>(SetParameter::WIRE_ID);
    registry = registry.register_with_id::<ExecuteTrigger>(ExecuteTrigger::WIRE_ID);
    registry = registry.register_with_id::<RegisterBox>(RegisterBox::WIRE_ID);
    registry = registry.register_with_id::<UnregisterBox>(UnregisterBox::WIRE_ID);
    registry = registry.register_with_id::<MintBox>(MintBox::WIRE_ID);
    registry = registry.register_with_id::<BurnBox>(BurnBox::WIRE_ID);
    registry = registry.register_with_id::<TransferBox>(TransferBox::WIRE_ID);
    registry = registry.register_with_id::<TransferAssetBatch>(TransferAssetBatch::WIRE_ID);
    registry = registry.register_with_id::<repo::RepoIsi>(repo::RepoIsi::WIRE_ID);
    registry = registry.register_with_id::<repo::ReverseRepoIsi>(repo::ReverseRepoIsi::WIRE_ID);
    registry = registry.register_with_id::<settlement::DvpIsi>(settlement::DvpIsi::WIRE_ID);
    registry = registry.register_with_id::<settlement::PvpIsi>(settlement::PvpIsi::WIRE_ID);
    registry = registry.register_with_id::<zk::ScheduleConfidentialPolicyTransition>(
        "zk::ScheduleConfidentialPolicyTransition",
    );
    registry = registry.register_with_id::<zk::CancelConfidentialPolicyTransition>(
        "zk::CancelConfidentialPolicyTransition",
    );
    registry = registry.register_with_id::<SetKeyValueBox>(SetKeyValueBox::WIRE_ID);
    registry = registry.register_with_id::<RemoveKeyValueBox>(RemoveKeyValueBox::WIRE_ID);
    registry = registry.register_with_id::<GrantBox>(GrantBox::WIRE_ID);
    registry = registry.register_with_id::<RevokeBox>(RevokeBox::WIRE_ID);
    registry = registry.register_with_id::<crate::isi::staking::ActivatePublicLaneValidator>(
        "iroha.staking.activate_public_lane_validator",
    );
    registry = registry.register_with_id::<crate::isi::staking::ExitPublicLaneValidator>(
        "iroha.staking.exit_public_lane_validator",
    );
    registry = registry.register_with_id::<Upgrade>(Upgrade::WIRE_ID);
    registry = registry.register_with_id::<CustomInstruction>(CustomInstruction::WIRE_ID);
    registry = registry.register_with_id::<InvalidInstruction>(InvalidInstruction::WIRE_ID);
    registry = registry.register_with_id::<consensus_keys::RegisterConsensusKey>(
        "consensus::RegisterConsensusKey",
    );
    registry = registry
        .register_with_id::<consensus_keys::RotateConsensusKey>("consensus::RotateConsensusKey");
    registry = registry
        .register_with_id::<consensus_keys::DisableConsensusKey>("consensus::DisableConsensusKey");
    registry = registry
        .register_with_id::<endorsement::RegisterDomainCommittee>("nexus::RegisterDomainCommittee");
    registry = registry.register_with_id::<endorsement::SetDomainEndorsementPolicy>(
        "nexus::SetDomainEndorsementPolicy",
    );
    registry = registry
        .register_with_id::<endorsement::SubmitDomainEndorsement>("nexus::SubmitDomainEndorsement");
    registry =
        registry.register_with_id::<domain_link::LinkAccountDomain>("identity::LinkAccountDomain");
    registry =
        registry.register_with_id::<domain_link::BindAccountAlias>("identity::BindAccountAlias");
    registry =
        registry.register_with_id::<domain_link::SetAccountLabel>("identity::SetAccountLabel");
    registry = registry
        .register_with_id::<domain_link::UnlinkAccountDomain>("identity::UnlinkAccountDomain");
    registry = registry.register_with_id::<ram_lfe::RegisterRamLfeProgramPolicy>(
        "identity::RegisterRamLfeProgramPolicy",
    );
    registry = registry.register_with_id::<ram_lfe::ActivateRamLfeProgramPolicy>(
        "identity::ActivateRamLfeProgramPolicy",
    );
    registry = registry.register_with_id::<ram_lfe::DeactivateRamLfeProgramPolicy>(
        "identity::DeactivateRamLfeProgramPolicy",
    );
    registry = registry.register_with_id::<identifier::RegisterIdentifierPolicy>(
        "identity::RegisterIdentifierPolicy",
    );
    registry = registry.register_with_id::<identifier::ActivateIdentifierPolicy>(
        "identity::ActivateIdentifierPolicy",
    );
    registry =
        registry.register_with_id::<identifier::ClaimIdentifier>("identity::ClaimIdentifier");
    registry =
        registry.register_with_id::<identifier::RevokeIdentifier>("identity::RevokeIdentifier");
    registry = registry.register_with_id::<asset_alias::SetAssetDefinitionAlias>(
        asset_alias::SetAssetDefinitionAlias::WIRE_ID,
    );
    registry = registry.register_with_id::<nexus::SetLaneRelayEmergencyValidators>(
        "nexus::SetLaneRelayEmergencyValidators",
    );
    registry = registry.register_with_id::<runtime_upgrade::ProposeRuntimeUpgrade>(
        runtime_upgrade::ProposeRuntimeUpgrade::WIRE_ID,
    );
    registry = registry.register_with_id::<runtime_upgrade::ActivateRuntimeUpgrade>(
        runtime_upgrade::ActivateRuntimeUpgrade::WIRE_ID,
    );
    registry = registry.register_with_id::<runtime_upgrade::CancelRuntimeUpgrade>(
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

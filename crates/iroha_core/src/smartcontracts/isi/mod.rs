//! This module contains enumeration of all possible Iroha Special
//! Instructions, generic instruction types and related
//! implementations.
pub mod account;
mod account_admission;
pub mod asset;
pub mod block;
/// Content lane instruction handlers.
pub mod content;
pub mod domain;
/// Native asset escrow instruction handlers.
pub mod escrow;
pub mod identifier;
pub mod kaigi;
/// Ministry agenda submission handlers.
pub mod ministry;
pub mod multisig;
/// Musubi package registry instruction handlers.
pub mod musubi;
pub mod nft;
/// Offline allowance settlement instruction handlers.
pub mod offline;
/// Oracle feed admission and aggregation instruction handlers.
pub mod oracle;
pub mod query;
pub mod ram_lfe;
pub mod repo;
pub mod rwa;
pub mod settlement;
/// SNS-backed ownership query handlers.
pub mod sns;
/// Viral social incentive instruction handlers.
pub mod social;
/// Soracloud lifecycle and runtime-state instruction handlers.
pub mod soracloud;
pub mod soradns;
/// `SoraFS` pin registry instruction handlers.
pub mod sorafs;
pub mod space_directory;
/// Public lane staking instruction handlers.
pub mod staking;
pub mod triggers;
pub mod tx;
/// Native SoraNet VPN lease escrow instruction handlers.
pub mod vpn;
pub mod world;

use eyre::Result;
pub use iroha_data_model::Registrable;
use iroha_data_model::{
    isi::{error::InstructionExecutionError as Error, *},
    prelude::*,
};
use iroha_logger::prelude::*;
use mv::storage::StorageReadOnly;

use super::Execute;
use crate::{
    smartcontracts::triggers::set::SetReadOnly,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

type InstructionHandler =
    fn(&InstructionBox, &AccountId, &mut StateTransaction<'_, '_>) -> Option<Result<(), Error>>;

fn dispatch_instruction<T: Execute + Clone + 'static>(
    instruction: &InstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Option<Result<(), Error>> {
    instruction
        .as_any()
        .downcast_ref::<T>()
        .map(|isi| isi.clone().execute(authority, state_transaction))
}

const INSTRUCTION_HANDLERS: &[InstructionHandler] = &[
    dispatch_instruction::<RegisterPeerWithPop>,
    dispatch_instruction::<RegisterBox>,
    dispatch_instruction::<UnregisterBox>,
    dispatch_instruction::<MintBox>,
    dispatch_instruction::<BurnBox>,
    dispatch_instruction::<TransferBox>,
    dispatch_instruction::<SetKeyValueBox>,
    dispatch_instruction::<RemoveKeyValueBox>,
    dispatch_instruction::<SetAssetKeyValue>,
    dispatch_instruction::<RemoveAssetKeyValue>,
    dispatch_instruction::<AddSignatory>,
    dispatch_instruction::<RemoveSignatory>,
    dispatch_instruction::<SetAccountQuorum>,
    dispatch_instruction::<GrantBox>,
    dispatch_instruction::<RevokeBox>,
    dispatch_instruction::<ExecuteTrigger>,
    dispatch_instruction::<SetParameter>,
    dispatch_instruction::<Upgrade>,
    dispatch_instruction::<Log>,
    dispatch_instruction::<iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease>,
    dispatch_instruction::<iroha_data_model::isi::account_alias_lease::RenewAccountAliasLease>,
    dispatch_instruction::<iroha_data_model::isi::sns::RegisterSnsName>,
    dispatch_instruction::<iroha_data_model::isi::sns::RenewSnsName>,
    dispatch_instruction::<iroha_data_model::isi::sns::TransferSnsName>,
    dispatch_instruction::<iroha_data_model::isi::sns::UpdateSnsNameControllers>,
    dispatch_instruction::<iroha_data_model::isi::sns::FreezeSnsName>,
    dispatch_instruction::<iroha_data_model::isi::sns::UnfreezeSnsName>,
    dispatch_instruction::<iroha_data_model::isi::InvalidInstruction>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::CreateKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::JoinKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::LeaveKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::EndKaigi>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::RecordKaigiUsage>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::SetKaigiRelayManifest>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::RegisterKaigiRelay>,
    dispatch_instruction::<iroha_data_model::isi::kaigi::ReportKaigiRelayHealth>,
    dispatch_instruction::<runtime_upgrade::ProposeRuntimeUpgrade>,
    dispatch_instruction::<runtime_upgrade::ActivateRuntimeUpgrade>,
    dispatch_instruction::<runtime_upgrade::CancelRuntimeUpgrade>,
    dispatch_instruction::<Mint<Numeric, Asset>>,
    dispatch_instruction::<Burn<Numeric, Asset>>,
    dispatch_instruction::<Transfer<Asset, Numeric, Account>>,
    dispatch_instruction::<TransferAssetBatch>,
    dispatch_instruction::<iroha_data_model::isi::SetAssetTransferFreeze>,
    dispatch_instruction::<iroha_data_model::isi::SetAssetTransferBlacklist>,
    dispatch_instruction::<iroha_data_model::isi::SetAssetTransferControl>,
    dispatch_instruction::<iroha_data_model::isi::repo::RepoInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::repo::RepoIsi>,
    dispatch_instruction::<iroha_data_model::isi::repo::ReverseRepoIsi>,
    dispatch_instruction::<iroha_data_model::isi::repo::RepoMarginCallIsi>,
    dispatch_instruction::<iroha_data_model::isi::rwa::RwaInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::rwa::RegisterRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::TransferRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::MergeRwas>,
    dispatch_instruction::<iroha_data_model::isi::rwa::RedeemRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::FreezeRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::UnfreezeRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::HoldRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::ReleaseRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::ForceTransferRwa>,
    dispatch_instruction::<iroha_data_model::isi::rwa::SetRwaControls>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RegisterPinManifest>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::ApprovePinManifest>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RetirePinManifest>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::BindManifestAlias>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RegisterProviderOwner>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::UnregisterProviderOwner>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RegisterCapacityDeclaration>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RecordCapacityTelemetry>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::RegisterCapacityDispute>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::IssueReplicationOrder>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::CompleteReplicationOrder>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::SetPricingSchedule>,
    dispatch_instruction::<iroha_data_model::isi::sorafs::UpsertProviderCredit>,
    dispatch_instruction::<iroha_data_model::isi::content::PublishContentBundle>,
    dispatch_instruction::<iroha_data_model::isi::content::RetireContentBundle>,
    dispatch_instruction::<iroha_data_model::isi::soradns::SubmitDirectoryDraft>,
    dispatch_instruction::<iroha_data_model::isi::soradns::PublishDirectory>,
    dispatch_instruction::<iroha_data_model::isi::soradns::RevokeResolver>,
    dispatch_instruction::<iroha_data_model::isi::soradns::UnrevokeResolver>,
    dispatch_instruction::<iroha_data_model::isi::soradns::AddReleaseSigner>,
    dispatch_instruction::<iroha_data_model::isi::soradns::RemoveReleaseSigner>,
    dispatch_instruction::<iroha_data_model::isi::soradns::SetDirectoryRotationPolicy>,
    dispatch_instruction::<iroha_data_model::isi::space_directory::PublishSpaceDirectoryManifest>,
    dispatch_instruction::<iroha_data_model::isi::space_directory::RevokeSpaceDirectoryManifest>,
    dispatch_instruction::<iroha_data_model::isi::domain_link::SetAccountAliasBinding>,
    dispatch_instruction::<iroha_data_model::isi::domain_link::SetPrimaryAccountAlias>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::ReplaceAccountController>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::SetAccountRecoveryPolicy>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::ClearAccountRecoveryPolicy>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::ProposeAccountRecovery>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::ApproveAccountRecovery>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::CancelAccountRecovery>,
    dispatch_instruction::<iroha_data_model::isi::account_recovery::FinalizeAccountRecovery>,
    dispatch_instruction::<iroha_data_model::isi::contract_alias::SetContractAlias>,
    dispatch_instruction::<iroha_data_model::isi::musubi::PublishMusubiRelease>,
    dispatch_instruction::<iroha_data_model::isi::musubi::YankMusubiRelease>,
    dispatch_instruction::<iroha_data_model::isi::musubi::SetMusubiShortAlias>,
    dispatch_instruction::<iroha_data_model::isi::musubi::AssertMusubiReleaseExists>,
    dispatch_instruction::<iroha_data_model::isi::identifier::RegisterIdentifierPolicy>,
    dispatch_instruction::<iroha_data_model::isi::identifier::ActivateIdentifierPolicy>,
    dispatch_instruction::<iroha_data_model::isi::identifier::ClaimIdentifier>,
    dispatch_instruction::<iroha_data_model::isi::identifier::RevokeIdentifier>,
    dispatch_instruction::<iroha_data_model::isi::ram_lfe::RegisterRamLfeProgramPolicy>,
    dispatch_instruction::<iroha_data_model::isi::ram_lfe::ActivateRamLfeProgramPolicy>,
    dispatch_instruction::<iroha_data_model::isi::ram_lfe::DeactivateRamLfeProgramPolicy>,
    dispatch_instruction::<iroha_data_model::isi::SetAssetDefinitionAlias>,
    dispatch_instruction::<iroha_data_model::isi::SetAssetDefinitionBalancePolicy>,
    dispatch_instruction::<iroha_data_model::isi::offline::IssueOfflineNoteV2>,
    dispatch_instruction::<iroha_data_model::isi::offline::RedeemOfflineNoteV2>,
    dispatch_instruction::<iroha_data_model::isi::offline::AuditOfflineNoteV2>,
    dispatch_instruction::<iroha_data_model::isi::social::ClaimTwitterFollowReward>,
    dispatch_instruction::<iroha_data_model::isi::social::SendToTwitter>,
    dispatch_instruction::<iroha_data_model::isi::social::CancelTwitterEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::OpenAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::AcceptAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::MarkEscrowPaymentSent>,
    dispatch_instruction::<iroha_data_model::isi::escrow::ReleaseAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::CancelAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::OpenEscrowDispute>,
    dispatch_instruction::<iroha_data_model::isi::escrow::ResolveEscrowDispute>,
    dispatch_instruction::<iroha_data_model::isi::escrow::OpenAnonymousAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::AcceptAnonymousAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::MarkAnonymousEscrowPaymentSent>,
    dispatch_instruction::<iroha_data_model::isi::escrow::ReleaseAnonymousAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::CancelAnonymousAssetEscrow>,
    dispatch_instruction::<iroha_data_model::isi::escrow::OpenAnonymousEscrowDispute>,
    dispatch_instruction::<iroha_data_model::isi::escrow::ResolveAnonymousEscrowDispute>,
    dispatch_instruction::<iroha_data_model::isi::vpn::OpenVpnLeaseEscrow>,
    dispatch_instruction::<iroha_data_model::isi::vpn::SettleVpnLease>,
    dispatch_instruction::<iroha_data_model::isi::vpn::RefundExpiredVpnLease>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::DeploySoracloudService>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::UpgradeSoracloudService>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RollbackSoracloudService>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::SetSoracloudServiceConfig>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::DeleteSoracloudServiceConfig>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::SetSoracloudServiceSecret>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::DeleteSoracloudServiceSecret>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::MutateSoracloudState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RunSoracloudFheJob>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RecordSoracloudDecryptionRequest>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::JoinSoracloudHfSharedLease>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::LeaveSoracloudHfSharedLease>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RenewSoracloudHfSharedLease>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AdvertiseSoracloudModelHost>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::HeartbeatSoracloudModelHost>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::WithdrawSoracloudModelHost>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ReconcileSoracloudModelHosts>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AdvertiseSoracloudInrouHost>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::WithdrawSoracloudInrouHost>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ReconcileSoracloudInrouPlacements>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ReportSoracloudModelHostViolation>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::DeploySoracloudAgentApartment>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RenewSoracloudAgentLease>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RestartSoracloudAgentApartment>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RevokeSoracloudAgentPolicy>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RequestSoracloudAgentWalletSpend>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ApproveSoracloudAgentWalletSpend>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::EnqueueSoracloudAgentMessage>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AcknowledgeSoracloudAgentMessage>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AllowSoracloudAgentAutonomyArtifact>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RunSoracloudAgentAutonomy>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RecordSoracloudAgentAutonomyExecution>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::StartSoracloudTrainingJob>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::CheckpointSoracloudTrainingJob>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RetrySoracloudTrainingJob>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RegisterSoracloudModelArtifact>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RegisterSoracloudModelWeight>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::PromoteSoracloudModelWeight>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RollbackSoracloudModelWeight>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RegisterSoracloudUploadedModelBundle>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AppendSoracloudUploadedModelChunk>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::FinalizeSoracloudUploadedModelBundle>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AdmitSoracloudPrivateCompileProfile>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AllowSoracloudUploadedModel>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::StartSoracloudPrivateInference>,
    dispatch_instruction::<
        iroha_data_model::isi::soracloud::RecordSoracloudPrivateInferenceCheckpoint,
    >,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AdvanceSoracloudRollout>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::SetSoracloudRuntimeState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::SetSoracloudInrouReplicaRuntimeState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ClearSoracloudInrouReplicaRuntimeState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ReportSoracloudServiceLeaseUsage>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RecordSoracloudMailboxMessage>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RecordSoracloudRuntimeReceipt>,
    dispatch_instruction::<iroha_data_model::isi::oracle::RegisterOracleFeed>,
    dispatch_instruction::<iroha_data_model::isi::oracle::SubmitOracleObservation>,
    dispatch_instruction::<iroha_data_model::isi::oracle::AggregateOracleFeed>,
    dispatch_instruction::<iroha_data_model::isi::staking::ActivatePublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::ExitPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::nexus::SetLaneRelayEmergencyValidators>,
    dispatch_instruction::<iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay>,
    dispatch_instruction::<iroha_data_model::isi::staking::RegisterPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::BondPublicLaneStake>,
    dispatch_instruction::<iroha_data_model::isi::staking::SchedulePublicLaneUnbond>,
    dispatch_instruction::<iroha_data_model::isi::staking::FinalizePublicLaneUnbond>,
    dispatch_instruction::<iroha_data_model::isi::staking::SlashPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::CancelConsensusEvidencePenalty>,
    dispatch_instruction::<iroha_data_model::isi::staking::RecordPublicLaneRewards>,
    dispatch_instruction::<iroha_data_model::isi::settlement::SettlementInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::settlement::DvpIsi>,
    dispatch_instruction::<iroha_data_model::isi::settlement::PvpIsi>,
    dispatch_instruction::<SetKeyValue<Trigger>>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::ActivateContractInstance>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::DeactivateContractInstance>,
    dispatch_instruction::<verifying_keys::RegisterVerifyingKey>,
    dispatch_instruction::<verifying_keys::UpdateVerifyingKey>,
    dispatch_instruction::<zk::RegisterZkAsset>,
    dispatch_instruction::<zk::ScheduleConfidentialPolicyTransition>,
    dispatch_instruction::<zk::CancelConfidentialPolicyTransition>,
    dispatch_instruction::<zk::Shield>,
    dispatch_instruction::<zk::ZkTransfer>,
    dispatch_instruction::<zk::Unshield>,
    dispatch_instruction::<zk::CreateElection>,
    dispatch_instruction::<zk::SubmitBallot>,
    dispatch_instruction::<zk::FinalizeElection>,
    dispatch_instruction::<zk::VerifyProof>,
    dispatch_instruction::<zk::PruneProofs>,
    dispatch_instruction::<iroha_data_model::isi::bridge::SubmitBridgeProof>,
    dispatch_instruction::<iroha_data_model::isi::bridge::RecordBridgeReceipt>,
    dispatch_instruction::<iroha_data_model::isi::bridge::RecordSccpMessage>,
    dispatch_instruction::<confidential::PublishPedersenParams>,
    dispatch_instruction::<confidential::SetPedersenParamsLifecycle>,
    dispatch_instruction::<confidential::PublishPoseidonParams>,
    dispatch_instruction::<confidential::SetPoseidonParamsLifecycle>,
    dispatch_instruction::<iroha_data_model::isi::consensus_keys::RegisterConsensusKey>,
    dispatch_instruction::<iroha_data_model::isi::consensus_keys::RotateConsensusKey>,
    dispatch_instruction::<iroha_data_model::isi::consensus_keys::DisableConsensusKey>,
    dispatch_instruction::<iroha_data_model::isi::endorsement::RegisterDomainCommittee>,
    dispatch_instruction::<iroha_data_model::isi::endorsement::SetDomainEndorsementPolicy>,
    dispatch_instruction::<iroha_data_model::isi::endorsement::SubmitDomainEndorsement>,
    dispatch_instruction::<iroha_data_model::isi::ministry::SubmitAgendaProposal>,
    dispatch_instruction::<iroha_data_model::isi::governance::ProposeDeployContract>,
    dispatch_instruction::<iroha_data_model::isi::governance::ProposeRuntimeUpgradeProposal>,
    dispatch_instruction::<iroha_data_model::isi::governance::CastZkBallot>,
    dispatch_instruction::<iroha_data_model::isi::governance::CastPlainBallot>,
    dispatch_instruction::<iroha_data_model::isi::governance::EnactReferendum>,
    dispatch_instruction::<iroha_data_model::isi::governance::FinalizeReferendum>,
    dispatch_instruction::<iroha_data_model::isi::governance::ApproveGovernanceProposal>,
    dispatch_instruction::<iroha_data_model::isi::governance::PersistCouncilForEpoch>,
    dispatch_instruction::<iroha_data_model::isi::governance::RecordCitizenServiceOutcome>,
    dispatch_instruction::<iroha_data_model::isi::governance::RegisterCitizen>,
    dispatch_instruction::<iroha_data_model::isi::governance::UnregisterCitizen>,
    dispatch_instruction::<iroha_data_model::isi::governance::SlashGovernanceLock>,
    dispatch_instruction::<iroha_data_model::isi::governance::RestituteGovernanceLock>,
];

pub(crate) fn execute_borrowed_instruction(
    instruction: &InstructionBox,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    iroha_logger::debug!(isi=%instruction, "Executing");

    if let Some(result) = INSTRUCTION_HANDLERS
        .iter()
        .find_map(|handler| handler(instruction, authority, state_transaction))
    {
        return result;
    }

    // Custom instructions are expected to be handled by a custom executor
    if instruction
        .as_any()
        .downcast_ref::<CustomInstruction>()
        .is_some()
    {
        return Err(Error::from(
            "Custom instructions require an executor upgrade",
        ));
    }

    // If we reach here, the instruction type is unknown or unregistered
    Err(Error::from("Unknown instruction type"))
}

impl Execute for InstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_borrowed_instruction(&self, authority, state_transaction)
    }
}

impl Execute for iroha_data_model::isi::InvalidInstruction {
    fn execute(
        self,
        _authority: &AccountId,
        _state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        Err(Error::from(format!(
            "invalid instruction payload: wire_id={} payload_hash={} message={}",
            self.wire_id,
            hex::encode(self.payload_hash),
            self.message
        )))
    }
}

impl Execute for RegisterBox {
    #[iroha_logger::log(name = "register", skip_all, fields(id))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Peer(isi) => isi.execute(authority, state_transaction),
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Role(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for UnregisterBox {
    #[iroha_logger::log(name = "unregister", skip_all, fields(id))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Peer(isi) => isi.execute(authority, state_transaction),
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Role(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for MintBox {
    #[iroha_logger::log(name = "Mint", skip_all, fields(destination))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Asset(isi) => isi.execute(authority, state_transaction),
            Self::TriggerRepetitions(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for BurnBox {
    #[iroha_logger::log(name = "burn", skip_all, fields(destination))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Asset(isi) => isi.execute(authority, state_transaction),
            Self::TriggerRepetitions(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for TransferBox {
    #[iroha_logger::log(name = "transfer", skip_all, fields(from, to))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Asset(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for SetKeyValueBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for RemoveKeyValueBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Domain(isi) => isi.execute(authority, state_transaction),
            Self::Account(isi) => isi.execute(authority, state_transaction),
            Self::AssetDefinition(isi) => isi.execute(authority, state_transaction),
            Self::Nft(isi) => isi.execute(authority, state_transaction),
            Self::Trigger(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for iroha_data_model::isi::rwa::RwaInstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Register(isi) => isi.execute(authority, state_transaction),
            Self::Transfer(isi) => isi.execute(authority, state_transaction),
            Self::Merge(isi) => isi.execute(authority, state_transaction),
            Self::Redeem(isi) => isi.execute(authority, state_transaction),
            Self::Freeze(isi) => isi.execute(authority, state_transaction),
            Self::Unfreeze(isi) => isi.execute(authority, state_transaction),
            Self::Hold(isi) => isi.execute(authority, state_transaction),
            Self::Release(isi) => isi.execute(authority, state_transaction),
            Self::ForceTransfer(isi) => isi.execute(authority, state_transaction),
            Self::SetControls(isi) => isi.execute(authority, state_transaction),
            Self::SetKeyValue(isi) => isi.execute(authority, state_transaction),
            Self::RemoveKeyValue(isi) => isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for GrantBox {
    #[iroha_logger::log(name = "grant", skip_all, fields(object))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Permission(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::Role(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::RolePermission(sub_isi) => sub_isi.execute(authority, state_transaction),
        }
    }
}

impl Execute for RevokeBox {
    #[iroha_logger::log(name = "revoke", skip_all, fields(object))]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            Self::Permission(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::Role(sub_isi) => sub_isi.execute(authority, state_transaction),
            Self::RolePermission(sub_isi) => sub_isi.execute(authority, state_transaction),
        }
    }
}

pub mod prelude {
    //! Re-export important traits and types for glob import `(::*)`
    pub use super::*;
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU32, sync::Arc};

    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        block::consensus::LaneBlockCommitment,
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::error::{InstructionExecutionError, InvalidParameterError},
        nexus::{
            AxtFastpqBinding, AxtProofEnvelope, DataSpaceCatalog, DataSpaceId, DataSpaceMetadata,
            LaneCatalog, LaneConfig, LaneFastpqProofMaterial, LaneId, LaneRelayEnvelope, ProofBlob,
        },
        permission,
    };
    use iroha_executor_data_model::permission::trigger::CanRegisterTrigger;
    use iroha_test_samples::{
        ALICE_ID, ALICE_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
        gen_account_in,
    };
    use tokio::test;

    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
        tx::AcceptedTransaction,
    };

    fn axt_test_digest(domain: &[u8], parts: &[&[u8]]) -> iroha_crypto::Hash {
        let mut payload = Vec::new();
        payload.extend_from_slice(domain);
        for part in parts {
            payload.extend_from_slice(part);
        }
        iroha_crypto::Hash::new(payload)
    }

    fn minimal_contract_artifact() -> (
        Vec<u8>,
        iroha_data_model::smart_contract::manifest::ContractManifest,
    ) {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let interface = ivm::EmbeddedContractInterfaceV1 {
            compiler_fingerprint: "isi-mod-test".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: "main".to_owned(),
                kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
                params: Vec::new(),
                return_type: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: None,
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            states: Vec::new(),
        };
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut artifact = meta.encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&code);
        let verified = ivm::verify_contract_artifact(&artifact).expect("valid test contract");
        (artifact, verified.manifest)
    }

    fn axt_proof_blob_for(
        dsid: DataSpaceId,
        manifest_root: [u8; 32],
        proof_seed: &[u8],
        expiry_slot: u64,
    ) -> ProofBlob {
        let source_tx_commitment = axt_test_digest(b"axt-isi-test:source-tx", &[proof_seed]);
        let claim_digest = axt_test_digest(b"axt-isi-test:claim", &[proof_seed]);
        let witness_commitment = axt_test_digest(b"axt-isi-test:witness", &[proof_seed]);
        let policy_commitment = axt_test_digest(b"axt-isi-test:policy", &[&manifest_root]);
        let binding = AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: dsid.as_u64(),
            source_dataspace: format!("isi-test-dataspace-{}", dsid.as_u64()),
            source_receipt_id: format!("receipt-{}", hex::encode(source_tx_commitment.as_ref())),
            source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(claim_digest.as_ref()),
            witness_commitment: hex::encode(witness_commitment.as_ref()),
            policy_commitment: hex::encode(policy_commitment.as_ref()),
            verified_effect_type: "test_effect".to_owned(),
            corridor: "isi-test-corridor".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: None,
        };
        let mut dsid_bytes = [0_u8; 16];
        dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
        let mut batch = fastpq_prover::TransitionBatch::new(
            fastpq_prover::AXT_DEFAULT_PARAMETER,
            fastpq_prover::PublicInputs {
                dsid: dsid_bytes,
                slot: expiry_slot,
                old_root: axt_test_digest(b"axt-isi-test:old-root", &[proof_seed]).into(),
                new_root: manifest_root,
                perm_root: axt_test_digest(b"axt-isi-test:perm-root", &[proof_seed]).into(),
                tx_set_hash: axt_test_digest(b"axt-isi-test:tx-set", &[proof_seed]).into(),
            },
        );
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/isi/proof".to_vec(),
            proof_seed.to_vec(),
            manifest_root.to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
        batch.sort();
        batch.metadata.insert(
            "entry_hash".to_owned(),
            source_tx_commitment.as_ref().to_vec(),
        );
        fastpq_prover::bind_axt_batch(&mut batch, &binding).expect("bind AXT ISI test batch");
        let proof = fastpq_prover::Prover::canonical_with_modes(
            fastpq_prover::AXT_DEFAULT_PARAMETER,
            fastpq_prover::ExecutionMode::Cpu,
            fastpq_prover::PoseidonExecutionMode::Cpu,
        )
        .expect("FASTPQ prover")
        .prove(&batch)
        .expect("FASTPQ proof");
        let fastpq_payload =
            fastpq_prover::encode_axt_fastpq_payload(&batch, proof).expect("AXT FASTPQ payload");
        let envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: fastpq_payload,
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        ProofBlob {
            payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
            expiry_slot: Some(expiry_slot),
        }
    }

    fn state_with_test_domains(kura: &Arc<Kura>) -> Result<State> {
        let world = World::with([], [], []);
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura.clone(), query_handle);
        let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let wonderland: DomainId = DomainId::try_new("wonderland", "universal")?;
        Register::domain(Domain::new(wonderland.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Register::account(Account::new(ALICE_ID.clone()))
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        let trigger_perm: permission::Permission = CanRegisterTrigger {
            authority: ALICE_ID.clone(),
        }
        .into();
        Grant::account_permission(trigger_perm, ALICE_ID.clone())
            .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(asset_definition_id.name().to_string()),
        )
        .execute(&SAMPLE_GENESIS_ACCOUNT_ID, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        Ok(state)
    }

    fn configure_lane_relay_catalogs(
        state_transaction: &mut StateTransaction<'_, '_>,
        dsid: DataSpaceId,
        lane_id: LaneId,
    ) {
        state_transaction.nexus.enabled = true;
        let dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: dsid,
            alias: format!("ds-{}", dsid.as_u64()),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("dataspace catalog");
        state_transaction.nexus.dataspace_catalog = dataspace_catalog.clone();
        state_transaction.world.dataspace_catalog = dataspace_catalog;
        let lane_count = NonZeroU32::new(lane_id.as_u32().saturating_add(1))
            .expect("lane count must be nonzero");
        state_transaction.nexus.lane_catalog = LaneCatalog::new(
            lane_count,
            vec![LaneConfig {
                id: lane_id,
                dataspace_id: dsid,
                alias: format!("lane-{}", lane_id.as_u32()),
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
    }

    fn sample_lane_relay_envelope(
        block_header: iroha_data_model::block::BlockHeader,
        lane_id: LaneId,
        dsid: DataSpaceId,
        manifest_root: [u8; 32],
        proof_digest: iroha_crypto::Hash,
    ) -> LaneRelayEnvelope {
        let settlement_commitment = LaneBlockCommitment {
            block_height: block_header.height().get(),
            lane_id,
            dataspace_id: dsid,
            tx_count: 1,
            total_local_micro: 76,
            total_xor_due_micro: 1,
            total_xor_after_haircut_micro: 1,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
        };
        let envelope = LaneRelayEnvelope::new(block_header, None, None, settlement_commitment, 0)
            .expect("valid lane relay envelope")
            .with_manifest_root(Some(manifest_root));
        let verified_at_height = envelope.block_height;
        envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest,
            verified_at_height,
        }))
    }

    #[test]
    async fn register_verified_lane_relay_instruction_box_is_registered() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();

        let settlement_commitment = LaneBlockCommitment {
            block_height: block_header.height().get(),
            lane_id: LaneId::new(3),
            dataspace_id: DataSpaceId::new(10),
            tx_count: 1,
            total_local_micro: 76,
            total_xor_due_micro: 1,
            total_xor_after_haircut_micro: 1,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
        };
        let manifest_root = [0x42; 32];
        let proof_blob = axt_proof_blob_for(
            DataSpaceId::new(10),
            manifest_root,
            b"register-lane-relay",
            block_header.height().get() + 10,
        );
        let proof_digest = iroha_crypto::Hash::new(&proof_blob.payload);
        let envelope = LaneRelayEnvelope::new(block_header, None, None, settlement_commitment, 0)?
            .with_manifest_root(Some(manifest_root));
        let verified_at_height = envelope.block_height;
        let envelope = envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest,
            verified_at_height,
        }));
        let instruction =
            InstructionBox::from(iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
                envelope,
                proof_blob,
            });

        let is_registered = INSTRUCTION_HANDLERS
            .iter()
            .any(|handler| handler(&instruction, &ALICE_ID, &mut state_transaction).is_some());

        assert!(
            is_registered,
            "RegisterVerifiedLaneRelay must be wired into INSTRUCTION_HANDLERS"
        );
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_mismatched_fastpq_digest() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();
        let dsid = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        configure_lane_relay_catalogs(&mut state_transaction, dsid, lane_id);

        let manifest_root = [0x42; 32];
        let proof_blob = axt_proof_blob_for(
            dsid,
            manifest_root,
            b"register-lane-relay-digest-mismatch",
            block_header.height().get() + 10,
        );
        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            manifest_root,
            iroha_crypto::Hash::new(b"wrong-axt-proof-payload"),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("mismatched proof digest must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("proof digest does not match proof_blob payload")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_future_fastpq_height() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();
        let dsid = DataSpaceId::new(10);
        let lane_id = LaneId::new(3);
        configure_lane_relay_catalogs(&mut state_transaction, dsid, lane_id);

        let manifest_root = [0x42; 32];
        let proof_blob = axt_proof_blob_for(
            dsid,
            manifest_root,
            b"register-lane-relay-future-height",
            block_header.height().get() + 10,
        );
        let mut envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            manifest_root,
            iroha_crypto::Hash::new(&proof_blob.payload),
        );
        envelope
            .fastpq_proof
            .as_mut()
            .expect("proof material")
            .verified_at_height = state_transaction.block_height().saturating_add(1);
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("future proof height must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("proof metadata height is in the future")
        ));
        Ok(())
    }

    #[test]
    async fn nft() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let nft_id: NftId = "rose$wonderland.universal".parse()?;
        let key = "Bytes".parse::<Name>()?;
        Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
            .execute(&account_id, &mut state_transaction)?;
        SetKeyValue::nft(nft_id.clone(), key.clone(), vec![1_u32, 2_u32, 3_u32])
            .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let state_view = state.view();
        let nft = state_view.world.nft(&nft_id)?;
        let value = nft.content.get(&key).cloned();
        assert_eq!(value, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn account_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let key = "Bytes".parse::<Name>()?;
        SetKeyValue::account(account_id.clone(), key.clone(), vec![1_u32, 2_u32, 3_u32])
            .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let bytes = state.view().world.map_account(&account_id, |account| {
            account.value().metadata().get(&key).cloned()
        })?;
        assert_eq!(bytes, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn account_metadata_limit() -> Result<()> {
        use std::str::FromStr as _;

        use iroha_data_model::{
            parameter::{CustomParameter, CustomParameterId},
            prelude::Parameter,
        };
        use iroha_primitives::json::Json;

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();

        // Set a very small metadata size limit via custom parameter
        let param_id = CustomParameterId::from_str("max_metadata_value_bytes")?;
        let small_limit = 16_u64;
        let set_param = SetParameter::new(Parameter::Custom(CustomParameter::new(
            param_id,
            Json::new(small_limit),
        )));
        set_param.execute(&account_id, &mut state_transaction)?;

        // Attempt to set a metadata value exceeding the limit
        let key = "TooBig".parse::<Name>()?;
        let big = Json::new("X".repeat(32)); // 32 > 16
        let res = SetKeyValue::account(account_id.clone(), key.clone(), big)
            .execute(&account_id, &mut state_transaction);
        assert!(matches!(res, Err(Error::InvalidParameter(_))));

        // Now lower the value and ensure it succeeds
        let ok = Json::new("Y".repeat(8));
        SetKeyValue::account(account_id.clone(), key.clone(), ok)
            .execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();
        Ok(())
    }

    #[test]
    async fn register_contract_manifest_is_queryable_without_permission() -> Result<()> {
        use iroha_data_model::{isi::smart_contract_code, query::smart_contract::prelude};

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let alice = ALICE_ID.clone();
        let (code, manifest) = minimal_contract_artifact();
        let h = manifest.code_hash.expect("manifest code hash");
        smart_contract_code::RegisterSmartContractBytes { code_hash: h, code }
            .execute(&alice, &mut stx)?;
        let manifest = manifest.signed(&ALICE_KEYPAIR);

        smart_contract_code::RegisterSmartContractCode {
            manifest: manifest.clone(),
        }
        .execute(&alice, &mut stx)?;

        stx.apply();
        state_block.commit().unwrap();

        // Verify it is stored
        let got = state.view().world().contract_manifests().get(&h).cloned();
        assert_eq!(got, Some(manifest.clone()));

        // Verify query returns it
        let q = prelude::FindContractManifestByCodeHash { code_hash: h };
        let out = <_ as crate::smartcontracts::ValidSingularQuery>::execute(&q, &state.view())?;
        assert_eq!(out, manifest);

        Ok(())
    }

    #[test]
    async fn register_contract_manifest_requires_provenance() -> Result<()> {
        use iroha_crypto::Hash;
        use iroha_data_model::{
            isi::smart_contract_code, permission, prelude as dm, smart_contract::manifest,
        };

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let alice = ALICE_ID.clone();
        let h = Hash::new(b"dummy_code");
        let manifest = manifest::ContractManifest {
            code_hash: Some(h),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };

        let token =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
        let perm: permission::Permission = token.into();
        dm::Grant::account_permission(perm, alice.clone()).execute(&alice, &mut stx)?;

        let err = smart_contract_code::RegisterSmartContractCode { manifest }
            .execute(&alice, &mut stx)
            .expect_err("missing provenance must fail");
        match err {
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                assert!(msg.contains("provenance"), "unexpected msg: {msg}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        Ok(())
    }

    #[test]
    async fn register_contract_manifest_rejects_wrong_signer() -> Result<()> {
        use iroha_crypto::Hash;
        use iroha_data_model::{
            isi::smart_contract_code, permission, prelude as dm, smart_contract::manifest,
        };

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut stx = state_block.transaction();

        let alice = ALICE_ID.clone();
        let h = Hash::new(b"dummy_code");
        let manifest = manifest::ContractManifest {
            code_hash: Some(h),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&KeyPair::random());

        let token =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
        let perm: permission::Permission = token.into();
        dm::Grant::account_permission(perm, alice.clone()).execute(&alice, &mut stx)?;

        let err = smart_contract_code::RegisterSmartContractCode { manifest }
            .execute(&alice, &mut stx)
            .expect_err("wrong signer must fail");
        match err {
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg)) => {
                assert!(
                    msg.contains("not authorised"),
                    "unexpected msg for wrong signer: {msg}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
        Ok(())
    }

    #[test]
    async fn burning_trigger_to_zero_removes_it() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "will_be_removed".parse::<TriggerId>()?;

        // Register the trigger with Exactly(1) repeats
        let register_trigger = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            ),
        ));
        register_trigger.execute(&account_id, &mut state_transaction)?;

        // Burn 1 repeat to reach zero; the trigger should be removed immediately
        Burn::trigger_repetitions(1, trigger_id.clone())
            .execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        // Verify trigger is no longer active
        let active = state
            .view()
            .world
            .triggers()
            .inspect_by_id(&trigger_id, |_| ())
            .is_some();
        assert!(!active, "trigger should be removed at zero repeats");

        Ok(())
    }

    #[test]
    async fn registering_zero_repeat_trigger_is_noop() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "no_effect".parse::<TriggerId>()?;

        // Attempt to register a trigger with Exactly(0) repeats
        let register_trigger = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(0),
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            ),
        ));
        register_trigger.execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        // The trigger should not be present/active
        let active = state
            .view()
            .world
            .triggers()
            .inspect_by_id(&trigger_id, |_| ())
            .is_some();
        assert!(!active, "zero-repeat triggers must not be registered");

        Ok(())
    }

    #[test]
    async fn register_box_trigger_executes() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let trigger_id = "boxed_trigger".parse::<TriggerId>()?;

        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(ALICE_ID.clone()),
            ),
        );
        RegisterBox::Trigger(Register::trigger(trigger))
            .execute(&ALICE_ID, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        let registered = state
            .view()
            .world
            .triggers()
            .inspect_by_id(&trigger_id, |_| ())
            .is_some();
        assert!(registered, "trigger should be registered via RegisterBox");

        Ok(())
    }

    #[test]
    async fn asset_definition_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let account_id = ALICE_ID.clone();
        let key = "Bytes".parse::<Name>()?;
        SetKeyValue::asset_definition(
            definition_id.clone(),
            key.clone(),
            vec![1_u32, 2_u32, 3_u32],
        )
        .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let value = state
            .view()
            .world
            .asset_definition(&definition_id)?
            .metadata()
            .get(&key)
            .cloned();
        assert_eq!(value, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn instruction_box_handles_asset_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal")?,
            "rose".parse()?,
        );
        let asset_id = AssetId::new(asset_definition_id, account_id.clone());
        Mint::asset_numeric(numeric!(1), asset_id.clone())
            .execute(&account_id, &mut state_transaction)?;

        let key = "note".parse::<Name>()?;
        let value = Json::from(norito::json!("demo"));
        InstructionBox::from(SetAssetKeyValue::new(asset_id.clone(), key.clone(), value))
            .execute(&account_id, &mut state_transaction)?;
        InstructionBox::from(RemoveAssetKeyValue::new(asset_id.clone(), key))
            .execute(&account_id, &mut state_transaction)?;

        state_transaction.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let metadata = view.world.asset_metadata().get(&asset_id);
        assert!(metadata.is_none(), "asset metadata should be cleared");
        Ok(())
    }

    #[test]
    async fn domain_metadata() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let domain_id = DomainId::try_new("wonderland", "universal")?;
        let account_id = ALICE_ID.clone();
        let key = "Bytes".parse::<Name>()?;
        SetKeyValue::domain(domain_id.clone(), key.clone(), vec![1_u32, 2_u32, 3_u32])
            .execute(&account_id, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();
        let bytes = state
            .view()
            .world
            .domain(&domain_id)?
            .metadata()
            .get(&key)
            .cloned();
        assert_eq!(bytes, Some(vec![1_u32, 2_u32, 3_u32,].into()));
        Ok(())
    }

    #[test]
    async fn executing_unregistered_trigger_should_return_error() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "test_trigger_id".parse()?;

        assert!(matches!(
            ExecuteTrigger::new(trigger_id)
                .execute(&account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::Find(_)
        ));

        state_transaction.apply();
        state_block.commit().unwrap();

        Ok(())
    }

    #[test]
    async fn unauthorized_trigger_execution_should_return_error() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let (fake_account_id, _fake_account_keypair) = gen_account_in("wonderland");
        let trigger_id = "test_trigger_id".parse::<TriggerId>()?;

        // register fake account
        let register_account = Register::account(Account::new(fake_account_id.clone()));
        register_account.execute(&account_id, &mut state_transaction)?;

        // register the trigger
        let register_trigger = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Indefinitely,
                account_id.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(account_id.clone()),
            ),
        ));

        register_trigger.execute(&account_id, &mut state_transaction)?;

        // execute with the valid account
        ExecuteTrigger::new(trigger_id.clone()).execute(&account_id, &mut state_transaction)?;

        // execute with the fake account
        assert!(matches!(
            ExecuteTrigger::new(trigger_id)
                .execute(&fake_account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::InvariantViolation(_)
        ));

        state_transaction.apply();
        state_block.commit().unwrap();

        Ok(())
    }

    #[test]
    async fn time_trigger_with_single_execution_is_not_mintable() -> Result<()> {
        use iroha_data_model::events::time::{ExecutionTime, Schedule, TimeEventFilter};

        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        let trigger_id = "single_time".parse::<TriggerId>()?;

        // Schedule with no period (single execution) is not mintable; repeats must be Exactly(1)
        let filter = TimeEventFilter::new(ExecutionTime::Schedule(Schedule {
            start_ms: 0,
            period_ms: None,
        }));

        let bad = Register::trigger(Trigger::new(
            trigger_id.clone(),
            Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(2), // invalid for non-mintable filter
                account_id.clone(),
                filter,
            ),
        ));
        assert!(matches!(
            bad.execute(&account_id, &mut state_transaction)
                .expect_err("expected error"),
            Error::Math(_)
        ));

        state_transaction.apply();
        state_block.commit().unwrap();
        Ok(())
    }

    #[test]
    async fn not_allowed_to_register_genesis_domain_but_genesis_account_can_be_linked() -> Result<()>
    {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let block_header = ValidBlock::new_dummy(&KeyPair::random().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_transaction = state_block.transaction();
        let account_id = ALICE_ID.clone();
        assert!(matches!(
            Register::domain(Domain::new(DomainId::try_new("genesis", "universal")?))
                .execute(&account_id, &mut state_transaction)
                .expect_err("Error expected"),
            Error::InvariantViolation(_)
        ));
        Register::account(Account::new(SAMPLE_GENESIS_ACCOUNT_ID.clone()))
            .execute(&account_id, &mut state_transaction)?;
        let genesis_account = state_transaction
            .world
            .account(&SAMPLE_GENESIS_ACCOUNT_ID)?;
        assert!(
            genesis_account.id() == &*SAMPLE_GENESIS_ACCOUNT_ID,
            "genesis account should remain canonical after registration"
        );
        state_transaction.apply();
        state_block.commit().unwrap();

        Ok(())
    }

    #[test]
    async fn transaction_signed_by_genesis_account_is_statelessly_accepted() -> Result<()> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };

        let tx = TransactionBuilder::new(chain_id.clone(), SAMPLE_GENESIS_ACCOUNT_ID.clone())
            .with_instructions([Log::new(
                Level::INFO,
                "genesis stateless admission".to_owned(),
            )])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let crypto_cfg = state.crypto();
        assert!(
            AcceptedTransaction::accept(
                tx,
                &chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref()
            )
            .is_ok(),
            "stateless admission should not special-case genesis authority"
        );
        Ok(())
    }
}

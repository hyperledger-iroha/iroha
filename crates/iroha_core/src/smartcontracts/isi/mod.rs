//! This module contains enumeration of all possible Iroha Special
//! Instructions, generic instruction types and related
//! implementations.
pub mod account;
mod account_admission;
pub mod asset;
pub mod block;
/// Content lane instruction handlers.
pub mod content;
/// DeFi-native instruction handlers.
pub mod defi;
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
    dispatch_instruction::<iroha_data_model::isi::defi::DeFiInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::defi::SubmitDefiIntent>,
    dispatch_instruction::<iroha_data_model::isi::defi::SettleDefiIntent>,
    dispatch_instruction::<iroha_data_model::isi::defi::RegisterDefiVault>,
    dispatch_instruction::<iroha_data_model::isi::defi::RecordDefiVaultRequest>,
    dispatch_instruction::<iroha_data_model::isi::defi::RegisterDefiOperator>,
    dispatch_instruction::<iroha_data_model::isi::defi::RecordDefiOperatorHeartbeat>,
    dispatch_instruction::<iroha_data_model::isi::defi::ConfigureDefiAmmHook>,
    dispatch_instruction::<iroha_data_model::isi::defi::RecordDefiHookExecution>,
    dispatch_instruction::<iroha_data_model::isi::defi::RegisterDefiMarginMarket>,
    dispatch_instruction::<iroha_data_model::isi::defi::UpdateDefiMarginAccount>,
    dispatch_instruction::<iroha_data_model::isi::defi::RegisterDefiRwaMarket>,
    dispatch_instruction::<iroha_data_model::isi::defi::ReportDefiRwaNav>,
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
    dispatch_instruction::<iroha_data_model::isi::space_directory::ExpireSpaceDirectoryManifest>,
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
    dispatch_instruction::<iroha_data_model::isi::offline::IssueOfflineNote>,
    dispatch_instruction::<iroha_data_model::isi::offline::RedeemOfflineNote>,
    dispatch_instruction::<iroha_data_model::isi::offline::AuditOfflineNote>,
    dispatch_instruction::<iroha_data_model::isi::offline::KagemushaTransfer>,
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
    dispatch_instruction::<iroha_data_model::isi::soracloud::DeploySoracloudAppInfra>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::UpgradeSoracloudAppInfra>,
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
    dispatch_instruction::<iroha_data_model::isi::soracloud::FinalizeSoracloudUploadedModelBundle>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::AdvanceSoracloudRollout>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::SetSoracloudRuntimeState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::SetSoracloudInrouReplicaRuntimeState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ClearSoracloudInrouReplicaRuntimeState>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::ReportSoracloudServiceLeaseUsage>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RecordSoracloudMailboxMessage>,
    dispatch_instruction::<iroha_data_model::isi::soracloud::RecordSoracloudRuntimeReceipt>,
    dispatch_instruction::<
        iroha_data_model::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt,
    >,
    dispatch_instruction::<iroha_data_model::isi::oracle::RegisterOracleFeed>,
    dispatch_instruction::<iroha_data_model::isi::oracle::SubmitOracleObservation>,
    dispatch_instruction::<iroha_data_model::isi::oracle::AggregateOracleFeed>,
    dispatch_instruction::<iroha_data_model::isi::oracle::OpenOracleDispute>,
    dispatch_instruction::<iroha_data_model::isi::oracle::ResolveOracleDispute>,
    dispatch_instruction::<iroha_data_model::isi::oracle::ProposeOracleChange>,
    dispatch_instruction::<iroha_data_model::isi::oracle::VoteOracleChangeStage>,
    dispatch_instruction::<iroha_data_model::isi::oracle::RollbackOracleChange>,
    dispatch_instruction::<iroha_data_model::isi::oracle::SubmitDefiOracleAttestation>,
    dispatch_instruction::<iroha_data_model::isi::oracle::RecordTwitterBinding>,
    dispatch_instruction::<iroha_data_model::isi::oracle::RevokeTwitterBinding>,
    dispatch_instruction::<iroha_data_model::isi::staking::RebindPublicLaneValidatorPeer>,
    dispatch_instruction::<iroha_data_model::isi::staking::ActivatePublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::ExitPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::nexus::SetLaneRelayEmergencyValidators>,
    dispatch_instruction::<iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay>,
    dispatch_instruction::<iroha_data_model::isi::nexus::RegisterVerifiedNexusFeeBudget>,
    dispatch_instruction::<iroha_data_model::isi::staking::RegisterPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::BondPublicLaneStake>,
    dispatch_instruction::<iroha_data_model::isi::staking::SchedulePublicLaneUnbond>,
    dispatch_instruction::<iroha_data_model::isi::staking::FinalizePublicLaneUnbond>,
    dispatch_instruction::<iroha_data_model::isi::staking::SlashPublicLaneValidator>,
    dispatch_instruction::<iroha_data_model::isi::staking::CancelConsensusEvidencePenalty>,
    dispatch_instruction::<iroha_data_model::isi::staking::RecordPublicLaneRewards>,
    dispatch_instruction::<iroha_data_model::isi::staking::ClaimPublicLaneRewards>,
    dispatch_instruction::<iroha_data_model::isi::settlement::SettlementInstructionBox>,
    dispatch_instruction::<iroha_data_model::isi::settlement::DvpIsi>,
    dispatch_instruction::<iroha_data_model::isi::settlement::PvpIsi>,
    dispatch_instruction::<SetKeyValue<Trigger>>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::ActivateContractInstance>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::DeactivateContractInstance>,
    dispatch_instruction::<iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes>,
    dispatch_instruction::<verifying_keys::RegisterVerifyingKey>,
    dispatch_instruction::<verifying_keys::UpdateVerifyingKey>,
    dispatch_instruction::<zk::RegisterZkAsset>,
    dispatch_instruction::<zk::RegisterAssetHiddenZkPool>,
    dispatch_instruction::<zk::ScheduleConfidentialPolicyTransition>,
    dispatch_instruction::<zk::CancelConfidentialPolicyTransition>,
    dispatch_instruction::<zk::Shield>,
    dispatch_instruction::<zk::ZkTransfer>,
    dispatch_instruction::<zk::AssetHiddenZkTransfer>,
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
    dispatch_instruction::<iroha_data_model::isi::governance::CastParliamentBallot>,
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

#[cfg(test)]
mod registry_dispatch_tests {
    use std::collections::BTreeSet;

    use super::*;

    fn handler_table_source() -> &'static str {
        include_str!("mod.rs")
            .split("const INSTRUCTION_HANDLERS")
            .nth(1)
            .and_then(|tail| tail.split("];").next())
            .expect("handler table source")
    }

    fn has_dispatch_handler(handler_table: &str, type_name: &str) -> bool {
        let root_type_name = type_name.split('<').next().unwrap_or(type_name);
        let leaf = root_type_name
            .rsplit("::")
            .next()
            .expect("type name has at least one segment");
        let imported = format!("::<{leaf}");
        let qualified = format!("::{leaf}");

        handler_table.contains(&imported) || handler_table.contains(&qualified)
    }

    #[test]
    fn default_instruction_registry_entries_have_core_dispatch_handlers() {
        let registry = iroha_data_model::isi::registry::default();
        let handler_table = handler_table_source();
        let custom_instruction = std::any::type_name::<CustomInstruction>();
        let missing = registry
            .names()
            .filter(|name| *name != custom_instruction)
            .filter(|name| !has_dispatch_handler(handler_table, name))
            .collect::<BTreeSet<_>>();

        assert!(
            missing.is_empty(),
            "default registry entries missing core dispatch handlers: {missing:?}"
        );
    }

    #[test]
    fn custom_instruction_is_only_default_registry_entry_without_core_dispatch_handler() {
        let registry = iroha_data_model::isi::registry::default();
        let handler_table = handler_table_source();
        let missing = registry
            .names()
            .filter(|name| !has_dispatch_handler(handler_table, name))
            .collect::<BTreeSet<_>>();
        let expected = BTreeSet::from([std::any::type_name::<CustomInstruction>()]);

        assert_eq!(
            missing, expected,
            "only CustomInstruction should require a custom executor"
        );
    }

    #[test]
    fn custom_instruction_stays_custom_executor_only() {
        let registry = iroha_data_model::isi::registry::default();
        let handler_table = handler_table_source();
        let custom_instruction = std::any::type_name::<CustomInstruction>();

        assert!(
            registry.contains(custom_instruction),
            "custom instructions must remain decodable for custom executors"
        );
        assert!(
            !has_dispatch_handler(handler_table, custom_instruction),
            "custom instructions must not be executable by the default core dispatcher"
        );
    }

    #[test]
    fn direct_grouped_variants_stay_out_of_default_registry_even_with_handlers() {
        let registry = iroha_data_model::isi::registry::default();
        let handler_table = handler_table_source();
        let direct_variants = [
            std::any::type_name::<iroha_data_model::isi::register::RegisterPeerWithPop>(),
            std::any::type_name::<Mint<Numeric, Asset>>(),
            std::any::type_name::<Burn<Numeric, Asset>>(),
            std::any::type_name::<Transfer<Asset, Numeric, Account>>(),
            std::any::type_name::<SetKeyValue<Trigger>>(),
            std::any::type_name::<iroha_data_model::isi::repo::RepoIsi>(),
            std::any::type_name::<iroha_data_model::isi::repo::ReverseRepoIsi>(),
            std::any::type_name::<iroha_data_model::isi::repo::RepoMarginCallIsi>(),
            std::any::type_name::<iroha_data_model::isi::rwa::RegisterRwa>(),
            std::any::type_name::<iroha_data_model::isi::rwa::TransferRwa>(),
            std::any::type_name::<iroha_data_model::isi::rwa::MergeRwas>(),
            std::any::type_name::<iroha_data_model::isi::rwa::RedeemRwa>(),
            std::any::type_name::<iroha_data_model::isi::rwa::FreezeRwa>(),
            std::any::type_name::<iroha_data_model::isi::rwa::UnfreezeRwa>(),
            std::any::type_name::<iroha_data_model::isi::rwa::HoldRwa>(),
            std::any::type_name::<iroha_data_model::isi::settlement::DvpIsi>(),
            std::any::type_name::<iroha_data_model::isi::settlement::PvpIsi>(),
        ];

        for name in direct_variants {
            assert!(
                has_dispatch_handler(handler_table, name),
                "{name} should remain an internal delegation target"
            );
            assert!(
                !registry.contains(name),
                "{name} is an internal handler target, not a public wire form"
            );
        }
    }

    #[test]
    fn removed_direct_stable_wire_ids_do_not_alias_boxed_dispatch_entries() {
        let registry = iroha_data_model::isi::registry::default();
        let removed_wire_ids = [
            iroha_data_model::isi::repo::RepoIsi::WIRE_ID,
            iroha_data_model::isi::repo::ReverseRepoIsi::WIRE_ID,
            iroha_data_model::isi::repo::RepoMarginCallIsi::WIRE_ID,
            iroha_data_model::isi::settlement::DvpIsi::WIRE_ID,
            iroha_data_model::isi::settlement::PvpIsi::WIRE_ID,
        ];

        for wire_id in removed_wire_ids {
            assert!(
                !registry.contains(wire_id),
                "{wire_id} must not alias any default dispatcher entry"
            );
        }
    }
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
        block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::error::{InstructionExecutionError, InvalidParameterError},
        nexus::{
            AxtEffectBinding, AxtFastpqBinding, AxtProofEnvelope, DataSpaceCatalog, DataSpaceId,
            DataSpaceMetadata, LANE_RELAY_FASTPQ_EFFECT_TYPE, LaneCatalog, LaneConfig,
            LaneFastpqProofMaterial, LaneId, LaneRelayEnvelope, ProofBlob, VerifiedLaneRelayRecord,
            VerifiedNexusFeeBudgetRecord, lane_relay_fastpq_claim_digest,
            nexus_fee_budget_claim_digest,
        },
        permission,
    };
    use iroha_executor_data_model::permission::trigger::CanRegisterTrigger;
    use iroha_primitives::numeric::Numeric;
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

    fn axt_lane_relay_proof_blob_for(
        envelope: &LaneRelayEnvelope,
        proof_seed: &[u8],
        expiry_slot: u64,
    ) -> ProofBlob {
        let manifest_root = envelope.manifest_root.expect("test relay manifest root");
        let relay_ref = envelope.relay_ref();
        let relay_ref_bytes = norito::to_bytes(&relay_ref).expect("encode relay ref");
        let source_tx_commitment =
            axt_test_digest(b"axt-isi-test:lane-relay-source-tx", &[&relay_ref_bytes]);
        let claim_digest =
            lane_relay_fastpq_claim_digest(envelope).expect("lane relay claim digest");
        let witness_commitment = axt_test_digest(
            b"axt-isi-test:lane-relay-witness",
            &[envelope.settlement_hash.as_ref()],
        );
        let policy_commitment =
            axt_test_digest(b"axt-isi-test:lane-relay-policy", &[&manifest_root]);
        let dsid = envelope.dataspace_id;
        let binding = AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: dsid.as_u64(),
            source_dataspace: format!("isi-test-dataspace-{}", dsid.as_u64()),
            source_receipt_id: format!("relay-{}", hex::encode(relay_ref_bytes)),
            source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(claim_digest.as_ref()),
            witness_commitment: hex::encode(witness_commitment.as_ref()),
            policy_commitment: hex::encode(policy_commitment.as_ref()),
            verified_effect_type: LANE_RELAY_FASTPQ_EFFECT_TYPE.to_owned(),
            corridor: "isi-test-lane-relay".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
            effect_binding: None,
        };
        let mut dsid_bytes = [0_u8; 16];
        dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
        let mut batch = fastpq_prover::TransitionBatch::new(
            fastpq_prover::AXT_DEFAULT_PARAMETER,
            fastpq_prover::PublicInputs {
                dsid: dsid_bytes,
                slot: expiry_slot,
                old_root: axt_test_digest(b"axt-isi-test:lane-relay-old-root", &[proof_seed])
                    .into(),
                new_root: manifest_root,
                perm_root: axt_test_digest(b"axt-isi-test:lane-relay-perm-root", &[proof_seed])
                    .into(),
                tx_set_hash: axt_test_digest(
                    b"axt-isi-test:lane-relay-tx-set",
                    &[claim_digest.as_ref()],
                )
                .into(),
            },
        );
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/isi/lane-relay".to_vec(),
            proof_seed.to_vec(),
            claim_digest.as_ref().to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
        batch.sort();
        batch.metadata.insert(
            "entry_hash".to_owned(),
            source_tx_commitment.as_ref().to_vec(),
        );
        fastpq_prover::bind_axt_batch(&mut batch, &binding).expect("bind AXT lane relay batch");
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
        let proof_envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: fastpq_payload,
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        ProofBlob {
            payload: norito::to_bytes(&proof_envelope).expect("encode proof envelope"),
            expiry_slot: Some(expiry_slot),
        }
    }

    fn axt_effect_proof_blob_for(
        envelope: &LaneRelayEnvelope,
        proof_seed: &[u8],
        expiry_slot: u64,
    ) -> ProofBlob {
        let manifest_root = envelope.manifest_root.expect("test relay manifest root");
        let relay_ref = envelope.relay_ref();
        let relay_ref_bytes = norito::to_bytes(&relay_ref).expect("encode relay ref");
        let source_tx_commitment = axt_test_digest(
            b"axt-isi-test:effect-source-tx",
            &[proof_seed, &relay_ref_bytes],
        );
        let claim_digest = axt_test_digest(
            b"axt-isi-test:effect-claim",
            &[source_tx_commitment.as_ref()],
        );
        let witness_commitment = axt_test_digest(b"axt-isi-test:effect-witness", &[proof_seed]);
        let policy_commitment = axt_test_digest(b"axt-isi-test:effect-policy", &[&manifest_root]);
        let dsid = envelope.dataspace_id;
        let binding = AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: dsid.as_u64(),
            source_dataspace: format!("isi-test-dataspace-{}", dsid.as_u64()),
            source_receipt_id: format!("effect-{}", hex::encode(source_tx_commitment.as_ref())),
            source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(claim_digest.as_ref()),
            witness_commitment: hex::encode(witness_commitment.as_ref()),
            policy_commitment: hex::encode(policy_commitment.as_ref()),
            verified_effect_type: "aed_to_pkr_settlement".to_owned(),
            corridor: "isi-test-effect".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()],
            effect_binding: Some(AxtEffectBinding {
                destination_domain: Some("hbl".to_owned()),
                destination_account_id: Some(ALICE_ID.to_string()),
                vault_account_id: None,
                issuance_account_id: None,
                source_asset_definition_id: Some("aed#cbuae".to_owned()),
                destination_asset_definition_id: Some("pkr#sbp".to_owned()),
                source_amount_i64: Some(10),
                destination_amount_i64: Some(760),
            }),
        };
        let mut dsid_bytes = [0_u8; 16];
        dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
        let mut batch = fastpq_prover::TransitionBatch::new(
            fastpq_prover::AXT_DEFAULT_PARAMETER,
            fastpq_prover::PublicInputs {
                dsid: dsid_bytes,
                slot: expiry_slot,
                old_root: axt_test_digest(b"axt-isi-test:effect-old-root", &[proof_seed]).into(),
                new_root: manifest_root,
                perm_root: axt_test_digest(b"axt-isi-test:effect-perm-root", &[proof_seed]).into(),
                tx_set_hash: axt_test_digest(
                    b"axt-isi-test:effect-tx-set",
                    &[claim_digest.as_ref()],
                )
                .into(),
            },
        );
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/isi/effect".to_vec(),
            proof_seed.to_vec(),
            claim_digest.as_ref().to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
        batch.sort();
        batch.metadata.insert(
            "entry_hash".to_owned(),
            source_tx_commitment.as_ref().to_vec(),
        );
        fastpq_prover::bind_axt_batch(&mut batch, &binding).expect("bind AXT effect batch");
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
        let proof_envelope = AxtProofEnvelope {
            dsid,
            manifest_root,
            da_commitment: None,
            proof: fastpq_payload,
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        };
        ProofBlob {
            payload: norito::to_bytes(&proof_envelope).expect("encode effect proof envelope"),
            expiry_slot: Some(expiry_slot),
        }
    }

    fn axt_fee_budget_proof_blob_for(
        sponsor: &AccountId,
        fee_asset_id: &str,
        verified_balance: &Numeric,
        manifest_root: [u8; 32],
        expiry_slot: u64,
    ) -> ProofBlob {
        let fee_asset_id = fee_asset_id.trim();
        let sponsor_bytes = sponsor.to_string();
        let balance_bytes = verified_balance.to_string();
        let source_tx_commitment = axt_test_digest(
            b"axt-isi-test:budget-source-tx",
            &[sponsor_bytes.as_bytes(), fee_asset_id.as_bytes()],
        );
        let claim_digest = nexus_fee_budget_claim_digest(sponsor, fee_asset_id, verified_balance);
        let witness_commitment = axt_test_digest(
            b"axt-isi-test:budget-witness",
            &[sponsor_bytes.as_bytes(), balance_bytes.as_bytes()],
        );
        let policy_commitment = axt_test_digest(b"axt-isi-test:budget-policy", &[&manifest_root]);
        let dsid = DataSpaceId::UNIVERSAL;
        let binding = AxtFastpqBinding {
            parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
            source_dsid: dsid.as_u64(),
            source_dataspace: "universal".to_owned(),
            source_receipt_id: format!("budget-{}", hex::encode(source_tx_commitment.as_ref())),
            source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
            claim_type: "authorization".to_owned(),
            claim_digest: hex::encode(claim_digest.as_ref()),
            witness_commitment: hex::encode(witness_commitment.as_ref()),
            policy_commitment: hex::encode(policy_commitment.as_ref()),
            verified_effect_type: "nexus_fee_budget".to_owned(),
            corridor: "isi-test-fee-budget".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: Some(AxtEffectBinding {
                destination_domain: None,
                destination_account_id: Some(sponsor.to_string()),
                vault_account_id: None,
                issuance_account_id: None,
                source_asset_definition_id: Some(fee_asset_id.to_owned()),
                destination_asset_definition_id: None,
                source_amount_i64: None,
                destination_amount_i64: None,
            }),
        };
        let mut dsid_bytes = [0_u8; 16];
        dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
        let mut batch = fastpq_prover::TransitionBatch::new(
            fastpq_prover::AXT_DEFAULT_PARAMETER,
            fastpq_prover::PublicInputs {
                dsid: dsid_bytes,
                slot: expiry_slot,
                old_root: axt_test_digest(
                    b"axt-isi-test:budget-old-root",
                    &[fee_asset_id.as_bytes()],
                )
                .into(),
                new_root: manifest_root,
                perm_root: axt_test_digest(
                    b"axt-isi-test:budget-perm-root",
                    &[sponsor_bytes.as_bytes()],
                )
                .into(),
                tx_set_hash: axt_test_digest(
                    b"axt-isi-test:budget-tx-set",
                    &[balance_bytes.as_bytes()],
                )
                .into(),
            },
        );
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/isi/nexus-fee-budget".to_vec(),
            sponsor_bytes.as_bytes().to_vec(),
            balance_bytes.as_bytes().to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
        batch.sort();
        batch.metadata.insert(
            "entry_hash".to_owned(),
            source_tx_commitment.as_ref().to_vec(),
        );
        fastpq_prover::bind_axt_batch(&mut batch, &binding).expect("bind AXT fee budget batch");
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
            committed_amount: Some(
                verified_balance
                    .try_mantissa_u128()
                    .expect("test balance is an integer u128"),
            ),
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
            native_amx_receipts: Vec::new(),
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

    fn relay_state_key_for_test(envelope: &LaneRelayEnvelope) -> Name {
        envelope
            .relay_ref()
            .relay_state_key()
            .parse()
            .expect("relay state key")
    }

    fn lane_relay_envelope_with_proof_payload(
        envelope: LaneRelayEnvelope,
        proof_blob: &ProofBlob,
        verified_at_height: u64,
    ) -> LaneRelayEnvelope {
        envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: iroha_crypto::Hash::new(&proof_blob.payload),
            verified_at_height,
        }))
    }

    fn verified_lane_relay_record_for_test(
        envelope: LaneRelayEnvelope,
        proof_blob: &ProofBlob,
        verified_at_height: u64,
    ) -> VerifiedLaneRelayRecord {
        let proof_envelope: AxtProofEnvelope =
            norito::decode_from_bytes(&proof_blob.payload).expect("decode proof envelope");
        let binding = proof_envelope
            .fastpq_binding
            .clone()
            .expect("test fastpq binding");
        let verified_fastpq = fastpq_prover::verify_axt_proof_envelope(&proof_envelope)
            .expect("verify test fastpq proof");
        VerifiedLaneRelayRecord::new(
            envelope,
            iroha_crypto::Hash::new(&proof_blob.payload),
            verified_fastpq.statement_digest,
            verified_fastpq.proof_digest,
            verified_at_height,
            proof_envelope.manifest_root,
            binding,
        )
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
            native_amx_receipts: Vec::new(),
        };
        let manifest_root = [0x42; 32];
        let envelope = LaneRelayEnvelope::new(block_header, None, None, settlement_commitment, 0)?
            .with_manifest_root(Some(manifest_root));
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay",
            envelope.block_height + 10,
        );
        let proof_digest = iroha_crypto::Hash::new(&proof_blob.payload);
        let verified_at_height = envelope.block_height;
        let envelope = envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest,
            verified_at_height,
        }));
        let instruction =
            InstructionBox::from(iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
                envelope,
                proof_blob,
                effect_proof_blob: None,
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
    async fn default_executor_rejects_invalid_instruction_placeholders() -> Result<()> {
        let state = State::new(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let mut state_block = state.block(valid_block.as_ref().header().clone());
        let mut state_transaction = state_block.transaction();
        let instruction = InstructionBox::from(iroha_data_model::isi::InvalidInstruction::new(
            "iroha.register",
            [0xAB; 32],
            "malformed boxed payload",
        ));

        let err = execute_borrowed_instruction(&instruction, &ALICE_ID, &mut state_transaction)
            .expect_err("invalid instruction placeholders must fail execution");

        assert!(matches!(
            err,
            InstructionExecutionError::Conversion(message)
                if message.contains("invalid instruction payload")
                    && message.contains("wire_id=iroha.register")
                    && message.contains("malformed boxed payload")
        ));
        Ok(())
    }

    #[test]
    async fn default_executor_rejects_custom_instruction_without_custom_executor() -> Result<()> {
        let state = State::new(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let mut state_block = state.block(valid_block.as_ref().header().clone());
        let mut state_transaction = state_block.transaction();
        let instruction = InstructionBox::from(CustomInstruction::new("requires custom executor"));

        let err = execute_borrowed_instruction(&instruction, &ALICE_ID, &mut state_transaction)
            .expect_err("custom instructions must not execute through the default dispatcher");

        assert!(matches!(
            err,
            InstructionExecutionError::Conversion(message)
                if message.contains("Custom instructions require an executor upgrade")
        ));
        Ok(())
    }

    #[test]
    async fn default_executor_rejects_opaque_instruction_even_with_registered_wire_id() -> Result<()>
    {
        let state = State::new(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let mut state_block = state.block(valid_block.as_ref().header().clone());
        let mut state_transaction = state_block.transaction();
        let log = Log::new(Level::INFO, "opaque default executor".to_owned());
        let (payload, flags) = norito::codec::encode_with_header_flags(&log);
        let framed = norito::core::frame_bare_with_header_flags::<Log>(&payload, flags)
            .expect("frame log payload");
        let opaque = iroha_data_model::isi::OpaqueInstruction::from_framed(Log::WIRE_ID, &framed)
            .expect("opaque payload");
        let instruction = InstructionBox::from(opaque);

        let err = execute_borrowed_instruction(&instruction, &ALICE_ID, &mut state_transaction)
            .expect_err("opaque instructions must not execute through the default dispatcher");

        assert!(matches!(
            err,
            InstructionExecutionError::Conversion(message)
                if message.contains("Unknown instruction type")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_nexus_fee_budget_persists_verified_cache_record() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_test_domains(&kura)?;
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();

        let sponsor = ALICE_ID.clone();
        let fee_asset_id = "xor#universal";
        let verified_balance = Numeric::from(10_u32);
        let manifest_root = [0x63; 32];
        state_transaction.nexus.enabled = true;
        state_transaction.nexus.fees.fee_asset_id = fee_asset_id.to_owned();

        let proof_blob = axt_fee_budget_proof_blob_for(
            &sponsor,
            fee_asset_id,
            &verified_balance,
            manifest_root,
            block_header.height().get() + 10,
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedNexusFeeBudget {
            sponsor_account_id: sponsor.clone(),
            fee_asset_id: fee_asset_id.to_owned(),
            verified_balance: verified_balance.clone(),
            manifest_root,
            proof_blob,
        };

        instruction.execute(&ALICE_ID, &mut state_transaction)?;
        state_transaction.apply();
        state_block.commit().unwrap();

        let key: Name =
            VerifiedNexusFeeBudgetRecord::state_key_for(&sponsor, fee_asset_id).parse()?;
        let view = state.view();
        let payload = view
            .world
            .smart_contract_state()
            .get(&key)
            .expect("verified fee budget cache record");
        let json: Json = norito::decode_from_bytes(payload)?;
        let record: VerifiedNexusFeeBudgetRecord = norito::json::from_slice(json.get().as_bytes())?;
        assert_eq!(record.sponsor_account_id, sponsor);
        assert_eq!(record.fee_asset_id, fee_asset_id);
        assert_eq!(record.verified_balance, verified_balance);
        assert_eq!(record.manifest_root, manifest_root);
        assert_eq!(
            record.fastpq_binding.verified_effect_type,
            "nexus_fee_budget"
        );
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_when_nexus_disabled() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();

        let manifest_root = [0x42; 32];
        let envelope = sample_lane_relay_envelope(
            block_header,
            LaneId::new(3),
            DataSpaceId::new(10),
            manifest_root,
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-nexus-disabled",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("disabled nexus must reject verified lane relay registration");
        assert!(matches!(
            err,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("requires nexus.enabled=true")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_unknown_lane_id() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();
        let dsid = DataSpaceId::new(10);
        configure_lane_relay_catalogs(&mut state_transaction, dsid, LaneId::new(3));

        let envelope = sample_lane_relay_envelope(
            block_header,
            LaneId::new(4),
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-unknown-lane",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("unknown lane id must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("unknown lane id 4")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_lane_dataspace_mismatch() -> Result<()> {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);
        let valid_block = ValidBlock::new_dummy(&KeyPair::random().into_parts().1);
        let block_header = valid_block.as_ref().header().clone();
        let mut state_block = state.block(block_header.clone());
        let mut state_transaction = state_block.transaction();
        let lane_dsid = DataSpaceId::new(10);
        let envelope_dsid = DataSpaceId::new(11);
        let lane_id = LaneId::new(3);
        configure_lane_relay_catalogs(&mut state_transaction, lane_dsid, lane_id);

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            envelope_dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-lane-dsid-mismatch",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("lane dataspace mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("belongs to dataspace")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_unknown_dataspace_id() -> Result<()> {
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
        let unrelated_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::UNIVERSAL,
            alias: "universal".to_owned(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("unrelated dataspace catalog");
        state_transaction.nexus.dataspace_catalog = unrelated_catalog.clone();
        state_transaction.world.dataspace_catalog = unrelated_catalog;

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-unknown-dsid",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("unknown dataspace id must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("unknown dataspace id 10")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_empty_proof_payload() -> Result<()> {
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

        let proof_blob = ProofBlob {
            payload: Vec::new(),
            expiry_slot: Some(state_transaction.block_height() + 10),
        };
        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(&proof_blob.payload),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("empty proof payload must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("proof payload is empty")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_malformed_proof_envelope() -> Result<()> {
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

        let proof_blob = ProofBlob {
            payload: vec![0xFF, 0x00, 0xFE],
            expiry_slot: Some(state_transaction.block_height() + 10),
        };
        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(&proof_blob.payload),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("malformed proof envelope must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("proof envelope decode failed")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_proof_manifest_root_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let mut proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-proof-manifest-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        proof_envelope.manifest_root = [0x43; 32];
        proof_blob.payload = norito::to_bytes(&proof_envelope)?;
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("proof manifest root mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("does not match the declared manifest_root")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_proof_dataspace_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let mut proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-proof-dsid-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        proof_envelope.dsid = DataSpaceId::new(11);
        proof_blob.payload = norito::to_bytes(&proof_envelope)?;
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("proof dataspace mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("does not match the declared manifest_root")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_stale_fastpq_height() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-stale-fastpq-height",
            state_transaction.block_height() + 10,
        );
        let stale_verified_at_height = envelope.block_height.saturating_sub(1);
        let envelope = envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: iroha_crypto::Hash::new(&proof_blob.payload),
            verified_at_height: stale_verified_at_height,
        }));
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("stale proof material height must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("FASTPQ binding failed verification")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_zero_like_fastpq_digest() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-zero-like-fastpq-digest",
            state_transaction.block_height() + 10,
        );
        let envelope = envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: iroha_crypto::Hash::prehashed([0; iroha_crypto::Hash::LENGTH]),
            verified_at_height: state_transaction.block_height(),
        }));
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("zero-like FastPQ digest must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("FASTPQ binding failed verification")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_envelope_block_height_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-block-height-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        envelope.block_height = envelope.block_height.saturating_add(1);
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("envelope block height mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("lane relay envelope failed verification")
                && message.contains("block height")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_settlement_lane_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-settlement-lane-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        envelope.settlement_commitment.lane_id = LaneId::new(4);
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("settlement lane mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("lane relay envelope failed verification")
                && message.contains("settlement")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_settlement_dataspace_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-settlement-dsid-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        envelope.settlement_commitment.dataspace_id = DataSpaceId::new(11);
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("settlement dataspace mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("lane relay envelope failed verification")
                && message.contains("settlement")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_settlement_hash_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-settlement-hash-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        envelope.settlement_hash = iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"register-lane-relay-bad-settlement-hash"),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("settlement hash mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("lane relay envelope failed verification")
                && message.contains("settlement")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_settlement_totals_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-settlement-totals-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        envelope
            .settlement_commitment
            .receipts
            .push(LaneSettlementReceipt {
                source_id: [0xA5; 32],
                local_amount_micro: 1,
                xor_due_micro: 1,
                xor_after_haircut_micro: 1,
                xor_variance_micro: 0,
                timestamp_ms: 1_700_000_001_000,
            });
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("settlement totals mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("lane relay envelope failed verification")
                && message.contains("settlement")
        ));
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
        let expiry_slot = block_header.height().get() + 10;
        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            manifest_root,
            iroha_crypto::Hash::new(b"wrong-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-digest-mismatch",
            expiry_slot,
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
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
    async fn register_verified_lane_relay_rejects_mismatched_claim_digest() -> Result<()> {
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
        let expiry_slot = block_header.height().get() + 10;
        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            manifest_root,
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let mut proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-claim-mismatch",
            expiry_slot,
        );
        let mut proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        proof_envelope
            .fastpq_binding
            .as_mut()
            .expect("test fastpq binding")
            .claim_digest = "ee".repeat(32);
        proof_blob.payload = norito::to_bytes(&proof_envelope)?;
        let envelope = envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: iroha_crypto::Hash::new(&proof_blob.payload),
            verified_at_height: state_transaction.block_height(),
        }));
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("mismatched claim digest must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("claim_digest mismatch")
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
        let expiry_slot = block_header.height().get() + 10;
        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            manifest_root,
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-future-height",
            expiry_slot,
        );
        let envelope = envelope.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: iroha_crypto::Hash::new(&proof_blob.payload),
            verified_at_height: state_transaction.block_height().saturating_add(1),
        }));
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
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
    async fn register_verified_lane_relay_rejects_missing_manifest_root() -> Result<()> {
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

        let envelope_with_manifest = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope_with_manifest,
            b"register-lane-relay-missing-manifest",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope_with_manifest.with_manifest_root(None),
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("missing manifest root must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("missing manifest_root")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_zero_manifest_root() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-zero-manifest",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("zero manifest root must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("manifest_root cannot be zeroed")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_expired_proof_blob() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-expired-proof",
            state_transaction.block_height().saturating_sub(1),
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("expired proof must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("proof expired")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_missing_fastpq_binding() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let mut proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-missing-binding",
            state_transaction.block_height() + 10,
        );
        let mut proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        proof_envelope.fastpq_binding = None;
        proof_blob.payload = norito::to_bytes(&proof_envelope)?;
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("missing fastpq binding must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("missing fastpq_binding")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_source_dsid_mismatch() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let mut proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-source-dsid-mismatch",
            state_transaction.block_height() + 10,
        );
        let mut proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        proof_envelope
            .fastpq_binding
            .as_mut()
            .expect("test fastpq binding")
            .source_dsid = dsid.as_u64() + 1;
        proof_blob.payload = norito::to_bytes(&proof_envelope)?;
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("source dataspace mismatch must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("source_dsid mismatch")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_wrong_effect_type() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let mut proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-wrong-effect-type",
            state_transaction.block_height() + 10,
        );
        let mut proof_envelope: AxtProofEnvelope = norito::decode_from_bytes(&proof_blob.payload)?;
        proof_envelope
            .fastpq_binding
            .as_mut()
            .expect("test fastpq binding")
            .verified_effect_type = "nexus_fee_budget".to_owned();
        proof_blob.payload = norito::to_bytes(&proof_envelope)?;
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("wrong effect type must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("effect must be lane_relay_block")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_persists_effect_proof_binding() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-effect-primary",
            state_transaction.block_height() + 10,
        );
        let effect_proof_blob = axt_effect_proof_blob_for(
            &envelope,
            b"register-lane-relay-effect-business",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let relay_state_key = relay_state_key_for_test(&envelope);
        let proof_payload_hash = iroha_crypto::Hash::new(&proof_blob.payload);
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: Some(effect_proof_blob),
        };

        instruction.execute(&ALICE_ID, &mut state_transaction)?;
        let payload = state_transaction
            .world
            .smart_contract_state
            .get(&relay_state_key)
            .expect("verified relay record persisted");
        let stored_json: Json = norito::decode_from_bytes(payload)?;
        let record: VerifiedLaneRelayRecord =
            norito::json::from_slice(stored_json.get().as_bytes())?;

        assert_eq!(record.proof_payload_hash, proof_payload_hash);
        assert_eq!(
            record.fastpq_binding.verified_effect_type,
            "aed_to_pkr_settlement"
        );
        assert!(record.fastpq_binding.effect_binding.is_some());
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_lane_proof_as_effect_proof() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-effect-lane-proof",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            effect_proof_blob: Some(proof_blob.clone()),
            proof_blob,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("lane proof in the effect slot must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("effect proof must not use lane_relay_block")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_malformed_existing_state() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-malformed-existing",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        state_transaction
            .world
            .smart_contract_state
            .insert(relay_state_key_for_test(&envelope), vec![0xFF]);
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("malformed existing state must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            ) if message.contains("stored")
        ));
        Ok(())
    }

    #[test]
    async fn register_verified_lane_relay_rejects_conflicting_existing_state() -> Result<()> {
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

        let envelope = sample_lane_relay_envelope(
            block_header,
            lane_id,
            dsid,
            [0x42; 32],
            iroha_crypto::Hash::new(b"placeholder-axt-proof-payload"),
        );
        let proof_blob = axt_lane_relay_proof_blob_for(
            &envelope,
            b"register-lane-relay-conflicting-existing",
            state_transaction.block_height() + 10,
        );
        let envelope = lane_relay_envelope_with_proof_payload(
            envelope,
            &proof_blob,
            state_transaction.block_height(),
        );
        let mut existing = verified_lane_relay_record_for_test(
            envelope.clone(),
            &proof_blob,
            state_transaction.block_height(),
        );
        existing.fastpq_statement_digest[0] ^= 0xFF;
        let existing_json = Json::try_new(existing)?;
        state_transaction.world.smart_contract_state.insert(
            relay_state_key_for_test(&envelope),
            norito::to_bytes(&existing_json)?,
        );
        let instruction = iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope,
            proof_blob,
            effect_proof_blob: None,
        };

        let err = instruction
            .execute(&ALICE_ID, &mut state_transaction)
            .expect_err("conflicting existing state must be rejected");
        assert!(matches!(
            err,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("conflicting verified lane relay")
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
            states: None,
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
            states: None,
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

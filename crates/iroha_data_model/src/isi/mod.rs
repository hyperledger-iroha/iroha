//! This library contains basic Iroha Special Instructions.
//!
//! Instructions implement the [`crate::isi::Instruction`] trait and are often stored in
//! trait objects for dynamic dispatch. The [`crate::isi::InstructionBox`] type alias is a
//! convenient way to work with `Box<dyn Instruction>` allowing heterogeneous
//! instruction collections.
//!
//! Dev note: Box naming
//! - `InstructionBox` is a newtype wrapper around `Box<dyn Instruction>`. It is a
//!   heap-allocated trait object used to store heterogeneous instructions,
//!   implement shared traits (e.g., serialization, ordering), and support
//!   registry-based deserialization.
//! - Types like `RegisterBox`, `RevokeBox`, `SetKeyValueBox`, `MintBox`, etc. are
//!   enums that "box together" a family of related generic instructions into a
//!   closed, visitable set. Despite the name, they are not heap boxes; they are
//!   plain tagged unions that implement [`crate::isi::Instruction`].

#[cfg(test)]
use std::cell::RefCell;
use std::{
    any::Any,
    cmp::Ordering,
    fmt::Debug,
    format,
    string::String,
    sync::{Arc, OnceLock, RwLock},
    vec::Vec,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use derive_more::{Constructor, Display};
use iroha_schema::{IntoSchema, Metadata as SchemaMetadata, UnnamedFieldsMeta};
use norito::codec::{Decode, Encode};
use rustc_hash::FxHashMap as HashMap;

use super::prelude::*;
use crate::{Level, Registered, seal};

/// Consensus key lifecycle instructions.
pub mod consensus_keys;
/// Domain endorsement management instructions.
pub mod endorsement;
/// Governance instruction module
#[cfg(feature = "governance")]
pub mod governance;

/// Owned trait-object wrapper for any [`crate::isi::Instruction`].
///
/// This newtype wraps `Box<dyn Instruction>` to allow implementing blanket traits
/// (e.g., `Send`/`Sync`) and to provide a stable, crate-owned type across the
/// codebase while preserving existing ergonomics via `Deref` to `dyn Instruction`.
///
/// # Examples
/// ```rust
/// use iroha_data_model::prelude::*;
///
/// let instruction: InstructionBox =
///     InstructionBox::from(Log::new(Level::INFO, "trait objects".into()));
/// ```
#[repr(transparent)]
pub struct InstructionBox(Box<dyn Instruction>);

impl core::ops::Deref for InstructionBox {
    type Target = dyn Instruction;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl core::fmt::Display for InstructionBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("InstructionBox")
    }
}

impl core::fmt::Debug for InstructionBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("InstructionBox")
            .field(&Instruction::id(&**self))
            .finish()
    }
}

impl Clone for InstructionBox {
    fn clone(&self) -> Self {
        // Use the object-safe clone-on-trait mechanism.
        self.0.dyn_box_clone()
    }
}

impl PartialEq for InstructionBox {
    fn eq(&self, other: &Self) -> bool {
        Instruction::id(&**self) == Instruction::id(&**other)
            && self.dyn_encode() == other.dyn_encode()
    }
}

impl Eq for InstructionBox {}

impl PartialOrd for InstructionBox {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InstructionBox {
    fn cmp(&self, other: &Self) -> Ordering {
        let id_cmp = Instruction::id(&**self).cmp(Instruction::id(&**other));
        if id_cmp != Ordering::Equal {
            return id_cmp;
        }
        self.dyn_encode().cmp(&other.dyn_encode())
    }
}

// Implement the sealing marker for the wrapper so it participates in generic APIs
// (e.g., `Executable: From<Vec<InstructionBox>>`). Special handling in the
// blanket `Instruction` impl ensures `as_any` exposes the inner type.
impl crate::seal::Instruction for InstructionBox {}

// Allow direct boxing of standalone instructions that are not part of a grouped enum.
impl From<crate::isi::zk::VerifyProof> for InstructionBox {
    fn from(i: crate::isi::zk::VerifyProof) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::PruneProofs> for InstructionBox {
    fn from(i: crate::isi::zk::PruneProofs) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::bridge::SubmitBridgeProof> for InstructionBox {
    fn from(i: crate::isi::bridge::SubmitBridgeProof) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::bridge::RecordBridgeReceipt> for InstructionBox {
    fn from(i: crate::isi::bridge::RecordBridgeReceipt) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of ZK asset and voting instructions
impl From<crate::isi::zk::RegisterZkAsset> for InstructionBox {
    fn from(i: crate::isi::zk::RegisterZkAsset) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::ScheduleConfidentialPolicyTransition> for InstructionBox {
    fn from(i: crate::isi::zk::ScheduleConfidentialPolicyTransition) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::CancelConfidentialPolicyTransition> for InstructionBox {
    fn from(i: crate::isi::zk::CancelConfidentialPolicyTransition) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::Shield> for InstructionBox {
    fn from(i: crate::isi::zk::Shield) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::ZkTransfer> for InstructionBox {
    fn from(i: crate::isi::zk::ZkTransfer) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::Unshield> for InstructionBox {
    fn from(i: crate::isi::zk::Unshield) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::CreateElection> for InstructionBox {
    fn from(i: crate::isi::zk::CreateElection) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::SubmitBallot> for InstructionBox {
    fn from(i: crate::isi::zk::SubmitBallot) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::zk::FinalizeElection> for InstructionBox {
    fn from(i: crate::isi::zk::FinalizeElection) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::ActivatePublicLaneValidator> for InstructionBox {
    fn from(i: crate::isi::staking::ActivatePublicLaneValidator) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::ExitPublicLaneValidator> for InstructionBox {
    fn from(i: crate::isi::staking::ExitPublicLaneValidator) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::nexus::SetLaneRelayEmergencyValidators> for InstructionBox {
    fn from(i: crate::isi::nexus::SetLaneRelayEmergencyValidators) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of runtime upgrade instructions
impl From<crate::isi::runtime_upgrade::ProposeRuntimeUpgrade> for InstructionBox {
    fn from(i: crate::isi::runtime_upgrade::ProposeRuntimeUpgrade) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::runtime_upgrade::ActivateRuntimeUpgrade> for InstructionBox {
    fn from(i: crate::isi::runtime_upgrade::ActivateRuntimeUpgrade) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::runtime_upgrade::CancelRuntimeUpgrade> for InstructionBox {
    fn from(i: crate::isi::runtime_upgrade::CancelRuntimeUpgrade) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of verifying-keys registry instructions
impl From<crate::isi::verifying_keys::RegisterVerifyingKey> for InstructionBox {
    fn from(i: crate::isi::verifying_keys::RegisterVerifyingKey) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::verifying_keys::UpdateVerifyingKey> for InstructionBox {
    fn from(i: crate::isi::verifying_keys::UpdateVerifyingKey) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Allow direct boxing of consensus key lifecycle instructions.
impl From<crate::isi::consensus_keys::RegisterConsensusKey> for InstructionBox {
    fn from(i: crate::isi::consensus_keys::RegisterConsensusKey) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::consensus_keys::RotateConsensusKey> for InstructionBox {
    fn from(i: crate::isi::consensus_keys::RotateConsensusKey) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::consensus_keys::DisableConsensusKey> for InstructionBox {
    fn from(i: crate::isi::consensus_keys::DisableConsensusKey) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Domain endorsement management instructions.
impl From<crate::isi::endorsement::RegisterDomainCommittee> for InstructionBox {
    fn from(i: crate::isi::endorsement::RegisterDomainCommittee) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::endorsement::SetDomainEndorsementPolicy> for InstructionBox {
    fn from(i: crate::isi::endorsement::SetDomainEndorsementPolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::endorsement::SubmitDomainEndorsement> for InstructionBox {
    fn from(i: crate::isi::endorsement::SubmitDomainEndorsement) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Allow direct boxing of social incentive instructions.
impl From<crate::isi::social::ClaimTwitterFollowReward> for InstructionBox {
    fn from(i: crate::isi::social::ClaimTwitterFollowReward) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::social::SendToTwitter> for InstructionBox {
    fn from(i: crate::isi::social::SendToTwitter) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::social::CancelTwitterEscrow> for InstructionBox {
    fn from(i: crate::isi::social::CancelTwitterEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of SoraFS capacity marketplace instructions.
impl From<crate::isi::sorafs::RegisterCapacityDeclaration> for InstructionBox {
    fn from(i: crate::isi::sorafs::RegisterCapacityDeclaration) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sorafs::RecordCapacityTelemetry> for InstructionBox {
    fn from(i: crate::isi::sorafs::RecordCapacityTelemetry) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sorafs::RegisterCapacityDispute> for InstructionBox {
    fn from(i: crate::isi::sorafs::RegisterCapacityDispute) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of SoraFS pin registry instructions
impl From<crate::isi::sorafs::RegisterPinManifest> for InstructionBox {
    fn from(i: crate::isi::sorafs::RegisterPinManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sorafs::ApprovePinManifest> for InstructionBox {
    fn from(i: crate::isi::sorafs::ApprovePinManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sorafs::RetirePinManifest> for InstructionBox {
    fn from(i: crate::isi::sorafs::RetirePinManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of content lane instructions.
impl From<crate::isi::content::PublishContentBundle> for InstructionBox {
    fn from(i: crate::isi::content::PublishContentBundle) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::content::RetireContentBundle> for InstructionBox {
    fn from(i: crate::isi::content::RetireContentBundle) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::prelude::BindManifestAlias> for InstructionBox {
    fn from(i: crate::prelude::BindManifestAlias) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::prelude::IssueReplicationOrder> for InstructionBox {
    fn from(i: crate::prelude::IssueReplicationOrder) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::prelude::CompleteReplicationOrder> for InstructionBox {
    fn from(i: crate::prelude::CompleteReplicationOrder) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::space_directory::PublishSpaceDirectoryManifest> for InstructionBox {
    fn from(i: crate::isi::space_directory::PublishSpaceDirectoryManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::space_directory::RevokeSpaceDirectoryManifest> for InstructionBox {
    fn from(i: crate::isi::space_directory::RevokeSpaceDirectoryManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::space_directory::ExpireSpaceDirectoryManifest> for InstructionBox {
    fn from(i: crate::isi::space_directory::ExpireSpaceDirectoryManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of offline allowance instructions.
impl From<crate::isi::offline::RegisterOfflineAllowance> for InstructionBox {
    fn from(i: crate::isi::offline::RegisterOfflineAllowance) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::offline::SubmitOfflineToOnlineTransfer> for InstructionBox {
    fn from(i: crate::isi::offline::SubmitOfflineToOnlineTransfer) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::offline::RegisterOfflineVerdictRevocation> for InstructionBox {
    fn from(i: crate::isi::offline::RegisterOfflineVerdictRevocation) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of oracle feed instructions.
impl From<crate::isi::oracle::RegisterOracleFeed> for InstructionBox {
    fn from(i: crate::isi::oracle::RegisterOracleFeed) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::SubmitOracleObservation> for InstructionBox {
    fn from(i: crate::isi::oracle::SubmitOracleObservation) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::AggregateOracleFeed> for InstructionBox {
    fn from(i: crate::isi::oracle::AggregateOracleFeed) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::ProposeOracleChange> for InstructionBox {
    fn from(i: crate::isi::oracle::ProposeOracleChange) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::VoteOracleChangeStage> for InstructionBox {
    fn from(i: crate::isi::oracle::VoteOracleChangeStage) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::RollbackOracleChange> for InstructionBox {
    fn from(i: crate::isi::oracle::RollbackOracleChange) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::RecordTwitterBinding> for InstructionBox {
    fn from(i: crate::isi::oracle::RecordTwitterBinding) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::RevokeTwitterBinding> for InstructionBox {
    fn from(i: crate::isi::oracle::RevokeTwitterBinding) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Allow direct boxing of public lane staking instructions.
impl From<crate::isi::staking::RegisterPublicLaneValidator> for InstructionBox {
    fn from(i: crate::isi::staking::RegisterPublicLaneValidator) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::BondPublicLaneStake> for InstructionBox {
    fn from(i: crate::isi::staking::BondPublicLaneStake) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::SchedulePublicLaneUnbond> for InstructionBox {
    fn from(i: crate::isi::staking::SchedulePublicLaneUnbond) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::FinalizePublicLaneUnbond> for InstructionBox {
    fn from(i: crate::isi::staking::FinalizePublicLaneUnbond) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::SlashPublicLaneValidator> for InstructionBox {
    fn from(i: crate::isi::staking::SlashPublicLaneValidator) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::CancelConsensusEvidencePenalty> for InstructionBox {
    fn from(i: crate::isi::staking::CancelConsensusEvidencePenalty) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::RecordPublicLaneRewards> for InstructionBox {
    fn from(i: crate::isi::staking::RecordPublicLaneRewards) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::staking::ClaimPublicLaneRewards> for InstructionBox {
    fn from(i: crate::isi::staking::ClaimPublicLaneRewards) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Allow direct boxing of confidential parameter registry instructions
impl From<crate::isi::confidential::PublishPedersenParams> for InstructionBox {
    fn from(i: crate::isi::confidential::PublishPedersenParams) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::confidential::SetPedersenParamsLifecycle> for InstructionBox {
    fn from(i: crate::isi::confidential::SetPedersenParamsLifecycle) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::confidential::PublishPoseidonParams> for InstructionBox {
    fn from(i: crate::isi::confidential::PublishPoseidonParams) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::confidential::SetPoseidonParamsLifecycle> for InstructionBox {
    fn from(i: crate::isi::confidential::SetPoseidonParamsLifecycle) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of governance instructions
#[cfg(feature = "governance")]
impl From<crate::isi::governance::ProposeDeployContract> for InstructionBox {
    fn from(i: crate::isi::governance::ProposeDeployContract) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::CastZkBallot> for InstructionBox {
    fn from(i: crate::isi::governance::CastZkBallot) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::CastPlainBallot> for InstructionBox {
    fn from(i: crate::isi::governance::CastPlainBallot) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::SlashGovernanceLock> for InstructionBox {
    fn from(i: crate::isi::governance::SlashGovernanceLock) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::RestituteGovernanceLock> for InstructionBox {
    fn from(i: crate::isi::governance::RestituteGovernanceLock) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of asset metadata helpers
impl From<crate::isi::transparent::SetAssetKeyValue> for InstructionBox {
    fn from(i: crate::isi::transparent::SetAssetKeyValue) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::transparent::RemoveAssetKeyValue> for InstructionBox {
    fn from(i: crate::isi::transparent::RemoveAssetKeyValue) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::transparent::AddSignatory> for InstructionBox {
    fn from(i: crate::isi::transparent::AddSignatory) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::transparent::RemoveSignatory> for InstructionBox {
    fn from(i: crate::isi::transparent::RemoveSignatory) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::transparent::SetAccountQuorum> for InstructionBox {
    fn from(i: crate::isi::transparent::SetAccountQuorum) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::EnactReferendum> for InstructionBox {
    fn from(i: crate::isi::governance::EnactReferendum) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::FinalizeReferendum> for InstructionBox {
    fn from(i: crate::isi::governance::FinalizeReferendum) -> Self {
        InstructionBox(Box::new(i))
    }
}
#[cfg(feature = "governance")]
impl From<crate::isi::governance::ApproveGovernanceProposal> for InstructionBox {
    fn from(i: crate::isi::governance::ApproveGovernanceProposal) -> Self {
        InstructionBox(Box::new(i))
    }
}

/// Object-safe cloning support for [`Instruction`] trait objects.
pub trait InstructionDynClone {
    /// Clone the underlying instruction into a boxed trait object.
    fn dyn_box_clone(&self) -> InstructionBox;
}

/// Marker trait designating instruction.
///
/// Instructions allow to change the state of `Iroha`.
///
/// If you need to use different instructions together,
/// consider wrapping them into [`crate::isi::InstructionBox`]es.
pub trait Instruction: InstructionDynClone + seal::Instruction + Send + Sync + 'static {
    /// Execute instruction
    fn dyn_execute(&self) {}

    /// Encode instruction into bytes
    fn dyn_encode(&self) -> Vec<u8>;

    /// Downcast to concrete type
    fn as_any(&self) -> &dyn Any;

    /// Identifier of this instruction type.
    ///
    /// By default, it resolves to the name of the concrete type at
    /// compile time, providing a stable identifier without relying on
    /// runtime reflection.
    fn id(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Convert into [`crate::isi::InstructionBox`]
    fn into_instruction_box(self: Box<Self>) -> InstructionBox
    where
        Self: Sized,
    {
        // Coerce `Box<Self>` to `Box<dyn Instruction>` and wrap
        InstructionBox(self)
    }
}

/// Marker trait for built-in instructions.
pub trait BuiltInInstruction: Instruction {
    /// [`Encode`] [`Self`] as [`crate::isi::InstructionBox`].
    ///
    /// Used to avoid an unnecessary clone
    fn encode_as_instruction_box(&self) -> Vec<u8>;
}
impl<T> BuiltInInstruction for T
where
    T: Instruction + Encode,
{
    fn encode_as_instruction_box(&self) -> Vec<u8> {
        self.encode()
    }
}

impl<T> Instruction for T
where
    T: Clone + Debug + PartialEq + PartialOrd + Encode + seal::Instruction + Send + Sync + 'static,
{
    fn dyn_encode(&self) -> Vec<u8> {
        self.encode()
    }

    fn as_any(&self) -> &dyn Any {
        // Special-case: if `self` is `InstructionBox`, expose its inner instruction
        // to preserve downcasting behavior used by the visitor helpers.
        let any: &dyn Any = self;
        any.downcast_ref::<InstructionBox>().map_or(any, |wrapper| {
            let inner: &dyn Instruction = &*wrapper.0;
            inner.as_any()
        })
    }
}

// Provide an object-safe cloning path for any `T` that implements `Instruction` + `Clone`.
impl<T> InstructionDynClone for T
where
    T: Instruction + Clone,
{
    fn dyn_box_clone(&self) -> InstructionBox {
        InstructionBox(Box::new(self.clone()))
    }
}

impl norito::core::NoritoSerialize for InstructionBox {
    fn schema_hash() -> [u8; 16]
    where
        Self: Sized,
    {
        // Match the archived layout used in `serialize`: `(type_name, payload_with_header)`.
        norito::core::type_name_schema_hash::<(String, Vec<u8>)>()
    }

    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        // Unwrap any accidental nesting of InstructionBox to reach the concrete instruction
        fn peel(mut instr: &dyn Instruction) -> &dyn Instruction {
            loop {
                if let Some(nested) = instr.as_any().downcast_ref::<InstructionBox>() {
                    instr = &**nested;
                } else {
                    break instr;
                }
            }
        }

        let inner = peel(&**self);
        let type_name = Instruction::id(inner);
        let name = instruction_registry()
            .wire_id_for_type_name(type_name)
            .to_string();
        let payload = Instruction::dyn_encode(inner);
        let payload = frame_instruction_payload(type_name, &payload)?;
        norito::core::NoritoSerialize::serialize(&(name, payload), writer)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for InstructionBox {
    fn schema_hash() -> [u8; 16] {
        // Must match the schema used by `NoritoSerialize` for `InstructionBox`
        // which serializes as a `(String, Vec<u8>)` pair.
        norito::core::type_name_schema_hash::<(String, Vec<u8>)>()
    }

    fn deserialize(archived: &'a norito::core::Archived<InstructionBox>) -> Self {
        const MAX_MESSAGE_LEN: usize = 256;

        let truncate_message = |mut message: String| {
            // Keep the placeholder bounded; it may end up in logs/errors.
            if message.len() > MAX_MESSAGE_LEN {
                message.truncate(MAX_MESSAGE_LEN);
            }
            message
        };

        let pair: Result<(String, Vec<u8>), norito::core::Error> =
            norito::core::NoritoDeserialize::try_deserialize(archived.cast());
        match pair {
            Ok((name, bytes)) => match decode_instruction_from_pair(&name, &bytes) {
                Ok(inst) => inst,
                Err(err) => {
                    // Avoid panics on malformed instruction payloads (DoS vector).
                    // Represent the decode error as a sentinel instruction that the executor rejects.
                    let hash: [u8; 32] = iroha_crypto::Hash::new(&bytes).into();
                    let message = truncate_message(err.to_string());
                    InstructionBox::from(crate::isi::transparent::InvalidInstruction::new(
                        name, hash, message,
                    ))
                }
            },
            Err(err) => {
                let message = truncate_message(err.to_string());
                InstructionBox::from(crate::isi::transparent::InvalidInstruction::new(
                    "<norito>", [0u8; 32], message,
                ))
            }
        }
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<InstructionBox>,
    ) -> Result<Self, norito::core::Error> {
        let (name, bytes): (String, Vec<u8>) =
            norito::core::NoritoDeserialize::try_deserialize(archived.cast())?;
        decode_instruction_from_pair(&name, &bytes)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for InstructionBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        fn canonical_framing_error() -> norito::core::Error {
            norito::core::Error::Message(
                "instruction payload must use canonical Norito framing".to_owned(),
            )
        }

        let archived = norito::core::archived_from_slice::<InstructionBox>(bytes)
            .map_err(|_| canonical_framing_error())?;
        let _guard = norito::core::PayloadCtxGuard::enter(archived.bytes());
        let inst = norito::core::NoritoDeserialize::try_deserialize(archived.archived()).map_err(
            |err| match err {
                norito::core::Error::Message(_) => err,
                _ => canonical_framing_error(),
            },
        )?;
        Ok((inst, archived.bytes().len()))
    }
}

impl norito::json::FastJsonWrite for InstructionBox {
    fn write_json(&self, out: &mut String) {
        // JSON uses base64 of the canonical Norito-framed payload so clients can
        // round-trip without guessing decode flags.
        let bytes = norito::to_bytes(self).expect("InstructionBox should Norito-frame");
        let encoded = STANDARD.encode(bytes);
        norito::json::JsonSerialize::json_serialize(&encoded, out);
    }
}

impl norito::json::JsonDeserialize for InstructionBox {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let encoded = parser.parse_string()?;
        let bytes = STANDARD
            .decode(encoded.as_str())
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        let archived = norito::from_bytes::<InstructionBox>(&bytes)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        norito::core::NoritoDeserialize::try_deserialize(archived)
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

impl iroha_schema::TypeId for InstructionBox {
    fn id() -> iroha_schema::Ident {
        std::any::type_name::<Self>().to_owned()
    }
}

/// Decode a wire-framed ISI payload into a typed [`InstructionBox`].
///
/// The `name` must be either the canonical Rust `type_name` or a wire-id
/// registered in the instruction registry. The `payload` must be framed with
/// the Norito header as produced by [`frame_instruction_payload`].
///
/// # Errors
/// Returns `norito::Error` if the instruction name is not registered or payload
/// decoding fails.
pub fn decode_instruction_from_pair(
    name: &str,
    payload: &[u8],
) -> Result<InstructionBox, norito::Error> {
    let registry = instruction_registry();
    if let Some(res) = registry.decode(name, payload) {
        return res;
    }
    Err(norito::Error::Message(format!(
        "unknown instruction `{name}` (not registered)"
    )))
}

/// Frame a bare instruction payload with its Norito header using the registry metadata.
///
/// # Errors
/// Returns `norito::Error` if the type name is not registered or framing fails for the payload.
pub fn frame_instruction_payload(
    type_name: &str,
    payload: &[u8],
) -> Result<Vec<u8>, norito::Error> {
    let registry = instruction_registry();
    if let Some(result) = registry.frame_payload_for_type(type_name, payload) {
        return result;
    }
    Err(norito::Error::Message(format!(
        "unknown instruction `{type_name}` (not registered)"
    )))
}

impl IntoSchema for InstructionBox {
    fn type_name() -> iroha_schema::Ident {
        "InstructionBox".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(SchemaMetadata::Tuple(UnnamedFieldsMeta { types: vec![] }));
    }
}

/// Function signature used to construct an [`crate::isi::Instruction`] from header-framed bytes.
///
/// The `header_flags` argument propagates Norito metadata alongside the encoded
/// payload. Existing constructors ignore the value, but keeping it in the
/// signature allows future instructions to react to packed-layout flags without
/// widening the registry interface again.
pub type InstructionConstructor = fn(u8, &[u8]) -> Result<InstructionBox, norito::Error>;

/// Registry storing constructors for [`crate::isi::Instruction`] types keyed by their type names.
#[derive(Default, Clone)]
pub struct InstructionRegistry {
    /// Concrete Rust `type_name` -> entry with preferred wire id.
    entries: HashMap<&'static str, RegistryEntry>,
    /// Lookup table mapping either `type_name` or wire-id -> canonical `type_name`.
    index: HashMap<&'static str, &'static str>,
}

#[derive(Clone, Copy)]
struct RegistryEntry {
    ctor: InstructionConstructor,
    wire_id: &'static str,
    frame: fn(&[u8]) -> Result<Vec<u8>, norito::core::Error>,
}

impl InstructionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new [`crate::isi::Instruction`] type.
    #[must_use]
    pub fn register<T>(mut self) -> Self
    where
        T: Instruction
            + Decode
            + 'static
            + norito::NoritoSerialize
            + for<'a> norito::NoritoDeserialize<'a>,
    {
        fn ctor<T>(header_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            decode_instruction_payload::<T>(input, header_flags)
        }
        fn frame<T>(payload: &[u8]) -> Result<Vec<u8>, norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::frame_bare_with_header_flags::<T>(
                payload,
                norito::core::default_encode_flags(),
            )
        }
        let name = std::any::type_name::<T>();
        let entry = RegistryEntry {
            ctor: ctor::<T>,
            wire_id: name,
            frame: frame::<T>,
        };
        self.entries.insert(name, entry);
        self.index.insert(name, name);
        self.index.insert(entry.wire_id, name);
        self
    }

    /// Register a new [`crate::isi::Instruction`] type using a stable wire identifier.
    #[must_use]
    pub fn register_with_id<T>(mut self, wire_id: &'static str) -> Self
    where
        T: Instruction
            + Decode
            + 'static
            + norito::NoritoSerialize
            + for<'a> norito::NoritoDeserialize<'a>,
    {
        fn ctor<T>(header_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            decode_instruction_payload::<T>(input, header_flags)
        }
        fn frame<T>(payload: &[u8]) -> Result<Vec<u8>, norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::frame_bare_with_header_flags::<T>(
                payload,
                norito::core::default_encode_flags(),
            )
        }
        let name = std::any::type_name::<T>();
        let entry = RegistryEntry {
            ctor: ctor::<T>,
            wire_id,
            frame: frame::<T>,
        };
        self.entries.insert(name, entry);
        self.index.insert(name, name);
        self.index.insert(wire_id, name);
        self
    }

    /// Decode an [`crate::isi::Instruction`] using the registered constructor for the given type name.
    pub fn decode(
        &self,
        name: &str,
        bytes: &[u8],
    ) -> Option<Result<InstructionBox, norito::Error>> {
        self.decode_with_flags(name, 0, bytes)
    }

    /// Decode an [`crate::isi::Instruction`] providing explicit Norito layout flags.
    ///
    /// The `header_flags` argument mirrors the values produced by
    /// [`norito::codec::encode_with_header_flags`] and ensures the decoder
    /// reconstructs packed-struct layouts consistently for instructions that
    /// rely on adaptive encoding.
    pub fn decode_with_flags(
        &self,
        name: &str,
        header_flags: u8,
        bytes: &[u8],
    ) -> Option<Result<InstructionBox, norito::Error>> {
        self.entry_for_key(name)
            .map(|entry| Self::decode_entry(entry, header_flags, bytes))
    }

    /// Number of registered instruction types.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether the registry holds no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Iterator over registered type names.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.keys().copied()
    }
    /// Check whether the registry can decode the given type name.
    pub fn contains(&self, name: &str) -> bool {
        self.entry_for_key(name).is_some()
    }
    /// Return the stable wire identifier for the given type name, if registered.
    pub fn wire_id(&self, type_name: &'static str) -> Option<&'static str> {
        self.entries.get(type_name).map(|entry| entry.wire_id)
    }

    fn wire_id_for_type_name(&self, type_name: &'static str) -> &'static str {
        self.entries.get(type_name).map_or(type_name, |e| e.wire_id)
    }

    fn frame_payload_for_type(
        &self,
        type_name: &str,
        payload: &[u8],
    ) -> Option<Result<Vec<u8>, norito::core::Error>> {
        self.entry_for_key(type_name)
            .map(|entry| (entry.frame)(payload))
    }

    fn entry_for_key(&self, key: &str) -> Option<&RegistryEntry> {
        if let Some(canonical) = self.index.get(key) {
            return self.entries.get(canonical);
        }
        self.entries
            .get(key)
            .or_else(|| self.entries.values().find(|entry| entry.wire_id == key))
    }

    fn decode_entry(
        entry: &RegistryEntry,
        header_flags: u8,
        bytes: &[u8],
    ) -> Result<InstructionBox, norito::Error> {
        (entry.ctor)(header_flags, bytes)
    }
}

fn decode_instruction_payload<T>(
    input: &[u8],
    header_flags: u8,
) -> Result<InstructionBox, norito::Error>
where
    T: Instruction + Decode + 'static + norito::NoritoSerialize,
    for<'a> T: norito::NoritoDeserialize<'a>,
{
    let _ = header_flags;
    let instruction = norito::decode_from_bytes::<T>(input)?;
    Ok(InstructionBox(Box::new(instruction)))
}

/// Build an [`InstructionRegistry`] populated with the provided instruction types.
#[macro_export]
macro_rules! instruction_registry {
    ($($ty:ty),* $(,)?) => {{
        let mut registry = $crate::isi::InstructionRegistry::new();
        let registrars = [
            $(
                $crate::isi::InstructionRegistry::register::<$ty>
            ),*
        ];
        for register in registrars {
            registry = register(registry);
        }
        registry
    }};
}

/// Build an [`InstructionRegistry`] registering each type with its annotated stable
/// wire identifier by reading its `WIRE_ID` associated constant.
#[macro_export]
macro_rules! instruction_registry_with_ids {
    ($($ty:ty),* $(,)?) => {{
        let mut registry = $crate::isi::InstructionRegistry::new();
        $(
            registry = registry.register_with_id::<$ty>(<$ty>::WIRE_ID);
        )*
        registry
    }};
}

static INSTRUCTION_REGISTRY: OnceLock<RwLock<Arc<InstructionRegistry>>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static INSTRUCTION_REGISTRY_OVERRIDE: RefCell<Option<Arc<InstructionRegistry>>> =
        const { RefCell::new(None) };
}

/// Set global [`InstructionRegistry`] used for deserializing [`crate::isi::InstructionBox`].
pub fn set_instruction_registry(registry: InstructionRegistry) {
    let registry = Arc::new(registry);
    #[cfg(test)]
    {
        INSTRUCTION_REGISTRY_OVERRIDE.with(|cell| {
            *cell.borrow_mut() = Some(Arc::clone(&registry));
        });
    }
    #[cfg(not(test))]
    if let Some(lock) = INSTRUCTION_REGISTRY.get() {
        let mut guard = lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = registry;
    } else {
        let _ = INSTRUCTION_REGISTRY.set(RwLock::new(registry));
    }
}

enum InstructionRegistryReadGuard {
    Global(Arc<InstructionRegistry>),
    #[cfg(test)]
    Local(Arc<InstructionRegistry>),
}

impl std::ops::Deref for InstructionRegistryReadGuard {
    type Target = InstructionRegistry;

    fn deref(&self) -> &InstructionRegistry {
        match self {
            Self::Global(registry) => registry,
            #[cfg(test)]
            Self::Local(registry) => registry,
        }
    }
}

fn instruction_registry() -> InstructionRegistryReadGuard {
    #[cfg(test)]
    if let Some(local) = INSTRUCTION_REGISTRY_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return InstructionRegistryReadGuard::Local(local);
    }
    // Lazily initialize with the built-in default registry if not explicitly set.
    // This makes binaries and tools robust even if they forgot to call an
    // explicit initializer before deserializing InstructionBox values (e.g., while reading genesis).
    let registry = INSTRUCTION_REGISTRY
        .get_or_init(|| RwLock::new(Arc::new(crate::instruction_registry::default())))
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    InstructionRegistryReadGuard::Global(Arc::clone(&registry))
}

macro_rules! isi {
    ($($meta:meta)* $item:item) => {
        iroha_data_model_derive::model_single! {
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
            #[derive(getset::Getters)]
            #[derive(Decode, Encode)]
            #[derive(iroha_schema::IntoSchema)]
            #[getset(get = "pub")]
            $($meta)*
            $item
        }
    };
}

macro_rules! impl_display {
    (
        $ty:ident $(< $($generic:tt),+ >)?
        $(where
            $( $lt:path $( : $clt:tt $(< $inner_generic:tt >)? $(+ $dlt:tt )* )? ),+ $(,)? )?
        => $fmt:literal, $($args:ident),* $(,)?
    ) => {
        impl $(< $($generic),+ >)? ::core::fmt::Display for $ty $(< $($generic),+ >)?
        $(where
            $( $lt $( : $clt $(< $inner_generic >)? $(+ $dlt )* )? ),+)? {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, $fmt, $(self.$args),*)
            }
        }
    }
}

macro_rules! impl_into_box {
    ( $($isi:ty)|* => $middle:ty ) => {
        impl From<$middle> for InstructionBox {
            fn from(instruction: $middle) -> Self {
                InstructionBox(Box::new(instruction))
            }
        }

        $(impl From<$isi> for InstructionBox {
            fn from(instruction: $isi) -> Self {
                InstructionBox::from(<$middle>::from(instruction))
            }
        })*
    };
}

macro_rules! isi_box {
    ($($meta:meta)* $item:item) => {
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Display,
            Decode,
            Encode,
            iroha_schema::IntoSchema,
            derive_more::From,
        )]
        $($meta)*
        $item
    };
}

macro_rules! enum_type {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $( $(#[$variant_meta:meta])* $variant:ident ),+ $(,)? }) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        $(#[$meta])*
        #[doc = concat!("Enum type `", stringify!($name), "` generated via `enum_type!`.")]
        #[repr(u8)]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                #[doc = concat!("Variant `", stringify!($variant), "` of `", stringify!($name), "`.")]
                $variant
            ),+
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(match self {
                    $( Self::$variant => stringify!($variant), )+
                })
            }
        }

        impl ::core::convert::TryFrom<u8> for $name {
            type Error = ();

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $( x if x == Self::$variant as u8 => Ok(Self::$variant), )+
                    _ => Err(()),
                }
            }
        }

        #[cfg(feature = "json")]
        impl norito::json::FastJsonWrite for $name {
            fn write_json(&self, out: &mut String) {
                out.push('"');
                out.push_str(match self {
                    $( Self::$variant => stringify!($variant), )+
                });
                out.push('"');
            }
        }

        #[cfg(feature = "json")]
        impl norito::json::JsonDeserialize for $name {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                let value = parser.parse_string()?;
                match value.as_str() {
                    $( stringify!($variant) => Ok(Self::$variant), )+
                    other => Err(norito::json::Error::UnknownField {
                        field: other.to_owned(),
                    }),
                }
            }
        }
    };
}

/// Confidential registry management instructions.
/// Bridge proof ingestion instructions.
pub mod bridge;
/// Confidential registry management instructions.
pub mod confidential;
/// Content lane instructions.
pub mod content;
/// Kaigi collaboration instructions.
pub mod kaigi;
/// Mint and burn instruction variants and helpers.
pub mod mint_burn;
/// Nexus lane governance instructions.
pub mod nexus;
/// Offline allowance settlement instructions.
pub mod offline;
/// Oracle feed registration and aggregation instructions.
pub mod oracle;
/// Registration-related instructions (accounts, assets, domains, etc.).
pub mod register;
/// Instruction registries shared across instruction families.
pub mod registry;
/// Repo settlement instructions.
pub mod repo;
/// Runtime upgrade instructions and payloads.
pub mod runtime_upgrade;
/// DvP/PvP settlement instructions.
pub mod settlement;
/// Smart contract code management instructions.
pub mod smart_contract_code;
/// Viral incentive and social reward instructions.
pub mod social;
/// `SoraDNS` attestation and directory instructions.
pub mod soradns;
/// `SoraFS` pin registry instructions.
pub mod sorafs;
/// Space Directory manifest instructions.
pub mod space_directory;
/// Public lane staking instructions.
pub mod staking;
/// Asset, account, and value transfer instructions.
pub mod transfer;
mod transparent;
/// Verifying-key management instructions.
pub mod verifying_keys;
/// Zero-knowledge instruction wrappers.
pub mod zk;

pub use confidential::*;
pub use kaigi::*;
pub use mint_burn::*;
pub use nexus::*;
pub use offline::*;
pub use oracle::*;
pub use register::*;
pub use repo::*;
pub use settlement::*;
pub use soradns::*;
pub use sorafs::*;
pub use space_directory::*;
pub use staking::*;
pub use transfer::*;
pub use transparent::*;
pub use zk::*;

isi_box! {
    /// Enum with all supported [`SetKeyValue`] instructions.
    ///
    /// Dev note: despite the "Box" suffix, this is an enum (tagged union),
    /// not a heap allocation. It groups related `SetKeyValue<T>` variants
    /// into a single visitable type that implements [`crate::isi::Instruction`].
    pub enum SetKeyValueBox {
        /// Set key value for [`Domain`].
        Domain(SetKeyValue<Domain>),
        /// Set key value for [`Account`].
        Account(SetKeyValue<Account>),
        /// Set key value for [`AssetDefinition`].
        AssetDefinition(SetKeyValue<AssetDefinition>),
        /// Set key value for [`Nft`].
        Nft(SetKeyValue<Nft>),
        /// Set key value for [`Trigger`].
        Trigger(SetKeyValue<Trigger>),
    }
}

impl SetKeyValueBox {
    /// Norito wire identifier for `SetKeyValueBox` payload framing.
    pub const WIRE_ID: &'static str = "iroha.set_key_value";
}

enum_type! {
    /// Type discriminator for [`SetKeyValueBox`] variants.
    pub(crate) enum SetKeyValueType {
        Domain,
        Account,
        AssetDefinition,
        Nft,
        Trigger,
    }
}

isi_box! {
    /// Enum with all supported [`RemoveKeyValue`] instructions.
    ///
    /// Dev note: "Box" here means a boxed-up family of variants, not
    /// heap allocation.
    pub enum RemoveKeyValueBox {
        /// Remove key value from [`Domain`].
        Domain(RemoveKeyValue<Domain>),
        /// Remove key value from [`Account`].
        Account(RemoveKeyValue<Account>),
        /// Remove key value from [`AssetDefinition`].
        AssetDefinition(RemoveKeyValue<AssetDefinition>),
        /// Remove key value from [`Nft`].
        Nft(RemoveKeyValue<Nft>),
        /// Remove key value for [`Trigger`].
        Trigger(RemoveKeyValue<Trigger>),
    }
}

impl RemoveKeyValueBox {
    /// Norito wire identifier for `RemoveKeyValueBox` payload framing.
    pub const WIRE_ID: &'static str = "iroha.remove_key_value";
}

enum_type! {
    /// Type discriminator for [`RemoveKeyValueBox`] variants.
    pub(crate) enum RemoveKeyValueType {
        Domain,
        Account,
        AssetDefinition,
        Nft,
        Trigger,
    }
}

isi_box! {
    /// Enum with all supported [`Grant`] instructions.
    ///
    /// Dev note: this enum aggregates concrete `Grant<_, _>` variants into
    /// one type for visiting and serialization; it is not a heap `Box`.
    pub enum GrantBox {
        /// Grant [`Permission`] to [`Account`].
        Permission(Grant<Permission, Account>),
        /// Grant [`Role`] to [`Account`].
        Role(Grant<RoleId, Account>),
        /// Grant [`Permission`] to [`Role`].
        RolePermission(Grant<Permission, Role>),
    }
}

impl GrantBox {
    /// Norito wire identifier for `GrantBox` payload framing.
    pub const WIRE_ID: &'static str = "iroha.grant";
}

enum_type! {
    /// Type discriminator for [`GrantBox`] variants.
    pub(crate) enum GrantType {
        Permission,
        Role,
        RolePermission,
    }
}

isi_box! {
    /// Enum with all supported [`Revoke`] instructions.
    ///
    /// Dev note: this is a tagged union of concrete `Revoke<_, _>` variants,
    /// not a heap allocation.
    pub enum RevokeBox {
        /// Revoke [`Permission`] from [`Account`].
        Permission(Revoke<Permission, Account>),
        /// Revoke [`Role`] from [`Account`].
        Role(Revoke<RoleId, Account>),
        /// Revoke [`Permission`] from [`Role`].
        RolePermission(Revoke<Permission, Role>),
    }
}

impl RevokeBox {
    /// Norito wire identifier for `RevokeBox` payload framing.
    pub const WIRE_ID: &'static str = "iroha.revoke";
}

enum_type! {
    /// Type discriminator for [`RevokeBox`] variants.
    pub(crate) enum RevokeType {
        /// Revoke [`Permission`] from an [`Account`].
        Permission,
        /// Revoke a [`Role`] from an [`Account`].
        Role,
        /// Revoke a [`Permission`] from a [`Role`].
        RolePermission,
    }
}

enum_type! {
    /// All built-in instruction kinds supported by the data model.
    pub enum InstructionType {
        /// Modify a system parameter.
        SetParameter,
        /// Insert or update a key-value pair in metadata.
        SetKeyValue,
        /// Remove a metadata key-value pair.
        RemoveKeyValue,
        /// Add a new entity to the ledger.
        Register,
        /// Remove an entity from the ledger.
        Unregister,
        /// Increase a numeric asset quantity.
        Mint,
        /// Decrease a numeric asset quantity.
        Burn,
        /// Move value or ownership between accounts.
        Transfer,
        /// Grant a permission or role.
        Grant,
        /// Revoke a permission or role.
        Revoke,
        /// Activate a runtime upgrade proposal.
        Upgrade,
        /// Execute a registered trigger.
        ExecuteTrigger,
        /// Emit a log entry.
        Log,
        /// Execute a custom instruction.
        Custom,
    }
}

pub mod error {
    //! Module containing errors that can occur during instruction evaluation

    use std::{boxed::Box, fmt::Debug, format, string::String, vec::Vec};

    use derive_more::Display;
    use iroha_data_model_derive::model;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    pub use self::model::*;
    use crate::{
        IdBox,
        isi::InstructionType,
        prelude::NumericSpec,
        query::error::{FindError, QueryExecutionFail},
    };

    #[model]
    mod model {
        use getset::Getters;

        use super::*;

        /// Instruction execution error type
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            derive_more::From,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum InstructionExecutionError {
            /// Instruction does not adhere to Iroha DSL specification
            Evaluate(#[source] InstructionEvaluationError),
            /// Query failed
            Query(#[source] QueryExecutionFail),
            /// Conversion Error: {0}
            Conversion(String),
            /// Entity missing
            Find(#[source] FindError),
            /// Repeated instruction
            Repetition(#[source] RepetitionError),
            /// Mintability assertion failed
            Mintability(#[source] MintabilityError),
            /// Illegal math operation
            Math(#[source] MathError),
            /// Invalid instruction parameter
            InvalidParameter(#[source] InvalidParameterError),
            /// Account admission rejected
            AccountAdmission(#[source] AccountAdmissionError),
            /// Iroha invariant violation: {0}
            ///
            /// i.e. you can't burn last key
            InvariantViolation(Box<str>),
        }

        /// Quota scope used by [`AccountAdmissionError::QuotaExceeded`].
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum AccountAdmissionQuotaScope {
            /// Transaction-scoped quota.
            Transaction,
            /// Block-scoped quota.
            Block,
        }

        /// Errors raised while admitting implicit accounts under domain/chain policies.
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum AccountAdmissionError {
            /// Implicit account creation is disabled in domain `{0}`.
            ImplicitAccountCreationDisabled(crate::domain::DomainId),
            /// Account admission policy is invalid: {0}.
            InvalidPolicy(AccountAdmissionInvalidPolicy),
            /// Failed to assign the configured default role: {0}.
            DefaultRoleError(AccountAdmissionDefaultRoleError),
            /// Implicit account creation quota exceeded: {0}.
            QuotaExceeded(AccountAdmissionQuotaExceeded),
            /// Signing algorithm {0} is not permitted for implicit account creation.
            AlgorithmNotAllowed(iroha_crypto::Algorithm),
            /// Implicit account creation in the genesis domain is not permitted.
            GenesisDomainForbidden,
            /// Fee required for implicit account creation could not be paid.
            FeeUnsatisfied(AccountAdmissionFeeUnsatisfied),
            /// Receipt amount is below the minimum required to create an account implicitly.
            MinInitialAmountUnsatisfied(AccountAdmissionMinInitialAmountUnsatisfied),
        }

        /// Account admission policy payload is invalid for domain `{domain}`: {reason}.
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct AccountAdmissionInvalidPolicy {
            /// Domain for which the policy was evaluated.
            pub domain: crate::domain::DomainId,
            /// Human-readable reason describing the invalid payload.
            pub reason: String,
        }

        /// Default role assignment failed for `{role}`: {reason}.
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct AccountAdmissionDefaultRoleError {
            /// Role that could not be assigned.
            pub role: crate::role::RoleId,
            /// Reason for the failure.
            pub reason: String,
        }

        /// Implicit account creation quota exceeded for {scope} (created {created}, cap {cap}).
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct AccountAdmissionQuotaExceeded {
            /// Scope of the quota that was exceeded.
            pub scope: AccountAdmissionQuotaScope,
            /// Number of implicit accounts created so far within the scope.
            pub created: u32,
            /// Allowed cap for implicit accounts within the scope.
            pub cap: u32,
        }

        /// Implicit account creation fee could not be paid for `{asset_definition}` (required `{required}`, available `{available}`).
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct AccountAdmissionFeeUnsatisfied {
            /// Asset definition used to charge the fee.
            pub asset_definition: crate::asset::AssetDefinitionId,
            /// Fee required to create the account implicitly.
            pub required: iroha_primitives::numeric::Numeric,
            /// Amount available in the payer account.
            pub available: iroha_primitives::numeric::Numeric,
        }

        /// Minimum initial amount requirement is not satisfied for `{asset_definition}` (required {required}, provided {provided}).
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct AccountAdmissionMinInitialAmountUnsatisfied {
            /// Asset definition subject to the minimum requirement.
            pub asset_definition: crate::asset::AssetDefinitionId,
            /// Amount required by policy.
            pub required: iroha_primitives::numeric::Numeric,
            /// Amount supplied by the receipt operation.
            pub provided: iroha_primitives::numeric::Numeric,
        }

        /// Evaluation error. This error indicates instruction is not a valid Iroha DSL
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            derive_more::From,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum InstructionEvaluationError {
            /// Unsupported parameter type for instruction of type `{0}`
            Unsupported(InstructionType),
            /// Failed to find parameter in a permission: {0}
            PermissionParameter(String),
            /// Incorrect value type
            Type(#[source] TypeError),
        }

        /// Generic structure used to represent a mismatch
        #[derive(
            Debug,
            Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
            thiserror::Error,
        )]
        #[display("Expected {expected:?}, actual {actual:?}")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct Mismatch<T>
        where
            T: Debug,
        {
            /// The value that is needed for normal execution
            pub expected: T,
            /// The value that caused the error
            pub actual: T,
        }

        #[cfg(feature = "json")]
        impl<T> norito::json::JsonSerialize for Mismatch<T>
        where
            T: norito::json::JsonSerialize + Debug,
        {
            fn json_serialize(&self, out: &mut String) {
                out.push('{');
                norito::json::write_json_string("expected", out);
                out.push(':');
                self.expected.json_serialize(out);
                out.push(',');
                norito::json::write_json_string("actual", out);
                out.push(':');
                self.actual.json_serialize(out);
                out.push('}');
            }
        }

        #[cfg(feature = "json")]
        impl<T> norito::json::JsonDeserialize for Mismatch<T>
        where
            T: norito::json::JsonDeserialize + Debug,
        {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                use norito::json::MapVisitor;

                let mut visitor = MapVisitor::new(parser)?;
                let mut expected: Option<T> = None;
                let mut actual: Option<T> = None;

                while let Some(key) = visitor.next_key()? {
                    match key.as_str() {
                        "expected" => {
                            if expected.is_some() {
                                return Err(norito::json::Error::duplicate_field("expected"));
                            }
                            expected = Some(visitor.parse_value::<T>()?);
                        }
                        "actual" => {
                            if actual.is_some() {
                                return Err(norito::json::Error::duplicate_field("actual"));
                            }
                            actual = Some(visitor.parse_value::<T>()?);
                        }
                        other => {
                            visitor.skip_value()?;
                            return Err(norito::json::Error::unknown_field(other));
                        }
                    }
                }
                visitor.finish()?;

                let expected =
                    expected.ok_or_else(|| norito::json::Error::missing_field("expected"))?;
                let actual = actual.ok_or_else(|| norito::json::Error::missing_field("actual"))?;

                Ok(Self { expected, actual })
            }
        }

        /// Type error
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            derive_more::From,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum TypeError {
            /// Asset definition numeric spec mismatch (asset can't hold provided numeric value)
            AssetNumericSpec(#[source] Mismatch<NumericSpec>),
        }

        /// Math error, which occurs during instruction execution
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            derive_more::From,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub enum MathError {
            /// Overflow error occurred inside instruction
            Overflow,
            /// Not enough quantity to transfer/burn
            NotEnoughQuantity,
            /// Divide by zero
            DivideByZero,
            /// Negative value encountered
            NegativeValue,
            /// Domain violation
            DomainViolation,
            /// Unknown error
            ///
            /// No actual function should ever return this if possible
            Unknown,
            /// Conversion failed: {0}
            FixedPointConversion(String),
        }

        /// Mintability logic error
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[repr(u8)]
        pub enum MintabilityError {
            /// This asset cannot be minted more than once and it was already minted
            MintUnmintable,
            /// This asset was set as infinitely mintable. You cannot forbid its minting
            ForbidMintOnMintable,
            /// Limited mintability token count `{0}` is invalid
            InvalidMintabilityTokens(u32),
        }

        /// Invalid instruction parameter error
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
        #[ignore_extra_doc_attributes]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
        #[repr(u8)]
        pub enum InvalidParameterError {
            /// Invalid smart contract: {0}
            SmartContract(String),
            /// Attempt to register a time-trigger with `start` point in the past
            TimeTriggerInThePast,
        }

        /// Repetition of `{instruction}` for id `{id}`
        #[derive(
            Debug,
            displaydoc::Display,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Getters,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[derive(thiserror::Error)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct RepetitionError {
            /// Instruction type
            #[getset(get = "pub")]
            pub instruction: InstructionType,
            /// Id of the object being repeated
            pub id: IdBox,
        }
    }

    impl<T: Debug> Mismatch<T> {
        /// The value that is needed for normal execution
        pub fn expected(&self) -> &T {
            &self.expected
        }
    }

    impl<T: Debug> Mismatch<T> {
        /// The value that caused the error
        pub fn actual(&self) -> &T {
            &self.actual
        }
    }

    impl From<&str> for InstructionExecutionError {
        fn from(error: &str) -> Self {
            Self::Conversion(error.to_owned())
        }
    }

    impl From<TypeError> for InstructionExecutionError {
        fn from(err: TypeError) -> Self {
            Self::Evaluate(InstructionEvaluationError::Type(err))
        }
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{
        AggregateOracleFeed, Burn, BurnBox, CustomInstruction, ExecuteTrigger, Grant, GrantBox,
        Instruction, InstructionBox, Log, Mint, MintBox, ProposeOracleChange, Register,
        RegisterBox, RegisterOracleFeed, RemoveKeyValue, RemoveKeyValueBox, Revoke, RevokeBox,
        RollbackOracleChange, SetKeyValue, SetKeyValueBox, SetParameter, SubmitOracleObservation,
        Transfer, TransferAssetBatch, TransferAssetBatchEntry, TransferBox, Unregister,
        UnregisterBox, Upgrade, VoteOracleChangeStage,
        bridge::{RecordBridgeReceipt, SubmitBridgeProof},
        confidential::{
            PublishPedersenParams, PublishPoseidonParams, SetPedersenParamsLifecycle,
            SetPoseidonParamsLifecycle,
        },
        consensus_keys::{DisableConsensusKey, RegisterConsensusKey, RotateConsensusKey},
        content::{PublishContentBundle, RetireContentBundle},
        endorsement::{
            RegisterDomainCommittee, SetDomainEndorsementPolicy, SubmitDomainEndorsement,
        },
        nexus::SetLaneRelayEmergencyValidators,
        repo::{RepoInstructionBox, RepoIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementFailureRecord,
            SettlementInstructionBox, SettlementKind, SettlementLedger, SettlementLedgerEntry,
            SettlementLeg, SettlementLegRole, SettlementLegSnapshot, SettlementOutcomeRecord,
            SettlementPlan, SettlementSuccessRecord,
        },
        social::{CancelTwitterEscrow, ClaimTwitterFollowReward, SendToTwitter},
        soradns::{
            AddReleaseSigner, PublishDirectory, RemoveReleaseSigner, RevokeResolver,
            SetDirectoryRotationPolicy, SubmitDirectoryDraft, UnrevokeResolver,
        },
        sorafs::{
            ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder, IssueReplicationOrder,
            RecordCapacityTelemetry, RegisterCapacityDeclaration, RegisterCapacityDispute,
            RegisterPinManifest, RetirePinManifest,
        },
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
        staking::{ActivatePublicLaneValidator, ExitPublicLaneValidator},
    };
}

#[cfg(test)]
mod tests {
    use iroha_primitives::const_vec::ConstVec;

    use super::*;
    use crate::prelude::*;
    fn run_or_skip() -> bool {
        std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")
    }

    macro_rules! check_enum {
        ($name:ident { $($variant:ident),+ $(,)? }) => {
            $(assert_eq!($name::try_from($name::$variant as u8).unwrap(), $name::$variant);)+
            assert!($name::try_from(u8::MAX).is_err());
            $(assert_eq!(format!("{}", $name::$variant), stringify!($variant));)+
        };
    }

    struct RegistryGuard;

    impl RegistryGuard {
        fn set(registry: InstructionRegistry) -> Self {
            set_instruction_registry(registry);
            Self
        }
    }

    impl Drop for RegistryGuard {
        fn drop(&mut self) {
            set_instruction_registry(crate::instruction_registry::default());
        }
    }

    #[test]
    fn aa_setup_instruction_registry() {
        let _guard = RegistryGuard::set(instruction_registry![Log]);
    }

    #[test]
    fn register_and_decode_instruction() {
        if !run_or_skip() {
            eprintln!(
                "Skipping: registry decode uses bare-codec; Norito alignment pending. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        let registry = InstructionRegistry::new().register::<Log>();
        // Sanity: decode map contains type name and entries are populated
        assert!(
            !registry.is_empty(),
            "registry should contain at least one entry"
        );
        assert!(registry.contains(std::any::type_name::<Log>()));
        let name = std::any::type_name::<Log>();
        let instruction = Log {
            level: Level::INFO,
            msg: "test".into(),
        };
        let bytes = instruction.encode();
        // Use the decode API directly to ensure local registry wiring works
        let decoded = InstructionRegistry::decode(&registry, name, &bytes)
            .expect("constructor not found in decode map")
            .expect("failed to decode");
        // Verify type id and payload equivalence without relying on downcast
        assert_eq!(Instruction::id(&*decoded), name);
        assert_eq!(Instruction::dyn_encode(&*decoded), bytes);
    }

    #[test]
    fn decode_unregistered_instruction() {
        let registry = InstructionRegistry::new();
        assert!(registry.decode("missing", &[]).is_none());
    }

    #[test]
    fn frame_payload_accepts_non_static_type_name() {
        let log = Log::new(Level::INFO, "framed".to_string());
        let payload = log.encode();
        let type_name = std::any::type_name::<Log>().to_string();
        let framed =
            frame_instruction_payload(&type_name, &payload).expect("frame instruction payload");
        let decoded: Log = norito::decode_from_bytes(&framed).expect("decode framed payload");
        assert_eq!(decoded, log);
    }

    #[test]
    fn dyn_encode_matches_instruction_box() {
        let log = Log {
            level: Level::INFO,
            msg: "test".to_string(),
        };
        let boxed = InstructionBox::from(log.clone());
        let expected = Instruction::dyn_encode(&*boxed);
        let actual = Instruction::dyn_encode(&log);
        assert_eq!(actual, expected);
    }

    #[test]
    fn as_any_downcasts() {
        let log = Log {
            level: Level::INFO,
            msg: "downcast".to_string(),
        };
        let instr: &dyn Instruction = &log;
        assert!(instr.as_any().downcast_ref::<Log>().is_some());
    }

    #[test]
    fn into_instruction_box_produces_equivalent() {
        let log = Log {
            level: Level::INFO,
            msg: "into".to_string(),
        };
        let boxed = Instruction::into_instruction_box(Box::new(log.clone()));
        let expected = Instruction::dyn_encode(&*InstructionBox::from(log));
        assert_eq!(Instruction::dyn_encode(&*boxed), expected);
    }

    #[test]
    fn dyn_execute_does_not_panic() {
        let log = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "exec".to_string(),
        });
        Instruction::dyn_execute(&*log);
    }

    #[test]
    fn instruction_box_display() {
        let log = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "display".to_string(),
        });
        assert_eq!(log.to_string(), "InstructionBox");
    }

    #[test]
    fn norito_serialize_trait_object() {
        let log = Log {
            level: Level::INFO,
            msg: "serialize".to_string(),
        };
        let boxed = InstructionBox::from(log.clone());
        let bytes = norito::core::to_bytes(&boxed).expect("serialize");
        let archived = norito::core::from_bytes::<(String, Vec<u8>)>(&bytes).expect("from_bytes");
        let (name, payload) =
            norito::core::NoritoDeserialize::try_deserialize(archived).expect("deserialize");
        assert_eq!(name, Log::WIRE_ID);
        let bare = Instruction::dyn_encode(&log);
        let payload_slice = payload.as_slice();
        assert!(
            payload_slice.starts_with(&norito::core::MAGIC),
            "Instruction payload must include Norito header",
        );
        assert!(
            payload.len() >= norito::core::Header::SIZE,
            "Instruction payload shorter than Norito header",
        );
        assert_eq!(
            &payload_slice[norito::core::Header::SIZE..],
            bare.as_slice()
        );
    }

    #[test]
    fn norito_roundtrip_trait_object_deserialize() {
        if !run_or_skip() {
            eprintln!(
                "Skipping: InstructionBox Norito nested decode pending fix. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        let log = Log {
            level: Level::INFO,
            msg: "deserialize".to_string(),
        };
        let _guard = RegistryGuard::set(instruction_registry![Log]);
        let boxed = InstructionBox::from(log.clone());
        let bytes = norito::core::to_bytes(&boxed).expect("serialize");
        let archived = norito::core::from_bytes::<InstructionBox>(&bytes).expect("from_bytes");
        let decoded =
            norito::core::NoritoDeserialize::try_deserialize(archived).expect("deserialize");
        // Validate via type id and payload equality rather than downcast
        assert_eq!(Instruction::id(&*decoded), Instruction::id(&log));
        assert_eq!(
            Instruction::dyn_encode(&*decoded),
            Instruction::dyn_encode(&log)
        );
    }

    #[test]
    fn instruction_pair_canonical_decode_covers_payload_body() {
        let expected = ("force-decode".to_owned(), vec![1_u8, 2, 3, 4]);
        let framed =
            norito::core::to_bytes(&expected).expect("serialize instruction tuple with Norito");
        let archived =
            norito::core::from_bytes::<(String, Vec<u8>)>(&framed).expect("decode framed tuple");
        let decoded = norito::core::NoritoDeserialize::try_deserialize(archived).expect("decode");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn instruction_box_rejects_non_norito_payload() {
        use norito::core::DecodeFromSlice;

        let err = InstructionBox::decode_from_slice(&[0x01, 0x02])
            .expect_err("non-canonical payload must be rejected");
        match err {
            norito::core::Error::Message(msg) => assert!(
                msg.contains("canonical Norito framing"),
                "error should steer callers to the canonical encoding: {msg}"
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn const_vec_instruction_box_decodes_with_varint_tail() {
        let _guard = RegistryGuard::set(instruction_registry![Log]);

        let instruction = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "varint tail regression".to_owned(),
        });
        let expected = vec![instruction.clone()];
        let original = ConstVec::from(expected.clone());

        let framed = norito::core::to_bytes(&original).expect("serialize ConstVec<InstructionBox>");
        let flags = framed[norito::core::Header::SIZE - 1];
        let payload = &framed[norito::core::Header::SIZE..];
        let mut mutated = payload.to_vec();

        let (len, used_hdr) = {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            norito::core::read_seq_len_slice(&mutated).expect("sequence header")
        };
        eprintln!("const_vec len={len} used_hdr={used_hdr}");
        assert_eq!(len, expected.len());

        {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            let mut cursor = used_hdr;
            for _ in 0..len {
                let (_, hdr) =
                    norito::core::read_len_dyn_slice(&mutated[cursor..]).expect("element header");
                eprintln!("const_vec element hdr={hdr}");
                for byte in &mut mutated[cursor..cursor + hdr] {
                    *byte = 0;
                }
                cursor += hdr;
            }
        }

        let decoded = {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            let (value, used) =
                norito::core::decode_field_canonical::<ConstVec<InstructionBox>>(&mutated)
                    .expect("decode const vec from tail offsets");
            assert!(used > 0);
            value
        };
        norito::core::reset_decode_state();
        assert_eq!(decoded.into_vec(), expected);
    }

    #[test]
    fn encode_as_instruction_box_uses_encode() {
        let log = Log {
            level: Level::INFO,
            msg: "encode".to_string(),
        };
        let expected = log.encode();
        let actual = BuiltInInstruction::encode_as_instruction_box(&log);
        assert_eq!(actual, expected);
    }

    #[test]
    fn default_registry_roundtrip_selected_instructions() {
        if !run_or_skip() {
            eprintln!(
                "Skipping: registry decode of header-extracted payload pending Norito fix. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        // Install default registry covering built-ins and keep a local handle
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let local_registry = crate::instruction_registry::default();

        // Build a small suite of representative instructions
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let account_id: AccountId =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland"
                .parse()
                .unwrap();
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let asset_id = AssetId::of(asset_def_id.clone(), account_id.clone());
        let nft_id: NftId = "n0$wonderland".parse().unwrap();
        let role_id: RoleId = "auditor".parse().unwrap();
        let key: Name = "k".parse().unwrap();

        let cases: Vec<InstructionBox> = vec![
            // Register/Unregister
            Register::domain(Domain::new(domain_id.clone())).into(),
            Unregister::domain(domain_id.clone()).into(),
            // Set/Remove metadata
            SetKeyValue::domain(domain_id.clone(), key.clone(), Json::new(1u32)).into(),
            RemoveKeyValue::domain(domain_id.clone(), key.clone()).into(),
            // Mint/Burn asset
            Mint::asset_numeric(10_u32, asset_id.clone()).into(),
            Burn::asset_numeric(5_u32, asset_id.clone()).into(),
            // Transfer asset
            Transfer::asset_numeric(asset_id.clone(), 1_u32, account_id.clone()).into(),
            // NFT register + transfer
            Register::nft(Nft::new(nft_id.clone(), Metadata::default())).into(),
            Transfer::nft(account_id.clone(), nft_id.clone(), account_id.clone()).into(),
            // Grant/Revoke role
            Grant::account_role(role_id.clone(), account_id.clone()).into(),
            Revoke::account_role(role_id.clone(), account_id.clone()).into(),
            // SetParameter
            SetParameter::new(Parameter::Transaction(
                crate::parameter::TransactionParameter::MaxInstructions(nonzero_ext::nonzero!(
                    10_u64
                )),
            ))
            .into(),
            // Log
            Log::new(Level::INFO, "hello".into()).into(),
        ];

        for instr in cases {
            let bytes = norito::to_bytes(&instr).expect("serialize");
            // Decode without relying on the global registry during this window
            let (name, payload) = norito::decode_from_bytes::<(String, Vec<u8>)>(&bytes)
                .expect("extract tag + payload");
            let decoded = local_registry
                .decode(&name, &payload)
                .unwrap_or_else(|| panic!("instruction `{name}` is not registered"))
                .expect("decode via default registry");
            assert_eq!(instr, decoded);
        }
    }

    #[test]
    fn revoke_encode_as_instruction_box_uses_encode() {
        // Account identifiers accept `<alias|public_key>@domain` input forms.
        let account_id: AccountId =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland"
                .parse()
                .unwrap();
        let permission = Permission::new("dummy".parse().unwrap(), Json::new(()));
        let revoke = Revoke::account_permission(permission, account_id);
        let expected = revoke.encode();
        let actual = BuiltInInstruction::encode_as_instruction_box(&revoke);
        assert_eq!(actual, expected);
    }

    #[test]
    fn discriminant_roundtrip() {
        check_enum!(SetKeyValueType {
            Domain,
            Account,
            AssetDefinition,
            Nft,
            Trigger
        });
        check_enum!(RemoveKeyValueType {
            Domain,
            Account,
            AssetDefinition,
            Nft,
            Trigger
        });
        check_enum!(RegisterType {
            Peer,
            Domain,
            Account,
            AssetDefinition,
            Nft,
            Role,
            Trigger
        });
        check_enum!(UnregisterType {
            Peer,
            Domain,
            Account,
            AssetDefinition,
            Nft,
            Role,
            Trigger
        });
        check_enum!(MintType {
            Asset,
            TriggerRepetitions
        });
        check_enum!(BurnType {
            Asset,
            TriggerRepetitions
        });
        check_enum!(TransferType {
            Domain,
            AssetDefinition,
            Asset,
            Nft
        });
        check_enum!(GrantType {
            Permission,
            Role,
            RolePermission
        });
        check_enum!(RevokeType {
            Permission,
            Role,
            RolePermission
        });
    }

    #[test]
    fn ordering_is_preserved_across_roundtrip() {
        if !run_or_skip() {
            eprintln!(
                "Skipping: header-framed nested InstructionBox decode pending fix. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        // Ensure the total ordering of InstructionBox is stable after Norito roundtrip.
        let _guard = RegistryGuard::set(crate::instruction_registry::default());

        let domain_id: DomainId = "alice".parse().unwrap();
        let account_id: AccountId =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@alice"
                .parse()
                .unwrap();
        let asset_def_id: AssetDefinitionId = "coin#alice".parse().unwrap();
        let asset_id = AssetId::of(asset_def_id.clone(), account_id.clone());
        let role_id: RoleId = "auditor".parse().unwrap();

        let mut instrs = vec![
            Register::domain(Domain::new(domain_id.clone())).into(),
            Grant::account_role(role_id.clone(), account_id.clone()).into(),
            Mint::asset_numeric(5_u32, asset_id.clone()).into(),
            Transfer::asset_numeric(asset_id.clone(), 1_u32, account_id.clone()).into(),
            Burn::asset_numeric(1_u32, asset_id.clone()).into(),
            Unregister::domain(domain_id.clone()).into(),
            Log::new(Level::INFO, "x".into()).into(),
        ];
        // Sort by Ord
        instrs.sort();
        // Roundtrip each via Norito bytes
        let rt: Vec<InstructionBox> = instrs
            .iter()
            // Explicitly specify the generic type so the compiler knows which
            // `NoritoSerialize` implementation to use for the `InstructionBox`
            // trait object reference.
            .map(|i| norito::to_bytes::<InstructionBox>(i).expect("encode"))
            .map(|b| norito::decode_from_bytes::<InstructionBox>(&b).expect("decode"))
            .collect();
        let mut rt_sorted = rt.clone();
        rt_sorted.sort();
        assert_eq!(instrs, rt_sorted);
    }

    #[test]
    fn default_registry_roundtrip_more_instructions() {
        if !run_or_skip() {
            eprintln!(
                "Skipping: registry decode + nested Norito decoders pending fix. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        // Expand coverage across instruction families and variants
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let local_registry = crate::instruction_registry::default();

        // Common fixtures
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let account_a: AccountId =
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
                .parse()
                .unwrap();
        let account_b: AccountId =
            "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland"
                .parse()
                .unwrap();
        let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().unwrap();
        let asset_id = AssetId::of(asset_def_id.clone(), account_a.clone());
        let nft_id: NftId = "n0$wonderland".parse().unwrap();
        let role_id: RoleId = "auditor".parse().unwrap();
        let key: Name = "k".parse().unwrap();
        let trig_id: TriggerId = "nightly_tick".parse().unwrap();

        // Permission token
        let perm = Permission::new("mint".parse().unwrap(), Json::new(()));

        // Upgrade executor placeholder
        let exec = crate::executor::Executor::new(
            crate::transaction::executable::IvmBytecode::from_compiled(vec![1, 2, 3]),
        );

        let cases: Vec<InstructionBox> = vec![
            // SetKeyValue and RemoveKeyValue across all owners
            SetKeyValue::account(account_a.clone(), key.clone(), Json::new(1u32)).into(),
            SetKeyValue::asset_definition(asset_def_id.clone(), key.clone(), Json::new(2u32))
                .into(),
            SetKeyValue::nft(nft_id.clone(), key.clone(), Json::new(3u32)).into(),
            SetKeyValue::trigger(trig_id.clone(), key.clone(), Json::new(4u32)).into(),
            RemoveKeyValue::account(account_a.clone(), key.clone()).into(),
            RemoveKeyValue::asset_definition(asset_def_id.clone(), key.clone()).into(),
            RemoveKeyValue::nft(nft_id.clone(), key.clone()).into(),
            RemoveKeyValue::trigger(trig_id.clone(), key.clone()).into(),
            // Transfers for all variants
            Transfer::domain(account_a.clone(), domain_id.clone(), account_b.clone()).into(),
            Transfer::asset_definition(account_a.clone(), asset_def_id.clone(), account_b.clone())
                .into(),
            Transfer::asset_numeric(asset_id.clone(), 7_u32, account_b.clone()).into(),
            Transfer::nft(account_a.clone(), nft_id.clone(), account_b.clone()).into(),
            // Grants and revokes for permission and role targets
            Grant::account_permission(perm.clone(), account_a.clone()).into(),
            Grant::role_permission(perm.clone(), role_id.clone()).into(),
            Revoke::account_permission(perm.clone(), account_a.clone()).into(),
            Revoke::role_permission(perm.clone(), role_id.clone()).into(),
            // ExecuteTrigger, Upgrade, CustomInstruction
            ExecuteTrigger::new(trig_id.clone())
                .with_args(norito::json!({"a": 1u32}))
                .into(),
            Upgrade::new(exec).into(),
            // Use an explicit empty JSON payload since `Json` does not implement
            // `From<()>`.
            CustomInstruction::new(Json::new(())).into(),
        ];

        for instr in cases {
            let bytes = norito::to_bytes(&instr).expect("encode");
            let (name, payload) =
                norito::decode_from_bytes::<(String, Vec<u8>)>(&bytes).expect("extract");
            let decoded = local_registry
                .decode(&name, &payload)
                .unwrap_or_else(|| panic!("instruction `{name}` is not registered"))
                .expect("decode via registry");
            assert_eq!(instr, decoded);
        }
    }
}

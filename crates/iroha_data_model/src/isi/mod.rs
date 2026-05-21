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

#![cfg_attr(test, allow(clippy::needless_pass_by_value))]

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
/// Ministry agenda intake instructions.
pub mod ministry;

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

/// Client-side wrapper preserving an instruction wire-id plus already encoded
/// payload bytes.
///
/// This is intended for compatibility flows where a remote node returns a draft
/// instruction in framed wire form and the local client needs to resubmit that
/// exact payload without understanding its full semantic schema.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OpaqueInstruction {
    wire_id: &'static str,
    bare_payload: Vec<u8>,
    framed_payload: Vec<u8>,
}

impl OpaqueInstruction {
    /// Build an opaque instruction from a framed wire payload returned by Torii.
    ///
    /// # Errors
    /// Returns [`norito::core::Error`] when the framed payload is malformed.
    pub fn from_framed(
        wire_id: impl Into<String>,
        framed_payload: &[u8],
    ) -> Result<Self, norito::core::Error> {
        let view = norito::core::from_bytes_view(framed_payload)?;
        Ok(Self {
            wire_id: Box::leak(wire_id.into().into_boxed_str()),
            bare_payload: view.as_bytes().to_vec(),
            framed_payload: framed_payload.to_vec(),
        })
    }

    /// Return the stable wire identifier carried by this opaque instruction.
    #[must_use]
    pub const fn wire_id(&self) -> &'static str {
        self.wire_id
    }

    /// Return the exact framed payload carried by this opaque instruction.
    #[must_use]
    pub fn framed_payload(&self) -> &[u8] {
        &self.framed_payload
    }
}

impl crate::seal::Instruction for OpaqueInstruction {}

impl Instruction for OpaqueInstruction {
    fn dyn_encode(&self) -> Vec<u8> {
        self.bare_payload.clone()
    }

    fn dyn_encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.bare_payload);
    }

    fn dyn_encode_capacity_hint(&self) -> Option<usize> {
        Some(self.bare_payload.len())
    }

    fn dyn_encoded_len(&self) -> Option<usize> {
        Some(self.bare_payload.len())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn id(&self) -> &'static str {
        self.wire_id
    }
}

impl From<OpaqueInstruction> for InstructionBox {
    fn from(i: OpaqueInstruction) -> Self {
        InstructionBox(Box::new(i))
    }
}

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
impl From<crate::isi::bridge::RecordSccpMessage> for InstructionBox {
    fn from(i: crate::isi::bridge::RecordSccpMessage) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::asset_alias::SetAssetDefinitionAlias> for InstructionBox {
    fn from(i: crate::isi::asset_alias::SetAssetDefinitionAlias) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::asset_alias::SetAssetDefinitionBalancePolicy> for InstructionBox {
    fn from(i: crate::isi::asset_alias::SetAssetDefinitionBalancePolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::asset_transfer_control::SetAssetTransferFreeze> for InstructionBox {
    fn from(i: crate::isi::asset_transfer_control::SetAssetTransferFreeze) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::asset_transfer_control::SetAssetTransferBlacklist> for InstructionBox {
    fn from(i: crate::isi::asset_transfer_control::SetAssetTransferBlacklist) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::asset_transfer_control::SetAssetTransferControl> for InstructionBox {
    fn from(i: crate::isi::asset_transfer_control::SetAssetTransferControl) -> Self {
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

impl From<crate::isi::staking::RebindPublicLaneValidatorPeer> for InstructionBox {
    fn from(i: crate::isi::staking::RebindPublicLaneValidatorPeer) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::CreateKaigi> for InstructionBox {
    fn from(i: crate::isi::kaigi::CreateKaigi) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::JoinKaigi> for InstructionBox {
    fn from(i: crate::isi::kaigi::JoinKaigi) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::LeaveKaigi> for InstructionBox {
    fn from(i: crate::isi::kaigi::LeaveKaigi) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::EndKaigi> for InstructionBox {
    fn from(i: crate::isi::kaigi::EndKaigi) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::RecordKaigiUsage> for InstructionBox {
    fn from(i: crate::isi::kaigi::RecordKaigiUsage) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::SetKaigiRelayManifest> for InstructionBox {
    fn from(i: crate::isi::kaigi::SetKaigiRelayManifest) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::RegisterKaigiRelay> for InstructionBox {
    fn from(i: crate::isi::kaigi::RegisterKaigiRelay) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::kaigi::ReportKaigiRelayHealth> for InstructionBox {
    fn from(i: crate::isi::kaigi::ReportKaigiRelayHealth) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::nexus::SetLaneRelayEmergencyValidators> for InstructionBox {
    fn from(i: crate::isi::nexus::SetLaneRelayEmergencyValidators) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::nexus::RegisterVerifiedLaneRelay> for InstructionBox {
    fn from(i: crate::isi::nexus::RegisterVerifiedLaneRelay) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::nexus::RegisterVerifiedNexusFeeBudget> for InstructionBox {
    fn from(i: crate::isi::nexus::RegisterVerifiedNexusFeeBudget) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::identifier::RegisterIdentifierPolicy> for InstructionBox {
    fn from(i: crate::isi::identifier::RegisterIdentifierPolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::identifier::ActivateIdentifierPolicy> for InstructionBox {
    fn from(i: crate::isi::identifier::ActivateIdentifierPolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::identifier::ClaimIdentifier> for InstructionBox {
    fn from(i: crate::isi::identifier::ClaimIdentifier) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::identifier::RevokeIdentifier> for InstructionBox {
    fn from(i: crate::isi::identifier::RevokeIdentifier) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::DeploySoracloudService> for InstructionBox {
    fn from(i: crate::isi::soracloud::DeploySoracloudService) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::UpgradeSoracloudService> for InstructionBox {
    fn from(i: crate::isi::soracloud::UpgradeSoracloudService) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::DeploySoracloudAppInfra> for InstructionBox {
    fn from(i: crate::isi::soracloud::DeploySoracloudAppInfra) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::UpgradeSoracloudAppInfra> for InstructionBox {
    fn from(i: crate::isi::soracloud::UpgradeSoracloudAppInfra) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RollbackSoracloudService> for InstructionBox {
    fn from(i: crate::isi::soracloud::RollbackSoracloudService) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::SetSoracloudServiceConfig> for InstructionBox {
    fn from(i: crate::isi::soracloud::SetSoracloudServiceConfig) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::DeleteSoracloudServiceConfig> for InstructionBox {
    fn from(i: crate::isi::soracloud::DeleteSoracloudServiceConfig) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::SetSoracloudServiceSecret> for InstructionBox {
    fn from(i: crate::isi::soracloud::SetSoracloudServiceSecret) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::DeleteSoracloudServiceSecret> for InstructionBox {
    fn from(i: crate::isi::soracloud::DeleteSoracloudServiceSecret) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::MutateSoracloudState> for InstructionBox {
    fn from(i: crate::isi::soracloud::MutateSoracloudState) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RunSoracloudFheJob> for InstructionBox {
    fn from(i: crate::isi::soracloud::RunSoracloudFheJob) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RecordSoracloudDecryptionRequest> for InstructionBox {
    fn from(i: crate::isi::soracloud::RecordSoracloudDecryptionRequest) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::JoinSoracloudHfSharedLease> for InstructionBox {
    fn from(i: crate::isi::soracloud::JoinSoracloudHfSharedLease) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::LeaveSoracloudHfSharedLease> for InstructionBox {
    fn from(i: crate::isi::soracloud::LeaveSoracloudHfSharedLease) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RenewSoracloudHfSharedLease> for InstructionBox {
    fn from(i: crate::isi::soracloud::RenewSoracloudHfSharedLease) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::AdvertiseSoracloudModelHost> for InstructionBox {
    fn from(i: crate::isi::soracloud::AdvertiseSoracloudModelHost) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::HeartbeatSoracloudModelHost> for InstructionBox {
    fn from(i: crate::isi::soracloud::HeartbeatSoracloudModelHost) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::WithdrawSoracloudModelHost> for InstructionBox {
    fn from(i: crate::isi::soracloud::WithdrawSoracloudModelHost) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::ReconcileSoracloudModelHosts> for InstructionBox {
    fn from(i: crate::isi::soracloud::ReconcileSoracloudModelHosts) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::AdvertiseSoracloudInrouHost> for InstructionBox {
    fn from(i: crate::isi::soracloud::AdvertiseSoracloudInrouHost) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::WithdrawSoracloudInrouHost> for InstructionBox {
    fn from(i: crate::isi::soracloud::WithdrawSoracloudInrouHost) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::ReconcileSoracloudInrouPlacements> for InstructionBox {
    fn from(i: crate::isi::soracloud::ReconcileSoracloudInrouPlacements) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::ReportSoracloudModelHostViolation> for InstructionBox {
    fn from(i: crate::isi::soracloud::ReportSoracloudModelHostViolation) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::DeploySoracloudAgentApartment> for InstructionBox {
    fn from(i: crate::isi::soracloud::DeploySoracloudAgentApartment) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RenewSoracloudAgentLease> for InstructionBox {
    fn from(i: crate::isi::soracloud::RenewSoracloudAgentLease) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RestartSoracloudAgentApartment> for InstructionBox {
    fn from(i: crate::isi::soracloud::RestartSoracloudAgentApartment) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RevokeSoracloudAgentPolicy> for InstructionBox {
    fn from(i: crate::isi::soracloud::RevokeSoracloudAgentPolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RequestSoracloudAgentWalletSpend> for InstructionBox {
    fn from(i: crate::isi::soracloud::RequestSoracloudAgentWalletSpend) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::ApproveSoracloudAgentWalletSpend> for InstructionBox {
    fn from(i: crate::isi::soracloud::ApproveSoracloudAgentWalletSpend) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::EnqueueSoracloudAgentMessage> for InstructionBox {
    fn from(i: crate::isi::soracloud::EnqueueSoracloudAgentMessage) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::AcknowledgeSoracloudAgentMessage> for InstructionBox {
    fn from(i: crate::isi::soracloud::AcknowledgeSoracloudAgentMessage) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::AllowSoracloudAgentAutonomyArtifact> for InstructionBox {
    fn from(i: crate::isi::soracloud::AllowSoracloudAgentAutonomyArtifact) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RunSoracloudAgentAutonomy> for InstructionBox {
    fn from(i: crate::isi::soracloud::RunSoracloudAgentAutonomy) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RecordSoracloudAgentAutonomyExecution> for InstructionBox {
    fn from(i: crate::isi::soracloud::RecordSoracloudAgentAutonomyExecution) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::StartSoracloudTrainingJob> for InstructionBox {
    fn from(i: crate::isi::soracloud::StartSoracloudTrainingJob) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::CheckpointSoracloudTrainingJob> for InstructionBox {
    fn from(i: crate::isi::soracloud::CheckpointSoracloudTrainingJob) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RetrySoracloudTrainingJob> for InstructionBox {
    fn from(i: crate::isi::soracloud::RetrySoracloudTrainingJob) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RegisterSoracloudModelArtifact> for InstructionBox {
    fn from(i: crate::isi::soracloud::RegisterSoracloudModelArtifact) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RegisterSoracloudModelWeight> for InstructionBox {
    fn from(i: crate::isi::soracloud::RegisterSoracloudModelWeight) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::PromoteSoracloudModelWeight> for InstructionBox {
    fn from(i: crate::isi::soracloud::PromoteSoracloudModelWeight) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RollbackSoracloudModelWeight> for InstructionBox {
    fn from(i: crate::isi::soracloud::RollbackSoracloudModelWeight) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RegisterSoracloudUploadedModelBundle> for InstructionBox {
    fn from(i: crate::isi::soracloud::RegisterSoracloudUploadedModelBundle) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::FinalizeSoracloudUploadedModelBundle> for InstructionBox {
    fn from(i: crate::isi::soracloud::FinalizeSoracloudUploadedModelBundle) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::AdvanceSoracloudRollout> for InstructionBox {
    fn from(i: crate::isi::soracloud::AdvanceSoracloudRollout) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::SetSoracloudRuntimeState> for InstructionBox {
    fn from(i: crate::isi::soracloud::SetSoracloudRuntimeState) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::SetSoracloudInrouReplicaRuntimeState> for InstructionBox {
    fn from(i: crate::isi::soracloud::SetSoracloudInrouReplicaRuntimeState) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::ClearSoracloudInrouReplicaRuntimeState> for InstructionBox {
    fn from(i: crate::isi::soracloud::ClearSoracloudInrouReplicaRuntimeState) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::ReportSoracloudServiceLeaseUsage> for InstructionBox {
    fn from(i: crate::isi::soracloud::ReportSoracloudServiceLeaseUsage) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RecordSoracloudMailboxMessage> for InstructionBox {
    fn from(i: crate::isi::soracloud::RecordSoracloudMailboxMessage) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RecordSoracloudRuntimeReceipt> for InstructionBox {
    fn from(i: crate::isi::soracloud::RecordSoracloudRuntimeReceipt) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt>
    for InstructionBox
{
    fn from(i: crate::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt) -> Self {
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

// Allow direct boxing of native asset escrow instructions.
impl From<crate::isi::escrow::OpenAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::OpenAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::AcceptAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::AcceptAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::MarkEscrowPaymentSent> for InstructionBox {
    fn from(i: crate::isi::escrow::MarkEscrowPaymentSent) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::ReleaseAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::ReleaseAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::CancelAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::CancelAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::OpenEscrowDispute> for InstructionBox {
    fn from(i: crate::isi::escrow::OpenEscrowDispute) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::ResolveEscrowDispute> for InstructionBox {
    fn from(i: crate::isi::escrow::ResolveEscrowDispute) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::OpenAnonymousAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::OpenAnonymousAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::AcceptAnonymousAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::AcceptAnonymousAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::MarkAnonymousEscrowPaymentSent> for InstructionBox {
    fn from(i: crate::isi::escrow::MarkAnonymousEscrowPaymentSent) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::ReleaseAnonymousAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::ReleaseAnonymousAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::CancelAnonymousAssetEscrow> for InstructionBox {
    fn from(i: crate::isi::escrow::CancelAnonymousAssetEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::OpenAnonymousEscrowDispute> for InstructionBox {
    fn from(i: crate::isi::escrow::OpenAnonymousEscrowDispute) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::escrow::ResolveAnonymousEscrowDispute> for InstructionBox {
    fn from(i: crate::isi::escrow::ResolveAnonymousEscrowDispute) -> Self {
        InstructionBox(Box::new(i))
    }
}

// Allow direct boxing of SoraNet VPN lease escrow instructions.
impl From<crate::isi::vpn::OpenVpnLeaseEscrow> for InstructionBox {
    fn from(i: crate::isi::vpn::OpenVpnLeaseEscrow) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::vpn::SettleVpnLease> for InstructionBox {
    fn from(i: crate::isi::vpn::SettleVpnLease) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::vpn::RefundExpiredVpnLease> for InstructionBox {
    fn from(i: crate::isi::vpn::RefundExpiredVpnLease) -> Self {
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
impl From<crate::prelude::SetPricingSchedule> for InstructionBox {
    fn from(i: crate::prelude::SetPricingSchedule) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::prelude::UpsertProviderCredit> for InstructionBox {
    fn from(i: crate::prelude::UpsertProviderCredit) -> Self {
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
impl From<crate::isi::domain_link::SetAccountAliasBinding> for InstructionBox {
    fn from(i: crate::isi::domain_link::SetAccountAliasBinding) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::domain_link::SetPrimaryAccountAlias> for InstructionBox {
    fn from(i: crate::isi::domain_link::SetPrimaryAccountAlias) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_alias_lease::AcquireAccountAliasLease> for InstructionBox {
    fn from(i: crate::isi::account_alias_lease::AcquireAccountAliasLease) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_alias_lease::RenewAccountAliasLease> for InstructionBox {
    fn from(i: crate::isi::account_alias_lease::RenewAccountAliasLease) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sns::RegisterSnsName> for InstructionBox {
    fn from(i: crate::isi::sns::RegisterSnsName) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sns::RenewSnsName> for InstructionBox {
    fn from(i: crate::isi::sns::RenewSnsName) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sns::TransferSnsName> for InstructionBox {
    fn from(i: crate::isi::sns::TransferSnsName) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sns::UpdateSnsNameControllers> for InstructionBox {
    fn from(i: crate::isi::sns::UpdateSnsNameControllers) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sns::FreezeSnsName> for InstructionBox {
    fn from(i: crate::isi::sns::FreezeSnsName) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::sns::UnfreezeSnsName> for InstructionBox {
    fn from(i: crate::isi::sns::UnfreezeSnsName) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::ReplaceAccountController> for InstructionBox {
    fn from(i: crate::isi::account_recovery::ReplaceAccountController) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::SetAccountRecoveryPolicy> for InstructionBox {
    fn from(i: crate::isi::account_recovery::SetAccountRecoveryPolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::ClearAccountRecoveryPolicy> for InstructionBox {
    fn from(i: crate::isi::account_recovery::ClearAccountRecoveryPolicy) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::ProposeAccountRecovery> for InstructionBox {
    fn from(i: crate::isi::account_recovery::ProposeAccountRecovery) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::ApproveAccountRecovery> for InstructionBox {
    fn from(i: crate::isi::account_recovery::ApproveAccountRecovery) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::CancelAccountRecovery> for InstructionBox {
    fn from(i: crate::isi::account_recovery::CancelAccountRecovery) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::account_recovery::FinalizeAccountRecovery> for InstructionBox {
    fn from(i: crate::isi::account_recovery::FinalizeAccountRecovery) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::contract_alias::SetContractAlias> for InstructionBox {
    fn from(i: crate::isi::contract_alias::SetContractAlias) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Allow direct boxing of Musubi package registry instructions.
impl From<crate::isi::musubi::PublishMusubiRelease> for InstructionBox {
    fn from(i: crate::isi::musubi::PublishMusubiRelease) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::musubi::YankMusubiRelease> for InstructionBox {
    fn from(i: crate::isi::musubi::YankMusubiRelease) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::musubi::SetMusubiShortAlias> for InstructionBox {
    fn from(i: crate::isi::musubi::SetMusubiShortAlias) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::musubi::AssertMusubiReleaseExists> for InstructionBox {
    fn from(i: crate::isi::musubi::AssertMusubiReleaseExists) -> Self {
        InstructionBox(Box::new(i))
    }
}
// Allow direct boxing of Offline V2 note instructions.
impl From<crate::isi::offline::IssueOfflineNoteV2> for InstructionBox {
    fn from(i: crate::isi::offline::IssueOfflineNoteV2) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::offline::RedeemOfflineNoteV2> for InstructionBox {
    fn from(i: crate::isi::offline::RedeemOfflineNoteV2) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::offline::AuditOfflineNoteV2> for InstructionBox {
    fn from(i: crate::isi::offline::AuditOfflineNoteV2) -> Self {
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

impl From<crate::isi::oracle::OpenOracleDispute> for InstructionBox {
    fn from(i: crate::isi::oracle::OpenOracleDispute) -> Self {
        InstructionBox(Box::new(i))
    }
}

impl From<crate::isi::oracle::ResolveOracleDispute> for InstructionBox {
    fn from(i: crate::isi::oracle::ResolveOracleDispute) -> Self {
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

// Allow direct boxing of SoraDNS resolver-directory instructions.
impl From<crate::isi::soradns::SubmitDirectoryDraft> for InstructionBox {
    fn from(i: crate::isi::soradns::SubmitDirectoryDraft) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::soradns::PublishDirectory> for InstructionBox {
    fn from(i: crate::isi::soradns::PublishDirectory) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::soradns::RevokeResolver> for InstructionBox {
    fn from(i: crate::isi::soradns::RevokeResolver) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::soradns::UnrevokeResolver> for InstructionBox {
    fn from(i: crate::isi::soradns::UnrevokeResolver) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::soradns::AddReleaseSigner> for InstructionBox {
    fn from(i: crate::isi::soradns::AddReleaseSigner) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::soradns::RemoveReleaseSigner> for InstructionBox {
    fn from(i: crate::isi::soradns::RemoveReleaseSigner) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::soradns::SetDirectoryRotationPolicy> for InstructionBox {
    fn from(i: crate::isi::soradns::SetDirectoryRotationPolicy) -> Self {
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
impl From<crate::isi::governance::ProposeRuntimeUpgradeProposal> for InstructionBox {
    fn from(i: crate::isi::governance::ProposeRuntimeUpgradeProposal) -> Self {
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
#[cfg(feature = "governance")]
impl From<crate::isi::governance::CastParliamentBallot> for InstructionBox {
    fn from(i: crate::isi::governance::CastParliamentBallot) -> Self {
        InstructionBox(Box::new(i))
    }
}
impl From<crate::isi::ministry::SubmitAgendaProposal> for InstructionBox {
    fn from(i: crate::isi::ministry::SubmitAgendaProposal) -> Self {
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

    /// Append encoded instruction bytes to `out`.
    fn dyn_encode_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.dyn_encode());
    }

    /// Return a best-effort capacity hint for [`Self::dyn_encode_into`].
    fn dyn_encode_capacity_hint(&self) -> Option<usize> {
        self.dyn_encoded_len()
    }

    /// Return the encoded instruction payload length without allocating when available.
    fn dyn_encoded_len(&self) -> Option<usize> {
        None
    }

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

    fn dyn_encode_into(&self, out: &mut Vec<u8>) {
        Encode::encode_to(self, out);
    }

    fn dyn_encode_capacity_hint(&self) -> Option<usize> {
        norito::NoritoSerialize::encoded_len_exact(self)
            .or_else(|| norito::NoritoSerialize::encoded_len_hint(self))
    }

    fn dyn_encoded_len(&self) -> Option<usize> {
        norito::NoritoSerialize::encoded_len_exact(self)
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

fn peel_instruction_box(mut instr: &dyn Instruction) -> &dyn Instruction {
    loop {
        if let Some(nested) = instr.as_any().downcast_ref::<InstructionBox>() {
            instr = &**nested;
        } else {
            break instr;
        }
    }
}

fn instruction_tuple_flags() -> u8 {
    let defaults = norito::core::default_encode_flags();
    let dynamic_mask = norito::core::header_flags::PACKED_SEQ;
    let static_defaults = defaults & !dynamic_mask;
    match norito::core::effective_decode_flags() {
        None => defaults,
        Some(0) => 0,
        Some(current) => {
            let current_dynamic = current & dynamic_mask;
            let current_static = current & !dynamic_mask;
            let effective_static = if current_static == 0 {
                static_defaults
            } else {
                current_static | static_defaults
            };
            current_dynamic | effective_static
        }
    }
}

type InstructionPayloadFrameWriter =
    fn(&mut dyn std::io::Write, &[u8], u8) -> Result<(), norito::core::Error>;

struct EncodedInstructionPayload {
    name: &'static str,
    bare_payload: Vec<u8>,
    framed_payload_len: usize,
    payload_header_flags: u8,
    write_framed_payload: InstructionPayloadFrameWriter,
}

fn write_raw_instruction_payload(
    writer: &mut dyn std::io::Write,
    payload: &[u8],
    _payload_header_flags: u8,
) -> Result<(), norito::core::Error> {
    std::io::Write::write_all(writer, payload)?;
    Ok(())
}

fn write_instruction_pair_fields<W: std::io::Write>(
    mut writer: W,
    name: &str,
    framed_payload_len: usize,
    bare_payload: &[u8],
    payload_header_flags: u8,
    write_framed_payload: InstructionPayloadFrameWriter,
) -> Result<(), norito::core::Error> {
    let flags = instruction_tuple_flags();

    let name_len = norito::core::len_prefix_len_with_flags(name.len(), flags)
        .checked_add(name.len())
        .ok_or(norito::core::Error::LengthMismatch)?;
    norito::core::write_len_with_flags(
        &mut writer,
        u64::try_from(name_len).map_err(|_| norito::core::Error::LengthMismatch)?,
        flags,
    )?;
    norito::core::write_len_with_flags(
        &mut writer,
        u64::try_from(name.len()).map_err(|_| norito::core::Error::LengthMismatch)?,
        flags,
    )?;
    std::io::Write::write_all(&mut writer, name.as_bytes())?;

    let payload_len = core::mem::size_of::<u64>()
        .checked_add(framed_payload_len)
        .ok_or(norito::core::Error::LengthMismatch)?;
    norito::core::write_len_with_flags(
        &mut writer,
        u64::try_from(payload_len).map_err(|_| norito::core::Error::LengthMismatch)?,
        flags,
    )?;
    norito::core::write_seq_len(
        &mut writer,
        u64::try_from(framed_payload_len).map_err(|_| norito::core::Error::LengthMismatch)?,
    )?;
    write_framed_payload(&mut writer, bare_payload, payload_header_flags)?;
    Ok(())
}

fn encoded_instruction_payload(instr: &InstructionBox) -> Option<EncodedInstructionPayload> {
    let inner = peel_instruction_box(&**instr);
    if let Some(opaque) = inner.as_any().downcast_ref::<OpaqueInstruction>() {
        return Some(EncodedInstructionPayload {
            name: opaque.wire_id,
            bare_payload: opaque.framed_payload.clone(),
            framed_payload_len: opaque.framed_payload.len(),
            payload_header_flags: 0,
            write_framed_payload: write_raw_instruction_payload,
        });
    }
    let type_name = Instruction::id(inner);
    let entry = {
        let registry = instruction_registry();
        registry.entry_for_type_name(type_name)?
    };
    let mut bare_payload = Vec::new();
    if let Some(hint) = {
        let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        Instruction::dyn_encode_capacity_hint(inner)
    } {
        bare_payload.reserve_exact(hint);
    }
    let _ = norito::codec::take_last_encode_flags();
    Instruction::dyn_encode_into(inner, &mut bare_payload);
    let payload_header_flags =
        norito::codec::take_last_encode_flags().unwrap_or_else(norito::core::default_encode_flags);
    let framed_payload_len = (entry.frame_len)(bare_payload.len())?;
    Some(EncodedInstructionPayload {
        name: entry.wire_id,
        bare_payload,
        framed_payload_len,
        payload_header_flags,
        write_framed_payload: entry.frame_write,
    })
}

#[cfg(test)]
fn encoded_instruction_pair_payload(instr: &InstructionBox) -> Option<(&'static str, Vec<u8>)> {
    let encoded = encoded_instruction_payload(instr)?;
    let mut payload = Vec::with_capacity(encoded.framed_payload_len);
    (encoded.write_framed_payload)(
        &mut payload,
        &encoded.bare_payload,
        encoded.payload_header_flags,
    )
    .ok()?;
    Some((encoded.name, payload))
}

fn encoded_instruction_pair_len(instr: &InstructionBox) -> Option<usize> {
    let inner = peel_instruction_box(&**instr);
    if let Some(opaque) = inner.as_any().downcast_ref::<OpaqueInstruction>() {
        return encoded_instruction_tuple_len(opaque.wire_id, opaque.framed_payload.len());
    }
    let type_name = Instruction::id(inner);
    let entry = {
        let registry = instruction_registry();
        registry.entry_for_type_name(type_name)?
    };
    let payload_len = {
        let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        Instruction::dyn_encoded_len(inner)?
    };
    let framed_payload_len = (entry.frame_len)(payload_len)?;
    encoded_instruction_tuple_len(entry.wire_id, framed_payload_len)
}

fn encoded_instruction_pair_hint(instr: &InstructionBox) -> Option<usize> {
    let inner = peel_instruction_box(&**instr);
    if let Some(opaque) = inner.as_any().downcast_ref::<OpaqueInstruction>() {
        return encoded_instruction_tuple_len(opaque.wire_id, opaque.framed_payload.len());
    }
    let type_name = Instruction::id(inner);
    let entry = {
        let registry = instruction_registry();
        registry.entry_for_type_name(type_name)?
    };
    let payload_len = {
        let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        Instruction::dyn_encode_capacity_hint(inner)?
    };
    let framed_payload_len = (entry.frame_len)(payload_len)?;
    encoded_instruction_tuple_len(entry.wire_id, framed_payload_len)
}

fn encoded_instruction_tuple_len(name: &str, framed_payload_len: usize) -> Option<usize> {
    let flags = instruction_tuple_flags();
    let name_len =
        norito::core::len_prefix_len_with_flags(name.len(), flags).checked_add(name.len())?;
    let payload_vec_len = core::mem::size_of::<u64>().checked_add(framed_payload_len)?;
    tuple_field_len_with_flags(name_len, flags)?
        .checked_add(tuple_field_len_with_flags(payload_vec_len, flags)?)
}

fn tuple_field_len_with_flags(elem_len: usize, flags: u8) -> Option<usize> {
    let prefix_len = norito::core::len_prefix_len_with_flags(elem_len, flags);
    prefix_len.checked_add(elem_len)
}

fn framed_instruction_payload_len_for<T>(payload_len: usize) -> Option<usize> {
    let align = core::mem::align_of::<norito::core::Archived<T>>();
    let padding = if align <= 1 {
        0
    } else {
        let remainder = norito::core::Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    };
    norito::core::Header::SIZE
        .checked_add(padding)?
        .checked_add(payload_len)
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
        let payload = encoded_instruction_payload(self).ok_or_else(|| {
            norito::core::Error::Message("failed to encode instruction payload".to_owned())
        })?;
        write_instruction_pair_fields(
            writer,
            payload.name,
            payload.framed_payload_len,
            &payload.bare_payload,
            payload.payload_header_flags,
            payload.write_framed_payload,
        )
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        encoded_instruction_pair_len(self).or_else(|| encoded_instruction_pair_hint(self))
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        encoded_instruction_pair_len(self)
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

        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(bytes) = norito::core::payload_slice_from_ptr(ptr) {
            match decode_instruction_pair_fields_from_slice(bytes) {
                Ok((name, payload, used)) if used == bytes.len() => {
                    return match decode_instruction_from_pair(name, payload) {
                        Ok(inst) => inst,
                        Err(err) => {
                            let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
                            let message = truncate_message(err.to_string());
                            InstructionBox::from(crate::isi::transparent::InvalidInstruction::new(
                                name.to_owned(),
                                hash,
                                message,
                            ))
                        }
                    };
                }
                Ok((name, payload, _used)) => {
                    let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
                    let message =
                        truncate_message(instruction_canonical_framing_error().to_string());
                    return InstructionBox::from(crate::isi::transparent::InvalidInstruction::new(
                        name.to_owned(),
                        hash,
                        message,
                    ));
                }
                Err(_) => {}
            }
        }

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
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(bytes) = norito::core::payload_slice_from_ptr(ptr) {
            match decode_instruction_from_borrowed_pair(bytes) {
                Ok((instruction, used)) if used == bytes.len() => return Ok(instruction),
                Ok((_instruction, _used)) => return Err(instruction_canonical_framing_error()),
                Err(_) => {}
            }
        }

        let (name, bytes): (String, Vec<u8>) =
            norito::core::NoritoDeserialize::try_deserialize(archived.cast())?;
        decode_instruction_from_pair(&name, &bytes)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for InstructionBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (inst, used) =
            decode_instruction_from_borrowed_pair(bytes).map_err(|err| match err {
                norito::core::Error::Message(_) => err,
                _ => instruction_canonical_framing_error(),
            })?;
        if used != bytes.len() {
            return Err(instruction_canonical_framing_error());
        }
        norito::core::note_payload_access(bytes, used);
        Ok((inst, used))
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

#[cfg(feature = "json")]
fn instruction_box_from_base64_literal(
    encoded: &str,
) -> Result<InstructionBox, norito::json::Error> {
    let bytes = STANDARD
        .decode(encoded.as_bytes())
        .map_err(|err| norito::json::Error::Message(err.to_string()))?;
    let archived = norito::from_bytes::<InstructionBox>(&bytes)
        .map_err(|err| norito::json::Error::Message(err.to_string()))?;
    norito::core::NoritoDeserialize::try_deserialize(archived)
        .map_err(|err| norito::json::Error::Message(err.to_string()))
}

#[cfg(feature = "json")]
fn json_required_string(map: &norito::json::Map, key: &str) -> Result<String, norito::json::Error> {
    map.get(key)
        .and_then(norito::json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| norito::json::Error::Message(format!("instruction `{key}` is required")))
}

#[cfg(feature = "json")]
fn json_optional_string(map: &norito::json::Map, key: &str) -> Option<String> {
    map.get(key)
        .and_then(norito::json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(feature = "json")]
fn json_required_bool(map: &norito::json::Map, key: &str) -> Result<bool, norito::json::Error> {
    map.get(key)
        .and_then(norito::json::Value::as_bool)
        .ok_or_else(|| norito::json::Error::Message(format!("instruction `{key}` must be a bool")))
}

#[cfg(feature = "json")]
fn json_numeric_opt(
    value: Option<&norito::json::Value>,
) -> Result<Option<iroha_primitives::numeric::Numeric>, norito::json::Error> {
    use std::str::FromStr as _;

    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if let Some(value) = value.as_u64() {
        return Ok(Some(iroha_primitives::numeric::Numeric::from(value)));
    }
    if let Some(value) = value.as_i64() {
        if value < 0 {
            return Err(norito::json::Error::Message(
                "asset transfer cap_amount must be non-negative".to_owned(),
            ));
        }
        return Ok(Some(iroha_primitives::numeric::Numeric::from(
            value.cast_unsigned(),
        )));
    }
    if let Some(value) = value.as_str() {
        let parsed = iroha_primitives::numeric::Numeric::from_str(value.trim())
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        return Ok(Some(parsed));
    }
    Err(norito::json::Error::Message(
        "asset transfer cap_amount must be a string, number, or null".to_owned(),
    ))
}

#[cfg(feature = "json")]
fn instruction_box_from_object(
    map: &norito::json::Map,
) -> Result<InstructionBox, norito::json::Error> {
    use std::str::FromStr as _;

    let name = json_required_string(map, "name")?;
    let params = map
        .get("params")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| {
            norito::json::Error::Message("instruction `params` must be an object".to_owned())
        })?;
    match name.as_str() {
        "SetAssetTransferFreeze" => {
            let account_id = crate::account::AccountId::parse_encoded(
                json_required_string(params, "account_id")?.as_str(),
            )
            .map(crate::account::ParsedAccountId::into_account_id)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            let asset_definition_id = crate::asset::AssetDefinitionId::from_str(
                json_required_string(params, "asset_definition_id")?.as_str(),
            )
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            Ok(
                crate::isi::asset_transfer_control::SetAssetTransferFreeze::new(
                    account_id,
                    asset_definition_id,
                    json_required_bool(params, "outgoing_frozen")?,
                    json_optional_string(params, "reason"),
                )
                .into(),
            )
        }
        "SetAssetTransferBlacklist" => {
            let account_id = crate::account::AccountId::parse_encoded(
                json_required_string(params, "account_id")?.as_str(),
            )
            .map(crate::account::ParsedAccountId::into_account_id)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            let asset_definition_id = crate::asset::AssetDefinitionId::from_str(
                json_required_string(params, "asset_definition_id")?.as_str(),
            )
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            Ok(
                crate::isi::asset_transfer_control::SetAssetTransferBlacklist::new(
                    account_id,
                    asset_definition_id,
                    json_required_bool(params, "blacklisted")?,
                )
                .into(),
            )
        }
        "SetAssetTransferControl" => {
            let account_id = crate::account::AccountId::parse_encoded(
                json_required_string(params, "account_id")?.as_str(),
            )
            .map(crate::account::ParsedAccountId::into_account_id)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            let asset_definition_id = crate::asset::AssetDefinitionId::from_str(
                json_required_string(params, "asset_definition_id")?.as_str(),
            )
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            let limits = params
                .get("limits")
                .and_then(norito::json::Value::as_array)
                .ok_or_else(|| {
                    norito::json::Error::Message("instruction `limits` must be an array".to_owned())
                })?
                .iter()
                .map(|entry| {
                    let entry = entry.as_object().ok_or_else(|| {
                        norito::json::Error::Message(
                            "each asset transfer limit must be an object".to_owned(),
                        )
                    })?;
                    let window = crate::asset::AssetTransferControlWindow::from_str(
                        json_required_string(entry, "window")?.as_str(),
                    )
                    .map_err(|err| norito::json::Error::Message(err.to_string()))?;
                    Ok(crate::asset::AssetTransferLimit {
                        window,
                        cap_amount: json_numeric_opt(entry.get("cap_amount"))?,
                    })
                })
                .collect::<Result<Vec<_>, norito::json::Error>>()?;
            Ok(
                crate::isi::asset_transfer_control::SetAssetTransferControl::new(
                    account_id,
                    asset_definition_id,
                    limits,
                )
                .into(),
            )
        }
        other => Err(norito::json::Error::Message(format!(
            "unsupported structured instruction `{other}`"
        ))),
    }
}

impl norito::json::JsonDeserialize for InstructionBox {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match norito::json::Value::json_deserialize(parser)? {
            norito::json::Value::String(encoded) => instruction_box_from_base64_literal(&encoded),
            norito::json::Value::Object(map) => instruction_box_from_object(&map),
            other => Err(norito::json::Error::Message(format!(
                "instruction JSON must be either a base64 string or an object, found {other:?}"
            ))),
        }
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
    let entry = {
        let registry = instruction_registry();
        registry.entry_for_key(name).copied()
    };
    if let Some(entry) = entry {
        return InstructionRegistry::decode_entry(&entry, 0, payload);
    }
    Err(norito::Error::Message(format!(
        "unknown instruction `{name}` (not registered)"
    )))
}

fn instruction_canonical_framing_error() -> norito::Error {
    norito::Error::Message("instruction payload must use canonical Norito framing".to_owned())
}

fn decode_instruction_pair_fields_from_slice(
    bytes: &[u8],
) -> Result<(&str, &[u8], usize), norito::Error> {
    let flags =
        norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags);
    let (name_field_len, name_hdr) = norito::core::read_len_from_slice_with_flags(bytes, flags)?;
    let name_field_start = name_hdr;
    let name_field_end = name_field_start
        .checked_add(name_field_len)
        .ok_or(norito::Error::LengthMismatch)?;
    let name_field = bytes
        .get(name_field_start..name_field_end)
        .ok_or(norito::Error::LengthMismatch)?;

    let (name_len, inner_name_hdr) =
        norito::core::read_len_from_slice_with_flags(name_field, flags)?;
    let name_start = inner_name_hdr;
    let name_end = name_start
        .checked_add(name_len)
        .ok_or(norito::Error::LengthMismatch)?;
    if name_end != name_field.len() {
        return Err(norito::Error::LengthMismatch);
    }
    let name = core::str::from_utf8(
        name_field
            .get(name_start..name_end)
            .ok_or(norito::Error::LengthMismatch)?,
    )
    .map_err(|_| norito::Error::InvalidUtf8)?;

    let payload_field_prefix = bytes
        .get(name_field_end..)
        .ok_or(norito::Error::LengthMismatch)?;
    let (payload_field_len, payload_hdr) =
        norito::core::read_len_from_slice_with_flags(payload_field_prefix, flags)?;
    let payload_field_start = name_field_end
        .checked_add(payload_hdr)
        .ok_or(norito::Error::LengthMismatch)?;
    let payload_field_end = payload_field_start
        .checked_add(payload_field_len)
        .ok_or(norito::Error::LengthMismatch)?;
    let payload_field = bytes
        .get(payload_field_start..payload_field_end)
        .ok_or(norito::Error::LengthMismatch)?;

    let (payload_len, payload_inner_hdr) = norito::core::read_seq_len_slice(payload_field)?;
    let payload_start = payload_inner_hdr;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or(norito::Error::LengthMismatch)?;
    if payload_end != payload_field.len() {
        return Err(norito::Error::LengthMismatch);
    }
    let payload = payload_field
        .get(payload_start..payload_end)
        .ok_or(norito::Error::LengthMismatch)?;
    Ok((name, payload, payload_field_end))
}

fn decode_instruction_from_borrowed_pair(
    bytes: &[u8],
) -> Result<(InstructionBox, usize), norito::Error> {
    let (name, payload, used) = decode_instruction_pair_fields_from_slice(bytes)?;
    let instruction = decode_instruction_from_pair(name, payload)?;
    Ok((instruction, used))
}

/// Frame a bare instruction payload with its Norito header using the registry metadata.
///
/// # Errors
/// Returns `norito::Error` if the type name is not registered or framing fails for the payload.
pub fn frame_instruction_payload(
    type_name: &str,
    payload: &[u8],
) -> Result<Vec<u8>, norito::Error> {
    let entry = {
        let registry = instruction_registry();
        registry.entry_for_key(type_name).copied()
    };
    if let Some(entry) = entry {
        let header_flags = norito::codec::take_last_encode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        return (entry.frame)(payload, header_flags);
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
    /// Lookup table mapping either `type_name` or wire-id -> registry entry.
    lookup: HashMap<&'static str, RegistryEntry>,
}

#[derive(Clone, Copy)]
struct RegistryEntry {
    ctor: InstructionConstructor,
    wire_id: &'static str,
    frame: fn(&[u8], u8) -> Result<Vec<u8>, norito::core::Error>,
    frame_write: InstructionPayloadFrameWriter,
    frame_len: fn(usize) -> Option<usize>,
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
        T: Instruction + Decode + 'static + norito::NoritoSerialize,
        for<'a> T: norito::NoritoDeserialize<'a>,
    {
        fn ctor<T>(header_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction + Decode + 'static + norito::NoritoSerialize,
            for<'a> T: norito::NoritoDeserialize<'a>,
        {
            decode_instruction_payload::<T>(input, header_flags)
        }
        fn frame<T>(payload: &[u8], header_flags: u8) -> Result<Vec<u8>, norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::frame_bare_with_header_flags::<T>(payload, header_flags)
        }
        fn frame_write<T>(
            writer: &mut dyn std::io::Write,
            payload: &[u8],
            header_flags: u8,
        ) -> Result<(), norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::write_bare_frame_with_header_flags::<T, _>(writer, payload, header_flags)
        }
        fn frame_len<T>(payload_len: usize) -> Option<usize>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            framed_instruction_payload_len_for::<T>(payload_len)
        }
        let name = std::any::type_name::<T>();
        let entry = RegistryEntry {
            ctor: ctor::<T>,
            wire_id: name,
            frame: frame::<T>,
            frame_write: frame_write::<T>,
            frame_len: frame_len::<T>,
        };
        if let Some(previous) = self.entries.insert(name, entry)
            && previous.wire_id != entry.wire_id
        {
            self.lookup.remove(previous.wire_id);
        }
        self.lookup.insert(name, entry);
        self.lookup.insert(entry.wire_id, entry);
        self
    }

    /// Register a new [`crate::isi::Instruction`] type using a stable wire identifier.
    #[must_use]
    pub fn register_with_id<T>(mut self, wire_id: &'static str) -> Self
    where
        T: Instruction + Decode + 'static + norito::NoritoSerialize,
        for<'a> T: norito::NoritoDeserialize<'a>,
    {
        fn ctor<T>(header_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction + Decode + 'static + norito::NoritoSerialize,
            for<'a> T: norito::NoritoDeserialize<'a>,
        {
            decode_instruction_payload::<T>(input, header_flags)
        }
        fn frame<T>(payload: &[u8], header_flags: u8) -> Result<Vec<u8>, norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::frame_bare_with_header_flags::<T>(payload, header_flags)
        }
        fn frame_write<T>(
            writer: &mut dyn std::io::Write,
            payload: &[u8],
            header_flags: u8,
        ) -> Result<(), norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::write_bare_frame_with_header_flags::<T, _>(writer, payload, header_flags)
        }
        fn frame_len<T>(payload_len: usize) -> Option<usize>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            framed_instruction_payload_len_for::<T>(payload_len)
        }
        let name = std::any::type_name::<T>();
        let entry = RegistryEntry {
            ctor: ctor::<T>,
            wire_id,
            frame: frame::<T>,
            frame_write: frame_write::<T>,
            frame_len: frame_len::<T>,
        };
        if let Some(previous) = self.entries.insert(name, entry)
            && previous.wire_id != entry.wire_id
        {
            self.lookup.remove(previous.wire_id);
        }
        self.lookup.insert(name, entry);
        self.lookup.insert(wire_id, entry);
        self
    }

    /// Register a new [`crate::isi::Instruction`] type using the direct slice decoder.
    ///
    /// This is intentionally opt-in because not every built-in instruction has a
    /// slice-safe decoder for all of its nested fields yet.
    #[must_use]
    pub(crate) fn register_slice<T>(mut self) -> Self
    where
        T: Instruction + Decode + 'static + norito::NoritoSerialize,
        for<'a> T: norito::NoritoDeserialize<'a> + norito::core::DecodeFromSlice<'a>,
    {
        fn ctor<T>(header_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction + Decode + 'static + norito::NoritoSerialize,
            for<'a> T: norito::NoritoDeserialize<'a> + norito::core::DecodeFromSlice<'a>,
        {
            decode_instruction_payload_from_slice::<T>(input, header_flags)
        }
        fn frame<T>(payload: &[u8], header_flags: u8) -> Result<Vec<u8>, norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::frame_bare_with_header_flags::<T>(payload, header_flags)
        }
        fn frame_write<T>(
            writer: &mut dyn std::io::Write,
            payload: &[u8],
            header_flags: u8,
        ) -> Result<(), norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::write_bare_frame_with_header_flags::<T, _>(writer, payload, header_flags)
        }
        fn frame_len<T>(payload_len: usize) -> Option<usize>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            framed_instruction_payload_len_for::<T>(payload_len)
        }
        let name = std::any::type_name::<T>();
        let entry = RegistryEntry {
            ctor: ctor::<T>,
            wire_id: name,
            frame: frame::<T>,
            frame_write: frame_write::<T>,
            frame_len: frame_len::<T>,
        };
        if let Some(previous) = self.entries.insert(name, entry)
            && previous.wire_id != entry.wire_id
        {
            self.lookup.remove(previous.wire_id);
        }
        self.lookup.insert(name, entry);
        self.lookup.insert(entry.wire_id, entry);
        self
    }

    /// Register a new [`crate::isi::Instruction`] type using a stable wire identifier
    /// and the direct slice decoder.
    ///
    /// This is intentionally opt-in because not every built-in instruction has a
    /// slice-safe decoder for all of its nested fields yet.
    #[must_use]
    pub(crate) fn register_with_id_slice<T>(mut self, wire_id: &'static str) -> Self
    where
        T: Instruction + Decode + 'static + norito::NoritoSerialize,
        for<'a> T: norito::NoritoDeserialize<'a> + norito::core::DecodeFromSlice<'a>,
    {
        fn ctor<T>(header_flags: u8, input: &[u8]) -> Result<InstructionBox, norito::Error>
        where
            T: Instruction + Decode + 'static + norito::NoritoSerialize,
            for<'a> T: norito::NoritoDeserialize<'a> + norito::core::DecodeFromSlice<'a>,
        {
            decode_instruction_payload_from_slice::<T>(input, header_flags)
        }
        fn frame<T>(payload: &[u8], header_flags: u8) -> Result<Vec<u8>, norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::frame_bare_with_header_flags::<T>(payload, header_flags)
        }
        fn frame_write<T>(
            writer: &mut dyn std::io::Write,
            payload: &[u8],
            header_flags: u8,
        ) -> Result<(), norito::core::Error>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            norito::core::write_bare_frame_with_header_flags::<T, _>(writer, payload, header_flags)
        }
        fn frame_len<T>(payload_len: usize) -> Option<usize>
        where
            T: Instruction
                + Decode
                + 'static
                + norito::NoritoSerialize
                + for<'a> norito::NoritoDeserialize<'a>,
        {
            framed_instruction_payload_len_for::<T>(payload_len)
        }
        let name = std::any::type_name::<T>();
        let entry = RegistryEntry {
            ctor: ctor::<T>,
            wire_id,
            frame: frame::<T>,
            frame_write: frame_write::<T>,
            frame_len: frame_len::<T>,
        };
        if let Some(previous) = self.entries.insert(name, entry)
            && previous.wire_id != entry.wire_id
        {
            self.lookup.remove(previous.wire_id);
        }
        self.lookup.insert(name, entry);
        self.lookup.insert(wire_id, entry);
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

    fn entry_for_type_name(&self, type_name: &'static str) -> Option<RegistryEntry> {
        self.entries.get(type_name).copied()
    }

    fn entry_for_key(&self, key: &str) -> Option<&RegistryEntry> {
        self.lookup.get(key)
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

fn decode_instruction_payload_from_slice<T>(
    input: &[u8],
    header_flags: u8,
) -> Result<InstructionBox, norito::Error>
where
    T: Instruction + Decode + 'static + norito::NoritoSerialize,
    for<'a> T: norito::NoritoDeserialize<'a> + norito::core::DecodeFromSlice<'a>,
{
    let _ = header_flags;
    let instruction = norito::core::decode_from_bytes::<T>(input)?;
    Ok(InstructionBox(Box::new(instruction)))
}

pub(crate) fn read_aos_field<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    flags: u8,
) -> Result<&'a [u8], norito::core::Error> {
    let remaining = bytes
        .get(*offset..)
        .ok_or(norito::core::Error::LengthMismatch)?;
    let (field_len, hdr) = norito::core::read_len_from_slice_with_flags(remaining, flags)?;
    let field_start = (*offset)
        .checked_add(hdr)
        .ok_or(norito::core::Error::LengthMismatch)?;
    let field_end = field_start
        .checked_add(field_len)
        .ok_or(norito::core::Error::LengthMismatch)?;
    let field = bytes
        .get(field_start..field_end)
        .ok_or(norito::core::Error::LengthMismatch)?;
    *offset = field_end;
    Ok(field)
}

pub(crate) fn decode_aos_canonical_field<T>(
    field: &[u8],
    flags: u8,
) -> Result<T, norito::core::Error>
where
    T: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let (value, used) = norito::core::decode_field_canonical::<T>(field)?;
    if used != field.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    Ok(value)
}

pub(crate) fn decode_aos_slice_field<T>(field: &[u8], flags: u8) -> Result<T, norito::core::Error>
where
    T: for<'de> norito::core::NoritoDeserialize<'de> + for<'de> norito::core::DecodeFromSlice<'de>,
{
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let (value, used) = norito::core::decode_field_canonical_from_slice::<T>(field)?;
    if used != field.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    Ok(value)
}

pub(crate) fn decode_packed_instruction_payload<T>(
    bytes: &[u8],
) -> Result<(T, usize), norito::core::Error>
where
    T: norito::codec::Decode,
{
    let _guard = norito::core::PayloadCtxGuard::enter(bytes);
    let mut cursor = std::io::Cursor::new(bytes);
    let decoded = <T as norito::codec::Decode>::decode(&mut cursor)?;
    let used =
        usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
    if used != bytes.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    norito::core::note_payload_access(bytes, used);
    Ok((decoded, used))
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
    Global(std::sync::RwLockReadGuard<'static, Arc<InstructionRegistry>>),
    #[cfg(test)]
    Local(Arc<InstructionRegistry>),
}

impl std::ops::Deref for InstructionRegistryReadGuard {
    type Target = InstructionRegistry;

    fn deref(&self) -> &InstructionRegistry {
        match self {
            Self::Global(registry) => {
                let registry: &Arc<InstructionRegistry> = registry;
                registry.as_ref()
            }
            #[cfg(test)]
            Self::Local(registry) => registry.as_ref(),
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
    InstructionRegistryReadGuard::Global(registry)
}

macro_rules! isi {
    ($(#[$meta:meta])* pub struct $name:ident $($rest:tt)*) => {
        iroha_data_model_derive::model_single! {
            #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
            #[derive(getset::Getters)]
            #[derive(Decode, Encode)]
            #[derive(iroha_schema::IntoSchema)]
            #[getset(get = "pub")]
            $(#[$meta])*
            pub struct $name $($rest)*
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

/// Canonical paid account-alias lease instructions.
pub mod account_alias_lease;
/// Native account controller replacement and social recovery instructions.
pub mod account_recovery;
/// Asset-definition alias binding instructions.
pub mod asset_alias;
/// Asset-scoped outbound transfer control instructions.
pub mod asset_transfer_control;
/// Confidential registry management instructions.
/// Bridge proof ingestion instructions.
pub mod bridge;
/// Confidential registry management instructions.
pub mod confidential;
/// Content lane instructions.
pub mod content;
/// Contract alias binding instructions.
pub mod contract_alias;
/// Account subject and domain link instructions.
pub mod domain_link;
/// Ledger-managed asset escrow instructions.
pub mod escrow;
/// Hidden-function-backed identifier policy instructions.
pub mod identifier;
/// Kaigi collaboration instructions.
pub mod kaigi;
/// Mint and burn instruction variants and helpers.
pub mod mint_burn;
/// Musubi package registry instructions.
pub mod musubi;
/// Nexus lane governance instructions.
pub mod nexus;
/// Offline allowance settlement instructions.
pub mod offline;
/// Oracle feed registration and aggregation instructions.
pub mod oracle;
/// Generic RAM-LFE program-policy instructions.
pub mod ram_lfe;
/// Registration-related instructions (accounts, assets, domains, etc.).
pub mod register;
/// Instruction registries shared across instruction families.
pub mod registry;
/// Repo settlement instructions.
pub mod repo;
/// Runtime upgrade instructions and payloads.
pub mod runtime_upgrade;
/// Real-world asset lot instructions.
pub mod rwa;
/// DvP/PvP settlement instructions.
pub mod settlement;
/// Smart contract code management instructions.
pub mod smart_contract_code;
/// Consensus-backed SNS mutation instructions.
pub mod sns;
/// Viral incentive and social reward instructions.
pub mod social;
/// Soracloud lifecycle and runtime-state instructions.
pub mod soracloud;
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
/// `SoraNet` VPN lease escrow instructions.
pub mod vpn;
/// Zero-knowledge instruction wrappers.
pub mod zk;

pub use account_alias_lease::*;
pub use account_recovery::*;
pub use asset_alias::*;
pub use asset_transfer_control::*;
pub use confidential::*;
pub use contract_alias::*;
pub use domain_link::*;
pub use identifier::*;
pub use kaigi::*;
pub use ministry::*;
pub use mint_burn::*;
pub use nexus::*;
pub use offline::*;
pub use oracle::*;
pub use ram_lfe::*;
pub use register::*;
pub use repo::*;
pub use settlement::*;
pub use sns::*;
pub use soradns::*;
pub use sorafs::*;
pub use space_directory::*;
pub use staking::*;
pub use transfer::*;
pub use transparent::*;
pub use vpn::*;
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
            /// Implicit account creation is disabled.
            ImplicitAccountCreationDisabled,
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

        /// Account admission policy payload is invalid: {reason}.
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
        Instruction, InstructionBox, Log, Mint, MintBox, OpenOracleDispute, ProposeOracleChange,
        RecordTwitterBinding, Register, RegisterBox, RegisterOracleFeed, RemoveKeyValue,
        RemoveKeyValueBox, ResolveOracleDispute, Revoke, RevokeBox, RevokeTwitterBinding,
        RollbackOracleChange, SetKeyValue, SetKeyValueBox, SetParameter, SubmitOracleObservation,
        Transfer, TransferAssetBatch, TransferAssetBatchEntry, TransferBox, Unregister,
        UnregisterBox, Upgrade, VoteOracleChangeStage,
        account_alias_lease::{AcquireAccountAliasLease, RenewAccountAliasLease},
        account_recovery::{
            ApproveAccountRecovery, CancelAccountRecovery, ClearAccountRecoveryPolicy,
            FinalizeAccountRecovery, ProposeAccountRecovery, ReplaceAccountController,
            SetAccountRecoveryPolicy,
        },
        asset_transfer_control::{
            SetAssetTransferBlacklist, SetAssetTransferControl, SetAssetTransferFreeze,
        },
        bridge::{RecordBridgeReceipt, RecordSccpMessage, SubmitBridgeProof},
        confidential::{
            PublishPedersenParams, PublishPoseidonParams, SetPedersenParamsLifecycle,
            SetPoseidonParamsLifecycle,
        },
        consensus_keys::{DisableConsensusKey, RegisterConsensusKey, RotateConsensusKey},
        content::{PublishContentBundle, RetireContentBundle},
        contract_alias::SetContractAlias,
        domain_link::{SetAccountAliasBinding, SetPrimaryAccountAlias},
        endorsement::{
            RegisterDomainCommittee, SetDomainEndorsementPolicy, SubmitDomainEndorsement,
        },
        escrow::{
            AcceptAnonymousAssetEscrow, AcceptAssetEscrow, CancelAnonymousAssetEscrow,
            CancelAssetEscrow, MarkAnonymousEscrowPaymentSent, MarkEscrowPaymentSent,
            OpenAnonymousAssetEscrow, OpenAnonymousEscrowDispute, OpenAssetEscrow,
            OpenEscrowDispute, ReleaseAnonymousAssetEscrow, ReleaseAssetEscrow,
            ResolveAnonymousEscrowDispute, ResolveEscrowDispute,
        },
        identifier::{
            ActivateIdentifierPolicy, ClaimIdentifier, RegisterIdentifierPolicy, RevokeIdentifier,
        },
        ministry::SubmitAgendaProposal,
        nexus::{RegisterVerifiedLaneRelay, SetLaneRelayEmergencyValidators},
        ram_lfe::{
            ActivateRamLfeProgramPolicy, DeactivateRamLfeProgramPolicy, RegisterRamLfeProgramPolicy,
        },
        repo::{RepoInstructionBox, RepoIsi, ReverseRepoIsi},
        rwa::{
            ForceTransferRwa, FreezeRwa, HoldRwa, MergeRwas, RedeemRwa, RegisterRwa, ReleaseRwa,
            RwaInstructionBox, SetRwaControls, TransferRwa, UnfreezeRwa,
        },
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementFailureRecord,
            SettlementInstructionBox, SettlementKind, SettlementLedger, SettlementLedgerEntry,
            SettlementLeg, SettlementLegRole, SettlementLegSnapshot, SettlementOutcomeRecord,
            SettlementPlan, SettlementSuccessRecord,
        },
        sns::{
            FreezeSnsName, RegisterSnsName, RenewSnsName, TransferSnsName, UnfreezeSnsName,
            UpdateSnsNameControllers,
        },
        social::{CancelTwitterEscrow, ClaimTwitterFollowReward, SendToTwitter},
        soracloud::{
            AdvanceSoracloudRollout, DeploySoracloudService, MutateSoracloudState,
            RecordSoracloudAgentAutonomyExecution, RecordSoracloudDecryptionRequest,
            RecordSoracloudMailboxMessage, RecordSoracloudPrivateUploadedModelExecutionReceipt,
            RecordSoracloudRuntimeReceipt, ReportSoracloudServiceLeaseUsage,
            RollbackSoracloudService, RunSoracloudFheJob, SetSoracloudRuntimeState,
            UpgradeSoracloudService,
        },
        soradns::{
            AddReleaseSigner, PublishDirectory, RemoveReleaseSigner, RevokeResolver,
            SetDirectoryRotationPolicy, SubmitDirectoryDraft, UnrevokeResolver,
        },
        sorafs::{
            ApprovePinManifest, BindManifestAlias, CompleteReplicationOrder, IssueReplicationOrder,
            RecordCapacityTelemetry, RegisterCapacityDeclaration, RegisterCapacityDispute,
            RegisterPinManifest, RetirePinManifest, SetPricingSchedule, UpsertProviderCredit,
        },
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
        staking::{
            ActivatePublicLaneValidator, BondPublicLaneStake, CancelConsensusEvidencePenalty,
            ClaimPublicLaneRewards, ExitPublicLaneValidator, FinalizePublicLaneUnbond,
            RebindPublicLaneValidatorPeer, RecordPublicLaneRewards, RegisterPublicLaneValidator,
            SchedulePublicLaneUnbond, SlashPublicLaneValidator,
        },
        vpn::{OpenVpnLeaseEscrow, RefundExpiredVpnLease, SettleVpnLease},
    };
}

#[cfg(test)]
mod tests {
    use iroha_primitives::const_vec::ConstVec;
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::prelude::*;
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

    fn test_domain_id() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("domain id")
    }

    fn framed_instruction_payload<T>(value: &T) -> Vec<u8>
    where
        T: Instruction + norito::codec::Encode + 'static + norito::core::NoritoSerialize,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(value);
        norito::core::frame_bare_with_header_flags::<T>(&payload, flags)
            .expect("frame instruction payload")
    }

    fn bare_instruction_pair(name: &str, framed_payload: Vec<u8>) -> Vec<u8> {
        let mut bytes = Vec::new();
        norito::core::NoritoSerialize::serialize(&(name.to_owned(), framed_payload), &mut bytes)
            .expect("serialize instruction pair");
        bytes
    }

    fn framed_instruction_pair(name: &str, framed_payload: Vec<u8>) -> Vec<u8> {
        norito::core::to_bytes(&(name.to_owned(), framed_payload))
            .expect("serialize framed instruction pair")
    }

    #[test]
    fn aa_setup_instruction_registry() {
        let _guard = RegistryGuard::set(instruction_registry![Log]);
    }

    #[test]
    fn register_and_decode_instruction() {
        let registry = InstructionRegistry::new().register_slice::<Log>();
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
        let (payload, flags) = norito::codec::encode_with_header_flags(&instruction);
        let bytes = norito::core::frame_bare_with_header_flags::<Log>(&payload, flags)
            .expect("frame instruction payload");
        // Use the decode API directly to ensure local registry wiring works
        let decoded = InstructionRegistry::decode(&registry, name, &bytes)
            .expect("constructor not found in decode map")
            .expect("failed to decode");
        // Verify type id and payload equivalence without relying on downcast
        assert_eq!(Instruction::id(&*decoded), name);
        assert_eq!(Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn decode_unregistered_instruction() {
        let registry = InstructionRegistry::new();
        assert!(registry.decode("missing", &[]).is_none());
    }

    #[test]
    fn record_sccp_message_registry_roundtrip_preserves_payload_bytes() {
        let registry = InstructionRegistry::new().register_slice::<RecordSccpMessage>();
        let _guard = RegistryGuard::set(registry);
        let instruction = RecordSccpMessage::new(vec![0xAA, 0xBB, 0xCC]);
        let (bytes, expected_flags) = norito::codec::encode_with_header_flags(&instruction);
        let framed = frame_instruction_payload(std::any::type_name::<RecordSccpMessage>(), &bytes)
            .expect("record sccp message must frame");
        let view = norito::core::from_bytes_view(&framed).expect("framed instruction payload");
        assert_eq!(view.flags(), expected_flags);
        let decoded =
            decode_instruction_from_pair(std::any::type_name::<RecordSccpMessage>(), &framed)
                .expect("record sccp message must decode");
        let decoded = decoded
            .as_any()
            .downcast_ref::<RecordSccpMessage>()
            .expect("decoded instruction type");
        assert_eq!(decoded.payload_bytes, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn registry_decode_accepts_misaligned_framed_payload() {
        let registry = InstructionRegistry::new().register_slice::<Log>();
        let name = std::any::type_name::<Log>();
        let instruction = Log::new(Level::INFO, "misaligned framed payload".to_owned());
        let payload = instruction.encode();
        let framed = frame_instruction_payload(name, &payload).expect("frame instruction payload");
        let mut misaligned = Vec::with_capacity(framed.len() + 1);
        misaligned.push(0xAA);
        misaligned.extend_from_slice(&framed);

        let decoded = InstructionRegistry::decode(&registry, name, &misaligned[1..])
            .expect("constructor not found in decode map")
            .expect("decode misaligned framed payload");

        assert_eq!(Instruction::id(&*decoded), name);
        assert_eq!(Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn record_sccp_registry_decode_accepts_misaligned_framed_payload() {
        let registry = InstructionRegistry::new().register_slice::<RecordSccpMessage>();
        let name = std::any::type_name::<RecordSccpMessage>();
        let instruction = RecordSccpMessage::new(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        let payload = instruction.encode();
        let framed = frame_instruction_payload(name, &payload).expect("frame instruction payload");
        let mut misaligned = Vec::with_capacity(framed.len() + 1);
        misaligned.push(0xAA);
        misaligned.extend_from_slice(&framed);

        let decoded = InstructionRegistry::decode(&registry, name, &misaligned[1..])
            .expect("constructor not found in decode map")
            .expect("decode misaligned framed payload");

        assert_eq!(Instruction::id(&*decoded), name);
        assert_eq!(Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn instruction_box_embeds_instruction_payload_with_recorded_flags() {
        let registry = InstructionRegistry::new().register_slice::<RecordSccpMessage>();
        let _guard = RegistryGuard::set(registry);
        let instruction = RecordSccpMessage::new(vec![0xAA, 0xBB, 0xCC]);
        let (_, expected_flags) = norito::codec::encode_with_header_flags(&instruction);
        let boxed = InstructionBox::from(instruction);

        let (_, framed_payload) =
            super::encoded_instruction_pair_payload(&boxed).expect("instruction pair payload");

        let view =
            norito::core::from_bytes_view(&framed_payload).expect("framed instruction payload");
        assert_eq!(view.flags(), expected_flags);
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
    fn dyn_encode_into_matches_dyn_encode() {
        let log = Log {
            level: Level::INFO,
            msg: "stream encode".to_string(),
        };
        let expected = Instruction::dyn_encode(&log);
        let mut actual = Vec::with_capacity(
            Instruction::dyn_encode_capacity_hint(&log).expect("encode capacity hint"),
        );

        Instruction::dyn_encode_into(&log, &mut actual);

        assert_eq!(actual, expected);
    }

    #[test]
    fn opaque_instruction_dyn_encode_into_appends_bare_payload() {
        let opaque = OpaqueInstruction {
            wire_id: "opaque/test",
            bare_payload: vec![0xAA, 0xBB, 0xCC],
            framed_payload: vec![0xF0, 0xF1, 0xF2],
        };
        let mut actual = vec![0x11];

        Instruction::dyn_encode_into(&opaque, &mut actual);

        assert_eq!(actual, vec![0x11, 0xAA, 0xBB, 0xCC]);
        assert_eq!(
            Instruction::dyn_encode_capacity_hint(&opaque),
            Some(opaque.bare_payload.len())
        );
    }

    #[test]
    fn opaque_instruction_serializes_preserved_framed_payload() {
        let log = Log::new(Level::INFO, "opaque preserve".to_owned());
        let wire_id = "opaque/test";
        let bare_payload = Instruction::dyn_encode(&log);
        let framed_payload = norito::core::frame_bare_with_header_flags::<Log>(
            &bare_payload,
            norito::core::default_encode_flags(),
        )
        .expect("frame payload");
        let opaque = InstructionBox::from(
            OpaqueInstruction::from_framed(wire_id, &framed_payload).expect("opaque payload"),
        );

        assert_eq!(Instruction::dyn_encode(&*opaque), bare_payload);
        assert_eq!(
            opaque
                .as_any()
                .downcast_ref::<OpaqueInstruction>()
                .expect("opaque")
                .framed_payload(),
            framed_payload.as_slice()
        );
        assert_eq!(
            norito::to_bytes(&opaque).expect("encode opaque"),
            norito::to_bytes(&(wire_id.to_owned(), framed_payload)).expect("encode pair"),
        );
    }

    #[test]
    fn opaque_instruction_rejects_unframed_or_malformed_payloads() {
        let log = Log::new(Level::INFO, "opaque reject raw".to_owned());
        let raw_payload = Instruction::dyn_encode(&log);

        assert!(
            OpaqueInstruction::from_framed("opaque/raw", &raw_payload).is_err(),
            "opaque instructions must carry a Norito-framed payload"
        );
        assert!(
            OpaqueInstruction::from_framed("opaque/short", &[0x01, 0x02]).is_err(),
            "short garbage must not be accepted as a framed opaque payload"
        );
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
    fn instruction_box_direct_serialize_matches_tuple_wire_layout() {
        let _guard = RegistryGuard::set(instruction_registry![Log]);
        let boxed = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "tuple layout".to_string(),
        });

        for flags in [0, norito::core::default_encode_flags()] {
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            let (name, payload) =
                super::encoded_instruction_pair_payload(&boxed).expect("instruction pair payload");
            let expected_pair = (name.to_owned(), payload);

            let mut expected = Vec::new();
            norito::core::NoritoSerialize::serialize(&expected_pair, &mut expected)
                .expect("serialize expected tuple");

            let mut actual = Vec::new();
            norito::core::NoritoSerialize::serialize(&boxed, &mut actual)
                .expect("serialize instruction box");

            assert_eq!(actual, expected, "flags=0x{flags:02x}");
        }
    }

    #[test]
    fn instruction_box_encoded_len_exact_matches_norito() {
        let boxed = InstructionBox::from(Log {
            level: Level::INFO,
            msg: "exact length".to_string(),
        });
        let expected = norito::core::to_bytes(&boxed)
            .expect("serialize instruction box")
            .len()
            - norito::core::Header::SIZE;

        assert_eq!(
            norito::core::NoritoSerialize::encoded_len_exact(&boxed)
                .expect("instruction box exact len"),
            expected
        );
    }

    #[test]
    fn instruction_box_len_hint_does_not_force_exact_inner_len() {
        let boxed = InstructionBox::from(CustomInstruction::new("custom length hint"));
        let exact = norito::core::NoritoSerialize::encoded_len_exact(&boxed);
        let hint = norito::core::NoritoSerialize::encoded_len_hint(&boxed)
            .expect("instruction box length hint");
        let actual = norito::core::to_bytes(&boxed)
            .expect("serialize instruction box")
            .len()
            - norito::core::Header::SIZE;

        assert!(
            exact.is_none() || exact == Some(actual),
            "exact length must be absent or byte-accurate"
        );
        assert!(hint >= actual, "length hint must not under-reserve");
    }

    #[test]
    fn norito_roundtrip_trait_object_deserialize() {
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
    fn borrowed_instruction_pair_decodes_without_owned_payload() {
        let _guard = RegistryGuard::set(instruction_registry![Log]);
        let expected = InstructionBox::from(Log::new(Level::INFO, "borrowed pair".to_owned()));
        let mut bytes = Vec::new();
        norito::core::NoritoSerialize::serialize(&expected, &mut bytes)
            .expect("serialize instruction box tuple");

        let (decoded, used) =
            super::decode_instruction_from_borrowed_pair(&bytes).expect("borrowed pair decode");

        assert_eq!(used, bytes.len());
        assert_eq!(Instruction::id(&*decoded), Instruction::id(&*expected));
        assert_eq!(
            Instruction::dyn_encode(&*decoded),
            Instruction::dyn_encode(&*expected)
        );
    }

    #[test]
    fn instruction_box_decode_from_slice_accepts_misaligned_borrowed_pair() {
        use norito::core::DecodeFromSlice;

        let _guard = RegistryGuard::set(instruction_registry![Log]);
        let expected = InstructionBox::from(Log::new(Level::INFO, "misaligned pair".to_owned()));
        let mut bytes = vec![0xAA];
        norito::core::NoritoSerialize::serialize(&expected, &mut bytes)
            .expect("serialize instruction box tuple");

        let (decoded, used) =
            InstructionBox::decode_from_slice(&bytes[1..]).expect("decode misaligned pair");

        assert_eq!(used, bytes.len() - 1);
        assert_eq!(Instruction::id(&*decoded), Instruction::id(&*expected));
        assert_eq!(
            Instruction::dyn_encode(&*decoded),
            Instruction::dyn_encode(&*expected)
        );
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
    fn instruction_box_lossy_deserialize_maps_malformed_pair_payload_to_invalid_instruction() {
        let malformed_pair_payload = vec![0xFF; 32];
        let framed = norito::core::frame_bare_with_header_flags::<InstructionBox>(
            &malformed_pair_payload,
            norito::core::default_encode_flags(),
        )
        .expect("frame malformed instruction-box payload");
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed).expect("instruction-box frame");

        assert!(
            norito::core::NoritoDeserialize::try_deserialize(archived).is_err(),
            "strict decode must reject malformed pair payloads"
        );

        let decoded: InstructionBox = norito::core::NoritoDeserialize::deserialize(archived);
        let invalid = decoded
            .as_any()
            .downcast_ref::<transparent::InvalidInstruction>()
            .expect("malformed pair becomes invalid placeholder");

        assert_eq!(invalid.wire_id, "<norito>");
        assert_eq!(invalid.payload_hash, [0; 32]);
        assert!(
            invalid.message.len() <= 256,
            "Norito tuple decode error should be bounded, got {} bytes",
            invalid.message.len()
        );
    }

    #[test]
    fn instruction_box_decoders_reject_registered_wire_id_with_unframed_payload() {
        use norito::core::DecodeFromSlice;

        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let malformed_payload = vec![0x01, 0x02, 0x03];
        let framed_pair = framed_instruction_pair(Log::WIRE_ID, malformed_payload.clone());
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed_pair).expect("instruction pair");

        assert!(
            norito::core::NoritoDeserialize::try_deserialize(archived).is_err(),
            "strict decode must reject unframed payload bytes for registered wire ids"
        );

        let bare_pair = bare_instruction_pair(Log::WIRE_ID, malformed_payload.clone());
        assert!(
            InstructionBox::decode_from_slice(&bare_pair).is_err(),
            "borrowed-pair decode must reject unframed payload bytes for registered wire ids"
        );

        let decoded: InstructionBox = norito::core::NoritoDeserialize::deserialize(archived);
        let invalid = decoded
            .as_any()
            .downcast_ref::<transparent::InvalidInstruction>()
            .expect("malformed registered payload becomes invalid placeholder");
        let expected_hash: [u8; 32] = iroha_crypto::Hash::new(&malformed_payload).into();

        assert_eq!(invalid.wire_id, Log::WIRE_ID);
        assert_eq!(invalid.payload_hash, expected_hash);
    }

    #[test]
    fn instruction_box_strict_decoders_reject_removed_direct_instruction_pairs() {
        use norito::core::DecodeFromSlice;

        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let direct_register = Register::domain(Domain::new(test_domain_id()));
        let direct_repo = repo::RepoMarginCallIsi::new(
            "instruction_box_removed_direct"
                .parse()
                .expect("repo agreement id"),
        );

        for (removed_name, framed_payload) in [
            (
                std::any::type_name::<Register<Domain>>(),
                framed_instruction_payload(&direct_register),
            ),
            (
                repo::RepoMarginCallIsi::WIRE_ID,
                framed_instruction_payload(&direct_repo),
            ),
        ] {
            let framed_pair = framed_instruction_pair(removed_name, framed_payload.clone());
            let archived = norito::core::from_bytes::<InstructionBox>(&framed_pair)
                .expect("instruction pair bytes");
            assert!(
                norito::core::NoritoDeserialize::try_deserialize(archived).is_err(),
                "{removed_name} must be rejected by strict InstructionBox deserialization"
            );

            let bare_pair = bare_instruction_pair(removed_name, framed_payload);
            assert!(
                InstructionBox::decode_from_slice(&bare_pair).is_err(),
                "{removed_name} must be rejected by canonical borrowed-pair decoding"
            );
        }
    }

    #[test]
    fn instruction_box_lossy_deserialize_maps_removed_direct_pair_to_invalid_instruction() {
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let direct_register = Register::domain(Domain::new(test_domain_id()));
        let removed_name = std::any::type_name::<Register<Domain>>();
        let framed_payload = framed_instruction_payload(&direct_register);
        let framed_pair = framed_instruction_pair(removed_name, framed_payload.clone());
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed_pair).expect("instruction pair");

        let decoded: InstructionBox = norito::core::NoritoDeserialize::deserialize(archived);
        let invalid = decoded
            .as_any()
            .downcast_ref::<transparent::InvalidInstruction>()
            .expect("removed direct instruction becomes invalid placeholder");
        let expected_hash: [u8; 32] = iroha_crypto::Hash::new(&framed_payload).into();

        assert_eq!(invalid.wire_id, removed_name);
        assert_eq!(invalid.payload_hash, expected_hash);
        assert!(
            invalid.message.contains("not registered")
                || invalid.message.contains("unknown instruction"),
            "invalid placeholder should preserve the decode failure: {}",
            invalid.message
        );
    }

    #[test]
    fn instruction_box_strict_decoders_reject_cross_family_instruction_pairs() {
        use norito::core::DecodeFromSlice;

        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let register_payload = framed_instruction_payload(&RegisterBox::Domain(Register::domain(
            Domain::new(test_domain_id()),
        )));
        let repo_payload = framed_instruction_payload(&repo::RepoInstructionBox::MarginCall(
            repo::RepoMarginCallIsi::new("instruction_box_cross_family".parse().expect("repo id")),
        ));

        for (spoofed_name, mismatched_payload) in [
            (MintBox::WIRE_ID, register_payload),
            (settlement::SettlementInstructionBox::WIRE_ID, repo_payload),
        ] {
            let framed_pair = framed_instruction_pair(spoofed_name, mismatched_payload.clone());
            let archived = norito::core::from_bytes::<InstructionBox>(&framed_pair)
                .expect("instruction pair bytes");
            assert!(
                norito::core::NoritoDeserialize::try_deserialize(archived).is_err(),
                "{spoofed_name} must reject a payload from another boxed family"
            );

            let bare_pair = bare_instruction_pair(spoofed_name, mismatched_payload);
            assert!(
                InstructionBox::decode_from_slice(&bare_pair).is_err(),
                "{spoofed_name} must reject mismatched borrowed-pair payloads"
            );
        }
    }

    #[test]
    fn instruction_box_lossy_deserialize_maps_cross_family_pair_to_invalid_instruction() {
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let framed_payload = framed_instruction_payload(&RegisterBox::Domain(Register::domain(
            Domain::new(test_domain_id()),
        )));
        let framed_pair = framed_instruction_pair(MintBox::WIRE_ID, framed_payload.clone());
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed_pair).expect("instruction pair");

        let decoded: InstructionBox = norito::core::NoritoDeserialize::deserialize(archived);
        let invalid = decoded
            .as_any()
            .downcast_ref::<transparent::InvalidInstruction>()
            .expect("cross-family instruction becomes invalid placeholder");
        let expected_hash: [u8; 32] = iroha_crypto::Hash::new(&framed_payload).into();

        assert_eq!(invalid.wire_id, MintBox::WIRE_ID);
        assert_eq!(invalid.payload_hash, expected_hash);
    }

    #[test]
    fn instruction_box_lossy_deserialize_bounds_unknown_wire_error_message() {
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let hostile_name = format!("iroha.{}", "x".repeat(2048));
        let framed_pair = framed_instruction_pair(&hostile_name, Vec::new());
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed_pair).expect("instruction pair");

        let decoded: InstructionBox = norito::core::NoritoDeserialize::deserialize(archived);
        let invalid = decoded
            .as_any()
            .downcast_ref::<transparent::InvalidInstruction>()
            .expect("unknown instruction becomes invalid placeholder");
        let expected_hash: [u8; 32] = iroha_crypto::Hash::new([]).into();

        assert_eq!(invalid.wire_id, hostile_name);
        assert_eq!(invalid.payload_hash, expected_hash);
        assert!(
            invalid.message.len() <= 256,
            "decode error should be bounded, got {} bytes",
            invalid.message.len()
        );
        assert!(
            invalid.message.contains("unknown instruction"),
            "invalid placeholder should explain the rejected wire id"
        );
    }

    #[test]
    fn instruction_box_decode_from_slice_rejects_trailing_bytes_after_valid_pair() {
        use norito::core::DecodeFromSlice;

        let _guard = RegistryGuard::set(instruction_registry![Log]);
        let boxed = InstructionBox::from(Log::new(Level::INFO, "pair tail".to_owned()));
        let mut bare_pair = Vec::new();
        norito::core::NoritoSerialize::serialize(&boxed, &mut bare_pair)
            .expect("serialize instruction box pair");
        bare_pair.extend_from_slice(&[0xAA, 0x55]);

        let err = InstructionBox::decode_from_slice(&bare_pair)
            .expect_err("trailing bytes after a valid pair must be rejected");
        match err {
            norito::core::Error::Message(msg) => assert!(
                msg.contains("canonical Norito framing"),
                "error should reject non-canonical trailing bytes: {msg}"
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn instruction_box_try_deserialize_rejects_trailing_bytes_inside_framed_pair() {
        let _guard = RegistryGuard::set(instruction_registry![Log]);
        let boxed = InstructionBox::from(Log::new(Level::INFO, "framed pair tail".to_owned()));
        let mut bare_pair = Vec::new();
        norito::core::NoritoSerialize::serialize(&boxed, &mut bare_pair)
            .expect("serialize instruction box pair");
        bare_pair.extend_from_slice(&[0xAA, 0x55]);
        let framed = norito::core::frame_bare_with_header_flags::<InstructionBox>(
            &bare_pair,
            norito::core::default_encode_flags(),
        )
        .expect("frame tailed instruction pair");
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed).expect("instruction box frame");

        let err = norito::core::NoritoDeserialize::try_deserialize(archived)
            .expect_err("framed instruction pairs with trailing bytes must be rejected");
        match err {
            norito::core::Error::Message(msg) => assert!(
                msg.contains("canonical Norito framing"),
                "error should reject non-canonical trailing bytes: {msg}"
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn instruction_box_lossy_deserialize_maps_trailing_pair_bytes_to_invalid_instruction() {
        let _guard = RegistryGuard::set(instruction_registry_with_ids![Log]);
        let boxed = InstructionBox::from(Log::new(Level::INFO, "lossy pair tail".to_owned()));
        let (wire_id, framed_payload) =
            encoded_instruction_pair_payload(&boxed).expect("encoded instruction payload");
        assert_eq!(wire_id, Log::WIRE_ID);
        let mut bare_pair = Vec::new();
        norito::core::NoritoSerialize::serialize(&boxed, &mut bare_pair)
            .expect("serialize instruction box pair");
        bare_pair.extend_from_slice(&[0xAA, 0x55]);
        let framed = norito::core::frame_bare_with_header_flags::<InstructionBox>(
            &bare_pair,
            norito::core::default_encode_flags(),
        )
        .expect("frame tailed instruction pair");
        let archived =
            norito::core::from_bytes::<InstructionBox>(&framed).expect("instruction box frame");

        let decoded: InstructionBox = norito::core::NoritoDeserialize::deserialize(archived);
        let invalid = decoded
            .as_any()
            .downcast_ref::<transparent::InvalidInstruction>()
            .expect("tailed instruction pair becomes invalid placeholder");
        let expected_hash: [u8; 32] = iroha_crypto::Hash::new(&framed_payload).into();

        assert_eq!(invalid.wire_id, Log::WIRE_ID);
        assert_eq!(invalid.payload_hash, expected_hash);
        assert!(
            invalid.message.contains("canonical Norito framing"),
            "invalid placeholder should explain non-canonical trailing bytes: {}",
            invalid.message
        );
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
    fn offline_note_v2_instructions_are_registered_and_boxable() {
        use crate::offline::{
            OfflineNoteAuditBundleV2, OfflineNoteIssueV2, OfflineNoteIssuedClaimV2,
            OfflineNoteKeyCertificateV2, OfflineNoteRecursiveProofV2, OfflineNoteRedeemV2,
        };
        use crate::proof::{ProofBox, VerifyingKeyId};
        use iroha_crypto::{Hash, Signature};

        let registry = crate::instruction_registry::default();
        let account_id = AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        );
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        let asset_id = AssetId::of(asset_definition_id, account_id.clone());
        let proof = OfflineNoteRecursiveProofV2 {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
            public_inputs_hash: Hash::new(b"offline-v2-public-inputs"),
            proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
        };
        let key_certificate = OfflineNoteKeyCertificateV2 {
            version: 2,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id: account_id.clone(),
            public_key: vec![0x01, 0x02, 0x03],
            assertion_scheme: "apple-appattest-counter-v1".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: Signature::from_bytes(&[0xAB; 64]),
        };

        let issue = crate::isi::offline::IssueOfflineNoteV2::new(OfflineNoteIssueV2 {
            note_commitment: Hash::new(b"note-commitment"),
            key_certificate: key_certificate.clone(),
            asset: asset_id.clone(),
            amount: Numeric::new(10, 0),
        });
        let redemption = crate::isi::offline::RedeemOfflineNoteV2::new(OfflineNoteRedeemV2 {
            source_note_commitment: Hash::new(b"note-commitment"),
            input_nullifiers: vec![Hash::new(b"input-nullifier")],
            sender_key_certificate: key_certificate.clone(),
            recipient: account_id,
            asset: asset_id.clone(),
            amount: Numeric::new(10, 0),
            recursive_proof: proof.clone(),
        });
        let audit = crate::isi::offline::AuditOfflineNoteV2::new(OfflineNoteAuditBundleV2 {
            token_id: Hash::new(b"token"),
            sender_key_certificate: key_certificate.clone(),
            input_nullifiers: vec![Hash::new(b"audit-nullifier")],
            input_claims: vec![
                OfflineNoteIssuedClaimV2::from_issue(&issue.issue).expect("audit input claim"),
            ],
            output_commitments: vec![Hash::new(b"output-note")],
            output_claims: vec![crate::offline::OfflineNoteAuditOutputClaimV2 {
                note_commitment: Hash::new(b"output-note"),
                key_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: proof,
        });

        let cases: Vec<(&'static str, InstructionBox, Vec<u8>)> = vec![
            (
                std::any::type_name::<crate::isi::offline::IssueOfflineNoteV2>(),
                issue.clone().into(),
                norito::to_bytes(&issue).expect("encode issue instruction"),
            ),
            (
                std::any::type_name::<crate::isi::offline::RedeemOfflineNoteV2>(),
                redemption.clone().into(),
                norito::to_bytes(&redemption).expect("encode redemption instruction"),
            ),
            (
                std::any::type_name::<crate::isi::offline::AuditOfflineNoteV2>(),
                audit.clone().into(),
                norito::to_bytes(&audit).expect("encode audit instruction"),
            ),
        ];

        for (type_name, instruction, payload) in cases {
            assert!(
                registry.contains(type_name),
                "default registry should contain {type_name}"
            );
            let decoded = registry
                .decode(type_name, &payload)
                .unwrap_or_else(|| panic!("missing decoder for {type_name}"))
                .expect("decode instruction through registry");
            assert_eq!(instruction, decoded);
        }
    }

    #[test]
    fn default_registry_roundtrip_selected_instructions() {
        // Install default registry covering built-ins and keep a local handle
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let local_registry = crate::instruction_registry::default();

        // Build a small suite of representative instructions
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let account_id = AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .unwrap(),
        );
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
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
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let signatory = "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
            .parse()
            .unwrap();
        let account_id = AccountId::new(signatory);
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
        // Ensure the total ordering of InstructionBox is stable after Norito roundtrip.
        let _guard = RegistryGuard::set(crate::instruction_registry::default());

        let domain_id: DomainId = DomainId::try_new("alice", "universal").unwrap();
        let account_id = AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        );
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("alice", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
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
        // Expand coverage across instruction families and variants
        let _guard = RegistryGuard::set(crate::instruction_registry::default());
        let local_registry = crate::instruction_registry::default();

        // Common fixtures
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let account_a = AccountId::new(
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .parse()
                .unwrap(),
        );
        let account_b = AccountId::new(
            "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                .parse()
                .unwrap(),
        );
        let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
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

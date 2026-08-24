//! Soracloud lifecycle instructions.
//!
//! These instructions move Soracloud service deployment state into the
//! authoritative on-chain world model instead of Torii-local file persistence.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        AgentApartmentManifestV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1, FheJobSpecV1,
        SecretEnvelopeV1, SoraAppInfraManifestV1, SoraAppInfraMutationPreconditionV1,
        SoraDeploymentBundleV1, SoraHfResourceProfileV1, SoraInrouHostCapabilityRecordV1,
        SoraInrouReplicaRuntimeStateV1, SoraModelHostCapabilityRecordV1,
        SoraModelHostViolationKindV1, SoraPrivateUploadedModelExecutionReceiptV1,
        SoraRuntimeReceiptV1, SoraServiceMailboxMessageV1, SoraServiceMutationPreconditionV1,
        SoraServiceRuntimeStateV1, SoraStateEncryptionV1, SoraStateMutationOperationV1,
        SoraUploadedModelBundleV1, SoracloudFheBootstrapKeyProofV1,
        SoracloudFheFullBootstrapExecutionProofV1, SoracloudFheGovernedMaterialV1,
        SoracloudFheInputAdmissionProofV1, SoracloudFhePolicyReferenceV1,
        SoracloudFhePublicKeyProofV1,
    },
    sorafs::pin_registry::StorageClass,
};
use core::cmp::Ordering;
use iroha_crypto::Hash;
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::BTreeMap;
fn encoded_order<T: Encode>(left: &T, right: &T) -> Ordering {
    left.encode().cmp(&right.encode())
}
fn decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
/// Admit a brand new Soracloud service deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DeploySoracloudService {
    /// Bundle being admitted.
    pub bundle: SoraDeploymentBundleV1,
    /// Authoritative config entries committed atomically; an empty map must be explicit.
    pub initial_service_configs: BTreeMap<String, Json>,
    /// Authoritative secret entries committed atomically; an empty map must be explicit.
    pub initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    /// Signed atomic condition requiring this service to remain absent until execution.
    pub precondition: SoraServiceMutationPreconditionV1,
    /// Provenance attestation over the bundle payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for DeploySoracloudService {}
impl PartialOrd for DeploySoracloudService {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Admit a new candidate revision for an existing Soracloud service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct UpgradeSoracloudService {
    /// Bundle being admitted as the candidate revision.
    pub bundle: SoraDeploymentBundleV1,
    /// Authoritative config entries committed atomically; an empty map must be explicit.
    pub initial_service_configs: BTreeMap<String, Json>,
    /// Authoritative secret entries committed atomically; an empty map must be explicit.
    pub initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    /// Signed atomic condition binding the exact active revision observed by the caller.
    pub precondition: SoraServiceMutationPreconditionV1,
    /// Provenance attestation over the bundle payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for UpgradeSoracloudService {}
impl PartialOrd for UpgradeSoracloudService {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Admit a brand new Soracloud app-level infrastructure topology.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DeploySoracloudAppInfra {
    /// App topology manifest being admitted.
    pub manifest: SoraAppInfraManifestV1,
    /// Signed atomic condition requiring this app topology to remain absent until execution.
    pub precondition: SoraAppInfraMutationPreconditionV1,
    /// Provenance attestation over the app topology payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for DeploySoracloudAppInfra {}
impl<'a> norito::core::DecodeFromSlice<'a> for DeploySoracloudAppInfra {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let manifest = super::decode_aos_canonical_field::<SoraAppInfraManifestV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let precondition = super::decode_aos_canonical_field::<SoraAppInfraMutationPreconditionV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let provenance = super::decode_aos_canonical_field::<ManifestProvenance>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                manifest,
                precondition,
                provenance,
            },
            offset,
        ))
    }
}
impl PartialOrd for DeploySoracloudAppInfra {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Admit an upgraded Soracloud app-level infrastructure topology.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct UpgradeSoracloudAppInfra {
    /// App topology manifest being admitted.
    pub manifest: SoraAppInfraManifestV1,
    /// Signed atomic condition binding the exact active topology observed by the caller.
    pub precondition: SoraAppInfraMutationPreconditionV1,
    /// Provenance attestation over the app topology payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for UpgradeSoracloudAppInfra {}
impl<'a> norito::core::DecodeFromSlice<'a> for UpgradeSoracloudAppInfra {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let manifest = super::decode_aos_canonical_field::<SoraAppInfraManifestV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let precondition = super::decode_aos_canonical_field::<SoraAppInfraMutationPreconditionV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let provenance = super::decode_aos_canonical_field::<ManifestProvenance>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                manifest,
                precondition,
                provenance,
            },
            offset,
        ))
    }
}
impl PartialOrd for UpgradeSoracloudAppInfra {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Roll a Soracloud service back to an already admitted revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RollbackSoracloudService {
    /// Service to roll back.
    pub service_name: Name,
    /// Explicit already-admitted target version.
    pub target_version: String,
    /// Provenance attestation over the rollback payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RollbackSoracloudService {}
impl PartialOrd for RollbackSoracloudService {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Record or replace an authoritative Soracloud service config entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SetSoracloudServiceConfig {
    /// Service whose config entry should be updated.
    pub service_name: Name,
    /// Stable service-scoped config identifier.
    pub config_name: String,
    /// Canonical typed config value.
    pub value_json: Json,
    /// Provenance attestation over the config payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for SetSoracloudServiceConfig {}
impl PartialOrd for SetSoracloudServiceConfig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Remove an authoritative Soracloud service config entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DeleteSoracloudServiceConfig {
    /// Service whose config entry should be removed.
    pub service_name: Name,
    /// Stable service-scoped config identifier.
    pub config_name: String,
    /// Provenance attestation over the config payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for DeleteSoracloudServiceConfig {}
impl PartialOrd for DeleteSoracloudServiceConfig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Record or replace an authoritative Soracloud service secret entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SetSoracloudServiceSecret {
    /// Service whose secret entry should be updated.
    pub service_name: Name,
    /// Stable service-scoped secret identifier.
    pub secret_name: String,
    /// Encrypted secret envelope.
    pub secret: SecretEnvelopeV1,
    /// Provenance attestation over the secret payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for SetSoracloudServiceSecret {}
impl PartialOrd for SetSoracloudServiceSecret {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Remove an authoritative Soracloud service secret entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DeleteSoracloudServiceSecret {
    /// Service whose secret entry should be removed.
    pub service_name: Name,
    /// Stable service-scoped secret identifier.
    pub secret_name: String,
    /// Provenance attestation over the secret payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for DeleteSoracloudServiceSecret {}
impl PartialOrd for DeleteSoracloudServiceSecret {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Record an ordered Soracloud state mutation against a declared binding.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct MutateSoracloudState {
    /// Service whose state binding should be mutated.
    pub service_name: Name,
    /// Binding being mutated.
    pub binding_name: Name,
    /// Canonical key under the binding prefix.
    pub state_key: String,
    /// Mutation mode to apply.
    pub operation: SoraStateMutationOperationV1,
    /// Declared payload size for upsert operations.
    #[norito(required)]
    pub value_size_bytes: Option<u64>,
    /// Full payload bytes for upsert operations.
    #[norito(required)]
    pub value_payload: Option<Vec<u8>>,
    /// Expected binding encryption mode.
    pub encryption: SoraStateEncryptionV1,
    /// Governance transaction hash attached to the mutation.
    pub governance_tx_hash: Hash,
    /// Optional verifier-backed proof admitting FHE ciphertext input metadata.
    #[norito(required)]
    pub fhe_input_admission_proof: Option<SoracloudFheInputAdmissionProofV1>,
    /// Provenance attestation over the mutation payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for MutateSoracloudState {}
impl PartialOrd for MutateSoracloudState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Register the first governance-authenticated FHE material version for a service policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RegisterSoracloudFhePolicy {
    /// Service that owns the policy.
    pub service_name: Name,
    /// Immutable first material version; its version must be one.
    pub material: SoracloudFheGovernedMaterialV1,
    /// Governance provenance attestation over the registration payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RegisterSoracloudFhePolicy {}
impl PartialOrd for RegisterSoracloudFhePolicy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Rotate a service-scoped FHE policy to the next immutable material version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RotateSoracloudFhePolicy {
    /// Service that owns the policy.
    pub service_name: Name,
    /// Exact active version that must still be current when rotation executes.
    pub expected_active: SoracloudFhePolicyReferenceV1,
    /// Immutable next material version.
    pub material: SoracloudFheGovernedMaterialV1,
    /// Governance provenance attestation over the rotation payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RotateSoracloudFhePolicy {}
impl PartialOrd for RotateSoracloudFhePolicy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Permanently revoke the exact active FHE policy version for a service.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RevokeSoracloudFhePolicy {
    /// Service that owns the policy.
    pub service_name: Name,
    /// Exact active version that must still be current when revocation executes.
    pub expected_active: SoracloudFhePolicyReferenceV1,
    /// Governance provenance attestation over the revocation payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RevokeSoracloudFhePolicy {}
impl PartialOrd for RevokeSoracloudFhePolicy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Record an ordered Soracloud FHE execution result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RunSoracloudFheJob {
    /// Service whose ciphertext state receives the job output.
    pub service_name: Name,
    /// Binding receiving the deterministic ciphertext output.
    pub binding_name: Name,
    /// Deterministic FHE job specification.
    pub job: FheJobSpecV1,
    /// Exact active governed material version authorized for this job.
    pub policy_reference: SoracloudFhePolicyReferenceV1,
    /// Optional verifier-backed proof for public BFV key material.
    #[norito(required)]
    pub public_key_proof: Option<SoracloudFhePublicKeyProofV1>,
    /// Optional verifier-backed proof for public bootstrap-key zero-refresh material.
    #[norito(required)]
    pub bootstrap_key_zero_refresh_proof: Option<SoracloudFheBootstrapKeyProofV1>,
    /// Verifier-backed proof for each full-bootstrap output ciphertext slot.
    pub full_bootstrap_execution_proofs: Vec<SoracloudFheFullBootstrapExecutionProofV1>,
    /// Provenance attestation over the job payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RunSoracloudFheJob {}
impl PartialOrd for RunSoracloudFheJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Record an ordered Soracloud decryption or health-access request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RecordSoracloudDecryptionRequest {
    /// Service whose ciphertext state is being requested.
    pub service_name: Name,
    /// Policy snapshot used to validate the request.
    pub policy: DecryptionAuthorityPolicyV1,
    /// Decryption request payload.
    pub request: DecryptionRequestV1,
    /// Provenance attestation over the request payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RecordSoracloudDecryptionRequest {}
impl PartialOrd for RecordSoracloudDecryptionRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Join or create a shared Hugging Face lease window on Soracloud.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct JoinSoracloudHfSharedLease {
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision for the canonical source.
    pub resolved_revision: String,
    /// Normalized model name used for Soracloud bindings.
    pub model_name: String,
    /// Service binding that will reuse the shared lease.
    pub service_name: Name,
    /// Optional apartment binding that will reuse the shared lease.
    #[norito(required)]
    pub apartment_name: Option<Name>,
    /// Requested shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window nominal price in `lease_asset_definition_id`.
    pub base_fee: Quantity,
    /// Canonical HF resource profile derived by the control plane.
    #[norito(required)]
    pub resource_profile: Option<SoraHfResourceProfileV1>,
    /// Exact first-release upper bound for the compute reservation charge.
    ///
    /// The transaction signer reviews this value together with the derived resource profile.
    /// Consensus rejects a join whose effective compute charge would exceed this bound.
    pub max_compute_reservation_fee: Quantity,
    /// Provenance attestation over the join payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for JoinSoracloudHfSharedLease {}
impl PartialOrd for JoinSoracloudHfSharedLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Leave the current shared Hugging Face lease window.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LeaveSoracloudHfSharedLease {
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision for the canonical source.
    pub resolved_revision: String,
    /// Shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Optional service binding being detached for audit context.
    #[norito(required)]
    pub service_name: Option<Name>,
    /// Optional apartment binding being detached for audit context.
    #[norito(required)]
    pub apartment_name: Option<Name>,
    /// Provenance attestation over the leave payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for LeaveSoracloudHfSharedLease {}
impl PartialOrd for LeaveSoracloudHfSharedLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Sponsor a fresh shared Hugging Face lease window after expiry or retirement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RenewSoracloudHfSharedLease {
    /// Hugging Face repository identifier.
    pub repo_id: String,
    /// Exact pinned revision for the canonical source.
    pub resolved_revision: String,
    /// Normalized model name used for Soracloud bindings.
    pub model_name: String,
    /// Service binding that will reuse the renewed shared lease.
    pub service_name: Name,
    /// Optional apartment binding that will reuse the renewed shared lease.
    #[norito(required)]
    pub apartment_name: Option<Name>,
    /// Requested shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window nominal price in `lease_asset_definition_id`.
    pub base_fee: Quantity,
    /// Canonical HF resource profile derived by the control plane.
    #[norito(required)]
    pub resource_profile: Option<SoraHfResourceProfileV1>,
    /// Provenance attestation over the renew payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RenewSoracloudHfSharedLease {}
impl PartialOrd for RenewSoracloudHfSharedLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Advertise validator-host capabilities for authoritative HF placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AdvertiseSoracloudModelHost {
    /// Capability advert being published by the validator.
    pub capability: SoraModelHostCapabilityRecordV1,
    /// Provenance attestation over the advert payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for AdvertiseSoracloudModelHost {}
impl PartialOrd for AdvertiseSoracloudModelHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Refresh the heartbeat TTL for an advertised validator host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct HeartbeatSoracloudModelHost {
    /// Validator account that owns the host advert.
    pub validator_account_id: AccountId,
    /// New heartbeat-expiry timestamp.
    pub heartbeat_expires_at_ms: u64,
    /// Provenance attestation over the heartbeat payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for HeartbeatSoracloudModelHost {}
impl PartialOrd for HeartbeatSoracloudModelHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Withdraw an advertised validator host from authoritative HF placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct WithdrawSoracloudModelHost {
    /// Validator account that owns the host advert.
    pub validator_account_id: AccountId,
    /// Provenance attestation over the withdrawal payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for WithdrawSoracloudModelHost {}
impl PartialOrd for WithdrawSoracloudModelHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Reconcile validator-host availability and expired HF lease windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReconcileSoracloudModelHosts;
impl crate::seal::Instruction for ReconcileSoracloudModelHosts {}
impl PartialOrd for ReconcileSoracloudModelHosts {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Advertise validator-host capabilities for authoritative Inrou placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AdvertiseSoracloudInrouHost {
    /// Capability advert being published by the validator.
    pub capability: SoraInrouHostCapabilityRecordV1,
    /// Provenance attestation over the advert payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for AdvertiseSoracloudInrouHost {}
impl PartialOrd for AdvertiseSoracloudInrouHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Withdraw an advertised validator host from authoritative Inrou placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct WithdrawSoracloudInrouHost {
    /// Validator account that owns the host advert.
    pub validator_account_id: AccountId,
    /// Provenance attestation over the withdrawal payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for WithdrawSoracloudInrouHost {}
impl PartialOrd for WithdrawSoracloudInrouHost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Reconcile active hosted Inrou placements against current host adverts and service leases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReconcileSoracloudInrouPlacements;
impl crate::seal::Instruction for ReconcileSoracloudInrouPlacements {}
impl PartialOrd for ReconcileSoracloudInrouPlacements {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Report authoritative evidence for a validator-host violation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ReportSoracloudModelHostViolation {
    /// Validator responsible for the violation.
    pub validator_account_id: AccountId,
    /// Violation class.
    pub kind: SoraModelHostViolationKindV1,
    /// Implicated placement when the violation is placement-scoped.
    #[norito(required)]
    pub placement_id: Option<Hash>,
    /// Optional explanatory detail attached to the evidence.
    #[norito(required)]
    pub detail: Option<String>,
}
impl crate::seal::Instruction for ReportSoracloudModelHostViolation {}
impl PartialOrd for ReportSoracloudModelHostViolation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Deploy a Soracloud agent apartment into authoritative world state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DeploySoracloudAgentApartment {
    /// Apartment manifest being admitted.
    pub manifest: AgentApartmentManifestV1,
    /// Requested lease duration in deterministic ticks.
    pub lease_ticks: u64,
    /// Initial autonomy budget ceiling for the apartment.
    pub autonomy_budget_units: u64,
    /// Provenance attestation over the deploy payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for DeploySoracloudAgentApartment {}
impl PartialOrd for DeploySoracloudAgentApartment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Renew a Soracloud agent apartment lease.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RenewSoracloudAgentLease {
    /// Apartment to renew.
    pub apartment_name: Name,
    /// Requested lease duration in deterministic ticks.
    pub lease_ticks: u64,
    /// Provenance attestation over the renew payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RenewSoracloudAgentLease {}
impl PartialOrd for RenewSoracloudAgentLease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Restart a Soracloud agent apartment process.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RestartSoracloudAgentApartment {
    /// Apartment to restart.
    pub apartment_name: Name,
    /// Human-readable restart reason.
    pub reason: String,
    /// Provenance attestation over the restart payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RestartSoracloudAgentApartment {}
impl PartialOrd for RestartSoracloudAgentApartment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Revoke an active Soracloud agent apartment policy capability.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RevokeSoracloudAgentPolicy {
    /// Apartment whose policy should change.
    pub apartment_name: Name,
    /// Capability identifier to revoke.
    pub capability: String,
    /// Optional human-readable reason.
    #[norito(required)]
    pub reason: Option<String>,
    /// Provenance attestation over the revoke payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RevokeSoracloudAgentPolicy {}
impl PartialOrd for RevokeSoracloudAgentPolicy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Submit a policy-gated wallet spend request for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RequestSoracloudAgentWalletSpend {
    /// Apartment initiating the spend.
    pub apartment_name: Name,
    /// Asset definition constrained by apartment policy.
    pub asset_definition: String,
    /// Requested nominal spend amount.
    pub amount: Quantity,
    /// Provenance attestation over the spend payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RequestSoracloudAgentWalletSpend {}
impl PartialOrd for RequestSoracloudAgentWalletSpend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Approve and apply a pending wallet spend request for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ApproveSoracloudAgentWalletSpend {
    /// Apartment owning the pending request.
    pub apartment_name: Name,
    /// Deterministic request identifier to approve.
    pub request_id: String,
    /// Provenance attestation over the approval payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for ApproveSoracloudAgentWalletSpend {}
impl PartialOrd for ApproveSoracloudAgentWalletSpend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Enqueue a deterministic mailbox message between agent apartments.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct EnqueueSoracloudAgentMessage {
    /// Sender apartment.
    pub from_apartment: Name,
    /// Recipient apartment.
    pub to_apartment: Name,
    /// Logical mailbox channel.
    pub channel: String,
    /// Message payload.
    pub payload: String,
    /// Provenance attestation over the message payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for EnqueueSoracloudAgentMessage {}
impl PartialOrd for EnqueueSoracloudAgentMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Acknowledge and consume a queued mailbox message for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AcknowledgeSoracloudAgentMessage {
    /// Apartment consuming the mailbox message.
    pub apartment_name: Name,
    /// Deterministic message identifier to acknowledge.
    pub message_id: String,
    /// Provenance attestation over the acknowledgement payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for AcknowledgeSoracloudAgentMessage {}
impl PartialOrd for AcknowledgeSoracloudAgentMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Allowlist an autonomy artifact for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AllowSoracloudAgentAutonomyArtifact {
    /// Apartment receiving the allowlist rule.
    pub apartment_name: Name,
    /// Artifact hash being allowlisted.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Provenance attestation over the allowlist payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for AllowSoracloudAgentAutonomyArtifact {}
impl PartialOrd for AllowSoracloudAgentAutonomyArtifact {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Approve a deterministic autonomy run for an agent apartment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RunSoracloudAgentAutonomy {
    /// Apartment owning the run.
    pub apartment_name: Name,
    /// Allowlisted artifact hash being executed.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(required)]
    pub provenance_hash: Option<String>,
    /// Budget units approved for the run.
    pub budget_units: u64,
    /// Human-readable run label.
    pub run_label: String,
    /// Optional canonical JSON body forwarded to the generated HF `/infer` handler.
    #[norito(required)]
    pub workflow_input_json: Option<String>,
    /// Provenance attestation over the autonomy-run payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RunSoracloudAgentAutonomy {}
impl PartialOrd for RunSoracloudAgentAutonomy {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Persist an authoritative apartment-level execution audit for a completed autonomy run.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RecordSoracloudAgentAutonomyExecution {
    /// Apartment that owns the executed run.
    pub apartment_name: Name,
    /// Stable approved run identifier.
    pub run_id: String,
    /// Process generation that executed the run.
    pub process_generation: u64,
    /// Whether the runtime completed the run successfully.
    pub succeeded: bool,
    /// Deterministic commitment over the execution outcome.
    pub result_commitment: Hash,
    /// Generated service name used for execution, when locally resolved.
    #[norito(required)]
    pub service_name: Option<Name>,
    /// Generated service version used for execution, when locally resolved.
    #[norito(required)]
    pub service_version: Option<String>,
    /// Generated handler used for execution, when locally resolved.
    #[norito(required)]
    pub handler_name: Option<Name>,
    /// Authoritative runtime receipt referenced by the execution, when one exists.
    #[norito(required)]
    pub runtime_receipt_id: Option<Hash>,
    /// Node-local journal artifact hash, when one was persisted.
    #[norito(required)]
    pub journal_artifact_hash: Option<Hash>,
    /// Node-local checkpoint artifact hash, when one was persisted.
    #[norito(required)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Human-readable runtime error, when execution failed.
    #[norito(required)]
    pub error: Option<String>,
}
impl crate::seal::Instruction for RecordSoracloudAgentAutonomyExecution {}
impl PartialOrd for RecordSoracloudAgentAutonomyExecution {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Start a deterministic Soracloud training job.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct StartSoracloudTrainingJob {
    /// Service that owns the training job.
    pub service_name: Name,
    /// Logical model name targeted by the job.
    pub model_name: String,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Size of the worker group.
    pub worker_group_size: u16,
    /// Total target step count.
    pub target_steps: u32,
    /// Required checkpoint interval.
    pub checkpoint_interval_steps: u32,
    /// Maximum retry count.
    pub max_retries: u8,
    /// Compute units consumed per worker-group step.
    pub step_compute_units: u64,
    /// Total compute budget for the job.
    pub compute_budget_units: u64,
    /// Total storage budget for checkpoints.
    pub storage_budget_bytes: u64,
    /// Provenance attestation over the job payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for StartSoracloudTrainingJob {}
impl PartialOrd for StartSoracloudTrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Record a deterministic Soracloud training checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CheckpointSoracloudTrainingJob {
    /// Service that owns the training job.
    pub service_name: Name,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Completed step count after this checkpoint.
    pub completed_step: u32,
    /// Checkpoint artifact size in bytes.
    pub checkpoint_size_bytes: u64,
    /// Metrics artifact hash for the checkpoint.
    pub metrics_hash: Hash,
    /// Provenance attestation over the checkpoint payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for CheckpointSoracloudTrainingJob {}
impl PartialOrd for CheckpointSoracloudTrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Move a deterministic Soracloud training job into retry-pending state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RetrySoracloudTrainingJob {
    /// Service that owns the training job.
    pub service_name: Name,
    /// Deterministic training-job identifier.
    pub job_id: String,
    /// Normalized retry reason.
    pub reason: String,
    /// Provenance attestation over the retry payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RetrySoracloudTrainingJob {}
impl PartialOrd for RetrySoracloudTrainingJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Register a deterministic Soracloud model artifact.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RegisterSoracloudModelArtifact {
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Training job that produced the artifact.
    pub training_job_id: String,
    /// Weight artifact hash.
    pub weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    pub dataset_ref: String,
    /// Training configuration hash.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash.
    pub provenance_attestation_hash: Hash,
    /// Provenance attestation over the artifact payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RegisterSoracloudModelArtifact {}
impl PartialOrd for RegisterSoracloudModelArtifact {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Register a deterministic Soracloud model-weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RegisterSoracloudModelWeight {
    /// Service that owns the model.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Version identifier being admitted.
    pub weight_version: String,
    /// Training job that produced this version.
    pub training_job_id: String,
    /// Optional lineage parent version.
    #[norito(required)]
    pub parent_version: Option<String>,
    /// Weight artifact hash.
    pub weight_artifact_hash: Hash,
    /// Dataset reference identifier.
    pub dataset_ref: String,
    /// Training configuration hash.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash.
    pub provenance_attestation_hash: Hash,
    /// Provenance attestation over the register payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RegisterSoracloudModelWeight {}
impl PartialOrd for RegisterSoracloudModelWeight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Promote an admitted Soracloud model-weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct PromoteSoracloudModelWeight {
    /// Service that owns the model.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Version being promoted.
    pub weight_version: String,
    /// Gate result that authorizes promotion.
    pub gate_approved: bool,
    /// Gate report hash attached to the promotion.
    pub gate_report_hash: Hash,
    /// Provenance attestation over the promote payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for PromoteSoracloudModelWeight {}
impl PartialOrd for PromoteSoracloudModelWeight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Roll a Soracloud model registry back to a prior admitted weight version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RollbackSoracloudModelWeight {
    /// Service that owns the model.
    pub service_name: Name,
    /// Logical model name.
    pub model_name: String,
    /// Target version to make current.
    pub target_version: String,
    /// Human-readable rollback reason.
    pub reason: String,
    /// Provenance attestation over the rollback payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RollbackSoracloudModelWeight {}
impl PartialOrd for RollbackSoracloudModelWeight {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Register an uploaded-model bundle root before encrypted chunks arrive.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RegisterSoracloudUploadedModelBundle {
    /// Deterministic uploaded-model bundle metadata.
    pub bundle: SoraUploadedModelBundleV1,
    /// Provenance attestation over the bundle payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for RegisterSoracloudUploadedModelBundle {}
impl PartialOrd for RegisterSoracloudUploadedModelBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Seal an uploaded-model bundle and publish its artifact metadata.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct FinalizeSoracloudUploadedModelBundle {
    /// Service that owns the artifact.
    pub service_name: Name,
    /// Logical model name in the canonical registry plane.
    pub model_name: String,
    /// Stable uploaded-model identifier.
    pub model_id: String,
    /// Artifact identifier bound to the uploaded model.
    pub artifact_id: String,
    /// Version label pinned by the artifact.
    pub weight_version: String,
    /// Canonical uploaded-model bundle root.
    pub bundle_root: Hash,
    /// Artifact hash for the uploaded payload.
    pub weight_artifact_hash: Hash,
    /// Dataset reference or upload source label.
    pub dataset_ref: String,
    /// Deterministic training/configuration hash for the upload.
    pub training_config_hash: Hash,
    /// Reproducibility metadata hash.
    pub reproducibility_hash: Hash,
    /// Provenance attestation hash for the canonical upload bundle.
    pub provenance_attestation_hash: Hash,
    /// Provenance attestation over the finalize payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for FinalizeSoracloudUploadedModelBundle {}
impl PartialOrd for FinalizeSoracloudUploadedModelBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Advance or roll back an in-flight Soracloud rollout.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct AdvanceSoracloudRollout {
    /// Service whose rollout should advance.
    pub service_name: Name,
    /// Deterministic rollout handle.
    pub rollout_handle: String,
    /// Health observation for the current rollout step.
    pub healthy: bool,
    /// Optional promotion target for healthy steps.
    #[norito(required)]
    pub promote_to_percent: Option<u8>,
    /// Governance transaction hash attached to the rollout step.
    pub governance_tx_hash: Hash,
    /// Provenance attestation over the rollout payload.
    pub provenance: ManifestProvenance,
}
impl crate::seal::Instruction for AdvanceSoracloudRollout {}
impl PartialOrd for AdvanceSoracloudRollout {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Upsert authoritative node/runtime state for a Soracloud service.
///
/// `CanManageSoracloud` holders may reconcile any service. Other callers must
/// be active public-lane validators assigned to the exact service revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SetSoracloudRuntimeState {
    /// Runtime state to persist.
    pub state: SoraServiceRuntimeStateV1,
}
impl crate::seal::Instruction for SetSoracloudRuntimeState {}
impl PartialOrd for SetSoracloudRuntimeState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Upsert authoritative runtime state for one placed Inrou replica.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SetSoracloudInrouReplicaRuntimeState {
    /// Runtime state to persist.
    pub state: SoraInrouReplicaRuntimeStateV1,
}
impl crate::seal::Instruction for SetSoracloudInrouReplicaRuntimeState {}
impl PartialOrd for SetSoracloudInrouReplicaRuntimeState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Clear authoritative runtime state for one placed Inrou replica.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ClearSoracloudInrouReplicaRuntimeState {
    /// Service whose replica state should be removed.
    pub service_name: Name,
    /// Service revision whose replica state should be removed.
    pub service_version: String,
    /// One-based placed replica slot whose state should be removed.
    pub replica_slot: u16,
}
impl crate::seal::Instruction for ClearSoracloudInrouReplicaRuntimeState {}
impl PartialOrd for ClearSoracloudInrouReplicaRuntimeState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Report authoritative leased-service usage observed by the runtime.
///
/// A live report requires the transaction authority to be an active
/// public-lane validator assigned to the exact service revision and replica
/// slot and current reporting epoch. A terminal report may instead come
/// from the exact former authority of an existing reporter checkpoint; the
/// first terminal update requires it to be open, while an exact finalized
/// replay is idempotent. Manager authority is not a substitute for either
/// reporter identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ReportSoracloudServiceLeaseUsage {
    /// Service whose hosted-service lease should be updated.
    pub service_name: Name,
    /// Current reporting epoch, or its exact successor when atomically opening
    /// a new epoch at the reporter-checkpoint hard limit.
    pub reporting_epoch: u64,
    /// Active service revision observed by the runtime.
    pub active_service_version: String,
    /// One-based placed replica slot whose reporter emitted the usage.
    pub replica_slot: u16,
    /// Monotonic egress bytes emitted by this exact lease/revision/slot/authority reporter.
    pub replica_accounted_egress_bytes: u64,
    /// Seal this reporter after its worker has stopped and all writes have joined.
    pub finalize_reporter: bool,
}
impl crate::seal::Instruction for ReportSoracloudServiceLeaseUsage {}
impl PartialOrd for ReportSoracloudServiceLeaseUsage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Persist an ordered Soracloud mailbox message.
///
/// The source service must be deployed. `CanManageSoracloud` holders may reconcile any source;
/// other callers must be active public-lane validators assigned to the source service's active
/// revision. Recorded message identifiers are immutable and cannot be replaced.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RecordSoracloudMailboxMessage {
    /// Mailbox message to persist.
    pub message: SoraServiceMailboxMessageV1,
}
impl crate::seal::Instruction for RecordSoracloudMailboxMessage {}
impl PartialOrd for RecordSoracloudMailboxMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Persist an authoritative Soracloud runtime receipt.
///
/// `CanManageSoracloud` holders may reconcile any service. Other callers must be active public-lane
/// validators assigned to the exact service revision and must identify themselves as the selected
/// validator in the receipt. Recorded receipt identifiers are immutable and cannot be replaced.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RecordSoracloudRuntimeReceipt {
    /// Runtime receipt to persist; `emitted_sequence` must be the zero submission sentinel.
    pub receipt: SoraRuntimeReceiptV1,
}
impl crate::seal::Instruction for RecordSoracloudRuntimeReceipt {}
impl PartialOrd for RecordSoracloudRuntimeReceipt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
/// Persist an authoritative private uploaded-model execution receipt.
///
/// This privileged ledger projection is restricted to `CanManageSoracloud` holders. Recorded
/// receipt identifiers are immutable and cannot be replaced.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct RecordSoracloudPrivateUploadedModelExecutionReceipt {
    /// Private uploaded-model execution receipt to persist.
    pub receipt: SoraPrivateUploadedModelExecutionReceiptV1,
}
impl crate::seal::Instruction for RecordSoracloudPrivateUploadedModelExecutionReceipt {}
impl PartialOrd for RecordSoracloudPrivateUploadedModelExecutionReceipt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}
fn soracloud_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
macro_rules! impl_soracloud_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = soracloud_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}
macro_rules! impl_soracloud_unit_decode_from_slice {
    ($ty:ty) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = soracloud_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                if !bytes.is_empty() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, 0);
                Ok((Self, 0))
            }
        }
    };
}
impl_soracloud_decode_from_slice!(DeploySoracloudService {
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    precondition: SoraServiceMutationPreconditionV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(UpgradeSoracloudService {
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    precondition: SoraServiceMutationPreconditionV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RollbackSoracloudService {
    service_name: Name,
    target_version: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(SetSoracloudServiceConfig {
    service_name: Name,
    config_name: String,
    value_json: Json,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(DeleteSoracloudServiceConfig {
    service_name: Name,
    config_name: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(SetSoracloudServiceSecret {
    service_name: Name,
    secret_name: String,
    secret: SecretEnvelopeV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(DeleteSoracloudServiceSecret {
    service_name: Name,
    secret_name: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(MutateSoracloudState {
    service_name: Name,
    binding_name: Name,
    state_key: String,
    operation: SoraStateMutationOperationV1,
    value_size_bytes: Option<u64>,
    value_payload: Option<Vec<u8>>,
    encryption: SoraStateEncryptionV1,
    governance_tx_hash: Hash,
    fhe_input_admission_proof: Option<SoracloudFheInputAdmissionProofV1>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RunSoracloudFheJob {
    service_name: Name,
    binding_name: Name,
    job: FheJobSpecV1,
    policy_reference: SoracloudFhePolicyReferenceV1,
    public_key_proof: Option<SoracloudFhePublicKeyProofV1>,
    bootstrap_key_zero_refresh_proof: Option<SoracloudFheBootstrapKeyProofV1>,
    full_bootstrap_execution_proofs: Vec<SoracloudFheFullBootstrapExecutionProofV1>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RegisterSoracloudFhePolicy {
    service_name: Name,
    material: SoracloudFheGovernedMaterialV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RotateSoracloudFhePolicy {
    service_name: Name,
    expected_active: SoracloudFhePolicyReferenceV1,
    material: SoracloudFheGovernedMaterialV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RevokeSoracloudFhePolicy {
    service_name: Name,
    expected_active: SoracloudFhePolicyReferenceV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RecordSoracloudDecryptionRequest {
    service_name: Name,
    policy: DecryptionAuthorityPolicyV1,
    request: DecryptionRequestV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(JoinSoracloudHfSharedLease {
    repo_id: String,
    resolved_revision: String,
    model_name: String,
    service_name: Name,
    apartment_name: Option<Name>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: AssetDefinitionId,
    base_fee: Quantity,
    resource_profile: Option<SoraHfResourceProfileV1>,
    max_compute_reservation_fee: Quantity,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(LeaveSoracloudHfSharedLease {
    repo_id: String,
    resolved_revision: String,
    storage_class: StorageClass,
    lease_term_ms: u64,
    service_name: Option<Name>,
    apartment_name: Option<Name>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RenewSoracloudHfSharedLease {
    repo_id: String,
    resolved_revision: String,
    model_name: String,
    service_name: Name,
    apartment_name: Option<Name>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: AssetDefinitionId,
    base_fee: Quantity,
    resource_profile: Option<SoraHfResourceProfileV1>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(AdvertiseSoracloudModelHost {
    capability: SoraModelHostCapabilityRecordV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(HeartbeatSoracloudModelHost {
    validator_account_id: AccountId,
    heartbeat_expires_at_ms: u64,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(WithdrawSoracloudModelHost {
    validator_account_id: AccountId,
    provenance: ManifestProvenance,
});
impl_soracloud_unit_decode_from_slice!(ReconcileSoracloudModelHosts);
impl_soracloud_decode_from_slice!(AdvertiseSoracloudInrouHost {
    capability: SoraInrouHostCapabilityRecordV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(WithdrawSoracloudInrouHost {
    validator_account_id: AccountId,
    provenance: ManifestProvenance,
});
impl_soracloud_unit_decode_from_slice!(ReconcileSoracloudInrouPlacements);
impl_soracloud_decode_from_slice!(ReportSoracloudModelHostViolation {
    validator_account_id: AccountId,
    kind: SoraModelHostViolationKindV1,
    placement_id: Option<Hash>,
    detail: Option<String>,
});
impl_soracloud_decode_from_slice!(DeploySoracloudAgentApartment {
    manifest: AgentApartmentManifestV1,
    lease_ticks: u64,
    autonomy_budget_units: u64,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RenewSoracloudAgentLease {
    apartment_name: Name,
    lease_ticks: u64,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RestartSoracloudAgentApartment {
    apartment_name: Name,
    reason: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RevokeSoracloudAgentPolicy {
    apartment_name: Name,
    capability: String,
    reason: Option<String>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RequestSoracloudAgentWalletSpend {
    apartment_name: Name,
    asset_definition: String,
    amount: Quantity,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(ApproveSoracloudAgentWalletSpend {
    apartment_name: Name,
    request_id: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(EnqueueSoracloudAgentMessage {
    from_apartment: Name,
    to_apartment: Name,
    channel: String,
    payload: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(AcknowledgeSoracloudAgentMessage {
    apartment_name: Name,
    message_id: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(AllowSoracloudAgentAutonomyArtifact {
    apartment_name: Name,
    artifact_hash: String,
    provenance_hash: Option<String>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RunSoracloudAgentAutonomy {
    apartment_name: Name,
    artifact_hash: String,
    provenance_hash: Option<String>,
    budget_units: u64,
    run_label: String,
    workflow_input_json: Option<String>,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RecordSoracloudAgentAutonomyExecution {
    apartment_name: Name,
    run_id: String,
    process_generation: u64,
    succeeded: bool,
    result_commitment: Hash,
    service_name: Option<Name>,
    service_version: Option<String>,
    handler_name: Option<Name>,
    runtime_receipt_id: Option<Hash>,
    journal_artifact_hash: Option<Hash>,
    checkpoint_artifact_hash: Option<Hash>,
    error: Option<String>,
});
impl_soracloud_decode_from_slice!(StartSoracloudTrainingJob {
    service_name: Name,
    model_name: String,
    job_id: String,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(CheckpointSoracloudTrainingJob {
    service_name: Name,
    job_id: String,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RetrySoracloudTrainingJob {
    service_name: Name,
    job_id: String,
    reason: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RegisterSoracloudModelArtifact {
    service_name: Name,
    model_name: String,
    training_job_id: String,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RegisterSoracloudModelWeight {
    service_name: Name,
    model_name: String,
    weight_version: String,
    training_job_id: String,
    parent_version: Option<String>,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(PromoteSoracloudModelWeight {
    service_name: Name,
    model_name: String,
    weight_version: String,
    gate_approved: bool,
    gate_report_hash: Hash,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RollbackSoracloudModelWeight {
    service_name: Name,
    model_name: String,
    target_version: String,
    reason: String,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(RegisterSoracloudUploadedModelBundle {
    bundle: SoraUploadedModelBundleV1,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(FinalizeSoracloudUploadedModelBundle {
    service_name: Name,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    bundle_root: Hash,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(AdvanceSoracloudRollout {
    service_name: Name,
    rollout_handle: String,
    healthy: bool,
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
    provenance: ManifestProvenance,
});
impl_soracloud_decode_from_slice!(SetSoracloudRuntimeState {
    state: SoraServiceRuntimeStateV1,
});
impl_soracloud_decode_from_slice!(SetSoracloudInrouReplicaRuntimeState {
    state: SoraInrouReplicaRuntimeStateV1,
});
impl_soracloud_decode_from_slice!(ClearSoracloudInrouReplicaRuntimeState {
    service_name: Name,
    service_version: String,
    replica_slot: u16,
});
impl_soracloud_decode_from_slice!(ReportSoracloudServiceLeaseUsage {
    service_name: Name,
    reporting_epoch: u64,
    active_service_version: String,
    replica_slot: u16,
    replica_accounted_egress_bytes: u64,
    finalize_reporter: bool,
});
impl_soracloud_decode_from_slice!(RecordSoracloudMailboxMessage {
    message: SoraServiceMailboxMessageV1,
});
impl_soracloud_decode_from_slice!(RecordSoracloudRuntimeReceipt {
    receipt: SoraRuntimeReceiptV1,
});
impl_soracloud_decode_from_slice!(RecordSoracloudPrivateUploadedModelExecutionReceipt {
    receipt: SoraPrivateUploadedModelExecutionReceiptV1,
});
#[cfg(test)]
mod tests {
    use super::*;
    use crate::isi::test_support::{assert_registry_decodes, assert_slice_roundtrip};
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    fn name(raw: &str) -> Name {
        raw.parse().expect("valid name")
    }
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked Soracloud ISI fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn hash(label: &str) -> Hash {
        Hash::new(label)
    }
    fn provenance(seed: u8) -> ManifestProvenance {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked Soracloud ISI provenance fixture keypair");
        let signature = Signature::try_new(key_pair.private_key(), b"soracloud-isi-slice")
            .expect("checked Soracloud ISI provenance fixture signature");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        }
    }
    #[cfg(feature = "json")]
    fn deployment_bundle() -> SoraDeploymentBundleV1 {
        norito::json::from_str(include_str!(
            "../../../../fixtures/soracloud/sora_deployment_bundle_v1.json"
        ))
        .expect("canonical deployment bundle fixture")
    }
    fn private_uploaded_model_execution_receipt()
    -> RecordSoracloudPrivateUploadedModelExecutionReceipt {
        RecordSoracloudPrivateUploadedModelExecutionReceipt {
            receipt: SoraPrivateUploadedModelExecutionReceiptV1 {
                schema_version:
                    crate::soracloud::SORA_PRIVATE_UPLOADED_MODEL_EXECUTION_RECEIPT_VERSION_V1,
                receipt_id: hash("private-receipt"),
                service_name: name("portal"),
                model_id: "upload-1".to_owned(),
                weight_version: "v1".to_owned(),
                runtime_version: "soracloud.quantized-cpu.v1".to_owned(),
                model_manifest_digest: crate::sorafs::pin_registry::ManifestDigest::new([0xA5; 32]),
                model_bundle_root: hash("bundle"),
                policy_id: "policy/v1".to_owned(),
                input_artifact: crate::soracloud::SoraPrivateModelArtifactRefV1 {
                    schema_version: crate::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
                    sorafs_manifest_digest: crate::sorafs::pin_registry::ManifestDigest::new(
                        [0xB1; 32],
                    ),
                    artifact_hash: hash("input-artifact"),
                    ciphertext_bytes: 32,
                    artifact_role: "input".to_owned(),
                },
                output_artifact: crate::soracloud::SoraPrivateModelArtifactRefV1 {
                    schema_version: crate::soracloud::SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
                    sorafs_manifest_digest: crate::sorafs::pin_registry::ManifestDigest::new(
                        [0xB2; 32],
                    ),
                    artifact_hash: hash("output-artifact"),
                    ciphertext_bytes: 32,
                    artifact_role: "output".to_owned(),
                },
                input_commitment: hash("input"),
                output_commitment: hash("output"),
                request_commitment: hash("request"),
                result_commitment: hash("result"),
                emitted_sequence: 1,
            },
        }
    }
    #[test]
    fn soracloud_decode_from_slice_roundtrips_simple_instructions() {
        assert_slice_roundtrip(RollbackSoracloudService {
            service_name: name("portal"),
            target_version: "2026.5".to_owned(),
            provenance: provenance(1),
        });
        assert_slice_roundtrip(SetSoracloudServiceConfig {
            service_name: name("portal"),
            config_name: "replicas".to_owned(),
            value_json: Json::from(norito::json!({"target": 2_u64})),
            provenance: provenance(2),
        });
        assert_slice_roundtrip(DeleteSoracloudServiceSecret {
            service_name: name("portal"),
            secret_name: "openai_api_key".to_owned(),
            provenance: provenance(3),
        });
        assert_slice_roundtrip(HeartbeatSoracloudModelHost {
            validator_account_id: account(4),
            heartbeat_expires_at_ms: 42_000,
            provenance: provenance(4),
        });
        assert_slice_roundtrip(WithdrawSoracloudInrouHost {
            validator_account_id: account(5),
            provenance: provenance(5),
        });
        assert_slice_roundtrip(ReconcileSoracloudModelHosts);
        assert_slice_roundtrip(ReconcileSoracloudInrouPlacements);
        assert_slice_roundtrip(ReportSoracloudModelHostViolation {
            validator_account_id: account(6),
            kind: SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
            placement_id: Some(hash("placement")),
            detail: Some("heartbeat expired".to_owned()),
        });
        assert_slice_roundtrip(RevokeSoracloudAgentPolicy {
            apartment_name: name("agent_home"),
            capability: "wallet.spend".to_owned(),
            reason: Some("limit exceeded".to_owned()),
            provenance: provenance(7),
        });
        assert_slice_roundtrip(RecordSoracloudAgentAutonomyExecution {
            apartment_name: name("agent_home"),
            run_id: "run-1".to_owned(),
            process_generation: 9,
            succeeded: true,
            result_commitment: hash("result"),
            service_name: Some(name("generated_service")),
            service_version: Some("1".to_owned()),
            handler_name: Some(name("infer")),
            runtime_receipt_id: Some(hash("receipt")),
            journal_artifact_hash: Some(hash("journal")),
            checkpoint_artifact_hash: Some(hash("checkpoint")),
            error: None,
        });
        assert_slice_roundtrip(RegisterSoracloudModelWeight {
            service_name: name("portal"),
            model_name: "vision".to_owned(),
            weight_version: "v2".to_owned(),
            training_job_id: "job-1".to_owned(),
            parent_version: Some("v1".to_owned()),
            weight_artifact_hash: hash("weight"),
            dataset_ref: "dataset://train".to_owned(),
            training_config_hash: hash("config"),
            reproducibility_hash: hash("repro"),
            provenance_attestation_hash: hash("attestation"),
            provenance: provenance(8),
        });
        assert_slice_roundtrip(ClearSoracloudInrouReplicaRuntimeState {
            service_name: name("portal"),
            service_version: "2026.5".to_owned(),
            replica_slot: 1,
        });
        assert_slice_roundtrip(ReportSoracloudServiceLeaseUsage {
            service_name: name("portal"),
            reporting_epoch: 1,
            active_service_version: "2026.5".to_owned(),
            replica_slot: 1,
            replica_accounted_egress_bytes: 4096,
            finalize_reporter: false,
        });
        assert_slice_roundtrip(private_uploaded_model_execution_receipt());
    }
    #[cfg(feature = "json")]
    #[test]
    fn rollback_instruction_v1_requires_one_explicit_closed_target() {
        let rollback = RollbackSoracloudService {
            service_name: name("portal"),
            target_version: "2026.5".to_owned(),
            provenance: provenance(9),
        };
        let canonical = norito::json::to_value(&rollback).expect("serialize rollback instruction");

        let mut unknown = canonical.clone();
        unknown
            .as_object_mut()
            .expect("rollback instruction JSON object")
            .insert(
                "previous_service_version".to_owned(),
                norito::json!("2026.4"),
            );
        norito::json::from_value::<RollbackSoracloudService>(unknown)
            .expect_err("retired rollback history fields must be rejected");

        let mut missing = canonical.clone();
        assert!(
            missing
                .as_object_mut()
                .expect("rollback instruction JSON object")
                .remove("target_version")
                .is_some()
        );
        norito::json::from_value::<RollbackSoracloudService>(missing)
            .expect_err("rollback target_version must not be omitted");

        let mut null = canonical;
        null.as_object_mut()
            .expect("rollback instruction JSON object")
            .insert("target_version".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<RollbackSoracloudService>(null)
            .expect_err("rollback target_version must not be null");
    }
    #[cfg(feature = "json")]
    #[test]
    fn soracloud_inrou_deployment_and_rollout_v1_reject_unknown_fields() {
        macro_rules! assert_unknown_rejected {
            ($ty:ty, $label:literal) => {{
                let error = norito::json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                    .expect_err(concat!($label, " must reject unknown fields"));
                assert!(
                    matches!(
                        error,
                        norito::json::Error::UnknownField { ref field }
                            if field == "retired_v0"
                    ),
                    "{} reported the wrong error: {error:?}",
                    $label
                );
            }};
        }

        assert_unknown_rejected!(DeploySoracloudService, "Soracloud deploy instruction");
        assert_unknown_rejected!(UpgradeSoracloudService, "Soracloud upgrade instruction");
        assert_unknown_rejected!(DeploySoracloudAppInfra, "Soracloud app deploy instruction");
        assert_unknown_rejected!(
            UpgradeSoracloudAppInfra,
            "Soracloud app upgrade instruction"
        );
        assert_unknown_rejected!(AdvanceSoracloudRollout, "Soracloud rollout instruction");
    }
    #[cfg(feature = "json")]
    #[test]
    fn soracloud_deploy_upgrade_and_rollout_v1_require_explicit_wire_keys() {
        macro_rules! assert_required_maps {
            ($value:expr, $ty:ty, $label:literal) => {{
                let canonical =
                    norito::json::to_value(&$value).expect(concat!("serialize ", $label));
                norito::json::from_value::<$ty>(canonical.clone()).expect(concat!(
                    "canonical ",
                    $label,
                    " must decode"
                ));
                for field in ["initial_service_configs", "initial_service_secrets"] {
                    let mut missing = canonical.clone();
                    assert!(
                        missing
                            .as_object_mut()
                            .expect(concat!($label, " JSON object"))
                            .remove(field)
                            .is_some()
                    );
                    norito::json::from_value::<$ty>(missing)
                        .expect_err(concat!($label, " must reject omitted material maps"));

                    let mut null = canonical.clone();
                    null.as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .insert(field.to_owned(), norito::json::Value::Null);
                    norito::json::from_value::<$ty>(null)
                        .expect_err(concat!($label, " must reject null material maps"));
                }
            }};
        }

        let bundle = deployment_bundle();
        assert_required_maps!(
            DeploySoracloudService {
                bundle: bundle.clone(),
                initial_service_configs: BTreeMap::new(),
                initial_service_secrets: BTreeMap::new(),
                precondition: SoraServiceMutationPreconditionV1::ServiceAbsent,
                provenance: provenance(14),
            },
            DeploySoracloudService,
            "Soracloud deploy instruction"
        );
        assert_required_maps!(
            UpgradeSoracloudService {
                bundle,
                initial_service_configs: BTreeMap::new(),
                initial_service_secrets: BTreeMap::new(),
                precondition: SoraServiceMutationPreconditionV1::ServiceAbsent,
                provenance: provenance(15),
            },
            UpgradeSoracloudService,
            "Soracloud upgrade instruction"
        );

        let rollout = AdvanceSoracloudRollout {
            service_name: name("portal"),
            rollout_handle: "portal:rollout:2".to_owned(),
            healthy: false,
            promote_to_percent: None,
            governance_tx_hash: hash("governance"),
            provenance: provenance(16),
        };
        let canonical =
            norito::json::to_value(&rollout).expect("serialize Soracloud rollout instruction");
        assert!(
            canonical
                .get("promote_to_percent")
                .is_some_and(norito::json::Value::is_null),
            "unhealthy rollout must serialize an explicit null promotion target"
        );
        norito::json::from_value::<AdvanceSoracloudRollout>(canonical.clone())
            .expect("explicit null rollout target must decode");
        let mut missing = canonical;
        assert!(
            missing
                .as_object_mut()
                .expect("rollout instruction JSON object")
                .remove("promote_to_percent")
                .is_some()
        );
        norito::json::from_value::<AdvanceSoracloudRollout>(missing)
            .expect_err("omitted rollout promotion target must be rejected");
    }
    #[cfg(feature = "json")]
    #[test]
    fn soracloud_service_control_v1_is_closed_and_requires_explicit_state_nulls() {
        macro_rules! assert_closed {
            ($value:expr, $ty:ty, $label:literal) => {{
                let mut value =
                    norito::json::to_value(&$value).expect(concat!("serialize ", $label));
                norito::json::from_value::<$ty>(value.clone()).expect(concat!(
                    "canonical ",
                    $label,
                    " must decode"
                ));
                value
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert("retired_v0".to_owned(), norito::json::Value::from(true));
                norito::json::from_value::<$ty>(value)
                    .expect_err(concat!($label, " must reject unknown fields"));
            }};
        }

        let secret = SecretEnvelopeV1 {
            schema_version: crate::soracloud::SECRET_ENVELOPE_VERSION_V1,
            encryption: crate::soracloud::SecretEnvelopeEncryptionV1::ClientCiphertext,
            key_id: "kms/test".to_owned(),
            key_version: std::num::NonZeroU32::new(1).expect("non-zero key version"),
            nonce: vec![1],
            ciphertext: vec![2],
            commitment: hash("secret"),
            aad_digest: None,
        };
        assert_closed!(
            SetSoracloudServiceConfig {
                service_name: name("portal"),
                config_name: "runtime".to_owned(),
                value_json: Json::from(norito::json!({"workers": 2_u64})),
                provenance: provenance(17),
            },
            SetSoracloudServiceConfig,
            "service config set instruction"
        );
        assert_closed!(
            DeleteSoracloudServiceConfig {
                service_name: name("portal"),
                config_name: "runtime".to_owned(),
                provenance: provenance(18),
            },
            DeleteSoracloudServiceConfig,
            "service config delete instruction"
        );
        assert_closed!(
            SetSoracloudServiceSecret {
                service_name: name("portal"),
                secret_name: "api_token".to_owned(),
                secret,
                provenance: provenance(19),
            },
            SetSoracloudServiceSecret,
            "service secret set instruction"
        );
        assert_closed!(
            DeleteSoracloudServiceSecret {
                service_name: name("portal"),
                secret_name: "api_token".to_owned(),
                provenance: provenance(20),
            },
            DeleteSoracloudServiceSecret,
            "service secret delete instruction"
        );

        let mutation = MutateSoracloudState {
            service_name: name("portal"),
            binding_name: name("private_state"),
            state_key: "/state/1".to_owned(),
            operation: SoraStateMutationOperationV1::Delete,
            value_size_bytes: None,
            value_payload: None,
            encryption: SoraStateEncryptionV1::ClientCiphertext,
            governance_tx_hash: hash("governance"),
            fhe_input_admission_proof: None,
            provenance: provenance(21),
        };
        assert_closed!(
            mutation.clone(),
            MutateSoracloudState,
            "state mutation instruction"
        );
        let canonical = norito::json::to_value(&mutation).expect("serialize state mutation");
        for field in [
            "value_size_bytes",
            "value_payload",
            "fhe_input_admission_proof",
        ] {
            assert!(
                canonical
                    .get(field)
                    .is_some_and(norito::json::Value::is_null),
                "state mutation must serialize explicit null `{field}`"
            );
            let mut missing = canonical.clone();
            assert!(
                missing
                    .as_object_mut()
                    .expect("state mutation JSON object")
                    .remove(field)
                    .is_some()
            );
            norito::json::from_value::<MutateSoracloudState>(missing)
                .expect_err("state mutation must reject an omitted nullable key");

            let mut explicit_null = canonical.clone();
            explicit_null
                .as_object_mut()
                .expect("state mutation JSON object")
                .insert(field.to_owned(), norito::json::Value::Null);
            norito::json::from_value::<MutateSoracloudState>(explicit_null)
                .expect("state mutation must accept an explicit null key");
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn soracloud_fhe_job_and_decryption_v1_are_closed_and_require_explicit_proofs() {
        let job = RunSoracloudFheJob {
            service_name: name("portal"),
            binding_name: name("private_state"),
            job: norito::json::from_str(include_str!(
                "../../../../fixtures/soracloud/fhe_job_spec_v1.json"
            ))
            .expect("canonical FHE job fixture"),
            policy_reference: SoracloudFhePolicyReferenceV1 {
                schema_version: crate::soracloud::SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1,
                policy_name: name("health_policy"),
                version: std::num::NonZeroU32::new(1).expect("non-zero policy version"),
                material_digest: hash("governed-fhe-material-v1"),
            },
            public_key_proof: None,
            bootstrap_key_zero_refresh_proof: None,
            full_bootstrap_execution_proofs: Vec::new(),
            provenance: provenance(22),
        };
        let canonical = norito::json::to_value(&job).expect("serialize FHE job instruction");
        norito::json::from_value::<RunSoracloudFheJob>(canonical.clone())
            .expect("canonical FHE job instruction must decode");
        for field in [
            "public_key_proof",
            "bootstrap_key_zero_refresh_proof",
            "full_bootstrap_execution_proofs",
        ] {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("FHE job instruction object")
                .remove(field);
            norito::json::from_value::<RunSoracloudFheJob>(missing)
                .expect_err("an omitted FHE proof key must fail");
        }
        for field in ["public_key_proof", "bootstrap_key_zero_refresh_proof"] {
            assert!(
                canonical
                    .get(field)
                    .is_some_and(norito::json::Value::is_null),
                "absent optional proof `{field}` must serialize as explicit null"
            );
        }
        assert_eq!(
            canonical
                .get("full_bootstrap_execution_proofs")
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(0),
            "an empty execution-proof list must serialize explicitly"
        );
        let mut unknown_job = canonical;
        unknown_job
            .as_object_mut()
            .expect("FHE job instruction object")
            .insert("legacy_proof".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<RunSoracloudFheJob>(unknown_job)
            .expect_err("an unknown FHE job instruction key must fail");

        let decryption = RecordSoracloudDecryptionRequest {
            service_name: name("portal"),
            policy: norito::json::from_str(include_str!(
                "../../../../fixtures/soracloud/decryption_authority_policy_v1.json"
            ))
            .expect("canonical decryption policy fixture"),
            request: norito::json::from_str(include_str!(
                "../../../../fixtures/soracloud/decryption_request_v1.json"
            ))
            .expect("canonical decryption request fixture"),
            provenance: provenance(23),
        };
        let mut unknown_decryption =
            norito::json::to_value(&decryption).expect("serialize decryption instruction");
        unknown_decryption
            .as_object_mut()
            .expect("decryption instruction object")
            .insert("legacy_request".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<RecordSoracloudDecryptionRequest>(unknown_decryption)
            .expect_err("an unknown decryption instruction key must fail");
    }
    #[cfg(feature = "json")]
    #[test]
    fn authenticated_soracloud_instruction_graph_rejects_unknown_fields() {
        macro_rules! assert_unknown_rejected {
            ($ty:ty, $label:literal) => {{
                let error = norito::json::from_str::<$ty>(r#"{"retired_v0":true}"#)
                    .expect_err(concat!($label, " must reject unknown fields"));
                assert!(
                    matches!(
                        error,
                        norito::json::Error::UnknownField { ref field }
                            if field == "retired_v0"
                    ),
                    "{} reported the wrong error: {error:?}",
                    $label
                );
            }};
        }

        assert_unknown_rejected!(
            RegisterSoracloudFhePolicy,
            "FHE policy-register instruction"
        );
        assert_unknown_rejected!(RotateSoracloudFhePolicy, "FHE policy-rotate instruction");
        assert_unknown_rejected!(RevokeSoracloudFhePolicy, "FHE policy-revoke instruction");
        assert_unknown_rejected!(JoinSoracloudHfSharedLease, "HF lease join instruction");
        assert_unknown_rejected!(LeaveSoracloudHfSharedLease, "HF lease leave instruction");
        assert_unknown_rejected!(RenewSoracloudHfSharedLease, "HF lease renew instruction");
        assert_unknown_rejected!(AdvertiseSoracloudModelHost, "model-host advert instruction");
        assert_unknown_rejected!(
            HeartbeatSoracloudModelHost,
            "model-host heartbeat instruction"
        );
        assert_unknown_rejected!(
            WithdrawSoracloudModelHost,
            "model-host withdraw instruction"
        );
        assert_unknown_rejected!(
            ReportSoracloudModelHostViolation,
            "model-host violation instruction"
        );
        assert_unknown_rejected!(DeploySoracloudAgentApartment, "agent deploy instruction");
        assert_unknown_rejected!(RenewSoracloudAgentLease, "agent lease-renew instruction");
        assert_unknown_rejected!(RestartSoracloudAgentApartment, "agent restart instruction");
        assert_unknown_rejected!(
            RevokeSoracloudAgentPolicy,
            "agent policy-revoke instruction"
        );
        assert_unknown_rejected!(
            RequestSoracloudAgentWalletSpend,
            "agent wallet-spend instruction"
        );
        assert_unknown_rejected!(
            ApproveSoracloudAgentWalletSpend,
            "agent wallet-approve instruction"
        );
        assert_unknown_rejected!(
            EnqueueSoracloudAgentMessage,
            "agent message-send instruction"
        );
        assert_unknown_rejected!(
            AcknowledgeSoracloudAgentMessage,
            "agent message-ack instruction"
        );
        assert_unknown_rejected!(
            AllowSoracloudAgentAutonomyArtifact,
            "agent artifact-allow instruction"
        );
        assert_unknown_rejected!(RunSoracloudAgentAutonomy, "agent autonomy-run instruction");
        assert_unknown_rejected!(
            RecordSoracloudAgentAutonomyExecution,
            "agent autonomy-execution instruction"
        );
        assert_unknown_rejected!(StartSoracloudTrainingJob, "training start instruction");
        assert_unknown_rejected!(
            CheckpointSoracloudTrainingJob,
            "training checkpoint instruction"
        );
        assert_unknown_rejected!(RetrySoracloudTrainingJob, "training retry instruction");
        assert_unknown_rejected!(
            RegisterSoracloudModelArtifact,
            "model-artifact register instruction"
        );
        assert_unknown_rejected!(
            RegisterSoracloudModelWeight,
            "model-weight register instruction"
        );
        assert_unknown_rejected!(
            PromoteSoracloudModelWeight,
            "model-weight promote instruction"
        );
        assert_unknown_rejected!(
            RollbackSoracloudModelWeight,
            "model-weight rollback instruction"
        );
        assert_unknown_rejected!(
            RegisterSoracloudUploadedModelBundle,
            "uploaded-model register instruction"
        );
        assert_unknown_rejected!(
            FinalizeSoracloudUploadedModelBundle,
            "uploaded-model finalize instruction"
        );
        assert_unknown_rejected!(SetSoracloudRuntimeState, "runtime-state instruction");
        assert_unknown_rejected!(
            ReportSoracloudServiceLeaseUsage,
            "service lease-usage instruction"
        );
        assert_unknown_rejected!(RecordSoracloudMailboxMessage, "mailbox-record instruction");
        assert_unknown_rejected!(RecordSoracloudRuntimeReceipt, "runtime-receipt instruction");
        assert_unknown_rejected!(
            RecordSoracloudPrivateUploadedModelExecutionReceipt,
            "private uploaded-model receipt instruction"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn authenticated_soracloud_instruction_graph_requires_explicit_nullable_keys() {
        macro_rules! assert_required_nulls {
            ($value:expr, $ty:ty, [$($field:literal),+ $(,)?], $label:literal) => {{
                let canonical =
                    norito::json::to_value(&$value).expect(concat!("serialize ", $label));
                norito::json::from_value::<$ty>(canonical.clone())
                    .expect(concat!("canonical ", $label, " must decode"));
                $(
                    assert!(
                        canonical
                            .get($field)
                            .is_some_and(norito::json::Value::is_null),
                        "{} must serialize `{}` as explicit null",
                        $label,
                        $field
                    );
                    let mut missing = canonical.clone();
                    assert!(
                        missing
                            .as_object_mut()
                            .expect(concat!($label, " JSON object"))
                            .remove($field)
                            .is_some()
                    );
                    norito::json::from_value::<$ty>(missing).expect_err(concat!(
                        $label,
                        " must reject an omitted nullable key"
                    ));

                    let mut explicit_null = canonical.clone();
                    explicit_null
                        .as_object_mut()
                        .expect(concat!($label, " JSON object"))
                        .insert($field.to_owned(), norito::json::Value::Null);
                    norito::json::from_value::<$ty>(explicit_null).expect(concat!(
                        $label,
                        " must accept an explicit null key"
                    ));
                )+
            }};
        }

        let lease_asset_definition_id = AssetDefinitionId::from_uuid_bytes([
            0xF0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 0xF2,
        ])
        .expect("fixed fixture asset identifier is canonical UUIDv4");
        let nominal_fee: Quantity = "1".parse().expect("valid nominal fee");
        assert_required_nulls!(
            JoinSoracloudHfSharedLease {
                repo_id: "openai/gpt-oss".to_owned(),
                resolved_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
                model_name: "gpt-oss".to_owned(),
                service_name: name("portal"),
                apartment_name: None,
                storage_class: StorageClass::Warm,
                lease_term_ms: 60_000,
                lease_asset_definition_id: lease_asset_definition_id.clone(),
                base_fee: nominal_fee.clone(),
                resource_profile: None,
                max_compute_reservation_fee: nominal_fee.clone(),
                provenance: provenance(24),
            },
            JoinSoracloudHfSharedLease,
            ["apartment_name", "resource_profile"],
            "HF lease join instruction"
        );
        assert_required_nulls!(
            LeaveSoracloudHfSharedLease {
                repo_id: "openai/gpt-oss".to_owned(),
                resolved_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
                storage_class: StorageClass::Warm,
                lease_term_ms: 60_000,
                service_name: None,
                apartment_name: None,
                provenance: provenance(25),
            },
            LeaveSoracloudHfSharedLease,
            ["service_name", "apartment_name"],
            "HF lease leave instruction"
        );
        assert_required_nulls!(
            RenewSoracloudHfSharedLease {
                repo_id: "openai/gpt-oss".to_owned(),
                resolved_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
                model_name: "gpt-oss".to_owned(),
                service_name: name("portal"),
                apartment_name: None,
                storage_class: StorageClass::Warm,
                lease_term_ms: 60_000,
                lease_asset_definition_id,
                base_fee: nominal_fee,
                resource_profile: None,
                provenance: provenance(26),
            },
            RenewSoracloudHfSharedLease,
            ["apartment_name", "resource_profile"],
            "HF lease renew instruction"
        );
        assert_required_nulls!(
            ReportSoracloudModelHostViolation {
                validator_account_id: account(27),
                kind: SoraModelHostViolationKindV1::AssignedHeartbeatMiss,
                placement_id: None,
                detail: None,
            },
            ReportSoracloudModelHostViolation,
            ["placement_id", "detail"],
            "model-host violation instruction"
        );
        assert_required_nulls!(
            RevokeSoracloudAgentPolicy {
                apartment_name: name("agent_home"),
                capability: "wallet.spend".to_owned(),
                reason: None,
                provenance: provenance(28),
            },
            RevokeSoracloudAgentPolicy,
            ["reason"],
            "agent policy-revoke instruction"
        );
        assert_required_nulls!(
            AllowSoracloudAgentAutonomyArtifact {
                apartment_name: name("agent_home"),
                artifact_hash: "artifact-v1".to_owned(),
                provenance_hash: None,
                provenance: provenance(29),
            },
            AllowSoracloudAgentAutonomyArtifact,
            ["provenance_hash"],
            "agent artifact-allow instruction"
        );
        assert_required_nulls!(
            RunSoracloudAgentAutonomy {
                apartment_name: name("agent_home"),
                artifact_hash: "artifact-v1".to_owned(),
                provenance_hash: None,
                budget_units: 10,
                run_label: "nightly".to_owned(),
                workflow_input_json: None,
                provenance: provenance(30),
            },
            RunSoracloudAgentAutonomy,
            ["provenance_hash", "workflow_input_json"],
            "agent autonomy-run instruction"
        );
        assert_required_nulls!(
            RecordSoracloudAgentAutonomyExecution {
                apartment_name: name("agent_home"),
                run_id: "run-1".to_owned(),
                process_generation: 1,
                succeeded: false,
                result_commitment: hash("failed-result"),
                service_name: None,
                service_version: None,
                handler_name: None,
                runtime_receipt_id: None,
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                error: None,
            },
            RecordSoracloudAgentAutonomyExecution,
            [
                "service_name",
                "service_version",
                "handler_name",
                "runtime_receipt_id",
                "journal_artifact_hash",
                "checkpoint_artifact_hash",
                "error",
            ],
            "agent autonomy-execution instruction"
        );
        assert_required_nulls!(
            RegisterSoracloudModelWeight {
                service_name: name("portal"),
                model_name: "vision".to_owned(),
                weight_version: "v2".to_owned(),
                training_job_id: "job-1".to_owned(),
                parent_version: None,
                weight_artifact_hash: hash("weight"),
                dataset_ref: "dataset://train".to_owned(),
                training_config_hash: hash("config"),
                reproducibility_hash: hash("repro"),
                provenance_attestation_hash: hash("attestation"),
                provenance: provenance(31),
            },
            RegisterSoracloudModelWeight,
            ["parent_version"],
            "model-weight register instruction"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn inrou_instruction_v1_wrappers_reject_unknown_fields() {
        macro_rules! assert_unknown_rejected {
            ($value:expr, $ty:ty, $label:literal) => {{
                let mut value =
                    norito::json::to_value(&$value).expect(concat!("serialize ", $label));
                value
                    .as_object_mut()
                    .expect(concat!($label, " JSON object"))
                    .insert("retired_v0".to_owned(), norito::json!(true));
                norito::json::from_value::<$ty>(value)
                    .expect_err(concat!($label, " must reject unknown fields"));
            }};
        }

        let validator = account(11);
        let peer_id =
            crate::peer::PeerId::from(validator.expect_single_signatory().clone()).to_string();
        assert_unknown_rejected!(
            AdvertiseSoracloudInrouHost {
                capability: SoraInrouHostCapabilityRecordV1 {
                    schema_version: crate::soracloud::SORA_INROU_HOST_CAPABILITY_RECORD_VERSION_V1,
                    validator_account_id: validator.clone(),
                    peer_id: peer_id.clone(),
                    supported_guest_isas: std::collections::BTreeSet::from([
                        crate::soracloud::SoraInrouGuestIsaV1::Aarch64,
                    ]),
                    max_hosted_replica_capacity:
                        crate::soracloud::SORA_INROU_HOSTED_REPLICA_CAPACITY_V1,
                    max_cpu_millis: 8_000,
                    max_memory_bytes: 8 * 1024 * 1024 * 1024,
                    max_storage_bytes: 64 * 1024 * 1024 * 1024,
                    geography_tags: std::collections::BTreeSet::new(),
                    observed_latency_ms: None,
                    advertised_at_ms: 1,
                    heartbeat_expires_at_ms: 2,
                },
                provenance: provenance(12),
            },
            AdvertiseSoracloudInrouHost,
            "Inrou host advert instruction"
        );
        assert_unknown_rejected!(
            WithdrawSoracloudInrouHost {
                validator_account_id: validator.clone(),
                provenance: provenance(13),
            },
            WithdrawSoracloudInrouHost,
            "Inrou host withdrawal instruction"
        );
        assert_unknown_rejected!(
            SetSoracloudInrouReplicaRuntimeState {
                state: SoraInrouReplicaRuntimeStateV1 {
                    schema_version: crate::soracloud::SORA_INROU_REPLICA_RUNTIME_STATE_VERSION_V1,
                    service_name: name("portal"),
                    service_version: "2026.5".to_owned(),
                    replica_slot: 1,
                    validator_account_id: validator,
                    peer_id,
                    selected_guest_isa: crate::soracloud::SoraInrouGuestIsaV1::Aarch64,
                    health_status: crate::soracloud::SoraServiceHealthStatusV1::Healthy,
                    load_factor_bps: 0,
                    materialized_bundle_hash: hash("bundle"),
                    reporting_epoch: 1,
                    accounted_egress_bytes: 0,
                    pending_mailbox_message_count: 0,
                    last_receipt_id: None,
                    updated_at_ms: 1,
                    last_error: None,
                },
            },
            SetSoracloudInrouReplicaRuntimeState,
            "Inrou runtime-state set instruction"
        );
        assert_unknown_rejected!(
            ClearSoracloudInrouReplicaRuntimeState {
                service_name: name("portal"),
                service_version: "2026.5".to_owned(),
                replica_slot: 1,
            },
            ClearSoracloudInrouReplicaRuntimeState,
            "Inrou runtime-state clear instruction"
        );
        norito::json::from_str::<ReconcileSoracloudInrouPlacements>(r#"{"retired_v0":true}"#)
            .expect_err("Inrou placement reconciliation must reject object fields");
    }
    #[test]
    fn soracloud_default_registry_decodes_type_names_and_stable_ids() {
        let registry = crate::isi::registry::default();
        let rollback = RollbackSoracloudService {
            service_name: name("portal"),
            target_version: "2026.5".to_owned(),
            provenance: provenance(9),
        };
        assert_registry_decodes(
            &registry,
            std::any::type_name::<RollbackSoracloudService>(),
            rollback.clone(),
        );
        assert_registry_decodes(&registry, "soracloud::RollbackSoracloudService", rollback);
        assert_registry_decodes(
            &registry,
            std::any::type_name::<SetSoracloudServiceConfig>(),
            SetSoracloudServiceConfig {
                service_name: name("portal"),
                config_name: "replicas".to_owned(),
                value_json: Json::from(norito::json!({"target": 3_u64})),
                provenance: provenance(10),
            },
        );
        assert_registry_decodes(
            &registry,
            std::any::type_name::<ReconcileSoracloudInrouPlacements>(),
            ReconcileSoracloudInrouPlacements,
        );
        assert_registry_decodes(
            &registry,
            "soracloud::ReconcileSoracloudInrouPlacements",
            ReconcileSoracloudInrouPlacements,
        );
    }
}

//! Soracloud lifecycle instructions.
//!
//! These instructions move Soracloud service deployment state into the
//! authoritative on-chain world model instead of Torii-local file persistence.

use core::cmp::Ordering;
use std::collections::BTreeMap;

use iroha_crypto::{Hash, fhe_bfv::BfvEvaluationKeyBundle};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    name::Name,
    smart_contract::manifest::ManifestProvenance,
    soracloud::{
        AgentApartmentManifestV1, DecryptionAuthorityPolicyV1, DecryptionRequestV1,
        FheExecutionPolicyV1, FheJobSpecV1, FheParamSetV1, SecretEnvelopeV1,
        SoraAppInfraManifestV1, SoraDeploymentBundleV1, SoraHfResourceProfileV1,
        SoraInrouHostCapabilityRecordV1, SoraInrouReplicaRuntimeStateV1,
        SoraModelHostCapabilityRecordV1, SoraModelHostViolationKindV1,
        SoraPrivateUploadedModelExecutionReceiptV1, SoraRuntimeReceiptV1,
        SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1, SoraStateEncryptionV1,
        SoraStateMutationOperationV1, SoraUploadedModelBundleV1,
    },
    sorafs::pin_registry::StorageClass,
};

fn encoded_order<T: Encode>(left: &T, right: &T) -> Ordering {
    left.encode().cmp(&right.encode())
}

fn decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

/// Admit a brand new Soracloud service deployment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DeploySoracloudService {
    /// Bundle being admitted.
    pub bundle: SoraDeploymentBundleV1,
    /// Optional authoritative config entries committed atomically with this deploy.
    #[norito(default)]
    pub initial_service_configs: BTreeMap<String, Json>,
    /// Optional authoritative secret entries committed atomically with this deploy.
    #[norito(default)]
    pub initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct UpgradeSoracloudService {
    /// Bundle being admitted as the candidate revision.
    pub bundle: SoraDeploymentBundleV1,
    /// Optional authoritative config entries committed atomically with this upgrade.
    #[norito(default)]
    pub initial_service_configs: BTreeMap<String, Json>,
    /// Optional authoritative secret entries committed atomically with this upgrade.
    #[norito(default)]
    pub initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DeploySoracloudAppInfra {
    /// App topology manifest being admitted.
    pub manifest: SoraAppInfraManifestV1,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct UpgradeSoracloudAppInfra {
    /// App topology manifest being admitted.
    pub manifest: SoraAppInfraManifestV1,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RollbackSoracloudService {
    /// Service to roll back.
    pub service_name: Name,
    /// Optional target version. When omitted, the latest non-current baseline is used.
    #[norito(default)]
    pub target_version: Option<String>,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    #[norito(default)]
    pub value_size_bytes: Option<u64>,
    /// Full payload bytes for upsert operations.
    #[norito(default)]
    pub value_payload: Option<Vec<u8>>,
    /// Expected binding encryption mode.
    pub encryption: SoraStateEncryptionV1,
    /// Governance transaction hash attached to the mutation.
    pub governance_tx_hash: Hash,
    /// Provenance attestation over the mutation payload.
    pub provenance: ManifestProvenance,
}

impl crate::seal::Instruction for MutateSoracloudState {}

impl PartialOrd for MutateSoracloudState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Record an ordered Soracloud FHE execution result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RunSoracloudFheJob {
    /// Service whose ciphertext state receives the job output.
    pub service_name: Name,
    /// Binding receiving the deterministic ciphertext output.
    pub binding_name: Name,
    /// Deterministic FHE job specification.
    pub job: FheJobSpecV1,
    /// Execution policy snapshot validated for this job.
    pub policy: FheExecutionPolicyV1,
    /// Parameter set validated for this job.
    pub param_set: FheParamSetV1,
    /// Public evaluation keys used for homomorphic execution.
    pub evaluation_keys: BfvEvaluationKeyBundle,
    /// Governance transaction hash attached to the job.
    pub governance_tx_hash: Hash,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    #[norito(default)]
    pub apartment_name: Option<Name>,
    /// Requested shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window price in nanos of `lease_asset_definition_id`.
    pub base_fee_nanos: u128,
    /// Canonical HF resource profile derived by the control plane.
    #[norito(default)]
    pub resource_profile: Option<SoraHfResourceProfileV1>,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    #[norito(default)]
    pub service_name: Option<Name>,
    /// Optional apartment binding being detached for audit context.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    #[norito(default)]
    pub apartment_name: Option<Name>,
    /// Requested shared storage class.
    pub storage_class: StorageClass,
    /// Shared lease window length in milliseconds.
    pub lease_term_ms: u64,
    /// Asset definition used for lease settlement.
    pub lease_asset_definition_id: AssetDefinitionId,
    /// Full-window price in nanos of `lease_asset_definition_id`.
    pub base_fee_nanos: u128,
    /// Canonical HF resource profile derived by the control plane.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Reconcile expired validator-host adverts against authoritative HF placements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReconcileSoracloudModelHosts;

impl crate::seal::Instruction for ReconcileSoracloudModelHosts {}

impl PartialOrd for ReconcileSoracloudModelHosts {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Advertise validator-host capabilities for authoritative Inrou placement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReconcileSoracloudInrouPlacements;

impl crate::seal::Instruction for ReconcileSoracloudInrouPlacements {}

impl PartialOrd for ReconcileSoracloudInrouPlacements {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Report authoritative evidence for a validator-host violation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReportSoracloudModelHostViolation {
    /// Validator responsible for the violation.
    pub validator_account_id: AccountId,
    /// Violation class.
    pub kind: SoraModelHostViolationKindV1,
    /// Implicated placement when the violation is placement-scoped.
    #[norito(default)]
    pub placement_id: Option<Hash>,
    /// Optional explanatory detail attached to the evidence.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RevokeSoracloudAgentPolicy {
    /// Apartment whose policy should change.
    pub apartment_name: Name,
    /// Capability identifier to revoke.
    pub capability: String,
    /// Optional human-readable reason.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RequestSoracloudAgentWalletSpend {
    /// Apartment initiating the spend.
    pub apartment_name: Name,
    /// Asset definition constrained by apartment policy.
    pub asset_definition: String,
    /// Requested spend amount in nanos.
    pub amount_nanos: u64,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AllowSoracloudAgentAutonomyArtifact {
    /// Apartment receiving the allowlist rule.
    pub apartment_name: Name,
    /// Artifact hash being allowlisted.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RunSoracloudAgentAutonomy {
    /// Apartment owning the run.
    pub apartment_name: Name,
    /// Allowlisted artifact hash being executed.
    pub artifact_hash: String,
    /// Optional provenance hash bound to the artifact.
    #[norito(default)]
    pub provenance_hash: Option<String>,
    /// Budget units approved for the run.
    pub budget_units: u64,
    /// Human-readable run label.
    pub run_label: String,
    /// Optional canonical JSON body forwarded to the generated HF `/infer` handler.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    #[norito(default)]
    pub service_name: Option<Name>,
    /// Generated service version used for execution, when locally resolved.
    #[norito(default)]
    pub service_version: Option<String>,
    /// Generated handler used for execution, when locally resolved.
    #[norito(default)]
    pub handler_name: Option<Name>,
    /// Authoritative runtime receipt referenced by the execution, when one exists.
    #[norito(default)]
    pub runtime_receipt_id: Option<Hash>,
    /// Node-local journal artifact hash, when one was persisted.
    #[norito(default)]
    pub journal_artifact_hash: Option<Hash>,
    /// Node-local checkpoint artifact hash, when one was persisted.
    #[norito(default)]
    pub checkpoint_artifact_hash: Option<Hash>,
    /// Human-readable runtime error, when execution failed.
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    #[norito(default)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AdvanceSoracloudRollout {
    /// Service whose rollout should advance.
    pub service_name: Name,
    /// Deterministic rollout handle.
    pub rollout_handle: String,
    /// Health observation for the current rollout step.
    pub healthy: bool,
    /// Optional promotion target for healthy steps.
    #[norito(default)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReportSoracloudServiceLeaseUsage {
    /// Service whose hosted-service lease should be updated.
    pub service_name: Name,
    /// Active service revision observed by the runtime.
    pub active_service_version: String,
    /// Total egress bytes accounted for the active lease so far.
    pub accounted_egress_bytes: u64,
}

impl crate::seal::Instruction for ReportSoracloudServiceLeaseUsage {}

impl PartialOrd for ReportSoracloudServiceLeaseUsage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Persist an ordered Soracloud mailbox message.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordSoracloudRuntimeReceipt {
    /// Runtime receipt to persist.
    pub receipt: SoraRuntimeReceiptV1,
}

impl crate::seal::Instruction for RecordSoracloudRuntimeReceipt {}

impl PartialOrd for RecordSoracloudRuntimeReceipt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(encoded_order(self, other))
    }
}

/// Persist an authoritative private uploaded-model execution receipt.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    provenance: ManifestProvenance,
});

impl_soracloud_decode_from_slice!(UpgradeSoracloudService {
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    provenance: ManifestProvenance,
});

impl_soracloud_decode_from_slice!(RollbackSoracloudService {
    service_name: Name,
    target_version: Option<String>,
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
    provenance: ManifestProvenance,
});

impl_soracloud_decode_from_slice!(RunSoracloudFheJob {
    service_name: Name,
    binding_name: Name,
    job: FheJobSpecV1,
    policy: FheExecutionPolicyV1,
    param_set: FheParamSetV1,
    evaluation_keys: BfvEvaluationKeyBundle,
    governance_tx_hash: Hash,
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
    base_fee_nanos: u128,
    resource_profile: Option<SoraHfResourceProfileV1>,
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
    base_fee_nanos: u128,
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
    amount_nanos: u64,
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
    active_service_version: String,
    accounted_egress_bytes: u64,
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
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use norito::core::DecodeFromSlice;

    use super::*;

    fn name(raw: &str) -> Name {
        raw.parse().expect("valid name")
    }

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn hash(label: &str) -> Hash {
        Hash::new(label)
    }

    fn provenance(seed: u8) -> ManifestProvenance {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let signature = Signature::new(key_pair.private_key(), b"soracloud-isi-slice");
        ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
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
            target_version: Some("2026.5".to_owned()),
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
            active_service_version: "2026.5".to_owned(),
            accounted_egress_bytes: 4096,
        });
        assert_slice_roundtrip(private_uploaded_model_execution_receipt());
    }

    #[test]
    fn soracloud_default_registry_decodes_type_names_and_stable_ids() {
        let registry = crate::isi::registry::default();
        let rollback = RollbackSoracloudService {
            service_name: name("portal"),
            target_version: Some("2026.5".to_owned()),
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

//! Public Torii DTOs for the first-release Offline lifecycle.

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

use iroha_crypto::Hash;
use iroha_data_model::{
    isi::offline::RegisterOfflineDeviceAttestation, transaction::signed::TransactionEntrypoint,
};

use crate::ErrorEnvelope;

pub use iroha_data_model::offline::{
    KagemushaRecipientPaymentRequestV2 as OfflineRecipientLineageRequest,
    KagemushaRecursiveSpendRedeemRequestV4 as OfflineRedeemRequest,
    KagemushaRecursiveSpendTopUpRequestV4 as OfflineTopUpRequest,
    OFFLINE_REDEEM_REQUEST_SCHEMA_NAME, OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME,
};

/// Stable public Norito schema name for the signed receiver-lineage request.
pub const OFFLINE_RECIPIENT_LINEAGE_REQUEST_SCHEMA_NAME: &str =
    "iroha.torii.v1.offline.recipient_lineage.request";
/// Stable public Norito schema name for the proof-bearing receiver-lineage response.
pub const OFFLINE_RECIPIENT_LINEAGE_RESPONSE_SCHEMA_NAME: &str =
    "iroha.torii.v1.offline.recipient_lineage.response";
/// Current proof-bearing receiver-lineage response layout.
pub const OFFLINE_RECIPIENT_LINEAGE_VERSION: u16 = 1;
/// Defensive response bound shared by maintained mobile clients.
pub const OFFLINE_RECIPIENT_LINEAGE_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Proof-bearing active registration lineage for one signed recipient request.
///
/// The committed transaction contains the exact
/// `RegisterOfflineDeviceAttestation` instruction. Its entrypoint/result Merkle
/// paths verify against `admission_block_header`, whose hash is finalized by
/// `admission_finality`. Hash strings are canonical lowercase 32-byte hex.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineRecipientRegistrationLineage {
    /// Response layout version.
    pub version: u16,
    /// Canonical digest of the exact signed payment request.
    pub request_digest: String,
    /// Iroha hash of the canonical registration archive.
    pub registration_hash: String,
    /// Governed policy hash recorded at registration admission.
    pub admission_policy_hash: String,
    /// Governed policy hash at `evaluated_block_height`.
    pub evaluated_policy_hash: String,
    /// Exact registration archive committed by the admitting transaction.
    pub registration: iroha_data_model::offline::OfflineDeviceAttestationRegistration,
    /// Height of the block containing the registration transaction.
    pub admission_block_height: u64,
    /// Header whose transaction/result roots authenticate the committed transaction.
    pub admission_block_header: iroha_data_model::block::BlockHeader,
    /// Canonical signed transaction hash, in lowercase hex.
    pub registration_transaction_hash: String,
    /// Exact committed transaction plus bounded entrypoint and result paths.
    pub registration_transaction: iroha_data_model::query::CommittedTransaction,
    /// Independently verifiable historical finality artifact for the admission block.
    pub admission_finality: iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
    /// Height of the immutable state snapshot used to resolve current policy/lifecycle.
    pub evaluated_block_height: u64,
    /// Canonical hash of the evaluated committed block.
    pub evaluated_block_hash: String,
}

impl OfflineRecipientRegistrationLineage {
    /// Verify every portable binding in this proof-bearing response.
    ///
    /// This is the maintained SDK/native-bridge verification boundary. It
    /// authenticates the signed request, exact registration tuple and
    /// lifetime, admitting transaction and both Merkle paths, admission block
    /// header, and the historical Sumeragi-v2 certificate against its frozen
    /// roster and proof-of-possession set. The caller supplies its trusted
    /// current time; the opaque local receiver-key reference is deliberately
    /// not treated as an on-chain registration field.
    pub fn verify_against(
        &self,
        request: &OfflineRecipientLineageRequest,
        verified_at_ms: u64,
        expected_evaluated_block_height: u64,
        expected_evaluated_block_hash: &[u8; 32],
    ) -> Result<(), String> {
        const MAX_MERKLE_HEIGHT: usize = 32;

        if self.version != OFFLINE_RECIPIENT_LINEAGE_VERSION
            || verified_at_ms == 0
            || expected_evaluated_block_height == 0
            || expected_evaluated_block_hash.iter().all(|byte| *byte == 0)
        {
            return Err("unsupported receiver-lineage version or verification time".to_owned());
        }
        request
            .validate_at(verified_at_ms)
            .map_err(|error| format!("recipient request validation failed: {error}"))?;
        let request_digest = request
            .digest()
            .map_err(|error| format!("recipient request digest failed: {error}"))?;
        ensure_exact_lower_hex("request_digest", &self.request_digest, &request_digest)?;

        let registration_archive = norito::to_bytes(&self.registration)
            .map_err(|error| format!("registration encoding failed: {error}"))?;
        let registration_hash = Hash::new(registration_archive);
        ensure_exact_lower_hex(
            "registration_hash",
            &self.registration_hash,
            registration_hash.as_ref(),
        )?;
        if &self.registration.account_id != request.recipient()
            || self.registration.device_id != request.receiver_device_id()
            || self.registration.asset_definition_id.as_ref() != Some(request.asset())
            || &self.registration.public_key != request.receiver_public_key()
        {
            return Err(
                "registration does not match recipient account, device, asset, and P-256 key"
                    .to_owned(),
            );
        }
        if self.registration.expires_at_ms < request.expires_at_ms()
            || self.registration.expires_at_ms <= verified_at_ms
        {
            return Err("registration does not cover the request lifetime".to_owned());
        }
        let admission_policy_hash =
            exact_lower_hex_32("admission_policy_hash", &self.admission_policy_hash)?;
        let evaluated_policy_hash =
            exact_lower_hex_32("evaluated_policy_hash", &self.evaluated_policy_hash)?;
        if admission_policy_hash != evaluated_policy_hash
            || admission_policy_hash.iter().all(|byte| *byte == 0)
        {
            return Err("registration policy was rotated or is invalid".to_owned());
        }
        if self.admission_block_height == 0
            || self.evaluated_block_height < self.admission_block_height
            || self.admission_block_header.height().get() != self.admission_block_height
        {
            return Err("registration admission/evaluation heights are inconsistent".to_owned());
        }
        let evaluated_block_hash =
            exact_lower_hex_32("evaluated_block_hash", &self.evaluated_block_hash)?;
        if self.evaluated_block_height != expected_evaluated_block_height
            || &evaluated_block_hash != expected_evaluated_block_hash
        {
            return Err("receiver lineage does not match the required readiness snapshot".to_owned());
        }

        let committed = &self.registration_transaction;
        if committed.merge_inclusion().is_some()
            || committed.block_hash() != &self.admission_block_header.hash()
            || committed.entrypoint_hash() != &committed.entrypoint().hash()
            || committed.result_hash() != &committed.result().hash()
            || committed.result().as_ref().is_err()
            || committed.entrypoint_proof().audit_path().len() > MAX_MERKLE_HEIGHT
            || committed.result_proof().audit_path().len() > MAX_MERKLE_HEIGHT
            || committed.entrypoint_proof().leaf_index()
                != committed.result_proof().leaf_index()
        {
            return Err("committed registration transaction is internally inconsistent".to_owned());
        }
        let TransactionEntrypoint::External(transaction) = committed.entrypoint() else {
            return Err("registration provenance is not an external signed transaction".to_owned());
        };
        ensure_exact_lower_hex(
            "registration_transaction_hash",
            &self.registration_transaction_hash,
            transaction.hash().as_ref(),
        )?;
        let matching_registrations = transaction
            .instructions()
            .explicit_instructions()
            .filter_map(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterOfflineDeviceAttestation>()
            })
            .filter(|instruction| instruction.registration() == &self.registration)
            .count();
        if matching_registrations != 1 {
            return Err(
                "admitting transaction must contain exactly one copy of the registration"
                    .to_owned(),
            );
        }
        let entry_root = self
            .admission_block_header
            .merkle_root()
            .ok_or_else(|| "admission header has no entrypoint Merkle root".to_owned())?;
        let result_root = self
            .admission_block_header
            .result_merkle_root()
            .ok_or_else(|| "admission header has no result Merkle root".to_owned())?;
        if !committed.entrypoint_proof().clone().verify(
            committed.entrypoint_hash(),
            &entry_root,
            MAX_MERKLE_HEIGHT,
        ) || !committed.result_proof().clone().verify(
            committed.result_hash(),
            &result_root,
            MAX_MERKLE_HEIGHT,
        ) {
            return Err("registration transaction Merkle proof is invalid".to_owned());
        }
        self.admission_finality
            .validate_for_header(&self.admission_block_header)
            .map_err(|error| format!("admission finality/header binding failed: {error}"))?;
        self.admission_finality
            .verify()
            .map_err(|error| format!("admission finality verification failed: {error}"))?;
        Ok(())
    }
}

fn exact_lower_hex_32(field: &str, value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be canonical lowercase 32-byte hex"));
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded)
        .map_err(|_| format!("{field} must be canonical lowercase 32-byte hex"))?;
    Ok(decoded)
}

fn ensure_exact_lower_hex(field: &str, value: &str, expected: &[u8]) -> Result<(), String> {
    let decoded = exact_lower_hex_32(field, value)?;
    if decoded.as_slice() != expected {
        return Err(format!("{field} does not match the canonical value"));
    }
    Ok(())
}

/// Finalized anchor returned by an applied offline top-up.
///
/// The underlying consensus wire type remains internally versioned, while the
/// first-release public transport surface exposes only this current name.
pub type OfflineTopUpAnchor = iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV4;

/// Finality proof returned with an applied offline top-up.
///
/// The first-release transport exposes the current typed consensus proof
/// directly. It is never wrapped as an opaque base64 payload and is required
/// before a wallet may initialize recursive spending from the returned anchor.
pub type OfflineTopUpFinalityProof = iroha_data_model::offline::KagemushaTopUpFinalityProofV2;

/// One machine-readable reason why an asset is not ready for offline payments.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineReadinessBlocker {
    /// Stable SDK-facing blocker code.
    pub code: String,
    /// Human-readable explanation; clients must not match this text.
    pub message: String,
}

/// Stable registry identity of the verifier selected for offline transfers.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineVerifierId {
    /// Proof-backend namespace of the registered key.
    pub backend: String,
    /// Human-readable key name within the backend namespace.
    pub name: String,
}

/// Active confidential-transfer verifier selected at a readiness snapshot.
///
/// This is the public, key-material-free subset of the authoritative registry
/// record. The inclusive activation and exclusive withdrawal bounds let a
/// wallet prove that the same verifier was active at
/// [`OfflineReadiness::evaluated_block_height`].
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
pub struct OfflineActiveTransferVerifier {
    /// Stable registry identity used by proof attachments and top-up anchors.
    pub id: OfflineVerifierId,
    /// Governance-managed monotonic record version.
    pub version: u32,
    /// Exact confidential-transfer circuit identifier.
    pub circuit_id: String,
    /// Lowercase hexadecimal commitment of the registered verifying key.
    pub commitment: String,
    /// Lowercase hexadecimal public-input schema hash.
    pub public_inputs_schema_hash: String,
    /// Maximum transfer-proof payload accepted by this registry record.
    pub max_proof_bytes: u32,
    /// First block at which the verifier is active, inclusive; zero means genesis.
    pub activation_height: u64,
    /// First block at which the verifier is inactive, exclusive; `None` means no scheduled withdrawal.
    pub withdrawal_height: Option<u64>,
}

/// Active public-to-confidential top-up shield verifier.
///
/// It uses the same key-material-free registry projection as a transfer
/// verifier, while the distinct readiness field prevents clients from using a
/// transfer key for issuance or treating shield readiness as peer-spend proof
/// readiness.
pub type OfflineActiveTopUpShieldVerifier = OfflineActiveTransferVerifier;

/// Active confidential-unshield verifier selected at the readiness snapshot.
pub type OfflineActiveUnshieldVerifier = OfflineActiveTransferVerifier;

/// Active ABI-21 V4 recursive StepEq verifier selected at the readiness snapshot.
pub type OfflineActiveRecursiveStepEqVerifier = OfflineActiveTransferVerifier;

/// Active ABI-21 V4 recursive StepEp verifier selected at the readiness snapshot.
pub type OfflineActiveRecursiveStepEpVerifier = OfflineActiveTransferVerifier;

/// Authenticated ABI-21 recursive release selected at a readiness snapshot.
///
/// Every digest is lowercase hexadecimal, non-zero, and distinct in public
/// JSON. The identity is emitted only after Core authenticates the release
/// policy, attestation, evidence, manifest, verifier records, and verifier-side
/// artifact bytes.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineAuthenticatedArtifactSet {
    /// Human-readable generation of the authenticated release.
    pub generation: String,
    /// Lowercase hexadecimal SHA-256 digest of the canonical release manifest.
    pub manifest_sha256: String,
    /// Lowercase hexadecimal SHA-256 digest of the locally trusted release policy.
    pub release_policy_sha256: String,
    /// Lowercase hexadecimal SHA-256 digest of the canonical signed release attestation.
    pub release_attestation_sha256: String,
    /// First height at which the release may issue notes.
    pub activation_height: u64,
    /// First height at which new issuance must stop.
    pub withdrawal_height: u64,
    /// Authenticated upper bound for one canonical proof-pair payload.
    pub max_proof_bytes: u32,
    /// Authoritative fixed scale of the asset bound to the release.
    pub asset_scale: u32,
}

impl norito::json::JsonDeserialize for OfflineActiveTransferVerifier {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::{Error, MapVisitor};

        let mut visitor = MapVisitor::new(parser)?;
        let mut id = None;
        let mut version = None;
        let mut circuit_id = None;
        let mut commitment = None;
        let mut public_inputs_schema_hash = None;
        let mut max_proof_bytes = None;
        let mut activation_height = None;
        let mut withdrawal_height = None;

        while let Some(key) = visitor.next_key()? {
            let field = key.as_str();
            match field {
                "id" => {
                    if id.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    id = Some(visitor.parse_value::<OfflineVerifierId>()?);
                }
                "version" => {
                    if version.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    version = Some(visitor.parse_value::<u32>()?);
                }
                "circuit_id" => {
                    if circuit_id.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    circuit_id = Some(visitor.parse_value::<String>()?);
                }
                "commitment" => {
                    if commitment.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    commitment = Some(visitor.parse_value::<String>()?);
                }
                "public_inputs_schema_hash" => {
                    if public_inputs_schema_hash.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    public_inputs_schema_hash = Some(visitor.parse_value::<String>()?);
                }
                "max_proof_bytes" => {
                    if max_proof_bytes.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    max_proof_bytes = Some(visitor.parse_value::<u32>()?);
                }
                "activation_height" => {
                    if activation_height.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    activation_height = Some(visitor.parse_value::<u64>()?);
                }
                "withdrawal_height" => {
                    if withdrawal_height.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    withdrawal_height = Some(visitor.parse_value::<Option<u64>>()?);
                }
                _ => return Err(Error::unknown_field(field.to_owned())),
            }
        }
        visitor.finish()?;

        Ok(Self {
            id: id.ok_or_else(|| Error::missing_field("id"))?,
            version: version.ok_or_else(|| Error::missing_field("version"))?,
            circuit_id: circuit_id.ok_or_else(|| Error::missing_field("circuit_id"))?,
            commitment: commitment.ok_or_else(|| Error::missing_field("commitment"))?,
            public_inputs_schema_hash: public_inputs_schema_hash
                .ok_or_else(|| Error::missing_field("public_inputs_schema_hash"))?,
            max_proof_bytes: max_proof_bytes
                .ok_or_else(|| Error::missing_field("max_proof_bytes"))?,
            activation_height: activation_height
                .ok_or_else(|| Error::missing_field("activation_height"))?,
            withdrawal_height: withdrawal_height
                .ok_or_else(|| Error::missing_field("withdrawal_height"))?,
        })
    }
}

/// Snapshot-bound readiness result for one asset definition.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
pub struct OfflineReadiness {
    /// Minimum native bridge ABI required by this chain build.
    pub required_bridge_abi_version: u32,
    /// Maximum peer-spend hop depth accepted by the protocol.
    pub max_hops: u32,
    /// Canonical asset definition evaluated by Torii.
    pub asset_definition_id: String,
    /// Authoritative scale from the live asset definition, or `None` when the
    /// definition is not fixed-scale and offline payments must remain disabled.
    pub asset_scale: Option<u32>,
    /// Committed block height whose state was evaluated.
    pub evaluated_block_height: u64,
    /// Lowercase hash of the same committed block, usable as an attestation anchor.
    pub evaluated_block_hash: String,
    /// Active confidential-transfer verifier at the evaluated height, or
    /// `None` together with a `transfer_verifier_unavailable` blocker.
    pub active_transfer_verifier: Option<OfflineActiveTransferVerifier>,
    /// Active top-up shield verifier at the evaluated height, or `None`
    /// together with a `topup_shield_verifier_unavailable` blocker.
    pub active_topup_shield_verifier: Option<OfflineActiveTopUpShieldVerifier>,
    /// Active confidential-unshield verifier at the evaluated height.
    pub active_unshield_verifier: Option<OfflineActiveUnshieldVerifier>,
    /// Active recursive StepEq verifier at the evaluated height.
    pub active_recursive_step_eq_verifier: Option<OfflineActiveRecursiveStepEqVerifier>,
    /// Active recursive StepEp verifier at the evaluated height.
    pub active_recursive_step_ep_verifier: Option<OfflineActiveRecursiveStepEpVerifier>,
    /// Exact authenticated ABI-21 release identity, or `None` with a recursive
    /// registry blocker.
    pub artifact_set: Option<OfflineAuthenticatedArtifactSet>,
    /// Whether the authenticated V4 material constructs the production backend.
    pub proof_backend_available: bool,
    /// Whether the exact authenticated artifact set, distinct active Eq/Ep
    /// records, and production backend make the recursive lineage path usable.
    pub recursive_lineage_supported: bool,
    /// Whether every requirement is satisfied at the evaluated snapshot.
    pub ready: bool,
    /// Complete known blocker set. `recursive_lineage_unavailable` is present
    /// exactly when `recursive_lineage_supported` is false.
    pub blockers: Vec<OfflineReadinessBlocker>,
}

impl norito::json::JsonDeserialize for OfflineReadiness {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::{Error, MapVisitor};

        let mut visitor = MapVisitor::new(parser)?;
        let mut required_bridge_abi_version = None;
        let mut max_hops = None;
        let mut asset_definition_id = None;
        let mut asset_scale = None;
        let mut evaluated_block_height = None;
        let mut evaluated_block_hash = None;
        let mut active_transfer_verifier = None;
        let mut active_topup_shield_verifier = None;
        let mut active_unshield_verifier = None;
        let mut active_recursive_step_eq_verifier = None;
        let mut active_recursive_step_ep_verifier = None;
        let mut artifact_set = None;
        let mut proof_backend_available = None;
        let mut recursive_lineage_supported = None;
        let mut ready = None;
        let mut blockers = None;

        while let Some(key) = visitor.next_key()? {
            let field = key.as_str();
            match field {
                "required_bridge_abi_version" => {
                    if required_bridge_abi_version.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    required_bridge_abi_version = Some(visitor.parse_value::<u32>()?);
                }
                "max_hops" => {
                    if max_hops.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    max_hops = Some(visitor.parse_value::<u32>()?);
                }
                "asset_definition_id" => {
                    if asset_definition_id.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    asset_definition_id = Some(visitor.parse_value::<String>()?);
                }
                "asset_scale" => {
                    if asset_scale.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    asset_scale = Some(visitor.parse_value::<Option<u32>>()?);
                }
                "evaluated_block_height" => {
                    if evaluated_block_height.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    evaluated_block_height = Some(visitor.parse_value::<u64>()?);
                }
                "evaluated_block_hash" => {
                    if evaluated_block_hash.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    evaluated_block_hash = Some(visitor.parse_value::<String>()?);
                }
                "active_transfer_verifier" => {
                    if active_transfer_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_transfer_verifier =
                        Some(visitor.parse_value::<Option<OfflineActiveTransferVerifier>>()?);
                }
                "active_topup_shield_verifier" => {
                    if active_topup_shield_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_topup_shield_verifier =
                        Some(visitor.parse_value::<Option<OfflineActiveTopUpShieldVerifier>>()?);
                }
                "active_unshield_verifier" => {
                    if active_unshield_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_unshield_verifier =
                        Some(visitor.parse_value::<Option<OfflineActiveUnshieldVerifier>>()?);
                }
                "active_recursive_step_eq_verifier" => {
                    if active_recursive_step_eq_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_recursive_step_eq_verifier = Some(
                        visitor.parse_value::<Option<OfflineActiveRecursiveStepEqVerifier>>()?,
                    );
                }
                "active_recursive_step_ep_verifier" => {
                    if active_recursive_step_ep_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_recursive_step_ep_verifier = Some(
                        visitor.parse_value::<Option<OfflineActiveRecursiveStepEpVerifier>>()?,
                    );
                }
                "artifact_set" => {
                    if artifact_set.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    artifact_set =
                        Some(visitor.parse_value::<Option<OfflineAuthenticatedArtifactSet>>()?);
                }
                "proof_backend_available" => {
                    if proof_backend_available.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    proof_backend_available = Some(visitor.parse_value::<bool>()?);
                }
                "recursive_lineage_supported" => {
                    if recursive_lineage_supported.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    recursive_lineage_supported = Some(visitor.parse_value::<bool>()?);
                }
                "ready" => {
                    if ready.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    ready = Some(visitor.parse_value::<bool>()?);
                }
                "blockers" => {
                    if blockers.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    blockers = Some(visitor.parse_value::<Vec<OfflineReadinessBlocker>>()?);
                }
                _ => return Err(Error::unknown_field(field.to_owned())),
            }
        }
        visitor.finish()?;

        Ok(Self {
            required_bridge_abi_version: required_bridge_abi_version
                .ok_or_else(|| Error::missing_field("required_bridge_abi_version"))?,
            max_hops: max_hops.ok_or_else(|| Error::missing_field("max_hops"))?,
            asset_definition_id: asset_definition_id
                .ok_or_else(|| Error::missing_field("asset_definition_id"))?,
            asset_scale: asset_scale.ok_or_else(|| Error::missing_field("asset_scale"))?,
            evaluated_block_height: evaluated_block_height
                .ok_or_else(|| Error::missing_field("evaluated_block_height"))?,
            evaluated_block_hash: evaluated_block_hash
                .ok_or_else(|| Error::missing_field("evaluated_block_hash"))?,
            active_transfer_verifier: active_transfer_verifier
                .ok_or_else(|| Error::missing_field("active_transfer_verifier"))?,
            active_topup_shield_verifier: active_topup_shield_verifier
                .ok_or_else(|| Error::missing_field("active_topup_shield_verifier"))?,
            active_unshield_verifier: active_unshield_verifier
                .ok_or_else(|| Error::missing_field("active_unshield_verifier"))?,
            active_recursive_step_eq_verifier: active_recursive_step_eq_verifier
                .ok_or_else(|| Error::missing_field("active_recursive_step_eq_verifier"))?,
            active_recursive_step_ep_verifier: active_recursive_step_ep_verifier
                .ok_or_else(|| Error::missing_field("active_recursive_step_ep_verifier"))?,
            artifact_set: artifact_set.ok_or_else(|| Error::missing_field("artifact_set"))?,
            proof_backend_available: proof_backend_available
                .ok_or_else(|| Error::missing_field("proof_backend_available"))?,
            recursive_lineage_supported: recursive_lineage_supported
                .ok_or_else(|| Error::missing_field("recursive_lineage_supported"))?,
            ready: ready.ok_or_else(|| Error::missing_field("ready"))?,
            blockers: blockers.ok_or_else(|| Error::missing_field("blockers"))?,
        })
    }
}

/// Offline lifecycle command selected by an operation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OfflineOperationKind {
    /// Move online value into an offline spendable note.
    #[norito(rename = "top_up")]
    TopUp,
    /// Move offline value back into an online account.
    #[norito(rename = "redeem")]
    Redeem,
}

/// Initial state returned after an offline command is accepted.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OfflineOperationState {
    /// The signed transaction has been accepted for asynchronous processing.
    #[norito(rename = "pending")]
    Pending,
}

/// Reference returned by an accepted offline command.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineOperationReference {
    /// Lowercase hexadecimal operation identifier.
    pub operation_id: String,
    /// Offline command kind.
    pub kind: OfflineOperationKind,
    /// Initial operation state.
    pub state: OfflineOperationState,
    /// Canonical signed transaction hash.
    pub transaction_hash: String,
    /// Relative URI of the operation status resource.
    pub status_uri: String,
    /// Signed request issuance time in Unix milliseconds.
    pub submitted_at_ms: u64,
}

/// Final result of an applied top-up operation.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineTopUpResult {
    /// Canonical signed transaction hash.
    pub transaction_hash: String,
    /// Finalized block height.
    pub finalized_block_height: u64,
    /// Finalized chain time in Unix milliseconds.
    pub server_time_ms: u64,
    /// Typed finalized top-up anchor consumed by the local wallet prover.
    pub anchor: OfflineTopUpAnchor,
    /// Typed consensus proof bound to the exact finalized top-up anchor.
    pub finality_proof: OfflineTopUpFinalityProof,
}

/// Final result of an applied redemption operation.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineRedeemResult {
    /// Canonical signed transaction hash.
    pub transaction_hash: String,
    /// Finalized block height.
    pub finalized_block_height: u64,
    /// Finalized chain time in Unix milliseconds.
    pub server_time_ms: u64,
}

/// Applied offline operation result, discriminated by command kind.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(tag = "kind", content = "result", rename_all = "snake_case")]
pub enum OfflineOperationResult {
    /// Applied top-up result.
    #[norito(rename = "top_up")]
    TopUp(OfflineTopUpResult),
    /// Applied redemption result.
    #[norito(rename = "redeem")]
    Redeem(OfflineRedeemResult),
}

/// Pollable terminal or non-terminal state of an offline operation.
#[derive(Debug, Clone, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(tag = "state", content = "value", rename_all = "snake_case")]
pub enum OfflineOperationStatus {
    /// The transaction is queued or awaiting finality.
    #[norito(rename = "pending")]
    Pending {
        /// Lowercase hexadecimal operation identifier.
        operation_id: String,
        /// Offline command kind.
        kind: OfflineOperationKind,
        /// Canonical signed transaction hash.
        transaction_hash: String,
        /// Signed request issuance time in Unix milliseconds.
        submitted_at_ms: u64,
    },
    /// The transaction was applied and finalized.
    #[norito(rename = "applied")]
    Applied {
        /// Lowercase hexadecimal operation identifier.
        operation_id: String,
        /// Operation-specific terminal result.
        result: OfflineOperationResult,
    },
    /// The transaction reached a terminal rejection.
    #[norito(rename = "rejected")]
    Rejected {
        /// Lowercase hexadecimal operation identifier.
        operation_id: String,
        /// Offline command kind.
        kind: OfflineOperationKind,
        /// Canonical signed transaction hash.
        transaction_hash: String,
        /// Stable typed Torii error.
        error: ErrorEnvelope,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Debug, JsonDeserialize, JsonSerialize, PartialEq, Eq)]
    struct JsonDefaultByteMappingProbe {
        fixed: [u8; 4],
        dynamic: Vec<u8>,
        keyed: BTreeMap<[u8; 2], u8>,
    }

    fn unavailable_readiness(asset_scale: Option<u32>) -> OfflineReadiness {
        let mut blockers = vec![
            OfflineReadinessBlocker {
                code: "transfer_verifier_unavailable".to_owned(),
                message: "The transfer verifier is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "topup_shield_verifier_unavailable".to_owned(),
                message: "The top-up shield verifier is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "unshield_verifier_unavailable".to_owned(),
                message: "The unshield verifier is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "recursive_v4_registry_unavailable".to_owned(),
                message: "The authenticated V4 registry is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "recursive_step_eq_verifier_unavailable".to_owned(),
                message: "The recursive StepEq verifier is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "recursive_step_ep_verifier_unavailable".to_owned(),
                message: "The recursive StepEp verifier is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "proof_backend_unavailable".to_owned(),
                message: "The proof backend is unavailable.".to_owned(),
            },
            OfflineReadinessBlocker {
                code: "recursive_lineage_unavailable".to_owned(),
                message: "Recursive lineage is unavailable.".to_owned(),
            },
        ];
        if asset_scale.is_none() {
            blockers.insert(
                0,
                OfflineReadinessBlocker {
                    code: "asset_scale_unavailable".to_owned(),
                    message: "The asset scale is unavailable.".to_owned(),
                },
            );
        }
        OfflineReadiness {
            required_bridge_abi_version:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            max_hops: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale,
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: None,
            active_topup_shield_verifier: None,
            active_unshield_verifier: None,
            active_recursive_step_eq_verifier: None,
            active_recursive_step_ep_verifier: None,
            artifact_set: None,
            proof_backend_available: false,
            recursive_lineage_supported: false,
            ready: false,
            blockers,
        }
    }

    #[test]
    fn norito_json_default_byte_and_map_key_mapping_is_exact() {
        let probe = JsonDefaultByteMappingProbe {
            fixed: [0x00, 0xab, 0x10, 0xff],
            dynamic: vec![0x00, 0xab, 0x10, 0xff],
            keyed: BTreeMap::from([([0x00, 0xff], 7)]),
        };

        let json = norito::json::to_string(&probe).expect("encode JSON mapping probe");
        assert_eq!(
            json,
            r#"{"fixed":"00AB10FF","dynamic":[0,171,16,255],"keyed":{"00FF":7}}"#
        );
        let decoded: JsonDefaultByteMappingProbe =
            norito::json::from_str(&json).expect("decode canonical JSON mapping probe");
        assert_eq!(decoded, probe);

        let lowercase: JsonDefaultByteMappingProbe = norito::json::from_str(
            r#"{"fixed":"00ab10ff","dynamic":[0,171,16,255],"keyed":{"00ff":7}}"#,
        )
        .expect("decode lowercase hexadecimal input");
        assert_eq!(lowercase, probe);

        let error = norito::json::from_str::<JsonDefaultByteMappingProbe>(
            r#"{"fixed":"00AB10FF","dynamic":[],"keyed":{"00FF":7,"00ff":8}}"#,
        )
        .expect_err("lexically distinct keys must not alias one typed map key");
        assert!(
            error.to_string().contains("duplicate field"),
            "unexpected duplicate-key error: {error}"
        );
    }

    #[test]
    fn readiness_roundtrips_through_both_public_representations() {
        let readiness = OfflineReadiness {
            required_bridge_abi_version: 21,
            max_hops: 8,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale: Some(9),
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: Some(OfflineActiveTransferVerifier {
                id: OfflineVerifierId {
                    backend: "halo2/ipa".to_owned(),
                    name: iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2.to_owned(),
                },
                version: 7,
                circuit_id: "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3"
                    .to_owned(),
                commitment: "cd".repeat(32),
                public_inputs_schema_hash: "ef".repeat(32),
                max_proof_bytes: 65_536,
                activation_height: 40,
                withdrawal_height: Some(80),
            }),
            active_topup_shield_verifier: Some(OfflineActiveTopUpShieldVerifier {
                id: OfflineVerifierId {
                    backend: "halo2/ipa".to_owned(),
                    name: iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2
                        .to_owned(),
                },
                version: 3,
                circuit_id: "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3"
                    .to_owned(),
                commitment: "12".repeat(32),
                public_inputs_schema_hash: "34".repeat(32),
                max_proof_bytes: 196_608,
                activation_height: 41,
                withdrawal_height: Some(81),
            }),
            active_unshield_verifier: Some(OfflineActiveUnshieldVerifier {
                id: OfflineVerifierId {
                    backend: "halo2/ipa".to_owned(),
                    name: iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2.to_owned(),
                },
                version: 4,
                circuit_id:
                    "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4"
                        .to_owned(),
                commitment: "23".repeat(32),
                public_inputs_schema_hash: "45".repeat(32),
                max_proof_bytes: 196_608,
                activation_height: 40,
                withdrawal_height: Some(80),
            }),
            active_recursive_step_eq_verifier: Some(OfflineActiveRecursiveStepEqVerifier {
                id: OfflineVerifierId {
                    backend: "halo2/ipa".to_owned(),
                    name: iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4.to_owned(),
                },
                version: 5,
                circuit_id:
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
                        .to_owned(),
                commitment: "56".repeat(32),
                public_inputs_schema_hash: "67".repeat(32),
                max_proof_bytes: 65_536,
                activation_height: 40,
                withdrawal_height: Some(80),
            }),
            active_recursive_step_ep_verifier: Some(OfflineActiveRecursiveStepEpVerifier {
                id: OfflineVerifierId {
                    backend: "halo2/ipa".to_owned(),
                    name: iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4.to_owned(),
                },
                version: 5,
                circuit_id:
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
                        .to_owned(),
                commitment: "78".repeat(32),
                public_inputs_schema_hash: "89".repeat(32),
                max_proof_bytes: 65_536,
                activation_height: 40,
                withdrawal_height: Some(80),
            }),
            artifact_set: Some(OfflineAuthenticatedArtifactSet {
                generation: "release-v4".to_owned(),
                manifest_sha256: "56".repeat(32),
                release_policy_sha256: "67".repeat(32),
                release_attestation_sha256: "78".repeat(32),
                activation_height: 40,
                withdrawal_height: 80,
                max_proof_bytes: 65_536,
                asset_scale: 9,
            }),
            proof_backend_available: true,
            recursive_lineage_supported: true,
            ready: true,
            blockers: Vec::new(),
        };

        let json = norito::json::to_vec(&readiness).expect("encode readiness JSON");
        let decoded_json: OfflineReadiness =
            norito::json::from_slice(&json).expect("decode readiness JSON");
        assert_eq!(decoded_json, readiness);

        let archive = norito::to_bytes(&readiness).expect("encode readiness Norito");
        let decoded_norito: OfflineReadiness =
            norito::decode_from_bytes(&archive).expect("decode readiness Norito");
        assert_eq!(decoded_norito, readiness);
    }

    #[test]
    fn readiness_json_rejects_unknown_members_and_type_confusion() {
        let readiness = unavailable_readiness(Some(9));
        let canonical = norito::json::to_string(&readiness).expect("encode readiness");
        let unknown = canonical.replacen('{', r#"{"future_metadata":null,"#, 1);
        let error = norito::json::from_str::<OfflineReadiness>(&unknown)
            .expect_err("unknown first-release readiness members fail closed");
        assert!(error.to_string().contains("unknown field"));

        let wrong_type = canonical.replace(
            r#""proof_backend_available":false"#,
            r#""proof_backend_available":"false""#,
        );
        let error = norito::json::from_str::<OfflineReadiness>(&wrong_type)
            .expect_err("declared readiness field typing is exact");
        assert!(error.to_string().contains("bool"));
    }

    #[test]
    fn readiness_json_requires_every_first_release_member() {
        let readiness = unavailable_readiness(Some(9));
        let canonical = norito::json::to_string(&readiness).expect("encode readiness");
        for member in [
            r#""asset_scale":9,"#,
            r#""active_transfer_verifier":null,"#,
            r#""active_unshield_verifier":null,"#,
            r#""active_recursive_step_eq_verifier":null,"#,
            r#""active_recursive_step_ep_verifier":null,"#,
            r#""artifact_set":null,"#,
            r#""proof_backend_available":false,"#,
        ] {
            let json = canonical.replacen(member, "", 1);
            let error = norito::json::from_str::<OfflineReadiness>(&json)
                .expect_err("first-release readiness members must not be defaulted");
            assert!(
                error.to_string().contains("missing field"),
                "unexpected missing-field error: {error}"
            );
        }
    }

    #[test]
    fn readiness_json_emits_unavailable_authorities_as_explicit_nulls() {
        let readiness = unavailable_readiness(None);

        let json = norito::json::to_string(&readiness).expect("encode unavailable readiness");
        assert!(json.contains(r#""asset_scale":null"#));
        assert!(json.contains(r#""active_transfer_verifier":null"#));
        assert!(json.contains(r#""active_topup_shield_verifier":null"#));
    }

    #[test]
    fn readiness_json_rejects_duplicate_nullable_authority_members() {
        for json in [
            r#"{"asset_definition_id":"xor#wonderland","asset_scale":null,"asset_scale":9,"evaluated_block_height":42,"evaluated_block_hash":"abababababababababababababababababababababababababababababababab","active_transfer_verifier":null,"ready":false,"blockers":[]}"#,
            r#"{"asset_definition_id":"xor#wonderland","asset_scale":9,"evaluated_block_height":42,"evaluated_block_hash":"abababababababababababababababababababababababababababababababab","active_transfer_verifier":null,"active_transfer_verifier":null,"ready":false,"blockers":[]}"#,
            r#"{"asset_definition_id":"xor#wonderland","asset_scale":9,"evaluated_block_height":42,"evaluated_block_hash":"abababababababababababababababababababababababababababababababab","active_transfer_verifier":null,"active_topup_shield_verifier":null,"active_topup_shield_verifier":null,"ready":false,"blockers":[]}"#,
        ] {
            let error = norito::json::from_str::<OfflineReadiness>(json)
                .expect_err("duplicate readiness authority member must fail closed");
            assert!(error.to_string().contains("duplicate field"));
        }
    }

    #[test]
    fn tagged_json_rejects_duplicate_discriminator_members() {
        for json in [
            r#"{"kind":"top_up","kind":"redeem","value":null}"#,
            r#"{"kind":"top_up","value":null,"value":null}"#,
        ] {
            let error = norito::json::from_str::<OfflineOperationKind>(json)
                .expect_err("duplicate enum envelope members must fail");
            assert!(
                error.to_string().contains("duplicate field"),
                "unexpected duplicate-member error: {error}"
            );
        }
    }

    #[test]
    fn operation_reference_is_direct_and_roundtrips() {
        let reference = OfflineOperationReference {
            operation_id: "11".repeat(32),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "22".repeat(32),
            status_uri: format!("/v1/offline/operations/{}", "11".repeat(32)),
            submitted_at_ms: 1_725_000_000_123,
        };

        let json = norito::json::to_vec(&reference).expect("encode operation reference JSON");
        let json_text = core::str::from_utf8(&json).expect("JSON is UTF-8");
        assert!(!json_text.contains("base64"));
        let decoded_json: OfflineOperationReference =
            norito::json::from_slice(&json).expect("decode operation reference JSON");
        assert_eq!(decoded_json, reference);

        let archive = norito::to_bytes(&reference).expect("encode operation reference Norito");
        let decoded_norito: OfflineOperationReference =
            norito::decode_from_bytes(&archive).expect("decode operation reference Norito");
        assert_eq!(decoded_norito, reference);
    }

    #[test]
    fn operation_reference_json_mapping_is_exact_and_lossless() {
        let operation_id = "11".repeat(32);
        let reference = OfflineOperationReference {
            operation_id: operation_id.clone(),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "22".repeat(32),
            status_uri: format!("/v1/offline/operations/{operation_id}"),
            submitted_at_ms: u64::MAX,
        };

        let json = norito::json::to_string(&reference).expect("encode operation reference JSON");
        assert_eq!(
            json,
            format!(
                concat!(
                    r#"{{"operation_id":"{operation_id}","kind":{{"kind":"top_up","value":null}},"#,
                    r#""state":{{"state":"pending","value":null}},"transaction_hash":"{transaction_hash}","#,
                    r#""status_uri":"/v1/offline/operations/{operation_id}","submitted_at_ms":18446744073709551615}}"#,
                ),
                operation_id = operation_id,
                transaction_hash = "22".repeat(32),
            )
        );
        let decoded: OfflineOperationReference =
            norito::json::from_str(&json).expect("decode lossless operation reference JSON");
        assert_eq!(decoded, reference);
    }

    #[test]
    fn operation_reference_json_rejects_duplicate_declared_fields() {
        let operation_id = "11".repeat(32);
        let json = format!(
            concat!(
                r#"{{"operation_id":"{operation_id}","operation_id":"{operation_id}","#,
                r#""kind":{{"kind":"top_up","value":null}},"state":{{"state":"pending","value":null}},"#,
                r#""transaction_hash":"{transaction_hash}","status_uri":"/v1/offline/operations/{operation_id}","#,
                r#""submitted_at_ms":1}}"#,
            ),
            operation_id = operation_id,
            transaction_hash = "22".repeat(32),
        );
        let error = norito::json::from_str::<OfflineOperationReference>(&json)
            .expect_err("duplicate operation_id must be rejected");
        assert!(error.to_string().contains("duplicate field `operation_id`"));
    }

    #[test]
    fn operation_kind_json_rejects_unknown_tags() {
        let error = norito::json::from_str::<OfflineOperationKind>(
            r#"{"kind":"unknown_command","value":null}"#,
        )
        .expect_err("unknown operation kind must be rejected");
        assert!(
            error
                .to_string()
                .contains("unknown variant `unknown_command`")
        );
    }

    #[test]
    fn operation_reference_golden_vector() {
        const EXPECTED_ARCHIVE_HEX: &str = "4e5254300000e8e2244e45e4be2a975e34957141128b00f0000000000000001f5b5402d6dc2092024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323258572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff";
        let reference = OfflineOperationReference {
            operation_id: "11".repeat(32),
            kind: OfflineOperationKind::TopUp,
            state: OfflineOperationState::Pending,
            transaction_hash: "22".repeat(32),
            status_uri: format!("/v1/offline/operations/{}", "11".repeat(32)),
            submitted_at_ms: u64::MAX,
        };
        let archive = norito::to_bytes(&reference).expect("encode golden operation reference");
        let archive_hex = archive
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(archive_hex, EXPECTED_ARCHIVE_HEX);
    }

    #[test]
    fn operation_status_golden_vectors() {
        const PENDING_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a009600000000000000bdfee2508f80055702000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff";
        const REJECTED_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a00b6000000000000009322104cda8e602a020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100";
        const APPLIED_REDEEM_ARCHIVE_HEX: &str = "4e5254300000fb04214104df1bdcd39249bddd4db23a00a00000000000000092cd6b32b062b3d30200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313159010000005441403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff082a00000000000000";
        let operation_id = "11".repeat(32);
        let pending = OfflineOperationStatus::Pending {
            operation_id: operation_id.clone(),
            kind: OfflineOperationKind::TopUp,
            transaction_hash: "22".repeat(32),
            submitted_at_ms: u64::MAX,
        };
        let rejected = OfflineOperationStatus::Rejected {
            operation_id: operation_id.clone(),
            kind: OfflineOperationKind::Redeem,
            transaction_hash: "22".repeat(32),
            error: ErrorEnvelope::new("offline_operation_rejected", "rejected"),
        };
        let applied_redeem = OfflineOperationStatus::Applied {
            operation_id,
            result: OfflineOperationResult::Redeem(OfflineRedeemResult {
                transaction_hash: "22".repeat(32),
                finalized_block_height: u64::MAX,
                server_time_ms: 42,
            }),
        };

        for (expected, status) in [
            (PENDING_ARCHIVE_HEX, pending),
            (REJECTED_ARCHIVE_HEX, rejected),
            (APPLIED_REDEEM_ARCHIVE_HEX, applied_redeem),
        ] {
            let archive = norito::to_bytes(&status).expect("encode golden operation status");
            let archive_hex = archive
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(archive_hex, expected);
        }
    }
}

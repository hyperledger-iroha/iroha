//! Public offline-wallet protocol capability and legacy proof-release diagnostics.
//!
//! These types live in the data model because they are shared by Torii
//! responses and node telemetry. Keeping them here prevents either transport
//! layer from depending on the other.
use iroha_schema::IntoSchema;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
/// Legacy machine-readable diagnostic for an explicitly requested proof release.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    IntoSchema,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineReadinessBlocker {
    /// Stable SDK-facing blocker code.
    pub code: String,
    /// Human-readable explanation; clients must not match this text.
    pub message: String,
}
/// Stable registry identity of the verifier selected for offline transfers.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    IntoSchema,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
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
#[derive(
    Debug, Clone, PartialEq, Eq, IntoSchema, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
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
pub type OfflineActiveTopUpShieldVerifier = OfflineActiveTransferVerifier;
/// Active confidential-unshield verifier selected at the readiness snapshot.
pub type OfflineActiveUnshieldVerifier = OfflineActiveTransferVerifier;
/// Active ABI-21 V4 recursive `StepEq` verifier selected at the readiness snapshot.
pub type OfflineActiveRecursiveStepEqVerifier = OfflineActiveTransferVerifier;
/// Active ABI-21 V4 recursive `StepEp` verifier selected at the readiness snapshot.
pub type OfflineActiveRecursiveStepEpVerifier = OfflineActiveTransferVerifier;
/// Authenticated ABI-21 recursive release selected at a readiness snapshot.
///
/// Every digest is lowercase hexadecimal, non-zero, and distinct in public
/// JSON. The identity is emitted only after Core authenticates the release
/// policy, attestation, evidence, manifest, verifier records, and verifier-side
/// artifact bytes.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    IntoSchema,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
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
/// Legacy snapshot-bound diagnostics for one asset-specific proof release.
///
/// This does not enroll an asset for offline use and is never node readiness.
#[derive(
    Debug, Clone, PartialEq, Eq, IntoSchema, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
pub struct OfflineReadiness {
    /// Exact peer-cash handoff/finality contract required by this chain build.
    pub cash_handoff_capability: String,
    /// Minimum native bridge ABI required by this chain build.
    pub required_bridge_abi_version: u32,
    /// Maximum peer-spend hop depth accepted by the protocol.
    pub max_hops: u32,
    /// Canonical asset definition evaluated by Torii.
    pub asset_definition_id: String,
    /// Authoritative scale from the live asset definition.
    pub asset_scale: Option<u32>,
    /// Committed block height whose state was evaluated.
    pub evaluated_block_height: u64,
    /// Lowercase hash of the same committed block, usable as an attestation anchor.
    pub evaluated_block_hash: String,
    /// Active confidential-transfer verifier at the evaluated height.
    pub active_transfer_verifier: Option<OfflineActiveTransferVerifier>,
    /// Active top-up shield verifier at the evaluated height.
    pub active_topup_shield_verifier: Option<OfflineActiveTopUpShieldVerifier>,
    /// Active confidential-unshield verifier at the evaluated height.
    pub active_unshield_verifier: Option<OfflineActiveUnshieldVerifier>,
    /// Active recursive `StepEq` verifier at the evaluated height.
    pub active_recursive_step_eq_verifier: Option<OfflineActiveRecursiveStepEqVerifier>,
    /// Active recursive `StepEp` verifier at the evaluated height.
    pub active_recursive_step_ep_verifier: Option<OfflineActiveRecursiveStepEpVerifier>,
    /// Exact authenticated ABI-21 release identity.
    pub artifact_set: Option<OfflineAuthenticatedArtifactSet>,
    /// Whether the authenticated V4 material constructs the production backend.
    pub proof_backend_available: bool,
    /// Whether recursive lineage verification is usable.
    pub recursive_lineage_supported: bool,
    /// Whether every requirement is satisfied at the evaluated snapshot.
    pub ready: bool,
    /// Complete known blocker set.
    pub blockers: Vec<OfflineReadinessBlocker>,
}
/// Universal offline protocol capability projection embedded in node status.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    IntoSchema,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct OfflineStatus {
    /// Legacy compatibility field. Offline capability is universal and does
    /// not impose a backend service-readiness gate.
    pub mandatory: bool,
    /// Exact irreversible peer-cash handoff contract.
    pub cash_handoff_capability: String,
    /// Exact native bridge ABI required for authenticated V4 artifacts.
    pub required_bridge_abi_version: u32,
    /// Maximum peer-spend hop depth accepted by the protocol.
    pub max_hops: u32,
    /// Protocol capability availability. This is independent of any asset or
    /// dataspace catalog.
    pub ready: bool,
    /// Optional diagnostics for release material explicitly referenced by
    /// offline operations, sorted by canonical asset definition id.
    pub assets: Vec<OfflineReadiness>,
    /// Command-specific proof-material diagnostics; never startup blockers.
    pub blockers: Vec<OfflineReadinessBlocker>,
}
impl norito::json::JsonDeserialize for OfflineReadiness {
    #[expect(
        clippy::too_many_lines,
        reason = "the ordered one-pass decoder explicitly rejects duplicate, unknown, and missing members for the fixed public V1 JSON shape"
    )]
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::{Error, MapVisitor};
        let mut visitor = MapVisitor::new(parser)?;
        let mut cash_handoff_capability = None;
        let mut required_bridge_abi_version = None;
        let mut max_hops = None;
        let mut asset_definition_id = None;
        let mut asset_scale = None;
        let mut evaluated_block_height = None;
        let mut evaluated_block_hash = None;
        let mut active_transfer_verifier = None;
        let mut active_topup_shield_verifier = None;
        let mut active_unshield_verifier = None;
        let mut step_eq_slot = None;
        let mut complementary_step_ep_slot = None;
        let mut artifact_set = None;
        let mut proof_backend_available = None;
        let mut recursive_lineage_supported = None;
        let mut ready = None;
        let mut blockers = None;
        while let Some(key) = visitor.next_key()? {
            let field = key.as_str();
            match field {
                "cash_handoff_capability" => {
                    if cash_handoff_capability.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    cash_handoff_capability = Some(visitor.parse_value::<String>()?);
                }
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
                    if step_eq_slot.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    step_eq_slot = Some(
                        visitor.parse_value::<Option<OfflineActiveRecursiveStepEqVerifier>>()?,
                    );
                }
                "active_recursive_step_ep_verifier" => {
                    if complementary_step_ep_slot.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    complementary_step_ep_slot = Some(
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
            cash_handoff_capability: cash_handoff_capability
                .ok_or_else(|| Error::missing_field("cash_handoff_capability"))?,
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
            active_recursive_step_eq_verifier: step_eq_slot
                .ok_or_else(|| Error::missing_field("active_recursive_step_eq_verifier"))?,
            active_recursive_step_ep_verifier: complementary_step_ep_slot
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
#[cfg(test)]
mod tests {
    use super::*;
    fn sample_verifier() -> OfflineActiveTransferVerifier {
        OfflineActiveTransferVerifier {
            id: OfflineVerifierId {
                backend: "stark".to_owned(),
                name: "offline-transfer".to_owned(),
            },
            version: 1,
            circuit_id: "offline_transfer_v1".to_owned(),
            commitment: "11".repeat(32),
            public_inputs_schema_hash: "22".repeat(32),
            max_proof_bytes: 1024,
            activation_height: 10,
            withdrawal_height: None,
        }
    }
    fn sample_readiness() -> OfflineReadiness {
        OfflineReadiness {
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            required_bridge_abi_version: 22,
            max_hops: 8,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale: Some(6),
            evaluated_block_height: 42,
            evaluated_block_hash: "33".repeat(32),
            active_transfer_verifier: Some(sample_verifier()),
            active_topup_shield_verifier: None,
            active_unshield_verifier: None,
            active_recursive_step_eq_verifier: None,
            active_recursive_step_ep_verifier: None,
            artifact_set: None,
            proof_backend_available: true,
            recursive_lineage_supported: false,
            ready: false,
            blockers: vec![OfflineReadinessBlocker {
                code: "recursive_lineage_unavailable".to_owned(),
                message: "recursive verifier unavailable".to_owned(),
            }],
        }
    }
    #[test]
    fn readiness_json_roundtrip_is_strict() {
        let readiness = sample_readiness();
        let json = norito::json::to_json(&readiness).expect("serialize readiness");
        let decoded: OfflineReadiness =
            norito::json::from_json(&json).expect("deserialize readiness");
        assert_eq!(decoded, readiness);
        let unknown = json.replacen("\"max_hops\":8", "\"unknown\":0,\"max_hops\":8", 1);
        assert!(norito::json::from_json::<OfflineReadiness>(&unknown).is_err());
        let duplicate = json.replacen("\"max_hops\":8", "\"max_hops\":8,\"max_hops\":8", 1);
        assert!(norito::json::from_json::<OfflineReadiness>(&duplicate).is_err());
    }
    #[test]
    fn universal_status_norito_roundtrip_preserves_exact_projection() {
        let status = OfflineStatus {
            mandatory: false,
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            required_bridge_abi_version: 22,
            max_hops: 8,
            ready: true,
            assets: Vec::new(),
            blockers: Vec::new(),
        };
        let bytes = norito::to_bytes(&status).expect("encode status");
        let decoded: OfflineStatus = norito::decode_from_bytes(&bytes).expect("decode status");
        assert_eq!(decoded, status);
    }
}

//! Public Torii DTOs for the first-release Offline lifecycle.

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

use crate::ErrorEnvelope;

pub use iroha_data_model::offline::{
    KagemushaRecursiveSpendRedeemRequestV2 as OfflineRedeemRequest,
    KagemushaRecursiveSpendTopUpRequestV2 as OfflineTopUpRequest,
    OFFLINE_REDEEM_REQUEST_SCHEMA_NAME, OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME,
};

/// Finalized anchor returned by an applied offline top-up.
///
/// The underlying consensus wire type remains internally versioned, while the
/// first-release public transport surface exposes only this current name.
pub type OfflineTopUpAnchor = iroha_data_model::offline::KagemushaRecursiveSpendTopUpAnchorV2;

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

/// Active V3 recursive transition verifier selected at the readiness snapshot.
pub type OfflineActiveRecursiveTransitionVerifier = OfflineActiveTransferVerifier;

/// Active V3 recursive state verifier selected at the readiness snapshot.
pub type OfflineActiveRecursiveStateVerifier = OfflineActiveTransferVerifier;

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
    /// Active recursive transition verifier at the evaluated height.
    pub active_recursive_transition_verifier: Option<OfflineActiveRecursiveTransitionVerifier>,
    /// Active recursive state verifier at the evaluated height.
    pub active_recursive_state_verifier: Option<OfflineActiveRecursiveStateVerifier>,
    /// Whether this Torii/Core build contains the sound V3 recursive backend.
    pub proof_backend_available: bool,
    /// Whether recursive branches can be verified and redeemed through the
    /// production lineage proof path.
    pub recursive_lineage_supported: bool,
    /// Whether every requirement is satisfied at the evaluated snapshot.
    pub ready: bool,
    /// Empty when `ready` is true; otherwise the complete known blocker set.
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
        let mut active_recursive_transition_verifier = None;
        let mut active_recursive_state_verifier = None;
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
                "active_recursive_transition_verifier" => {
                    if active_recursive_transition_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_recursive_transition_verifier = Some(
                        visitor
                            .parse_value::<Option<OfflineActiveRecursiveTransitionVerifier>>()?,
                    );
                }
                "active_recursive_state_verifier" => {
                    if active_recursive_state_verifier.is_some() {
                        return Err(Error::duplicate_field(field));
                    }
                    active_recursive_state_verifier =
                        Some(visitor.parse_value::<Option<OfflineActiveRecursiveStateVerifier>>()?);
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
            active_recursive_transition_verifier: active_recursive_transition_verifier
                .ok_or_else(|| Error::missing_field("active_recursive_transition_verifier"))?,
            active_recursive_state_verifier: active_recursive_state_verifier
                .ok_or_else(|| Error::missing_field("active_recursive_state_verifier"))?,
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
            required_bridge_abi_version: 19,
            max_hops: 64,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale: Some(9),
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: Some(OfflineActiveTransferVerifier {
                id: OfflineVerifierId {
                    backend: "halo2/ipa".to_owned(),
                    name: "confidential-transfer-v2".to_owned(),
                },
                version: 7,
                circuit_id: "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified"
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
                    name: "kagemusha-topup-shield-v2".to_owned(),
                },
                version: 3,
                circuit_id: "kagemusha-topup-shield-v2".to_owned(),
                commitment: "12".repeat(32),
                public_inputs_schema_hash: "34".repeat(32),
                max_proof_bytes: 196_608,
                activation_height: 41,
                withdrawal_height: Some(81),
            }),
            active_unshield_verifier: None,
            active_recursive_transition_verifier: None,
            active_recursive_state_verifier: None,
            proof_backend_available: false,
            recursive_lineage_supported: false,
            ready: false,
            blockers: vec![OfflineReadinessBlocker {
                code: "proof_backend_unavailable".to_owned(),
                message: "Proof backend is unavailable.".to_owned(),
            }],
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
        let readiness = OfflineReadiness {
            required_bridge_abi_version: 19,
            max_hops: 64,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale: Some(9),
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: None,
            active_topup_shield_verifier: None,
            active_unshield_verifier: None,
            active_recursive_transition_verifier: None,
            active_recursive_state_verifier: None,
            proof_backend_available: false,
            recursive_lineage_supported: false,
            ready: false,
            blockers: Vec::new(),
        };
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
        let readiness = OfflineReadiness {
            required_bridge_abi_version: 19,
            max_hops: 64,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale: Some(9),
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: None,
            active_topup_shield_verifier: None,
            active_unshield_verifier: None,
            active_recursive_transition_verifier: None,
            active_recursive_state_verifier: None,
            proof_backend_available: false,
            recursive_lineage_supported: false,
            ready: false,
            blockers: Vec::new(),
        };
        let canonical = norito::json::to_string(&readiness).expect("encode readiness");
        for member in [
            r#""asset_scale":9,"#,
            r#""active_transfer_verifier":null,"#,
            r#""active_unshield_verifier":null,"#,
            r#""active_recursive_transition_verifier":null,"#,
            r#""active_recursive_state_verifier":null,"#,
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
        let readiness = OfflineReadiness {
            required_bridge_abi_version: 19,
            max_hops: 64,
            asset_definition_id: "xor#wonderland".to_owned(),
            asset_scale: None,
            evaluated_block_height: 42,
            evaluated_block_hash: "ab".repeat(32),
            active_transfer_verifier: None,
            active_topup_shield_verifier: None,
            active_unshield_verifier: None,
            active_recursive_transition_verifier: None,
            active_recursive_state_verifier: None,
            proof_backend_available: false,
            recursive_lineage_supported: false,
            ready: false,
            blockers: vec![
                OfflineReadinessBlocker {
                    code: "asset_scale_unavailable".to_owned(),
                    message: "The asset scale is unavailable.".to_owned(),
                },
                OfflineReadinessBlocker {
                    code: "transfer_verifier_unavailable".to_owned(),
                    message: "The transfer verifier is unavailable.".to_owned(),
                },
                OfflineReadinessBlocker {
                    code: "topup_shield_verifier_unavailable".to_owned(),
                    message: "The top-up shield verifier is unavailable.".to_owned(),
                },
            ],
        };

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

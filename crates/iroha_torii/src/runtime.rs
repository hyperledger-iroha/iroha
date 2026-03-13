//! Runtime upgrade app API handlers.

use std::sync::Arc;

use axum::{extract::Path, response::IntoResponse};
use iroha_core::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::Algorithm;
use iroha_data_model::account::curve::CurveId;
use iroha_logger::warn;
use mv::storage::StorageReadOnly;
use norito::derive::{NoritoDeserialize, NoritoSerialize};

use crate::{
    NoritoJson,
    json_macros::{JsonDeserialize, JsonSerialize},
};

const CURVE_REGISTRY_VERSION: u32 = 1;

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Node capabilities advert (subset)
pub struct NodeCapabilitiesResponse {
    /// Supported ABI versions on this node (active set)
    pub supported_abi_versions: Vec<u16>,
    /// Default Kotodama compile target (highest active ABI version)
    pub default_compile_target: u16,
    /// Data model compatibility version for SDK handshakes.
    pub data_model_version: u32,
    /// Cryptography capabilities (SM, default hashes, allow-lists)
    pub crypto: NodeCryptoCapabilities,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Crypto capability advert (currently SM-focused).
pub struct NodeCryptoCapabilities {
    /// SM cryptography capability manifest.
    pub sm: NodeSmCapabilities,
    /// Curve capability advert anchored to the registry.
    pub curves: NodeCurveCapabilities,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// SM capability manifest exported by the node.
pub struct NodeSmCapabilities {
    /// Whether SM helpers are enabled in this node.
    pub enabled: bool,
    /// Default transaction hash algorithm.
    pub default_hash: String,
    /// Admission allow-list for signing algorithms.
    pub allowed_signing: Vec<String>,
    /// Default SM2 distinguishing identifier.
    pub sm2_distid_default: String,
    /// Whether the OpenSSL/Tongsuo preview backend is toggled on.
    pub openssl_preview: bool,
    /// Acceleration advert (scalar/NEON policy).
    pub acceleration: NodeSmAcceleration,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Hardware/software acceleration advert for SM algorithms.
pub struct NodeSmAcceleration {
    /// Scalar implementation availability (always true).
    pub scalar: bool,
    /// NEON accelerated SM3 hashing available.
    pub neon_sm3: bool,
    /// NEON accelerated SM4 block operations available.
    pub neon_sm4: bool,
    /// Dispatch policy string (`auto`, `force-enable`, `force-disable`, `scalar-only`).
    pub policy: String,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Curve capability advert emitted by `/v2/node/capabilities`.
pub struct NodeCurveCapabilities {
    /// Registry version referenced by this advert.
    pub registry_version: u32,
    /// Allowed curve identifiers (as published in the registry).
    pub allowed_curve_ids: Vec<u8>,
    /// Bitmap of allowed curve identifiers (bit `i` ⇒ curve id `i`).
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub allowed_curve_bitmap: Vec<u64>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// JSON summary of runtime-related metrics of interest
pub struct RuntimeMetricsResponse {
    /// Count of active ABI versions
    pub active_abi_versions_count: u64,
    /// Upgrade lifecycle event counters
    pub upgrade_events_total: UpgradeEventsCounters,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct UpgradeEventsCounters {
    pub proposed: u64,
    pub activated: u64,
    pub canceled: u64,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct RuntimeAbiActiveResponse {
    pub active_versions: Vec<u16>,
    pub default_compile_target: u16,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Response with the node's canonical ABI hash for the active policy.
pub struct RuntimeAbiHashResponse {
    /// Policy label (first release: always "V1").
    pub policy: String,
    /// 32-byte lowercase hex digest of the ABI surface.
    pub abi_hash_hex: String,
}

// Tests omitted to keep feature-gating friction low; behavior is trivial (hash compute) and
// exercised by doc-sync tests in the ivm crate.

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct RuntimeUpgradeListItem {
    pub id_hex: String,
    pub record: iroha_data_model::runtime::RuntimeUpgradeRecord,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct RuntimeUpgradesListResponse {
    pub items: Vec<RuntimeUpgradeListItem>,
}

/// GET /v2/runtime/abi/active
pub async fn handle_runtime_abi_active(
    state: Arc<iroha_core::state::State>,
) -> Result<RuntimeAbiActiveResponse, crate::Error> {
    let world = state.world_view();
    let active = world.active_abi_versions();
    let mut list: Vec<u16> = active.into_iter().collect();
    list.sort_unstable();
    let default = *list.last().unwrap_or(&1);
    Ok(RuntimeAbiActiveResponse {
        active_versions: list,
        default_compile_target: default,
    })
}

/// GET /v2/node/capabilities — advertise supported ABI versions and defaults
pub async fn handle_node_capabilities(
    state: Arc<iroha_core::state::State>,
) -> Result<NodeCapabilitiesResponse, crate::Error> {
    let world = state.world_view();
    let mut list: Vec<u16> = world.active_abi_versions().into_iter().collect();
    list.sort_unstable();
    let default = *list.last().unwrap_or(&1);
    let crypto_cfg = state.crypto();
    let allowed_signing: Vec<String> = crypto_cfg
        .allowed_signing
        .iter()
        .map(|algo| algo.as_static_str().to_string())
        .collect();
    let curve_caps = summarize_curve_capabilities(&crypto_cfg);
    #[cfg(feature = "sm")]
    let (neon_sm3, neon_sm4, policy_string) = {
        let advert = iroha_crypto::sm::acceleration_advert();
        (
            advert.neon_sm3,
            advert.neon_sm4,
            advert.as_policy_str().to_string(),
        )
    };
    #[cfg(not(feature = "sm"))]
    let (neon_sm3, neon_sm4, policy_string) = (false, false, "scalar-only".to_string());
    Ok(NodeCapabilitiesResponse {
        supported_abi_versions: list,
        default_compile_target: default,
        data_model_version: iroha_data_model::DATA_MODEL_VERSION,
        crypto: NodeCryptoCapabilities {
            sm: NodeSmCapabilities {
                enabled: crypto_cfg.sm_helpers_enabled(),
                default_hash: crypto_cfg.default_hash.clone(),
                allowed_signing,
                sm2_distid_default: crypto_cfg.sm2_distid_default.clone(),
                openssl_preview: crypto_cfg.enable_sm_openssl_preview,
                acceleration: NodeSmAcceleration {
                    scalar: true,
                    neon_sm3,
                    neon_sm4,
                    policy: policy_string,
                },
            },
            curves: curve_caps,
        },
    })
}

fn summarize_curve_capabilities(
    crypto: &iroha_config::parameters::actual::Crypto,
) -> NodeCurveCapabilities {
    let mut ids = crypto.allowed_curve_ids.clone();
    if ids.is_empty() {
        ids = iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
            &crypto.allowed_signing,
        );
    }
    if ids.is_empty() {
        warn!(
            target: "iroha_torii::runtime",
            "allowed_curve_ids resolved to an empty list; defaulting to ed25519-only advert"
        );
        ids.push(CurveId::ED25519.as_u8());
    }
    ids.sort_unstable();
    ids.dedup();
    let bitmap = curve_bitmap_from_ids(&ids);
    NodeCurveCapabilities {
        registry_version: CURVE_REGISTRY_VERSION,
        allowed_curve_ids: ids,
        allowed_curve_bitmap: bitmap,
    }
}

fn curve_bitmap_from_ids(ids: &[u8]) -> Vec<u64> {
    if ids.is_empty() {
        return Vec::new();
    }
    // 256 identifier slots => four 64-bit lanes; trim trailing zeros later.
    let mut lanes = [0u64; 4];
    for &id in ids {
        let lane = (id / 64) as usize;
        let offset = id % 64;
        if let Some(slot) = lanes.get_mut(lane) {
            *slot |= 1u64 << offset;
        }
    }
    let mut vec = lanes.to_vec();
    while vec.len() > 1 && matches!(vec.last(), Some(&0)) {
        vec.pop();
    }
    if vec.len() == 1 && vec[0] == 0 {
        vec.clear();
    }
    vec
}

#[cfg(test)]
mod bitmap_tests {
    use super::curve_bitmap_from_ids;

    #[test]
    fn bitmap_handles_edges() {
        assert!(curve_bitmap_from_ids(&[]).is_empty());
        assert_eq!(curve_bitmap_from_ids(&[0]), vec![1]);
        assert_eq!(
            curve_bitmap_from_ids(&[1, 63]),
            vec![(1u64 << 1) | (1u64 << 63)]
        );
        // Highest identifier should land in the final lane.
        let mut ids = vec![255];
        let bitmap = curve_bitmap_from_ids(&ids);
        assert_eq!(bitmap.len(), 4);
        assert_eq!(bitmap[3], 1u64 << 63);
        assert_eq!(bitmap[0..3], [0, 0, 0]);
        // Multiple lanes retain intermediate zeros.
        ids.extend([0, 128]);
        let bitmap = curve_bitmap_from_ids(&ids);
        assert_eq!(bitmap, vec![1, 0, 1, 1u64 << 63]);
    }
}

/// GET /v2/runtime/metrics — expose runtime metrics summary
pub async fn handle_runtime_metrics(
    state: Arc<iroha_core::state::State>,
) -> Result<RuntimeMetricsResponse, crate::Error> {
    let world = state.world_view();
    let active_count = world.active_abi_versions().len() as u64;
    let mut proposed: u64 = 0;
    let mut activated: u64 = 0;
    let mut canceled: u64 = 0;
    for (_id, rec) in world.runtime_upgrades().iter() {
        proposed = proposed.saturating_add(1);
        match rec.status {
            iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(_) => {
                activated = activated.saturating_add(1)
            }
            iroha_data_model::runtime::RuntimeUpgradeStatus::Canceled => {
                canceled = canceled.saturating_add(1)
            }
            _ => {}
        }
    }
    Ok(RuntimeMetricsResponse {
        active_abi_versions_count: active_count,
        upgrade_events_total: UpgradeEventsCounters {
            proposed,
            activated,
            canceled,
        },
    })
}

/// GET /v2/runtime/abi/hash — return the canonical ABI hash for the node's active policy.
pub async fn handle_runtime_abi_hash(
    _state: Arc<iroha_core::state::State>,
) -> Result<RuntimeAbiHashResponse, crate::Error> {
    // First release: single policy V1
    let h = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    Ok(RuntimeAbiHashResponse {
        policy: "V1".to_string(),
        abi_hash_hex: hex::encode::<[u8; 32]>(h),
    })
}

/// GET /v2/runtime/upgrades
///
/// # Errors
/// Returns an error if the state view cannot be acquired or serialized.
pub async fn handle_runtime_upgrades_list(
    state: Arc<iroha_core::state::State>,
) -> Result<RuntimeUpgradesListResponse, crate::Error> {
    let world = state.world_view();
    let mut items: Vec<RuntimeUpgradeListItem> = Vec::new();
    for (id, rec) in world.runtime_upgrades().iter() {
        items.push(RuntimeUpgradeListItem {
            id_hex: hex::encode(id.0),
            record: rec.clone(),
        });
    }
    // Stable sort: by start_height then abi_version
    items.sort_by_key(|it| {
        (
            it.record.manifest.start_height,
            it.record.manifest.abi_version,
        )
    });
    Ok(RuntimeUpgradesListResponse { items })
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize)]
pub struct ProposeUpgradeDto(pub iroha_data_model::runtime::RuntimeUpgradeManifest);

#[derive(Debug, JsonSerialize, NoritoSerialize)]
pub struct TxInstr {
    pub wire_id: String,
    pub payload_hex: String,
}

fn instruction_box_to_tx_instr(boxed: iroha_data_model::isi::InstructionBox) -> TxInstr {
    use iroha_data_model::isi::Instruction;

    let wire_id = Instruction::id(&*boxed).to_string();
    let payload = Instruction::dyn_encode(&*boxed);
    TxInstr {
        wire_id,
        payload_hex: hex::encode(payload),
    }
}

#[derive(Debug, JsonSerialize, NoritoSerialize)]
pub struct ProposeUpgradeResponse {
    pub ok: bool,
    pub tx_instructions: Vec<TxInstr>,
}

/// POST /v2/runtime/upgrades/propose
pub async fn handle_runtime_propose_upgrade(
    NoritoJson(ProposeUpgradeDto(manifest)): NoritoJson<ProposeUpgradeDto>,
) -> Result<ProposeUpgradeResponse, crate::Error> {
    let manifest_bytes = norito::to_bytes(&manifest).map_err(|e| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "encode manifest: {e}"
            )),
        ))
    })?;
    let isi = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes };
    let boxed: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![instruction_box_to_tx_instr(boxed)];
    Ok(ProposeUpgradeResponse {
        ok: true,
        tx_instructions,
    })
}

#[derive(Debug, JsonSerialize, NoritoSerialize)]
/// Response payload describing the outcome of runtime activation/cancellation helpers.
pub struct ActivateCancelResponse {
    /// Indicates whether the operation succeeded.
    pub ok: bool,
    /// Instructions (if any) that must be signed and submitted by the caller.
    pub tx_instructions: Vec<TxInstr>,
}

impl IntoResponse for ActivateCancelResponse {
    fn into_response(self) -> axum::response::Response {
        crate::JsonBody(self).into_response()
    }
}

/// POST /v2/runtime/upgrades/activate/{id}
///
/// # Errors
/// Returns an error when the provided upgrade identifier is malformed or activation fails.
pub async fn handle_runtime_activate_upgrade(
    Path(id): Path<String>,
) -> Result<ActivateCancelResponse, crate::Error> {
    let s = id.trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion("invalid id".into()),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid id length".into(),
                ),
            ),
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let isi = iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade {
        id: iroha_data_model::runtime::RuntimeUpgradeId(arr),
    };
    let boxed: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![instruction_box_to_tx_instr(boxed)];
    Ok(ActivateCancelResponse {
        ok: true,
        tx_instructions,
    })
}

/// POST /v2/runtime/upgrades/cancel/{id}
///
/// # Errors
/// Returns an error when the identifier cannot be decoded or cancellation fails.
pub async fn handle_runtime_cancel_upgrade(
    Path(id): Path<String>,
) -> Result<ActivateCancelResponse, crate::Error> {
    let s = id.trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion("invalid id".into()),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid id length".into(),
                ),
            ),
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let isi = iroha_data_model::isi::runtime_upgrade::CancelRuntimeUpgrade {
        id: iroha_data_model::runtime::RuntimeUpgradeId(arr),
    };
    let boxed: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![instruction_box_to_tx_instr(boxed)];
    Ok(ActivateCancelResponse {
        ok: true,
        tx_instructions,
    })
}

#[cfg(test)]
mod tests {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::State};

    use super::*;

    #[tokio::test]
    async fn runtime_abi_hash_matches_ivm() {
        // Build a minimal state (not used by the handler, but required by signature)
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        let resp = handle_runtime_abi_hash(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert_eq!(resp.policy, "V1");
        // Expected hex length for 32 bytes
        assert_eq!(resp.abi_hash_hex.len(), 64);
        let expected = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        assert_eq!(resp.abi_hash_hex, hex::encode::<[u8; 32]>(expected));
    }

    #[tokio::test]
    async fn node_capabilities_contains_v1() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        let resp = handle_node_capabilities(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert!(resp.supported_abi_versions.contains(&1));
        assert_eq!(resp.default_compile_target, 1);
        assert_eq!(resp.crypto.curves.registry_version, CURVE_REGISTRY_VERSION);
        assert!(
            resp.crypto
                .curves
                .allowed_curve_ids
                .contains(&CurveId::ED25519.as_u8()),
            "expected ED25519 curve id to be advertised"
        );
    }

    #[tokio::test]
    async fn runtime_metrics_defaults() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        let resp = handle_runtime_metrics(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert_eq!(resp.active_abi_versions_count, 1);
        assert_eq!(resp.upgrade_events_total.proposed, 0);
        assert_eq!(resp.upgrade_events_total.activated, 0);
        assert_eq!(resp.upgrade_events_total.canceled, 0);
    }
}

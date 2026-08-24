//! Public offline-wallet protocol capability.
//!
//! The first-release capability is universal: it describes the protocol surface compiled into
//! every app-api node and never carries asset, verifier, release, or backend-readiness state.

use iroha_schema::IntoSchema;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

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
    /// Exact irreversible peer-cash handoff contract.
    pub cash_handoff_capability: String,
    /// Exact native bridge ABI required for authenticated V4 artifacts.
    pub required_bridge_abi_version: u32,
    /// Maximum peer-spend hop depth accepted by the protocol.
    pub max_hops: u32,
    /// Protocol capability availability. This is independent of backend and asset state.
    pub ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn universal_status() -> OfflineStatus {
        OfflineStatus {
            cash_handoff_capability: "cash_handoff_v1".to_owned(),
            required_bridge_abi_version: 22,
            max_hops: 8,
            ready: true,
        }
    }

    #[test]
    fn universal_status_norito_roundtrip_preserves_exact_projection() {
        let status = universal_status();
        let bytes = norito::to_bytes(&status).expect("encode status");
        let decoded: OfflineStatus = norito::decode_from_bytes(&bytes).expect("decode status");
        assert_eq!(decoded, status);
    }

    #[test]
    fn universal_status_json_rejects_retired_readiness_fields() {
        let canonical = norito::json::to_json(&universal_status()).expect("serialize status");
        let decoded: OfflineStatus = norito::json::from_json(&canonical).expect("decode status");
        assert_eq!(decoded, universal_status());

        for retired in [
            r#""assets":[],"#,
            r#""blockers":[],"#,
            r#""mandatory":false,"#,
            r#""asset_definition_id":"xor#wonderland","#,
        ] {
            let payload = canonical.replacen('{', &format!("{{{retired}"), 1);
            assert!(
                norito::json::from_json::<OfflineStatus>(&payload).is_err(),
                "retired field was accepted: {payload}"
            );
        }
    }
}

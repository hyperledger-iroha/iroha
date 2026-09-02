//! Canonical response envelopes for the Sumeragi evidence audit API.
//!
//! Torii and every binary client must use these shared types directly. Norito
//! frame schema identity is nominal, so a structurally identical private
//! response type is not wire-compatible with these public envelopes.

use iroha_data_model::block::consensus::EvidenceRecord;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Maximum page size accepted by the first-release evidence-list route.
pub const SUMERAGI_EVIDENCE_LIST_MAX_LIMIT: u32 = 1_000;
/// Default page size used when the evidence-list request omits `limit`.
pub const SUMERAGI_EVIDENCE_LIST_DEFAULT_LIMIT: u32 = 50;
/// Maximum page offset accepted by the first-release evidence-list route.
pub const SUMERAGI_EVIDENCE_LIST_MAX_OFFSET: u32 = 10_000;
/// Maximum JSON or Norito body returned by the evidence-count route.
pub const SUMERAGI_EVIDENCE_COUNT_RESPONSE_MAX_BYTES: usize = 1024;
/// Maximum JSON body returned by the evidence-list route.
pub const SUMERAGI_EVIDENCE_LIST_JSON_RESPONSE_MAX_BYTES: usize = 1024 * 1024;
/// Maximum canonical Norito body returned by the evidence-list route.
pub const SUMERAGI_EVIDENCE_LIST_NORITO_RESPONSE_MAX_BYTES: usize = 17 * 1024 * 1024;
/// Stable Norito schema name for the evidence-count response.
pub const SUMERAGI_EVIDENCE_COUNT_RESPONSE_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.sumeragi.evidence.count.response";
/// Stable Norito schema name for the evidence-list wire response.
pub const SUMERAGI_EVIDENCE_LIST_WIRE_RESPONSE_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.sumeragi.evidence.list.response";

/// Exact response returned by `/v1/sumeragi/evidence/count`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
)]
#[norito(
    schema_name = "iroha.torii.v1.sumeragi.evidence.count.response",
    deny_unknown_fields
)]
pub struct SumeragiEvidenceCountResponse {
    /// Total number of committed evidence records.
    pub count: u64,
}

/// Exact Norito response returned by `/v1/sumeragi/evidence`.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(schema_name = "iroha.torii.v1.sumeragi.evidence.list.response")]
pub struct SumeragiEvidenceListWireResponse {
    /// Total number of matching committed evidence records before pagination.
    pub total: u64,
    /// Bounded page of complete committed evidence records.
    pub items: Vec<EvidenceRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_envelopes_use_their_public_schema_names() {
        assert_eq!(
            <SumeragiEvidenceCountResponse as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(SUMERAGI_EVIDENCE_COUNT_RESPONSE_SCHEMA_NAME_V1)
        );
        assert_eq!(
            <SumeragiEvidenceListWireResponse as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(SUMERAGI_EVIDENCE_LIST_WIRE_RESPONSE_SCHEMA_NAME_V1)
        );
    }

    #[test]
    fn count_json_contract_is_exact() {
        let decoded: SumeragiEvidenceCountResponse =
            norito::json::from_str(r#"{"count":7}"#).expect("decode exact count response");
        assert_eq!(decoded.count, 7);
        for invalid in [r#"{}"#, r#"{"count":"7"}"#, r#"{"count":7,"extra":0}"#] {
            assert!(
                norito::json::from_str::<SumeragiEvidenceCountResponse>(invalid).is_err(),
                "invalid count response must be rejected: {invalid}"
            );
        }
    }
}

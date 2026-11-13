//! Selected kernel custody of native escrow completion captures.
//!
//! The frozen asset-escrow record is unchanged. This independent table is installed only
//! after earlier live custody drains and binds each new escrow to its original execution.

use norito::codec::{Decode, Encode};

use super::obligation::CompletionCaptureV1;

/// Service that retains native escrow execution when ordinary bindings change.
pub const SERVICE: &str = "native_escrow_capture";
/// Immutable semantics of the selected native custody service.
pub const SERVICE_CLOSURE: &[u8] =
    b"iroha:runtime:native-escrow-capture:v1:original-custody-kind-and-completion-closure";

/// Native owner of one escrow's progression and terminal custody operations.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::native_escrow::NativeEscrowKindV1")]
#[norito(tag = "kind", content = "content")]
pub enum NativeEscrowKindV1 {
    /// Seller/buyer acceptance, payment, release and dispute resolution.
    Marketplace,
    /// Generic native lock with drawdown, cancellation and expiry.
    Lock,
    /// Native all-of conditions and refund expiry.
    Conditional,
    /// Authenticated contract-controller instructions.
    ControlledLock,
    /// Native orderbook operations, conservatively pinned as one batch call graph.
    OrderbookLock,
}

/// Immutable opening identity and execution retained while the authoritative row is live.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::native_escrow::NativeEscrowCaptureV1")]
pub struct NativeEscrowCaptureV1 {
    /// Native owner of the escrow; no caller selects a different terminal corridor.
    pub kind: NativeEscrowKindV1,
    /// Digest of the immutable custody identity, excluding mutable progress fields.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub opening: [u8; 32],
    /// Exact original binding graph, storage and root-wide execution profiles.
    pub capture: CompletionCaptureV1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::definition::DefinitionId;

    #[test]
    fn native_escrow_records_roundtrip_every_owner_with_bounded_json() {
        for kind in [
            NativeEscrowKindV1::Marketplace,
            NativeEscrowKindV1::Lock,
            NativeEscrowKindV1::Conditional,
            NativeEscrowKindV1::ControlledLock,
            NativeEscrowKindV1::OrderbookLock,
        ] {
            let record = NativeEscrowCaptureV1 {
                kind,
                opening: [7; 32],
                capture: CompletionCaptureV1 {
                    kernel: DefinitionId([1; 32]),
                    commitment_profile: DefinitionId([2; 32]),
                    services_profile: DefinitionId([3; 32]),
                    host_profiles: vec![],
                    terminal: vec![],
                    default_implementation: None,
                    nested: vec![],
                },
            };
            let bytes = norito::encode_canonical(&record).unwrap();
            assert_eq!(
                norito::decode_canonical::<NativeEscrowCaptureV1>(&bytes).unwrap(),
                record
            );
            let json = norito::json::to_json(&record).unwrap();
            assert_eq!(
                norito::json::from_str::<NativeEscrowCaptureV1>(&json).unwrap(),
                record
            );
            assert_eq!(
                norito::json::to_json_bounded(&record, json.len()).unwrap(),
                json
            );
            assert_eq!(
                norito::json::to_json_bounded(&record, json.len() - 1),
                Err(norito::json::BoundedJsonError::BodyTooLarge)
            );
        }
    }
}

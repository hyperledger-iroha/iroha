//! Selected native AMX completion custody without changing the AMX protocol records.

use norito::codec::{Decode, Encode};

use super::obligation::CompletionCaptureV1;
use iroha_model_base::topology::DataSpaceId;

/// Service that retains original global decision and native participant settlement semantics.
pub const SERVICE: &str = "native_amx_capture";
/// Immutable semantics, including the global automatic deadline finalizer.
pub const SERVICE_CLOSURE: &[u8] =
    b"iroha:runtime:native-amx-capture:v1:original-decision-settlement-and-deadline-cohorts";

/// Exact native owner of one transaction's pending completion on this root.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::native_amx::NativeAmxKindV1")]
#[norito(tag = "kind", content = "content")]
pub enum NativeAmxKindV1 {
    /// The global root has recorded Begin and still owes its certified decision.
    GlobalDecision,
    /// The named native participant has voted Yes and still holds its original monetary leg.
    ParticipantSettlement(DataSpaceId),
}

/// Kernel-owned capture bound to an authoritative immutable AMX opening.
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
#[norito_schema(name = "iroha_data_model::runtime::native_amx::NativeAmxCaptureV1")]
pub struct NativeAmxCaptureV1 {
    /// Global decision or exact local participant settlement.
    pub kind: NativeAmxKindV1,
    /// Original canonical transaction identity.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub tx: [u8; 32],
    /// Digest of immutable Begin or Yes/custody fields; progress is excluded.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub opening: [u8; 32],
    /// Original definitions, storage and root-wide services; native AMX captures no IVM host.
    pub capture: CompletionCaptureV1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::definition::DefinitionId;

    #[test]
    fn native_amx_records_roundtrip_each_owner_with_bounded_json() {
        for kind in [
            NativeAmxKindV1::GlobalDecision,
            NativeAmxKindV1::ParticipantSettlement(DataSpaceId::new(u64::MAX)),
        ] {
            let record = NativeAmxCaptureV1 {
                kind,
                tx: [5; 32],
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
                norito::decode_canonical::<NativeAmxCaptureV1>(&bytes).unwrap(),
                record
            );
            let json = norito::json::to_json(&record).unwrap();
            assert_eq!(
                norito::json::from_str::<NativeAmxCaptureV1>(&json).unwrap(),
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

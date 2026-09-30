//! Original native World content for KAGEMUSHA issuer authority selection.
//!
//! Decoding this response grants no authority. Clients independently authenticate
//! the node, challenge, current finality prefix, selected root and release policy.
//! Complete snapshot preimages then bind the actual typed values to that execution.

use iroha_data_model::{
    asset::AssetDefinition,
    kagemusha::KagemushaGovernedVerifierRegistryV1,
    nexus::AxtAssetIncarnationV1,
    sumeragi_finality::{SumeragiFinalityAttestation, WorldStateSnapshotV1},
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Canonical relative resource prefix; the suffix is one exact asset definition ID.
pub const KAGEMUSHA_AUTHORITY_STATE_ROUTE_PREFIX_V1: &str = "/v1/kagemusha/authority-state/";
/// Reader ceiling for the snapshot, genesis/tip original proofs and typed values.
/// This is a reader bound, not a native consensus invalidity verdict.
pub const KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1: usize = 128 * 1024 * 1024;

/// Data-only current native response; no caller-selected root is embedded.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_torii_shared::kagemusha_state::KagemushaAuthorityStateV1",
    frame = "iroha.torii.v1.kagemusha.authority-state.response"
)]
pub struct KagemushaAuthorityStateV1 {
    /// Installed node's challenged statement for the same durable applied tip.
    pub attestation: SumeragiFinalityAttestation,
    /// Every canonical World element's complete hash preimage at that tip.
    pub world_snapshot: WorldStateSnapshotV1,
    /// Original selected definition, including its scale and ownership policy.
    pub asset_definition: AssetDefinition,
    /// Original incarnation of that definition's current registration.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Complete original governed registry, including release status and revocation.
    pub verifier_registry: KagemushaGovernedVerifierRegistryV1,
}

/// Exact bounded canonical decoding, without finality or installed authority admission.
///
/// # Errors
/// Empty, oversized, noncanonical or malformed original response.
pub fn decode_unverified_kagemusha_authority_state_v1(
    bytes: &[u8],
) -> Result<KagemushaAuthorityStateV1, norito::Error> {
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1 {
        return Err(norito::Error::Message(
            "native authority state exceeds reader bound".into(),
        ));
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::canonical_decode_limits(KAGEMUSHA_AUTHORITY_STATE_MAX_BYTES_V1),
    )
}

/// Borrowed encoder of the sole owned response layout. It clones no World graphs.
#[derive(NoritoSerialize, JsonSerialize)]
pub struct KagemushaAuthorityStateRefV1<'a> {
    attestation: FieldRef<'a, SumeragiFinalityAttestation>,
    world_snapshot: FieldRef<'a, WorldStateSnapshotV1>,
    asset_definition: FieldRef<'a, AssetDefinition>,
    asset_incarnation: FieldRef<'a, AxtAssetIncarnationV1>,
    verifier_registry: FieldRef<'a, KagemushaGovernedVerifierRegistryV1>,
}
impl<'a> KagemushaAuthorityStateRefV1<'a> {
    /// Borrow already retained originals for an admitted encoder. No authority is granted.
    #[must_use]
    pub fn new(
        attestation: &'a SumeragiFinalityAttestation,
        world_snapshot: &'a WorldStateSnapshotV1,
        asset_definition: &'a AssetDefinition,
        asset_incarnation: &'a AxtAssetIncarnationV1,
        verifier_registry: &'a KagemushaGovernedVerifierRegistryV1,
    ) -> Self {
        Self {
            attestation: FieldRef(attestation),
            world_snapshot: FieldRef(world_snapshot),
            asset_definition: FieldRef(asset_definition),
            asset_incarnation: FieldRef(asset_incarnation),
            verifier_registry: FieldRef(verifier_registry),
        }
    }
}
impl norito::NoritoSchema for KagemushaAuthorityStateRefV1<'_> {
    fn nominal_name() -> String {
        <KagemushaAuthorityStateV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <KagemushaAuthorityStateV1 as norito::NoritoSchema>::frame_name()
    }
}

// Payload-only forwarding preserves each field's exact codec and bounded JSON writer.
struct FieldRef<'a, T>(&'a T);
impl<T: norito::core::SerializePayload> norito::core::SerializePayload for FieldRef<'_, T> {
    fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::SerializePayload::serialize(self.0, out)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_hint(self.0)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_exact(self.0)
    }
}
impl<T: norito::json::JsonSerialize> norito::json::JsonSerialize for FieldRef<'_, T> {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}

#[cfg(test)]
mod tests;

//! Canonical request and response records for state-reading and VRF syscalls.
//!
//! These records live in the shared ABI crate so the compiler, standalone VM, and ledger host all
//! use one nominal Norito schema at every protocol boundary.
use iroha_data_model::NetworkId;
use norito::core::DecodeLimits;
use norito::{Decode, Encode};
/// Maximum canonical request frame accepted by either V1 VRF verification syscall.
pub const MAX_VRF_VERIFY_PAYLOAD_BYTES_V1: usize = 64 * 1024;
/// Maximum number of verification items accepted by one V1 VRF batch.
pub const MAX_VRF_VERIFY_BATCH_ITEMS_V1: usize = 16;
/// Resource budget for decoding one attacker-controlled V1 VRF request frame.
///
/// Byte-vector fields may consume the complete frame budget, while cumulative elements stay bounded
/// by the same 64 KiB wire contract. The eightfold native-allocation allowance covers nested field
/// accounting, owned byte vectors, and the outer batch vector without weakening the hard bound.
/// Sixteen nested length-delimited scopes exceed the depth required by either the single or batch
/// nominal schema.
pub const VRF_VERIFY_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MAX_VRF_VERIFY_PAYLOAD_BYTES_V1,
    MAX_VRF_VERIFY_PAYLOAD_BYTES_V1,
    MAX_VRF_VERIFY_PAYLOAD_BYTES_V1,
    MAX_VRF_VERIFY_PAYLOAD_BYTES_V1 * 8,
    16,
);
/// Request for recent commitment roots associated with an asset definition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::RootsGetRequest",
    frame = "iroha.ivm.v1.RootsGetRequest"
)]
pub struct RootsGetRequest {
    /// Canonical asset-definition identifier.
    pub asset_id: String,
    /// Maximum number of historical roots to return.
    pub max: u32,
}
/// Recent commitment roots returned by the ledger host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::RootsGetResponse",
    frame = "iroha.ivm.v1.RootsGetResponse"
)]
pub struct RootsGetResponse {
    /// Latest commitment root.
    pub latest: [u8; 32],
    /// Historical roots in canonical host order.
    pub roots: Vec<[u8; 32]>,
    /// Height associated with `latest`.
    pub height: u32,
}
/// Request for the finalized tally of one private election.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::VoteGetTallyRequest",
    frame = "iroha.ivm.v1.VoteGetTallyRequest"
)]
pub struct VoteGetTallyRequest {
    /// Canonical governance selector V1 identifying the election.
    pub election_id: String,
}
impl VoteGetTallyRequest {
    /// Return whether the request carries a canonical governance selector V1.
    #[must_use]
    pub fn is_valid_v1(&self) -> bool {
        iroha_data_model::governance::is_valid_governance_selector_v1(&self.election_id)
    }
}
/// Finalized election tally returned by the ledger host.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::VoteGetTallyResponse",
    frame = "iroha.ivm.v1.VoteGetTallyResponse"
)]
pub struct VoteGetTallyResponse {
    /// Whether the election has been finalized.
    pub finalized: bool,
    /// Candidate totals in canonical candidate order.
    pub tally: Vec<u64>,
}
/// Single VRF verification request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::VrfVerifyRequest",
    frame = "iroha.ivm.v1.VrfVerifyRequest"
)]
pub struct VrfVerifyRequest {
    /// Variant: 1 = proof in G2; 2 = proof in G1.
    pub variant: u8,
    /// Public-key bytes.
    pub pk: Vec<u8>,
    /// Proof/signature bytes.
    pub proof: Vec<u8>,
    /// Exact genesis-derived network identity.
    pub network_id: NetworkId,
    /// Input message bytes.
    pub input: Vec<u8>,
}
/// Batch verification request whose outputs preserve item order.
///
/// Runtime admission requires `1..=MAX_VRF_VERIFY_BATCH_ITEMS_V1` items and a complete canonical
/// frame no larger than [`MAX_VRF_VERIFY_PAYLOAD_BYTES_V1`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::VrfVerifyBatchRequest",
    frame = "iroha.ivm.v1.VrfVerifyBatchRequest"
)]
pub struct VrfVerifyBatchRequest {
    /// Items to verify in order.
    pub items: Vec<VrfVerifyRequest>,
}
/// Request for a persisted VRF epoch-seed snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::VrfEpochSeedRequest",
    frame = "iroha.ivm.v1.VrfEpochSeedRequest"
)]
pub struct VrfEpochSeedRequest {
    /// Epoch to fetch from world-state storage.
    pub epoch: u64,
    /// If true and `epoch` is absent, return the latest known epoch seed.
    pub fallback_to_latest: bool,
}
/// Persisted VRF epoch-seed response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "ivm_abi::host_payload::VrfEpochSeedResponse",
    frame = "iroha.ivm.v1.VrfEpochSeedResponse"
)]
pub struct VrfEpochSeedResponse {
    /// Whether a seed snapshot was found.
    pub found: bool,
    /// Epoch associated with `seed`.
    pub epoch: u64,
    /// Seed bytes, all zero when `found` is false.
    pub seed: [u8; 32],
}
#[cfg(test)]
mod tests {
    use super::VoteGetTallyRequest;
    #[test]
    fn vote_tally_request_uses_canonical_governance_selector_v1() {
        for election_id in ["election-1", "A9_selector~with.dots"] {
            assert!(
                VoteGetTallyRequest {
                    election_id: election_id.to_owned(),
                }
                .is_valid_v1()
            );
        }
        for election_id in ["", ".hidden", "election/alias", "election\nalias"] {
            assert!(
                !VoteGetTallyRequest {
                    election_id: election_id.to_owned(),
                }
                .is_valid_v1()
            );
        }
        assert!(
            VoteGetTallyRequest {
                election_id: "a".repeat(128),
            }
            .is_valid_v1()
        );
        assert!(
            !VoteGetTallyRequest {
                election_id: "a".repeat(129),
            }
            .is_valid_v1()
        );
    }
}

#[cfg(test)]
mod captured_frame_identity_tests {
    #[test]
    fn observed_declared_identities() {
        crate::captured_identity_tests::assert_bidirectional::<super::RootsGetRequest>(
            "ivm_abi::host_payload::RootsGetRequest",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::RootsGetResponse>(
            "ivm_abi::host_payload::RootsGetResponse",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::VoteGetTallyRequest>(
            "ivm_abi::host_payload::VoteGetTallyRequest",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::VoteGetTallyResponse>(
            "ivm_abi::host_payload::VoteGetTallyResponse",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::VrfVerifyRequest>(
            "ivm_abi::host_payload::VrfVerifyRequest",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::VrfVerifyBatchRequest>(
            "ivm_abi::host_payload::VrfVerifyBatchRequest",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::VrfEpochSeedRequest>(
            "ivm_abi::host_payload::VrfEpochSeedRequest",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::VrfEpochSeedResponse>(
            "ivm_abi::host_payload::VrfEpochSeedResponse",
        );
    }
}

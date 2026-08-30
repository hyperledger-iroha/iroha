//! Canonical first-release wire contract for hardware-guarded offline balances.

use super::{
    KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2, KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2,
    KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, KagemushaValidationError,
    is_kagemusha_network_id,
};
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{NetworkId, account::AccountId, asset::AssetDefinitionId};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Version carried by every clean-slate offline-cash wire value.
pub const OFFLINE_CASH_WIRE_VERSION_V1: u16 = 1;
/// Text transport discriminator for canonical unpadded base64url messages.
pub const OFFLINE_CASH_TEXT_PREFIX_V1: &str = "kgm2:";
/// Maximum canonical receiver-request bytes.
pub const OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1: usize = 768;
/// Maximum canonical sender-response bytes.
pub const OFFLINE_CASH_PAYMENT_MAX_BYTES_V1: usize = 7_936;
/// Maximum canonical receiver-acknowledgement bytes.
pub const OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1: usize = 256;
/// Qualification target for the complete three-message raw session.
pub const OFFLINE_CASH_SESSION_TARGET_BYTES_V1: usize = 8_960;
/// Absolute pre-decode raw session limit implied by the text envelope.
pub const OFFLINE_CASH_SESSION_MAX_BYTES_V1: usize = 9_211;
/// Absolute complete text-session limit.
pub const OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1: usize = 12_288;
/// Qualification target for the two current recursive proofs.
pub const OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1: usize = 6_144;
/// Absolute byte limit for the two current recursive proofs.
pub const OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1: usize = 6_400;
/// Maximum bytes in either parity's current proof.
pub const OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1: usize = 3_200;
/// Exact public words in the shared recursive-pair binding.
pub const OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1: usize = 136;
/// Exact little-endian bytes in the circuit's expanded public binding.
pub const OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1: usize =
    OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1 * 4;
/// Exact canonical Norito field-payload bytes in the final-State compact binding.
///
/// A standalone framed archive additionally carries its Norito header and
/// alignment padding; the payment embeds this payload inside its one outer frame.
pub const OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1: usize = 4 + 32 + 32 + 32;
/// Exact canonical bytes hashed to join the two GuardBundle parity children.
pub const OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1: usize = 4 + 32 + 32;
/// Clean Offline Cash V1 carried-lineage wire version.
pub const OFFLINE_CASH_IPA_LINEAGE_VERSION_V1: u16 = 1;
/// Fixed IPA round count authenticated by the k=16 Offline Cash profile.
pub const OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1: u32 = 16;
/// Exact number of round-challenge scalars in one carried lineage.
pub const OFFLINE_CASH_IPA_LINEAGE_CHALLENGES_V1: usize =
    OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1 as usize;
/// Exact bytes occupied by the fixed scalar challenge array.
pub const OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1: usize =
    OFFLINE_CASH_IPA_LINEAGE_CHALLENGES_V1 * 32;
/// Exact scalar-and-point cryptographic payload bytes in one carried lineage.
pub const OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1: usize =
    OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1 + 32;
/// Exact canonical Norito field-payload bytes including version and round count.
///
/// A standalone framed archive additionally carries its Norito header and
/// alignment padding; the payment embeds this payload inside its one outer frame.
pub const OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1: usize = core::mem::size_of::<u16>()
    + core::mem::size_of::<u32>()
    + OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1;
/// Exact field-neutral `u128` public cells for one carried lineage.
pub const OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1: usize =
    2 * OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1 as usize + 4;
/// Maximum encrypted credit-opening bytes carried by a sender response.
pub const OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1: usize = 384;

const REQUEST_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-request-signing";
const REQUEST_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-request";
const PUBLIC_KEY_REFERENCE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:receiver-key-reference";
const X25519_FIELD_MODULUS_LITTLE_ENDIAN: [u8; 32] = [
    0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];
const TRANSITION_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:send-split-transition";
const STATEMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:send-split-statement";
const PAYMENT_DIGEST_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment";
const ACKNOWLEDGEMENT_SIGNING_DOMAIN: &[u8] = b"iroha:offline-cash:v1:acknowledgement-signing";
const GUARD_BUNDLE_PAIR_BINDING_DIGEST_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:guard-bundle-pair-binding";

/// Public send-split statement decided by both Pasta parities.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashTransferStatementV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Asset transferred by this relation.
    pub asset: AssetDefinitionId,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Positive transfer amount in atomic units.
    pub amount: u128,
    /// Digest of the exact receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Sender balance commitment consumed by the split.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_before: [u8; 32],
    /// Persisted sender-remainder commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_after: [u8; 32],
    /// Receiver balance commitment named by the request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_before: [u8; 32],
    /// Receiver-bound credit commitment produced by the split.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment: [u8; 32],
    /// Digest authorized by the sender hardware guard.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub transition_digest: [u8; 32],
}

/// Compact sender-produced portion of an offline-cash transfer statement.
///
/// The signed receiver request supplies the release, network, asset, scale,
/// amount, request digest, and receiver-before commitment. Keeping only the
/// three sender-produced commitments on the payment wire avoids carrying a second,
/// unauthenticated copy of request authority. Consumers must reconstruct and
/// validate the full [`OfflineCashTransferStatementV1`], including its derived
/// canonical transition digest, before proof use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashTransferResultV1 {
    /// Sender balance commitment consumed by the split.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_before: [u8; 32],
    /// Persisted sender-remainder commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub sender_after: [u8; 32],
    /// Receiver-bound credit commitment produced by the split.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub credit_commitment: [u8; 32],
}

/// Recursive topology authenticated by one paired-proof binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(rename_all = "snake_case")]
#[repr(u32)]
pub enum OfflineCashRecursivePairTopologyV1 {
    /// Final `State` wrapper over `StateLeaf` and `GuardBundle`.
    State = 1,
    /// Internal `GuardBundle` wrapper over its relation and hardware children.
    GuardBundle = 2,
}

/// Field-neutral k=16 IPA lineage carried by one final Pasta proof.
///
/// This data-model type enforces only fixed wire geometry. Curve-specific
/// canonical scalar parsing, compressed-point parsing, subgroup membership,
/// and the non-identity check remain in Core's parity-specific verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashIpaLineageV1 {
    /// Exact clean-V1 lineage version.
    pub version: u16,
    /// Exact authenticated IPA round count; fixed to sixteen.
    pub round_count: u32,
    /// Ordered concatenated canonical-width scalar encodings, parsed by Core per parity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub round_challenges: [u8; OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1],
    /// Canonical compressed accumulated generator, parsed by Core per parity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub folded_generator: [u8; 32],
}

impl norito::NoritoSerialize for OfflineCashIpaLineageV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&self.round_count.to_le_bytes())?;
        writer.write_all(&self.round_challenges)?;
        writer.write_all(&self.folded_generator)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for OfflineCashIpaLineageV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Offline Cash V1 lineage must use the exact fixed-width encoding")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (lineage, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        norito::core::note_payload_access(bytes, used);
        Ok(lineage)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for OfflineCashIpaLineageV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let payload = bytes
            .get(..OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let version = u16::from_le_bytes(
            payload[..2]
                .try_into()
                .expect("exact two-byte lineage version"),
        );
        let round_count = u32::from_le_bytes(
            payload[2..6]
                .try_into()
                .expect("exact four-byte lineage round count"),
        );
        let mut round_challenges = [0_u8; OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1];
        round_challenges
            .copy_from_slice(&payload[6..6 + OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1]);
        let mut folded_generator = [0_u8; 32];
        folded_generator.copy_from_slice(
            &payload[6 + OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1
                ..OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1],
        );
        let lineage = Self {
            version,
            round_count,
            round_challenges,
            folded_generator,
        };
        lineage
            .validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((lineage, OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1))
    }
}

impl OfflineCashIpaLineageV1 {
    /// Construct the fixed k=16 lineage from canonical-width encodings.
    ///
    /// # Errors
    ///
    /// Returns an error for an all-zero compressed-point encoding.
    pub fn new(
        round_challenges: [[u8; 32]; OFFLINE_CASH_IPA_LINEAGE_CHALLENGES_V1],
        folded_generator: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        let mut encoded = [0_u8; OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1];
        for (target, challenge) in encoded.chunks_exact_mut(32).zip(round_challenges) {
            target.copy_from_slice(&challenge);
        }
        Self::from_encoded(encoded, folded_generator)
    }

    /// Construct from the exact concatenated scalar encodings used on wire.
    ///
    /// # Errors
    ///
    /// Returns an error for an all-zero compressed-point encoding.
    pub fn from_encoded(
        round_challenges: [u8; OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1],
        folded_generator: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        let lineage = Self {
            version: OFFLINE_CASH_IPA_LINEAGE_VERSION_V1,
            round_count: OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1,
            round_challenges,
            folded_generator,
        };
        lineage.validate()?;
        Ok(lineage)
    }

    /// Validate fixed wire geometry before any curve-aware parsing.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong version or round count, or for an
    /// all-zero compressed-point encoding.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_IPA_LINEAGE_VERSION_V1
            || self.round_count != OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1
            || self.folded_generator == [0; 32]
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.carried_lineage",
            });
        }
        Ok(())
    }

    /// Return the exact 36 field-neutral public `u128` cells.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed wire geometry is invalid.
    pub fn instance_limbs(
        &self,
    ) -> Result<[u128; OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1], KagemushaValidationError> {
        self.validate()?;
        let mut limbs = [0_u128; OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1];
        limbs[0] = u128::from(self.version);
        limbs[1] = u128::from(self.round_count);
        let mut offset = 2;
        for bytes in self
            .round_challenges
            .chunks_exact(32)
            .chain(core::iter::once(self.folded_generator.as_slice()))
        {
            for chunk in bytes.chunks_exact(16) {
                limbs[offset] =
                    u128::from_le_bytes(chunk.try_into().expect("exact 16-byte lineage limb"));
                offset += 1;
            }
        }
        debug_assert_eq!(offset, OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1);
        Ok(limbs)
    }
}

/// Compact cross-parity binding for deferred recursive verifier equations.
///
/// The final-State wire stores topology, the two 32-byte audit identities, and
/// a domain-separated digest joining the exact GuardBundle pair binding seen
/// by both parity wrappers. Circuit roles, stage ranges, gate tags, and
/// equation counts are fixed by the authenticated wrapper protocol/VK. Both
/// final Pasta proofs expand this value to the same canonical 136 public words
/// and constrain the reciprocal parity's point equations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashRecursivePairBindingV1 {
    /// Fixed recursive topology encoded as the canonical `u32` role value.
    topology: u32,
    /// Eq/Fp scalar-side deferred-equation audit identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub eq_audit_digest: [u8; 32],
    /// Ep/Fq scalar-side deferred-equation audit identity.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ep_audit_digest: [u8; 32],
    /// Domain-separated SHA-256 digest of the exact 68-byte GuardBundle binding.
    ///
    /// This is non-zero for final State and fixed to zero for GuardBundle.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub child_pair_binding_digest: [u8; 32],
}

impl norito::NoritoSerialize for OfflineCashRecursivePairBindingV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.topology.to_le_bytes())?;
        writer.write_all(&self.eq_audit_digest)?;
        writer.write_all(&self.ep_audit_digest)?;
        writer.write_all(&self.child_pair_binding_digest)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for OfflineCashRecursivePairBindingV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("Offline Cash V1 recursive-pair binding must use the compact fixed encoding")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (binding, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        norito::core::note_payload_access(bytes, used);
        Ok(binding)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for OfflineCashRecursivePairBindingV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let payload = bytes
            .get(..OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let topology = u32::from_le_bytes(
            payload[..4]
                .try_into()
                .expect("exact four-byte recursive-pair topology"),
        );
        let mut eq_audit_digest = [0_u8; 32];
        eq_audit_digest.copy_from_slice(&payload[4..36]);
        let mut ep_audit_digest = [0_u8; 32];
        ep_audit_digest.copy_from_slice(&payload[36..68]);
        let mut child_pair_binding_digest = [0_u8; 32];
        child_pair_binding_digest.copy_from_slice(&payload[68..100]);
        let binding = Self {
            topology,
            eq_audit_digest,
            ep_audit_digest,
            child_pair_binding_digest,
        };
        binding
            .validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((
            binding,
            OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1,
        ))
    }
}

const RECURSIVE_PAIR_BINDING_ABI_V1: u32 = 1;
const RECURSIVE_PAIR_TRANSCRIPT_V1: u32 = 1;
const RECURSIVE_PAIR_POSEIDON_WIDTH_V1: u32 = 3;
const RECURSIVE_PAIR_POSEIDON_RATE_V1: u32 = 2;
const RECURSIVE_PAIR_POSEIDON_FULL_ROUNDS_V1: u32 = 8;
const RECURSIVE_PAIR_POSEIDON_PARTIAL_ROUNDS_V1: u32 = 57;
const RECURSIVE_PAIR_POSEIDON_SECURE_MDS_V1: u32 = 0;
const RECURSIVE_PAIR_PARITY_COUNT_V1: u32 = 2;
const RECURSIVE_PAIR_DIGEST_WORDS_V1: u32 = 8;
const RECURSIVE_PAIR_ABI_WORD_V1: usize = 0;
const RECURSIVE_PAIR_TOPOLOGY_WORD_V1: usize = 1;
const RECURSIVE_PAIR_TRANSCRIPT_WORD_V1: usize = 2;
const RECURSIVE_PAIR_POSEIDON_WIDTH_WORD_V1: usize = 3;
const RECURSIVE_PAIR_POSEIDON_RATE_WORD_V1: usize = 4;
const RECURSIVE_PAIR_POSEIDON_FULL_ROUNDS_WORD_V1: usize = 5;
const RECURSIVE_PAIR_POSEIDON_PARTIAL_ROUNDS_WORD_V1: usize = 6;
const RECURSIVE_PAIR_POSEIDON_SECURE_MDS_WORD_V1: usize = 7;
const RECURSIVE_PAIR_PARITY_COUNT_WORD_V1: usize = 8;
const RECURSIVE_PAIR_CHILD_COUNT_WORD_V1: usize = 9;
const RECURSIVE_PAIR_PARENT_ROLE_WORD_V1: usize = 10;
const RECURSIVE_PAIR_CHILD_ROLE_WORD_START_V1: usize = 11;
const RECURSIVE_PAIR_COMMON_ABI_WORDS_WORD_V1: usize = 17;
const RECURSIVE_PAIR_DIGEST_WORDS_WORD_V1: usize = 18;
const RECURSIVE_PAIR_HEADER_WORDS_V1: usize = 32;
const RECURSIVE_PAIR_EQ_AUDIT_WORD_START_V1: usize = 32;
const RECURSIVE_PAIR_EP_AUDIT_WORD_START_V1: usize = 40;
const RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1: usize = 48;
const RECURSIVE_PAIR_RESERVED_WORD_START_V1: usize = 56;

const _: () = assert!(RECURSIVE_PAIR_HEADER_WORDS_V1 == RECURSIVE_PAIR_EQ_AUDIT_WORD_START_V1);
const _: () =
    assert!(RECURSIVE_PAIR_EQ_AUDIT_WORD_START_V1 + 8 == RECURSIVE_PAIR_EP_AUDIT_WORD_START_V1);
const _: () = assert!(
    RECURSIVE_PAIR_EP_AUDIT_WORD_START_V1 + 8 == RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1
);
const _: () = assert!(
    RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1 + 8 == RECURSIVE_PAIR_RESERVED_WORD_START_V1
);
const _: () =
    assert!(RECURSIVE_PAIR_RESERVED_WORD_START_V1 <= OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1);

impl OfflineCashRecursivePairBindingV1 {
    /// Construct the internal GuardBundle binding from two audit digests.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty or aliased audit identity.
    pub fn new_guard_bundle(
        eq_audit_digest: [u8; 32],
        ep_audit_digest: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        Self::from_parts(
            OfflineCashRecursivePairTopologyV1::GuardBundle,
            eq_audit_digest,
            ep_audit_digest,
            [0; 32],
        )
    }

    /// Construct the final-State binding and join one exact GuardBundle pair.
    ///
    /// # Errors
    ///
    /// Returns an error if either binding is malformed or the child binding is
    /// not the internal GuardBundle topology.
    pub fn new_state(
        eq_audit_digest: [u8; 32],
        ep_audit_digest: [u8; 32],
        guard_bundle: &Self,
    ) -> Result<Self, KagemushaValidationError> {
        let child_pair_binding_digest =
            offline_cash_guard_bundle_pair_binding_digest_v1(guard_bundle)?;
        Self::from_parts(
            OfflineCashRecursivePairTopologyV1::State,
            eq_audit_digest,
            ep_audit_digest,
            child_pair_binding_digest,
        )
    }

    /// Validate topology-specific audit and child-binding identities.
    ///
    /// # Errors
    ///
    /// Returns an error when the binding cannot represent a complete paired
    /// recursive verifier audit.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let topology = parse_recursive_pair_topology(self.topology)?;
        Self::from_parts(
            topology,
            self.eq_audit_digest,
            self.ep_audit_digest,
            self.child_pair_binding_digest,
        )
        .map(|_| ())
    }

    /// Expand the compact wire into the exact 136-word circuit representation.
    ///
    /// # Errors
    ///
    /// Returns an error when fixed metadata or reserved zeros are non-canonical.
    pub fn canonical_words(
        &self,
    ) -> Result<[u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1], KagemushaValidationError> {
        self.validate()?;
        Ok(encode_recursive_pair_words(
            parse_recursive_pair_topology(self.topology)?,
            self.eq_audit_digest,
            self.ep_audit_digest,
            self.child_pair_binding_digest,
        ))
    }

    /// Strictly recover the recursive topology.
    pub fn topology(&self) -> Result<OfflineCashRecursivePairTopologyV1, KagemushaValidationError> {
        self.validate()?;
        parse_recursive_pair_topology(self.topology)
    }

    /// Strictly decode the 136-word representation and reject any non-zero
    /// reserved word or non-canonical fixed metadata.
    ///
    /// # Errors
    ///
    /// Returns an error unless re-encoding produces the exact input words.
    pub fn from_canonical_words(
        words: [u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1],
    ) -> Result<Self, KagemushaValidationError> {
        let topology = parse_recursive_pair_topology(words[RECURSIVE_PAIR_TOPOLOGY_WORD_V1])?;
        let eq_audit_digest =
            read_recursive_pair_digest_words(&words, RECURSIVE_PAIR_EQ_AUDIT_WORD_START_V1);
        let ep_audit_digest =
            read_recursive_pair_digest_words(&words, RECURSIVE_PAIR_EP_AUDIT_WORD_START_V1);
        let child_pair_binding_digest = read_recursive_pair_digest_words(
            &words,
            RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1,
        );
        let binding = Self::from_parts(
            topology,
            eq_audit_digest,
            ep_audit_digest,
            child_pair_binding_digest,
        )?;
        if binding.canonical_words()? != words {
            return Err(recursive_pair_binding_error());
        }
        Ok(binding)
    }

    /// Return the exact 68-byte GuardBundle digest preimage.
    ///
    /// The bytes are the little-endian topology discriminant followed by the
    /// Eq and Ep audit digests. The child-binding digest is required to be zero
    /// and is deliberately absent from this fixed projection.
    ///
    /// # Errors
    ///
    /// Returns an error unless this is a canonical GuardBundle binding.
    pub fn guard_bundle_canonical_bytes68(
        &self,
    ) -> Result<[u8; OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1], KagemushaValidationError>
    {
        self.validate()?;
        if parse_recursive_pair_topology(self.topology)?
            != OfflineCashRecursivePairTopologyV1::GuardBundle
        {
            return Err(recursive_pair_binding_error());
        }
        let mut bytes = [0_u8; OFFLINE_CASH_GUARD_BUNDLE_PAIR_BINDING_BYTES_V1];
        bytes[..4].copy_from_slice(&self.topology.to_le_bytes());
        bytes[4..36].copy_from_slice(&self.eq_audit_digest);
        bytes[36..68].copy_from_slice(&self.ep_audit_digest);
        Ok(bytes)
    }

    /// Verify that this final-State binding commits to `guard_bundle` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong topology or a mismatched child digest.
    pub fn validate_state_child_binding(
        &self,
        guard_bundle: &Self,
    ) -> Result<(), KagemushaValidationError> {
        self.validate()?;
        if parse_recursive_pair_topology(self.topology)?
            != OfflineCashRecursivePairTopologyV1::State
            || self.child_pair_binding_digest
                != offline_cash_guard_bundle_pair_binding_digest_v1(guard_bundle)?
        {
            return Err(recursive_pair_binding_error());
        }
        Ok(())
    }

    fn from_parts(
        topology: OfflineCashRecursivePairTopologyV1,
        eq_audit_digest: [u8; 32],
        ep_audit_digest: [u8; 32],
        child_pair_binding_digest: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        if eq_audit_digest == [0; 32]
            || ep_audit_digest == [0; 32]
            || eq_audit_digest == ep_audit_digest
            || match topology {
                OfflineCashRecursivePairTopologyV1::State => child_pair_binding_digest == [0; 32],
                OfflineCashRecursivePairTopologyV1::GuardBundle => {
                    child_pair_binding_digest != [0; 32]
                }
            }
        {
            return Err(recursive_pair_binding_error());
        }
        Ok(Self {
            topology: topology as u32,
            eq_audit_digest,
            ep_audit_digest,
            child_pair_binding_digest,
        })
    }
}

/// Hash the exact GuardBundle pair binding joined by both final State proofs.
///
/// The SHA-256 message is `domain || 0 || u64_le(68) || canonical_bytes68`.
/// This framing is source-authoritative for both host and circuit code.
///
/// # Errors
///
/// Returns an error unless `guard_bundle` is the canonical GuardBundle topology.
pub fn offline_cash_guard_bundle_pair_binding_digest_v1(
    guard_bundle: &OfflineCashRecursivePairBindingV1,
) -> Result<[u8; 32], KagemushaValidationError> {
    Ok(
        Sha256::digest(offline_cash_guard_bundle_pair_binding_digest_message_v1(
            guard_bundle,
        )?)
        .into(),
    )
}

/// Return the source-authoritative SHA-256 message for the GuardBundle join.
///
/// # Errors
///
/// Returns an error unless `guard_bundle` is the canonical GuardBundle topology.
pub fn offline_cash_guard_bundle_pair_binding_digest_message_v1(
    guard_bundle: &OfflineCashRecursivePairBindingV1,
) -> Result<Vec<u8>, KagemushaValidationError> {
    let bytes = guard_bundle.guard_bundle_canonical_bytes68()?;
    let mut message = Vec::with_capacity(
        GUARD_BUNDLE_PAIR_BINDING_DIGEST_DOMAIN.len()
            + 1
            + core::mem::size_of::<u64>()
            + bytes.len(),
    );
    message.extend_from_slice(GUARD_BUNDLE_PAIR_BINDING_DIGEST_DOMAIN);
    message.push(0);
    message.extend_from_slice(
        &u64::try_from(bytes.len())
            .expect("fixed GuardBundle binding length fits u64")
            .to_le_bytes(),
    );
    message.extend_from_slice(&bytes);
    Ok(message)
}

fn recursive_pair_binding_error() -> KagemushaValidationError {
    KagemushaValidationError::InvalidRecursiveSpendProof {
        field: "offline_cash.proof.recursive_pair_binding",
    }
}

fn parse_recursive_pair_topology(
    value: u32,
) -> Result<OfflineCashRecursivePairTopologyV1, KagemushaValidationError> {
    match value {
        value if value == OfflineCashRecursivePairTopologyV1::State as u32 => {
            Ok(OfflineCashRecursivePairTopologyV1::State)
        }
        value if value == OfflineCashRecursivePairTopologyV1::GuardBundle as u32 => {
            Ok(OfflineCashRecursivePairTopologyV1::GuardBundle)
        }
        _ => Err(recursive_pair_binding_error()),
    }
}

fn encode_recursive_pair_words(
    topology: OfflineCashRecursivePairTopologyV1,
    eq_audit_digest: [u8; 32],
    ep_audit_digest: [u8; 32],
    child_pair_binding_digest: [u8; 32],
) -> [u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1] {
    let (child_count, parent_role, child_roles, common_abi_words) =
        recursive_pair_topology_metadata(topology);
    let mut words = [0_u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1];
    words[RECURSIVE_PAIR_ABI_WORD_V1] = RECURSIVE_PAIR_BINDING_ABI_V1;
    words[RECURSIVE_PAIR_TOPOLOGY_WORD_V1] = topology as u32;
    words[RECURSIVE_PAIR_TRANSCRIPT_WORD_V1] = RECURSIVE_PAIR_TRANSCRIPT_V1;
    words[RECURSIVE_PAIR_POSEIDON_WIDTH_WORD_V1] = RECURSIVE_PAIR_POSEIDON_WIDTH_V1;
    words[RECURSIVE_PAIR_POSEIDON_RATE_WORD_V1] = RECURSIVE_PAIR_POSEIDON_RATE_V1;
    words[RECURSIVE_PAIR_POSEIDON_FULL_ROUNDS_WORD_V1] = RECURSIVE_PAIR_POSEIDON_FULL_ROUNDS_V1;
    words[RECURSIVE_PAIR_POSEIDON_PARTIAL_ROUNDS_WORD_V1] =
        RECURSIVE_PAIR_POSEIDON_PARTIAL_ROUNDS_V1;
    words[RECURSIVE_PAIR_POSEIDON_SECURE_MDS_WORD_V1] = RECURSIVE_PAIR_POSEIDON_SECURE_MDS_V1;
    words[RECURSIVE_PAIR_PARITY_COUNT_WORD_V1] = RECURSIVE_PAIR_PARITY_COUNT_V1;
    words[RECURSIVE_PAIR_CHILD_COUNT_WORD_V1] = child_count;
    words[RECURSIVE_PAIR_PARENT_ROLE_WORD_V1] = parent_role;
    words[RECURSIVE_PAIR_CHILD_ROLE_WORD_START_V1..RECURSIVE_PAIR_COMMON_ABI_WORDS_WORD_V1]
        .copy_from_slice(&child_roles);
    words[RECURSIVE_PAIR_COMMON_ABI_WORDS_WORD_V1] = common_abi_words;
    words[RECURSIVE_PAIR_DIGEST_WORDS_WORD_V1] = RECURSIVE_PAIR_DIGEST_WORDS_V1;
    write_recursive_pair_digest_words(
        &mut words,
        RECURSIVE_PAIR_EQ_AUDIT_WORD_START_V1,
        eq_audit_digest,
    );
    write_recursive_pair_digest_words(
        &mut words,
        RECURSIVE_PAIR_EP_AUDIT_WORD_START_V1,
        ep_audit_digest,
    );
    write_recursive_pair_digest_words(
        &mut words,
        RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1,
        child_pair_binding_digest,
    );
    words
}

const fn recursive_pair_topology_metadata(
    topology: OfflineCashRecursivePairTopologyV1,
) -> (u32, u32, [u32; 6], u32) {
    match topology {
        // State(1) <- StateLeaf(7), GuardBundle(5).
        OfflineCashRecursivePairTopologyV1::State => (2, 1, [7, 5, 0, 0, 0, 0], 229),
        // GuardBundle(5) <- GuardUse(2), PlatformBind(3), AndroidKeyCert(4),
        // GuardBundleLeaf(8), and the two role-specialized P256V3(6) children.
        OfflineCashRecursivePairTopologyV1::GuardBundle => (6, 5, [2, 3, 4, 8, 6, 6], 184),
    }
}

fn write_recursive_pair_digest_words(
    words: &mut [u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1],
    offset: usize,
    digest: [u8; 32],
) {
    for (target, chunk) in words[offset..offset + 8]
        .iter_mut()
        .zip(digest.chunks_exact(4))
    {
        *target = u32::from_le_bytes(chunk.try_into().expect("four-byte digest limb"));
    }
}

fn read_recursive_pair_digest_words(
    words: &[u32; OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1],
    offset: usize,
) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (target, word) in digest.chunks_exact_mut(4).zip(&words[offset..offset + 8]) {
        target.copy_from_slice(&word.to_le_bytes());
    }
    digest
}

/// Closed paired-Pasta proof with one shared reciprocal-recursion binding.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPairedProofV1 {
    /// Wire version.
    pub version: u16,
    /// Current Eq/Fp ordinary Poseidon IPA proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub eq_proof: Vec<u8>,
    /// Current Ep/Fq ordinary Poseidon IPA proof.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ep_proof: Vec<u8>,
    /// Eq/Fp parity-local lineage folded by the final Eq wrapper.
    pub eq_carried_lineage: OfflineCashIpaLineageV1,
    /// Ep/Fq parity-local lineage folded by the final Ep wrapper.
    pub ep_carried_lineage: OfflineCashIpaLineageV1,
    /// Shared canonical reciprocal deferred-equation binding.
    pub recursive_pair_binding: OfflineCashRecursivePairBindingV1,
}

/// Receiver-created request bound to its one current balance head.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPaymentRequestV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Requested asset.
    pub asset: AssetDefinitionId,
    /// Authoritative asset scale.
    pub scale: u32,
    /// Positive requested amount in atomic units.
    pub amount: u128,
    /// Recipient account identity.
    pub recipient: AccountId,
    /// Current receiver balance commitment that the credit must consume.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_balance_commitment: [u8; 32],
    /// Domain-separated reference to the request-signing and credit-encryption keys.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_key_reference: [u8; 32],
    /// Strict canonical X25519 public key for the receiver-only credit envelope.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_encryption_public_key: [u8; 32],
    /// Canonical uncompressed P-256 request-signing key.
    pub receiver_public_key: KagemushaDevicePublicKeyV2,
    /// Unique receiver nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_id: [u8; 32],
    /// Request creation time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Exclusive request expiry in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Authenticated hardware-policy registry root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub hardware_policy_id: [u8; 32],
    /// Low-S P-256 signature over the exact unsigned request.
    pub signature: KagemushaDeviceSignatureV2,
}

/// Sender response containing one receiver-bound credit proof.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashPaymentV1 {
    /// Wire version.
    pub version: u16,
    /// Sender-produced statement fields; request-owned fields are reconstructed.
    pub transfer: OfflineCashTransferResultV1,
    /// Paired ordinary proofs, carried lineages, and reciprocal audit binding.
    pub proof: OfflineCashPairedProofV1,
    /// Receiver-only encrypted credit opening.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub encrypted_credit: Vec<u8>,
}

/// Receiver acknowledgement emitted only after locally persisting `ReceiveFold`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineCashAcknowledgementV1 {
    /// Wire version.
    pub version: u16,
    /// Authenticated release identifier.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub release_id: [u8; 32],
    /// Digest of the accepted receiver request.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub request_digest: [u8; 32],
    /// Digest of the accepted sender response.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payment_digest: [u8; 32],
    /// Newly persisted receiver balance commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub receiver_balance_commitment: [u8; 32],
    /// Receiver persistence time in Unix milliseconds.
    pub acknowledged_at_ms: u64,
    /// Low-S P-256 signature over the acknowledgement fields.
    pub signature: KagemushaDeviceSignatureV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.payment-request-signing-preimage")]
struct PaymentRequestSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    recipient: AccountId,
    receiver_balance_commitment: [u8; 32],
    recipient_key_reference: [u8; 32],
    recipient_encryption_public_key: [u8; 32],
    receiver_public_key: KagemushaDevicePublicKeyV2,
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    hardware_policy_id: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
struct TransferTransitionPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    request_digest: [u8; 32],
    sender_before: [u8; 32],
    sender_after: [u8; 32],
    receiver_before: [u8; 32],
    credit_commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
struct PaymentDigestPreimageV1 {
    request_digest: [u8; 32],
    semantic_digest: [u8; 32],
    payment: OfflineCashPaymentV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
#[norito(schema_name = "iroha.offline-cash.v1.acknowledgement-signing-preimage")]
struct AcknowledgementSigningPreimageV1 {
    domain: Vec<u8>,
    version: u16,
    release_id: [u8; 32],
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    receiver_balance_commitment: [u8; 32],
    acknowledged_at_ms: u64,
}

/// Encode the exact canonical receiver-request bytes authorized by hardware.
///
/// This constructor is the single cross-crate signing contract. Keeping the
/// private Norito preimage here prevents a second Rust type name or field
/// layout from silently changing the canonical header and invalidating an
/// otherwise correct P-256 signature.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails.
#[allow(clippy::too_many_arguments)]
pub fn offline_cash_payment_request_signing_bytes_v1(
    version: u16,
    release_id: [u8; 32],
    network_id: &NetworkId,
    asset: &AssetDefinitionId,
    scale: u32,
    amount: u128,
    recipient: &AccountId,
    receiver_balance_commitment: [u8; 32],
    recipient_key_reference: [u8; 32],
    recipient_encryption_public_key: [u8; 32],
    receiver_public_key: KagemushaDevicePublicKeyV2,
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    hardware_policy_id: [u8; 32],
) -> Result<Vec<u8>, KagemushaValidationError> {
    Ok(norito::encode_canonical(
        &PaymentRequestSigningPreimageV1 {
            domain: REQUEST_SIGNING_DOMAIN.to_vec(),
            version,
            release_id,
            network_id: *network_id,
            asset: asset.clone(),
            scale,
            amount,
            recipient: recipient.clone(),
            receiver_balance_commitment,
            recipient_key_reference,
            recipient_encryption_public_key,
            receiver_public_key,
            request_id,
            issued_at_ms,
            expires_at_ms,
            hardware_policy_id,
        },
    )?)
}

/// Encode the exact canonical post-persistence acknowledgement bytes.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails.
pub fn offline_cash_acknowledgement_signing_bytes_v1(
    version: u16,
    release_id: [u8; 32],
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    receiver_balance_commitment: [u8; 32],
    acknowledged_at_ms: u64,
) -> Result<Vec<u8>, KagemushaValidationError> {
    Ok(norito::encode_canonical(
        &AcknowledgementSigningPreimageV1 {
            domain: ACKNOWLEDGEMENT_SIGNING_DOMAIN.to_vec(),
            version,
            release_id,
            request_digest,
            payment_digest,
            receiver_balance_commitment,
            acknowledged_at_ms,
        },
    )?)
}

fn digest_encoded<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], KagemushaValidationError> {
    let message = canonical_digest_message(domain, value)?;
    Ok(Sha256::digest(message).into())
}

fn canonical_digest_message<T: Encode>(
    domain: &[u8],
    value: &T,
) -> Result<Vec<u8>, KagemushaValidationError> {
    let bytes = norito::encode_canonical(value)?;
    let mut message = Vec::with_capacity(
        domain
            .len()
            .saturating_add(1)
            .saturating_add(core::mem::size_of::<u64>())
            .saturating_add(bytes.len()),
    );
    message.extend_from_slice(domain);
    message.push(0);
    message.extend_from_slice(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    message.extend_from_slice(&bytes);
    Ok(message)
}

fn require_nonzero(field: &'static str, value: [u8; 32]) -> Result<(), KagemushaValidationError> {
    if value == [0; 32] {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field });
    }
    Ok(())
}

fn require_encoded_size<T: Encode>(
    value: &T,
    max: usize,
) -> Result<usize, KagemushaValidationError> {
    let actual = norito::encode_canonical(value)?.len();
    if actual > max {
        return Err(KagemushaValidationError::EncodedSizeExceeded { actual, max });
    }
    Ok(actual)
}

/// Decode one already byte-capped canonical frame under resource limits that
/// are installed before derive-generated sequence decoders can reserve space.
///
/// This is intentionally narrower than generic [`Decode`]: callers handling
/// untrusted Offline Cash wire bytes must route through the public typed
/// entrypoints below so the outer cap is checked before the header or any
/// declared collection length is interpreted.
fn decode_bounded_canonical<T>(bytes: &[u8], max: usize) -> Result<T, KagemushaValidationError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.len() > max {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: bytes.len(),
            max,
        });
    }
    let limits = norito::canonical_decode_limits(bytes.len());
    Ok(norito::decode_canonical_with_limits(bytes, limits)?)
}

fn is_canonical_x25519_public_key_v1(public_key: [u8; 32]) -> bool {
    for index in (0..public_key.len()).rev() {
        if public_key[index] < X25519_FIELD_MODULUS_LITTLE_ENDIAN[index] {
            return true;
        }
        if public_key[index] > X25519_FIELD_MODULUS_LITTLE_ENDIAN[index] {
            return false;
        }
    }
    false
}

/// Validate the sole strict X25519 encoding admitted for an Offline Cash V1 recipient.
///
/// # Errors
///
/// Returns an error for a non-canonical field encoding or a low-order public key.
pub fn validate_offline_cash_recipient_encryption_public_key_v1(
    public_key: [u8; 32],
) -> Result<(), KagemushaValidationError> {
    if !is_canonical_x25519_public_key_v1(public_key)
        || X25519Sha256::decode_public_key(&public_key).is_err()
    {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "offline_cash.request.recipient_encryption_public_key",
        });
    }
    Ok(())
}

/// Derive the stable receiver-key reference carried by a payment request.
///
/// The reference binds the independently scoped P-256 request-signing key and
/// X25519 credit-encryption key. The signed request is therefore the sole
/// authority joining the two device identities.
#[must_use]
pub fn offline_cash_receiver_key_reference_v1(
    request_signing_public_key: &KagemushaDevicePublicKeyV2,
    recipient_encryption_public_key: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PUBLIC_KEY_REFERENCE_DOMAIN);
    hasher.update([0]);
    hasher.update(
        u64::try_from(request_signing_public_key.as_sec1_bytes().len())
            .expect("P-256 public-key width fits u64")
            .to_le_bytes(),
    );
    hasher.update(request_signing_public_key.as_sec1_bytes());
    hasher.update(
        u64::try_from(recipient_encryption_public_key.len())
            .expect("X25519 public-key width fits u64")
            .to_le_bytes(),
    );
    hasher.update(recipient_encryption_public_key);
    hasher.finalize().into()
}

impl OfflineCashPaymentRequestV1 {
    /// Return the exact bytes signed by the receiver device.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        offline_cash_payment_request_signing_bytes_v1(
            self.version,
            self.release_id,
            &self.network_id,
            &self.asset,
            self.scale,
            self.amount,
            &self.recipient,
            self.receiver_balance_commitment,
            self.recipient_key_reference,
            self.recipient_encryption_public_key,
            self.receiver_public_key,
            self.request_id,
            self.issued_at_ms,
            self.expires_at_ms,
            self.hardware_policy_id,
        )
    }

    /// Decode, canonicalize, and validate one exact bounded receiver request.
    ///
    /// The outer byte cap is enforced before Norito reads a header or declared
    /// sequence length. Decoding then runs under payload-derived sequence and
    /// cumulative allocation limits and rejects any non-canonical byte form.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid request.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, KagemushaValidationError> {
        let request: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        request.validate()?;
        Ok(request)
    }

    /// Validate context, bounds, key binding, signature, and canonical size.
    ///
    /// # Errors
    ///
    /// Returns an error when any first-release request invariant fails.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.version",
            });
        }
        require_nonzero("offline_cash.request.release_id", self.release_id)?;
        require_nonzero(
            "offline_cash.request.receiver_balance_commitment",
            self.receiver_balance_commitment,
        )?;
        require_nonzero("offline_cash.request.request_id", self.request_id)?;
        require_nonzero(
            "offline_cash.request.hardware_policy_id",
            self.hardware_policy_id,
        )?;
        if !is_kagemusha_network_id(&self.network_id) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.network_id",
            });
        }
        if self.amount == 0 || self.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.amount",
            });
        }
        let ttl = self.expires_at_ms.checked_sub(self.issued_at_ms).ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.expires_at_ms",
            },
        )?;
        if ttl == 0 || ttl > KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.expires_at_ms",
            });
        }
        self.receiver_public_key.validate()?;
        validate_offline_cash_recipient_encryption_public_key_v1(
            self.recipient_encryption_public_key,
        )?;
        if self.recipient_key_reference
            != offline_cash_receiver_key_reference_v1(
                &self.receiver_public_key,
                self.recipient_encryption_public_key,
            )
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.recipient_key_reference",
            });
        }
        self.signature
            .verify(&self.receiver_public_key, &self.canonical_signing_bytes()?)?;
        require_encoded_size(self, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Return the canonical request identity consumed by `SendSplit`.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        digest_encoded(REQUEST_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashTransferStatementV1 {
    fn validate_without_transition(&self) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || !is_kagemusha_network_id(&self.network_id)
            || self.amount == 0
            || self.scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.statement.header",
            });
        }
        for (field, value) in [
            ("offline_cash.statement.release_id", self.release_id),
            ("offline_cash.statement.request_digest", self.request_digest),
            ("offline_cash.statement.sender_before", self.sender_before),
            ("offline_cash.statement.sender_after", self.sender_after),
            (
                "offline_cash.statement.receiver_before",
                self.receiver_before,
            ),
            (
                "offline_cash.statement.credit_commitment",
                self.credit_commitment,
            ),
        ] {
            require_nonzero(field, value)?;
        }
        let commitments = [
            self.sender_before,
            self.sender_after,
            self.receiver_before,
            self.credit_commitment,
        ];
        for left in 0..commitments.len() {
            for right in left + 1..commitments.len() {
                if commitments[left] == commitments[right] {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "offline_cash.statement.commitments",
                    });
                }
            }
        }
        Ok(())
    }

    fn transition_preimage(&self) -> TransferTransitionPreimageV1 {
        TransferTransitionPreimageV1 {
            domain: TRANSITION_DIGEST_DOMAIN.to_vec(),
            version: self.version,
            release_id: self.release_id,
            network_id: self.network_id,
            asset: self.asset.clone(),
            scale: self.scale,
            amount: self.amount,
            request_digest: self.request_digest,
            sender_before: self.sender_before,
            sender_after: self.sender_after,
            receiver_before: self.receiver_before,
            credit_commitment: self.credit_commitment,
        }
    }

    /// Compute the sender-hardware transition digest from all other fields.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_transition_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        digest_encoded(TRANSITION_DIGEST_DOMAIN, &self.transition_preimage())
    }

    /// Return the exact bytes hashed for the sender-hardware transition digest.
    ///
    /// This is the source-authoritative canonical Norito/SHA-256 bridge used by
    /// the private STATE witness. It does not define another wire encoding.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical Norito encoding fails.
    pub fn canonical_transition_digest_message(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        canonical_digest_message(TRANSITION_DIGEST_DOMAIN, &self.transition_preimage())
    }

    /// Populate the canonical transition digest.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn seal_transition(mut self) -> Result<Self, KagemushaValidationError> {
        self.validate_without_transition()?;
        let transition_digest = self.expected_transition_digest()?;
        require_nonzero(
            "offline_cash.statement.transition_digest",
            transition_digest,
        )?;
        self.transition_digest = transition_digest;
        Ok(self)
    }

    /// Validate the exact public send-split binding.
    ///
    /// # Errors
    ///
    /// Returns an error when context, amount, commitment, or transition binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_without_transition()?;
        require_nonzero(
            "offline_cash.statement.transition_digest",
            self.transition_digest,
        )?;
        if self.transition_digest != self.expected_transition_digest()? {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.statement.transition_digest",
            });
        }
        Ok(())
    }

    /// Return the common semantic digest constrained by both Pasta parities.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        digest_encoded(STATEMENT_DIGEST_DOMAIN, self)
    }

    /// Return the exact bytes hashed for the common semantic digest.
    ///
    /// Validation is intentionally identical to [`Self::canonical_digest`], so
    /// a circuit witness cannot obtain bytes for a malformed statement through
    /// this typed entrypoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement is invalid or cannot be encoded.
    pub fn canonical_semantic_digest_message(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate()?;
        canonical_digest_message(STATEMENT_DIGEST_DOMAIN, self)
    }
}

impl OfflineCashTransferResultV1 {
    /// Extract the compact payment carrier from one full statement and prove
    /// that the signed request reconstructs that statement exactly.
    ///
    /// # Errors
    ///
    /// Returns an error when either value is invalid or any request-owned
    /// statement field differs from the signed request.
    pub fn from_statement_against(
        statement: &OfflineCashTransferStatementV1,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationError> {
        statement.validate()?;
        let result = Self {
            sender_before: statement.sender_before,
            sender_after: statement.sender_after,
            credit_commitment: statement.credit_commitment,
        };
        if result.reconstruct_statement(request)? != *statement {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.transfer.request_binding",
            });
        }
        Ok(result)
    }

    /// Reconstruct the exact proof-bound statement from this compact carrier
    /// and the signed receiver request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request, commitments, or canonical transition
    /// digest is invalid.
    pub fn reconstruct_statement(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<OfflineCashTransferStatementV1, KagemushaValidationError> {
        request.validate()?;
        let request_digest = digest_encoded(REQUEST_DIGEST_DOMAIN, request)?;
        OfflineCashTransferStatementV1 {
            version: request.version,
            release_id: request.release_id,
            network_id: request.network_id,
            asset: request.asset.clone(),
            scale: request.scale,
            amount: request.amount,
            request_digest,
            sender_before: self.sender_before,
            sender_after: self.sender_after,
            receiver_before: request.receiver_balance_commitment,
            credit_commitment: self.credit_commitment,
            transition_digest: [0; 32],
        }
        .seal_transition()
    }
}

impl OfflineCashPairedProofV1 {
    /// Validate fixed parity roles, ordinary-proof caps, and recursive binding.
    ///
    /// # Errors
    ///
    /// Returns an error when the paired proof is empty, oversized, aliased, or mis-bound.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.version",
            });
        }
        if self.eq_proof.is_empty()
            || self.ep_proof.is_empty()
            || self.eq_proof == self.ep_proof
            || self.eq_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.ep_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || self.eq_proof.len() + self.ep_proof.len() > OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.current",
            });
        }
        if self.eq_carried_lineage == self.ep_carried_lineage {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.parity_lineage_alias",
            });
        }
        self.eq_carried_lineage.validate()?;
        self.ep_carried_lineage.validate()?;
        self.recursive_pair_binding.validate()?;
        if self.recursive_pair_binding.topology()? != OfflineCashRecursivePairTopologyV1::State {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.recursive_pair_binding",
            });
        }
        Ok(())
    }
}

impl OfflineCashPaymentV1 {
    fn validated_statement_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<OfflineCashTransferStatementV1, KagemushaValidationError> {
        let statement = self.reconstruct_statement(request)?;
        self.proof.validate()?;
        if self.encrypted_credit.is_empty()
            || self.encrypted_credit.len() > OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.payment.encrypted_credit",
            });
        }
        require_encoded_size(self, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
        Ok(statement)
    }

    /// Reconstruct the exact statement constrained by both proofs.
    ///
    /// # Errors
    ///
    /// Returns an error when the signed request or compact transfer carrier is
    /// invalid or their canonical transition binding does not match.
    pub fn reconstruct_statement(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<OfflineCashTransferStatementV1, KagemushaValidationError> {
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1 || request.version != self.version {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.payment.version",
            });
        }
        self.transfer.reconstruct_statement(request)
    }

    /// Decode, canonicalize, and validate one exact bounded sender response.
    ///
    /// The outer byte cap is enforced before Norito reads a header or declared
    /// sequence length. Decoding then runs under payload-derived sequence and
    /// cumulative allocation limits and reconstructs the exact request-owned
    /// proof statement. Semantic binding is completed when Core verifies the
    /// paired proofs against that reconstructed statement.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid response.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<Self, KagemushaValidationError> {
        let payment: Self = decode_bounded_canonical(bytes, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
        payment.validate_against(request)?;
        Ok(payment)
    }

    /// Structurally validate this response in one signed receiver-request context.
    ///
    /// # Errors
    ///
    /// Returns an error when the request, compact carrier, proof shape, or size is invalid.
    pub fn validate_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<(), KagemushaValidationError> {
        self.validated_statement_against(request)?;
        Ok(())
    }

    /// Return the canonical response digest after validating its receiver request.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is invalid or cannot be encoded.
    pub fn canonical_digest_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<[u8; 32], KagemushaValidationError> {
        let statement = self.validated_statement_against(request)?;
        let request_digest = statement.request_digest;
        let semantic_digest = digest_encoded(STATEMENT_DIGEST_DOMAIN, &statement)?;
        digest_encoded(
            PAYMENT_DIGEST_DOMAIN,
            &PaymentDigestPreimageV1 {
                request_digest,
                semantic_digest,
                payment: self.clone(),
            },
        )
    }
}

impl OfflineCashAcknowledgementV1 {
    /// Decode, canonicalize, and validate one exact bounded acknowledgement.
    ///
    /// The outer byte cap is enforced before Norito reads a header or declared
    /// sequence length. Decoding then runs under payload-derived sequence and
    /// cumulative allocation limits and verifies the request/response binding.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, malformed, non-canonical, or invalid acknowledgement.
    pub fn decode_canonical_exact_against(
        bytes: &[u8],
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<Self, KagemushaValidationError> {
        let acknowledgement: Self =
            decode_bounded_canonical(bytes, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        acknowledgement.validate_against(request, payment)?;
        Ok(acknowledgement)
    }

    /// Return the exact bytes signed after persisting the receiver balance.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        offline_cash_acknowledgement_signing_bytes_v1(
            self.version,
            self.release_id,
            self.request_digest,
            self.payment_digest,
            self.receiver_balance_commitment,
            self.acknowledged_at_ms,
        )
    }

    /// Validate this acknowledgement against its request and response.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, time, persistence-head, signature, or size binding fails.
    pub fn validate_against(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<(), KagemushaValidationError> {
        payment.validate_against(request)?;
        let request_digest = request.canonical_digest()?;
        let payment_digest = payment.canonical_digest_against(request)?;
        if self.version != OFFLINE_CASH_WIRE_VERSION_V1
            || self.release_id != request.release_id
            || self.request_digest != request_digest
            || self.payment_digest != payment_digest
            || self.receiver_balance_commitment == [0; 32]
            || self.receiver_balance_commitment == request.receiver_balance_commitment
            || self.receiver_balance_commitment == payment.transfer.credit_commitment
            || self.acknowledged_at_ms < request.issued_at_ms
            || self.acknowledged_at_ms >= request.expires_at_ms
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.acknowledgement.binding",
            });
        }
        self.signature.verify(
            &request.receiver_public_key,
            &self.canonical_signing_bytes()?,
        )?;
        require_encoded_size(self, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
        Ok(())
    }
}

/// State of one non-authorizing Offline Cash V1 verification transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashVerificationTranscriptStateV1 {
    /// The signed receiver request passed structural validation.
    ReceiveRequestValidated,
    /// The sender response passed structural validation.
    PaymentValidated,
    /// The post-persistence receiver acknowledgement passed structural validation.
    AcknowledgementValidated,
}

impl OfflineCashVerificationTranscriptStateV1 {
    /// Deprecated compatibility spelling for [`Self::ReceiveRequestValidated`].
    #[allow(non_upper_case_globals)]
    #[deprecated(
        note = "use OfflineCashVerificationTranscriptStateV1::ReceiveRequestValidated; this data-model value does not own a wallet runtime"
    )]
    pub const ReceiveRequestReady: Self = Self::ReceiveRequestValidated;

    /// Deprecated compatibility spelling for [`Self::PaymentValidated`].
    #[allow(non_upper_case_globals)]
    #[deprecated(
        note = "use OfflineCashVerificationTranscriptStateV1::PaymentValidated; structural validation is not a wallet commit"
    )]
    pub const PaymentCommitted: Self = Self::PaymentValidated;

    /// Deprecated compatibility spelling for [`Self::AcknowledgementValidated`].
    #[allow(non_upper_case_globals)]
    #[deprecated(
        note = "use OfflineCashVerificationTranscriptStateV1::AcknowledgementValidated; validation is not lifecycle acknowledgement authority"
    )]
    pub const Acknowledged: Self = Self::AcknowledgementValidated;
}

/// Result of structurally validating a peer message into a verification transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashVerificationTranscriptEventV1 {
    /// A previously unseen payment passed structural validation.
    PaymentValidated,
    /// The exact already-validated payment was replayed idempotently.
    PaymentValidationReplay,
    /// A previously unseen acknowledgement passed structural validation.
    AcknowledgementValidated,
    /// The exact already-validated acknowledgement was replayed idempotently.
    AcknowledgementValidationReplay,
}

impl OfflineCashVerificationTranscriptEventV1 {
    /// Deprecated compatibility spelling for [`Self::PaymentValidated`].
    #[allow(non_upper_case_globals)]
    #[deprecated(
        note = "use OfflineCashVerificationTranscriptEventV1::PaymentValidated; structural validation is not a wallet commit"
    )]
    pub const PaymentCommitted: Self = Self::PaymentValidated;

    /// Deprecated compatibility spelling for [`Self::PaymentValidationReplay`].
    #[allow(non_upper_case_globals)]
    #[deprecated(note = "use OfflineCashVerificationTranscriptEventV1::PaymentValidationReplay")]
    pub const PaymentReplay: Self = Self::PaymentValidationReplay;

    /// Deprecated compatibility spelling for [`Self::AcknowledgementValidated`].
    #[allow(non_upper_case_globals)]
    #[deprecated(
        note = "use OfflineCashVerificationTranscriptEventV1::AcknowledgementValidated; validation is not lifecycle acknowledgement authority"
    )]
    pub const Acknowledged: Self = Self::AcknowledgementValidated;

    /// Deprecated compatibility spelling for [`Self::AcknowledgementValidationReplay`].
    #[allow(non_upper_case_globals)]
    #[deprecated(
        note = "use OfflineCashVerificationTranscriptEventV1::AcknowledgementValidationReplay"
    )]
    pub const AcknowledgementReplay: Self = Self::AcknowledgementValidationReplay;
}

/// Structural, non-authorizing transcript for one Offline Cash V1 handoff.
///
/// The transcript owns typed canonical values and records them only after wire,
/// signature, request-binding, and aggregate-size validation succeeds. It does
/// not authenticate a release registry, authorize opaque proof bytes, own a
/// secure-device session, mutate a balance, or publish an outbox record;
/// production callers must first pass the payment through Core's terminal
/// verifier, directly or through the authenticated native facade, bound to an
/// [`super::OfflineCashAuthenticatedReleaseV1`].
#[derive(Debug)]
pub struct OfflineCashVerificationTranscriptV1 {
    request: OfflineCashPaymentRequestV1,
    expected_release_id: [u8; 32],
    expected_artifact_manifest_sha256: [u8; 32],
    validated_payment: Option<OfflineCashPaymentV1>,
    validated_acknowledgement: Option<OfflineCashAcknowledgementV1>,
}

impl OfflineCashVerificationTranscriptV1 {
    /// Create a structural transcript after validating the exact
    /// receiver request and caller-supplied release identities.
    ///
    /// These digest arguments are bookkeeping, not release authentication.
    /// Production callers derive both from an authenticated release capability;
    /// the native bridge remains fail-closed until its governed registry exists.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or oversized.
    pub fn new(
        request: OfflineCashPaymentRequestV1,
        expected_release_id: [u8; 32],
        expected_artifact_manifest_sha256: [u8; 32],
    ) -> Result<Self, KagemushaValidationError> {
        request.validate()?;
        if expected_release_id == [0; 32]
            || expected_artifact_manifest_sha256 == [0; 32]
            || request.release_id != expected_release_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.verification_transcript.release_binding",
            });
        }
        Ok(Self {
            request,
            expected_release_id,
            expected_artifact_manifest_sha256,
            validated_payment: None,
            validated_acknowledgement: None,
        })
    }

    /// Return the current monotonic transcript state.
    #[must_use]
    pub const fn state(&self) -> OfflineCashVerificationTranscriptStateV1 {
        if self.validated_acknowledgement.is_some() {
            OfflineCashVerificationTranscriptStateV1::AcknowledgementValidated
        } else if self.validated_payment.is_some() {
            OfflineCashVerificationTranscriptStateV1::PaymentValidated
        } else {
            OfflineCashVerificationTranscriptStateV1::ReceiveRequestValidated
        }
    }

    /// Return the exact signed receiver request.
    #[must_use]
    pub const fn request(&self) -> &OfflineCashPaymentRequestV1 {
        &self.request
    }

    /// Return the release identifier pinned by the signed runtime manifest.
    #[must_use]
    pub const fn expected_release_id(&self) -> [u8; 32] {
        self.expected_release_id
    }

    /// Return the caller-declared artifact-manifest digest used for bookkeeping.
    ///
    /// This accessor is not evidence that the manifest was installed or authenticated.
    #[must_use]
    pub const fn expected_artifact_manifest_sha256(&self) -> [u8; 32] {
        self.expected_artifact_manifest_sha256
    }

    /// Return the structurally validated payment, when present.
    #[must_use]
    pub const fn validated_payment(&self) -> Option<&OfflineCashPaymentV1> {
        self.validated_payment.as_ref()
    }

    /// Return the structurally validated acknowledgement, when present.
    #[must_use]
    pub const fn validated_acknowledgement(&self) -> Option<&OfflineCashAcknowledgementV1> {
        self.validated_acknowledgement.as_ref()
    }

    /// Structurally validate and record a sender payment.
    ///
    /// This method does not authorize opaque proofs or authenticate a release;
    /// callers must first obtain a successful Core/native terminal-verifier decision.
    ///
    /// Exact replay is idempotent. A different payment or any payment applied
    /// after acknowledgement is rejected without changing the transcript.
    ///
    /// # Errors
    ///
    /// Returns an error when the payment is invalid, mismatched, or conflicts
    /// with already-validated transcript state.
    pub fn validate_payment(
        &mut self,
        payment: OfflineCashPaymentV1,
    ) -> Result<OfflineCashVerificationTranscriptEventV1, KagemushaValidationError> {
        payment.validate_against(&self.request)?;
        if self.validated_acknowledgement.is_some() {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.verification_transcript.payment_after_acknowledgement",
            });
        }
        if let Some(existing) = self.validated_payment.as_ref() {
            return if existing == &payment {
                Ok(OfflineCashVerificationTranscriptEventV1::PaymentValidationReplay)
            } else {
                Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "offline_cash.verification_transcript.conflicting_payment",
                })
            };
        }
        self.validated_payment = Some(payment);
        Ok(OfflineCashVerificationTranscriptEventV1::PaymentValidated)
    }

    /// Structurally validate and record a receiver acknowledgement.
    ///
    /// Exact replay is idempotent. A different acknowledgement is rejected
    /// without changing the transcript.
    ///
    /// # Errors
    ///
    /// Returns an error when no payment was validated or when the
    /// acknowledgement is invalid, mismatched, oversized, or conflicting.
    pub fn validate_acknowledgement(
        &mut self,
        acknowledgement: OfflineCashAcknowledgementV1,
    ) -> Result<OfflineCashVerificationTranscriptEventV1, KagemushaValidationError> {
        let payment = self.validated_payment.as_ref().ok_or(
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.verification_transcript.acknowledgement_before_payment",
            },
        )?;
        acknowledgement.validate_against(&self.request, payment)?;
        validate_offline_cash_session_v1(&self.request, payment, &acknowledgement)?;
        if let Some(existing) = self.validated_acknowledgement.as_ref() {
            return if existing == &acknowledgement {
                Ok(OfflineCashVerificationTranscriptEventV1::AcknowledgementValidationReplay)
            } else {
                Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "offline_cash.verification_transcript.conflicting_acknowledgement",
                })
            };
        }
        self.validated_acknowledgement = Some(acknowledgement);
        Ok(OfflineCashVerificationTranscriptEventV1::AcknowledgementValidated)
    }

    /// Deprecated compatibility accessor for [`Self::validated_payment`].
    #[must_use]
    #[deprecated(
        note = "use validated_payment; this transcript does not own committed wallet state"
    )]
    pub const fn payment(&self) -> Option<&OfflineCashPaymentV1> {
        self.validated_payment()
    }

    /// Deprecated compatibility accessor for [`Self::validated_acknowledgement`].
    #[must_use]
    #[deprecated(note = "use validated_acknowledgement")]
    pub const fn acknowledgement(&self) -> Option<&OfflineCashAcknowledgementV1> {
        self.validated_acknowledgement()
    }

    /// Deprecated compatibility method for [`Self::validate_payment`].
    #[deprecated(
        note = "use validate_payment; this transcript does not commit secure-device or wallet state"
    )]
    pub fn accept_payment(
        &mut self,
        payment: OfflineCashPaymentV1,
    ) -> Result<OfflineCashVerificationTranscriptEventV1, KagemushaValidationError> {
        self.validate_payment(payment)
    }

    /// Deprecated compatibility method for [`Self::validate_acknowledgement`].
    #[deprecated(note = "use validate_acknowledgement")]
    pub fn accept_acknowledgement(
        &mut self,
        acknowledgement: OfflineCashAcknowledgementV1,
    ) -> Result<OfflineCashVerificationTranscriptEventV1, KagemushaValidationError> {
        self.validate_acknowledgement(acknowledgement)
    }
}

/// Deprecated compatibility alias for the non-authorizing transcript state.
#[deprecated(
    note = "use OfflineCashVerificationTranscriptStateV1; the data-model type is not a wallet session"
)]
pub type OfflineCashWalletSessionStateV1 = OfflineCashVerificationTranscriptStateV1;

/// Deprecated compatibility alias for the non-authorizing transcript event.
#[deprecated(
    note = "use OfflineCashVerificationTranscriptEventV1; the data-model type does not authorize wallet effects"
)]
pub type OfflineCashWalletSessionEventV1 = OfflineCashVerificationTranscriptEventV1;

/// Deprecated compatibility alias for the non-authorizing verification transcript.
#[deprecated(
    note = "use OfflineCashVerificationTranscriptV1; Core owns the opaque wallet-runtime facade"
)]
pub type OfflineCashWalletSessionV1 = OfflineCashVerificationTranscriptV1;

fn offline_cash_text_max_for_raw(raw_max: usize) -> usize {
    OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(raw_max)
}

fn encode_offline_cash_text_v1<T: Encode>(
    value: &T,
    raw_max: usize,
) -> Result<String, KagemushaValidationError> {
    let bytes = norito::encode_canonical(value)?;
    if bytes.len() > raw_max {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: bytes.len(),
            max: raw_max,
        });
    }
    Ok(format!(
        "{OFFLINE_CASH_TEXT_PREFIX_V1}{}",
        URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn decode_offline_cash_text_v1(
    text: &str,
    raw_max: usize,
    field: &'static str,
) -> Result<Vec<u8>, KagemushaValidationError> {
    let text_max = offline_cash_text_max_for_raw(raw_max);
    if text.len() > text_max {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: text.len(),
            max: text_max,
        });
    }
    let encoded = text
        .strip_prefix(OFFLINE_CASH_TEXT_PREFIX_V1)
        .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof { field })?;
    if encoded.is_empty()
        || encoded.contains('=')
        || !encoded.is_ascii()
        || encoded.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field });
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof { field })?;
    if bytes.len() > raw_max || URL_SAFE_NO_PAD.encode(&bytes) != encoded {
        return Err(KagemushaValidationError::InvalidRecursiveSpendProof { field });
    }
    Ok(bytes)
}

/// Canonical kgm2 text adapter for peer transports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OfflineCashPeerAdapterV1;

impl OfflineCashPeerAdapterV1 {
    /// Encode one validated receiver request as canonical unpadded kgm2 text.
    ///
    /// # Errors
    ///
    /// Returns an error when the request is invalid or oversized.
    pub fn encode_payment_request(
        &self,
        request: &OfflineCashPaymentRequestV1,
    ) -> Result<String, KagemushaValidationError> {
        request.validate()?;
        encode_offline_cash_text_v1(request, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)
    }

    /// Decode and validate one canonical unpadded kgm2 receiver request.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed, padded, non-canonical, oversized, or invalid request.
    pub fn decode_payment_request(
        &self,
        text: &str,
    ) -> Result<OfflineCashPaymentRequestV1, KagemushaValidationError> {
        let bytes = decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
            "offline_cash.peer.request_text",
        )?;
        OfflineCashPaymentRequestV1::decode_canonical_exact(&bytes)
    }

    /// Encode one validated sender payment as canonical unpadded kgm2 text.
    ///
    /// # Errors
    ///
    /// Returns an error when the payment does not bind the request or is oversized.
    pub fn encode_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> Result<String, KagemushaValidationError> {
        payment.validate_against(request)?;
        encode_offline_cash_text_v1(payment, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)
    }

    /// Decode and validate one canonical unpadded kgm2 sender payment.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed, padded, non-canonical, oversized,
    /// invalid, or request-mismatched payment.
    pub fn decode_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        text: &str,
    ) -> Result<OfflineCashPaymentV1, KagemushaValidationError> {
        let bytes = decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            "offline_cash.peer.payment_text",
        )?;
        OfflineCashPaymentV1::decode_canonical_exact_against(&bytes, request)
    }

    /// Encode one validated acknowledgement as canonical unpadded kgm2 text.
    ///
    /// # Errors
    ///
    /// Returns an error when the acknowledgement does not bind the request and
    /// payment or is oversized.
    pub fn encode_acknowledgement(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        acknowledgement: &OfflineCashAcknowledgementV1,
    ) -> Result<String, KagemushaValidationError> {
        acknowledgement.validate_against(request, payment)?;
        encode_offline_cash_text_v1(acknowledgement, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)
    }

    /// Decode and validate one canonical unpadded kgm2 acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed, padded, non-canonical, oversized,
    /// invalid, or session-mismatched acknowledgement.
    pub fn decode_acknowledgement(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        text: &str,
    ) -> Result<OfflineCashAcknowledgementV1, KagemushaValidationError> {
        let bytes = decode_offline_cash_text_v1(
            text,
            OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            "offline_cash.peer.acknowledgement_text",
        )?;
        OfflineCashAcknowledgementV1::decode_canonical_exact_against(&bytes, request, payment)
    }
}

fn unpadded_base64url_len(raw_len: usize) -> usize {
    raw_len / 3 * 4
        + match raw_len % 3 {
            0 => 0,
            1 => 2,
            _ => 3,
        }
}

fn validate_offline_cash_raw_session_size_v1(raw: usize) -> Result<(), KagemushaValidationError> {
    if raw > OFFLINE_CASH_SESSION_MAX_BYTES_V1 {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: raw,
            max: OFFLINE_CASH_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(())
}

/// Validate the complete request/response/acknowledgement session and return its raw size.
///
/// # Errors
///
/// Returns an error when a message is invalid or the aggregate raw/text envelope is oversized.
pub fn validate_offline_cash_session_v1(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    acknowledgement: &OfflineCashAcknowledgementV1,
) -> Result<usize, KagemushaValidationError> {
    acknowledgement.validate_against(request, payment)?;
    let lengths = [
        require_encoded_size(request, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?,
        require_encoded_size(payment, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?,
        require_encoded_size(acknowledgement, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?,
    ];
    let raw = lengths.iter().sum::<usize>();
    validate_offline_cash_raw_session_size_v1(raw)?;
    let text = lengths
        .iter()
        .map(|length| OFFLINE_CASH_TEXT_PREFIX_V1.len() + unpadded_base64url_len(*length))
        .sum::<usize>();
    if text > OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1 {
        return Err(KagemushaValidationError::EncodedSizeExceeded {
            actual: text,
            max: OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
        });
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::DomainId, offline::kagemusha_test_network_id};
    use iroha_crypto::{Algorithm, KeyPair};
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    const EQ_FOLDED_GENERATOR_V1: [u8; 32] = [
        0x00, 0x00, 0x00, 0x00, 0x21, 0xeb, 0x46, 0x8c, 0xdd, 0xa8, 0x94, 0x09, 0xfc, 0x98, 0x46,
        0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x40,
    ];
    const EP_FOLDED_GENERATOR_V1: [u8; 32] = [
        0x00, 0x00, 0x00, 0x00, 0xed, 0x30, 0x2d, 0x99, 0x1b, 0xf9, 0x4c, 0x09, 0xfc, 0x98, 0x46,
        0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x40,
    ];

    fn bare_norito_payload<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let mut bytes = Vec::new();
        let mut encoder = norito::core::Encoder::for_buffer(&mut bytes);
        value
            .serialize(&mut encoder)
            .expect("encode bare Norito payload");
        bytes
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn account() -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes((&[7_u8; 32]).into()).expect("P-256 signing key")
    }

    fn recipient_encryption_public_key() -> [u8; 32] {
        [
            0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
            0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
            0xaa, 0x9b, 0x4e, 0x6a,
        ]
    }

    fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV2 {
        let signature: Signature = key.sign(bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical signature")
    }

    fn guard_bundle_pair_binding() -> OfflineCashRecursivePairBindingV1 {
        OfflineCashRecursivePairBindingV1::new_guard_bundle([0xA1; 32], [0xB2; 32])
            .expect("canonical GuardBundle pair binding")
    }

    fn recursive_pair_binding() -> OfflineCashRecursivePairBindingV1 {
        OfflineCashRecursivePairBindingV1::new_state(
            [0xC3; 32],
            [0xD4; 32],
            &guard_bundle_pair_binding(),
        )
        .expect("canonical recursive pair binding")
    }

    fn ipa_lineage(challenge_start: u8, folded_generator: [u8; 32]) -> OfflineCashIpaLineageV1 {
        OfflineCashIpaLineageV1::new(
            std::array::from_fn(|index| {
                let mut encoded = [0_u8; 32];
                let challenge =
                    challenge_start + u8::try_from(index).expect("lineage index fits u8");
                encoded[0] = challenge;
                encoded
            }),
            folded_generator,
        )
        .expect("fixed-shape lineage")
    }

    #[test]
    fn ipa_lineage_has_exact_fixed_wire_and_36_cell_projection() {
        let lineage = ipa_lineage(1, EQ_FOLDED_GENERATOR_V1);
        assert!(lineage.validate().is_ok());
        let limbs = lineage.instance_limbs().expect("lineage instance limbs");
        assert_eq!(limbs.len(), OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1);
        assert_eq!(limbs[0], u128::from(OFFLINE_CASH_IPA_LINEAGE_VERSION_V1));
        assert_eq!(
            limbs[1],
            u128::from(OFFLINE_CASH_IPA_LINEAGE_ROUND_COUNT_V1)
        );
        assert_eq!(limbs[2], 1);
        assert_eq!(limbs[3], 0);
        assert_eq!(limbs[32], 16);
        assert_eq!(limbs[33], 0);
        assert_eq!(
            limbs[34],
            u128::from_le_bytes(
                EQ_FOLDED_GENERATOR_V1[..16]
                    .try_into()
                    .expect("first generator limb")
            )
        );
        assert_eq!(
            limbs[35],
            u128::from_le_bytes(
                EQ_FOLDED_GENERATOR_V1[16..]
                    .try_into()
                    .expect("second generator limb")
            )
        );
        assert_eq!(OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1, 544);
        assert_eq!(OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1, 550);
        let payload = bare_norito_payload(&lineage);
        assert_eq!(payload.len(), OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1);
        assert_eq!(&payload[..2], &lineage.version.to_le_bytes());
        assert_eq!(&payload[2..6], &lineage.round_count.to_le_bytes());
        assert_eq!(
            &payload[6..6 + OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1],
            &lineage.round_challenges
        );
        assert_eq!(
            &payload[6 + OFFLINE_CASH_IPA_LINEAGE_CHALLENGE_BYTES_V1..],
            &lineage.folded_generator
        );
        assert_eq!(
            norito::NoritoSerialize::encoded_len_exact(&lineage),
            Some(OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1)
        );
        let mut payload_with_trailer = payload.clone();
        payload_with_trailer.push(0xFF);
        let (decoded_prefix, used) =
            <OfflineCashIpaLineageV1 as norito::core::DecodeFromSlice>::decode_from_slice(
                &payload_with_trailer,
            )
            .expect("decode one fixed lineage from a containing payload");
        assert_eq!(decoded_prefix, lineage);
        assert_eq!(used, OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1);
        let framed = norito::encode_canonical(&lineage).expect("encode canonical lineage frame");
        assert_eq!(
            norito::decode_canonical::<OfflineCashIpaLineageV1>(&framed)
                .expect("decode canonical lineage frame"),
            lineage
        );
        let mut framed_with_trailer = framed;
        framed_with_trailer.push(0xFF);
        assert!(
            norito::decode_canonical::<OfflineCashIpaLineageV1>(&framed_with_trailer).is_err(),
            "the framed ingress must reject bytes after one exact lineage"
        );

        let mut truncated = payload.clone();
        truncated.pop();
        assert!(
            <OfflineCashIpaLineageV1 as norito::core::DecodeFromSlice>::decode_from_slice(
                &truncated
            )
            .is_err()
        );
        let mut noncanonical_payload = payload.clone();
        noncanonical_payload[0] ^= 1;
        assert!(
            <OfflineCashIpaLineageV1 as norito::core::DecodeFromSlice>::decode_from_slice(
                &noncanonical_payload
            )
            .is_err()
        );

        let mut invalid_version = lineage;
        invalid_version.version ^= 1;
        assert!(invalid_version.validate().is_err());
        let mut invalid_rounds = lineage;
        invalid_rounds.round_count -= 1;
        assert!(invalid_rounds.validate().is_err());
        let mut zero_point = lineage;
        zero_point.folded_generator = [0; 32];
        assert!(zero_point.validate().is_err());
    }

    #[test]
    fn recursive_pair_binding_is_compact_on_wire_and_expands_strictly() {
        let guard_bundle = guard_bundle_pair_binding();
        let binding = recursive_pair_binding();
        let words = binding.canonical_words().expect("canonical words");
        assert_eq!(words.len(), OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1);
        assert_eq!(words[RECURSIVE_PAIR_ABI_WORD_V1], 1);
        assert_eq!(words[RECURSIVE_PAIR_TOPOLOGY_WORD_V1], 1);
        assert_eq!(words[RECURSIVE_PAIR_TRANSCRIPT_WORD_V1], 1);
        assert_eq!(
            &words
                [RECURSIVE_PAIR_CHILD_ROLE_WORD_START_V1..RECURSIVE_PAIR_COMMON_ABI_WORDS_WORD_V1],
            &[7, 5, 0, 0, 0, 0]
        );
        assert_eq!(words[RECURSIVE_PAIR_COMMON_ABI_WORDS_WORD_V1], 229);
        assert_eq!(words[RECURSIVE_PAIR_DIGEST_WORDS_WORD_V1], 8);
        assert!(
            words[RECURSIVE_PAIR_DIGEST_WORDS_WORD_V1 + 1..RECURSIVE_PAIR_HEADER_WORDS_V1]
                .iter()
                .all(|word| *word == 0)
        );
        assert!(
            words[RECURSIVE_PAIR_RESERVED_WORD_START_V1..]
                .iter()
                .all(|word| *word == 0)
        );
        assert_eq!(
            OfflineCashRecursivePairBindingV1::from_canonical_words(words)
                .expect("strict State binding roundtrip"),
            binding
        );
        assert_eq!(binding.eq_audit_digest, [0xC3; 32]);
        assert_eq!(binding.ep_audit_digest, [0xD4; 32]);
        assert_ne!(binding.child_pair_binding_digest, [0; 32]);
        assert!(binding.validate_state_child_binding(&guard_bundle).is_ok());

        let guard_bundle_bytes = guard_bundle
            .guard_bundle_canonical_bytes68()
            .expect("canonical GuardBundle bytes");
        assert_eq!(guard_bundle_bytes.len(), 68);
        assert_eq!(&guard_bundle_bytes[..4], &2_u32.to_le_bytes());
        assert_eq!(&guard_bundle_bytes[4..36], &[0xA1; 32]);
        assert_eq!(&guard_bundle_bytes[36..], &[0xB2; 32]);
        assert_eq!(
            binding.child_pair_binding_digest,
            offline_cash_guard_bundle_pair_binding_digest_v1(&guard_bundle)
                .expect("GuardBundle pair digest")
        );
        assert_eq!(
            binding.child_pair_binding_digest,
            [
                0xf9, 0x0a, 0x58, 0xd1, 0xc4, 0xe9, 0xc6, 0x7b, 0x98, 0x04, 0x39, 0x0d, 0x53, 0x3a,
                0x0f, 0xe7, 0x47, 0xa4, 0x13, 0x79, 0x55, 0xad, 0xe3, 0x0d, 0x9d, 0x89, 0xf6, 0xe4,
                0x36, 0x73, 0x96, 0x80,
            ],
            "GuardBundle join digest framing must remain source-stable"
        );

        let encoded_binding = bare_norito_payload(&binding);
        let public_words_bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(
            encoded_binding.len(),
            OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1
        );
        assert_eq!(&encoded_binding[..4], &binding.topology.to_le_bytes());
        assert_eq!(&encoded_binding[4..36], &binding.eq_audit_digest);
        assert_eq!(&encoded_binding[36..68], &binding.ep_audit_digest);
        assert_eq!(
            &encoded_binding[68..100],
            &binding.child_pair_binding_digest
        );
        assert_eq!(
            norito::NoritoSerialize::encoded_len_exact(&binding),
            Some(OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1)
        );
        let mut binding_with_trailer = encoded_binding.clone();
        binding_with_trailer.push(0xFF);
        let (decoded_prefix, used) =
            <OfflineCashRecursivePairBindingV1 as norito::core::DecodeFromSlice>::decode_from_slice(
                &binding_with_trailer,
            )
            .expect("decode one compact binding from a containing payload");
        assert_eq!(decoded_prefix, binding);
        assert_eq!(used, OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1);
        let framed =
            norito::encode_canonical(&binding).expect("encode canonical recursive-pair frame");
        assert_eq!(
            norito::decode_canonical::<OfflineCashRecursivePairBindingV1>(&framed)
                .expect("decode canonical recursive-pair frame"),
            binding
        );
        let mut framed_with_trailer = framed;
        framed_with_trailer.push(0xFF);
        assert!(
            norito::decode_canonical::<OfflineCashRecursivePairBindingV1>(&framed_with_trailer)
                .is_err(),
            "the framed ingress must reject bytes after one exact binding"
        );
        let mut wrong_topology_payload = encoded_binding.clone();
        wrong_topology_payload[..4].copy_from_slice(&3_u32.to_le_bytes());
        assert!(
            <OfflineCashRecursivePairBindingV1 as norito::core::DecodeFromSlice>::decode_from_slice(
                &wrong_topology_payload
            )
            .is_err()
        );
        let mut missing_child_join_payload = encoded_binding.clone();
        missing_child_join_payload[68..100].fill(0);
        assert!(
            <OfflineCashRecursivePairBindingV1 as norito::core::DecodeFromSlice>::decode_from_slice(
                &missing_child_join_payload
            )
            .is_err()
        );
        assert_eq!(
            public_words_bytes.len(),
            OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1
        );

        for index in 0..RECURSIVE_PAIR_DIGEST_WORDS_WORD_V1 + 1 {
            let mut mutated = words;
            mutated[index] ^= 1;
            assert!(
                OfflineCashRecursivePairBindingV1::from_canonical_words(mutated).is_err(),
                "fixed header word {index} must be canonical"
            );
        }
        for index in (RECURSIVE_PAIR_DIGEST_WORDS_WORD_V1 + 1)..RECURSIVE_PAIR_HEADER_WORDS_V1 {
            let mut mutated = words;
            mutated[index] = 1;
            assert!(
                OfflineCashRecursivePairBindingV1::from_canonical_words(mutated).is_err(),
                "reserved header word {index} must be zero"
            );
        }
        for index in
            RECURSIVE_PAIR_RESERVED_WORD_START_V1..OFFLINE_CASH_RECURSIVE_PAIR_BINDING_WORDS_V1
        {
            let mut mutated = words;
            mutated[index] = 1;
            assert!(
                OfflineCashRecursivePairBindingV1::from_canonical_words(mutated).is_err(),
                "reserved binding word {index} must be zero"
            );
        }

        let mut zero_child_digest = words;
        zero_child_digest[RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1
            ..RECURSIVE_PAIR_RESERVED_WORD_START_V1]
            .fill(0);
        assert!(
            OfflineCashRecursivePairBindingV1::from_canonical_words(zero_child_digest).is_err(),
            "final State must carry a non-zero GuardBundle join"
        );

        let guard_bundle_words = guard_bundle
            .canonical_words()
            .expect("canonical GuardBundle words");
        assert!(
            guard_bundle_words[RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1..]
                .iter()
                .all(|word| *word == 0)
        );
        assert_eq!(
            OfflineCashRecursivePairBindingV1::from_canonical_words(guard_bundle_words)
                .expect("strict GuardBundle binding roundtrip"),
            guard_bundle
        );
        let mut nonzero_guard_bundle_child = guard_bundle_words;
        nonzero_guard_bundle_child[RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1] = 1;
        assert!(
            OfflineCashRecursivePairBindingV1::from_canonical_words(nonzero_guard_bundle_child)
                .is_err(),
            "GuardBundle must not carry another child-binding digest"
        );

        let alternate_guard_bundle =
            OfflineCashRecursivePairBindingV1::new_guard_bundle([0xA2; 32], [0xB2; 32])
                .expect("alternate GuardBundle binding");
        assert!(
            binding
                .validate_state_child_binding(&alternate_guard_bundle)
                .is_err(),
            "a same-shape GuardBundle splice must fail"
        );
        let mut tampered_child_digest = words;
        tampered_child_digest[RECURSIVE_PAIR_CHILD_BINDING_DIGEST_WORD_START_V1] ^= 1;
        let tampered =
            OfflineCashRecursivePairBindingV1::from_canonical_words(tampered_child_digest)
                .expect("non-zero digest mutation remains a canonical standalone binding");
        assert!(
            tampered
                .validate_state_child_binding(&guard_bundle)
                .is_err(),
            "child-digest tampering must fail contextual validation"
        );
    }

    fn request() -> OfflineCashPaymentRequestV1 {
        let signing_key = signing_key();
        let encoded = signing_key.verifying_key().to_encoded_point(false);
        let public_key =
            KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded.as_bytes()).expect("public key");
        let encryption_public_key = recipient_encryption_public_key();
        let placeholder = sign(&signing_key, b"placeholder");
        let mut request = OfflineCashPaymentRequestV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: [1; 32],
            network_id: kagemusha_test_network_id(b"offline-cash-v1"),
            asset: asset(),
            scale: 4,
            amount: 12_345,
            recipient: account(),
            receiver_balance_commitment: [2; 32],
            recipient_key_reference: offline_cash_receiver_key_reference_v1(
                &public_key,
                encryption_public_key,
            ),
            recipient_encryption_public_key: encryption_public_key,
            receiver_public_key: public_key,
            request_id: [3; 32],
            issued_at_ms: 1_000,
            expires_at_ms: 61_000,
            hardware_policy_id: [4; 32],
            signature: placeholder,
        };
        request.signature = sign(
            &signing_key,
            &request.canonical_signing_bytes().expect("request bytes"),
        );
        request
    }

    #[test]
    fn request_binds_distinct_strict_signing_and_encryption_keys() {
        let request = request();
        assert!(request.validate().is_ok());
        assert!(
            norito::encode_canonical(&request)
                .expect("canonical request")
                .len()
                <= OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1
        );

        let mut low_order = request.clone();
        low_order.recipient_encryption_public_key = [0; 32];
        low_order.recipient_key_reference = offline_cash_receiver_key_reference_v1(
            &low_order.receiver_public_key,
            low_order.recipient_encryption_public_key,
        );
        low_order.signature = sign(
            &signing_key(),
            &low_order
                .canonical_signing_bytes()
                .expect("low-order key still has signing bytes"),
        );
        assert!(matches!(
            low_order.validate(),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.recipient_encryption_public_key"
            })
        ));

        let mut noncanonical = request.clone();
        noncanonical.recipient_encryption_public_key = X25519_FIELD_MODULUS_LITTLE_ENDIAN;
        noncanonical.recipient_key_reference = offline_cash_receiver_key_reference_v1(
            &noncanonical.receiver_public_key,
            noncanonical.recipient_encryption_public_key,
        );
        noncanonical.signature = sign(
            &signing_key(),
            &noncanonical
                .canonical_signing_bytes()
                .expect("non-canonical key still has signing bytes"),
        );
        assert!(matches!(
            noncanonical.validate(),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.request.recipient_encryption_public_key"
            })
        ));

        let mut substituted = request.clone();
        substituted.recipient_encryption_public_key[0] ^= 1;
        assert!(substituted.validate().is_err());
    }

    fn payment(request: &OfflineCashPaymentRequestV1) -> OfflineCashPaymentV1 {
        let request_digest = request.canonical_digest().expect("request digest");
        let statement = OfflineCashTransferStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: request.release_id,
            network_id: request.network_id,
            asset: request.asset.clone(),
            scale: request.scale,
            amount: request.amount,
            request_digest,
            sender_before: [5; 32],
            sender_after: [6; 32],
            receiver_before: request.receiver_balance_commitment,
            credit_commitment: [7; 32],
            transition_digest: [0; 32],
        }
        .seal_transition()
        .expect("seal transition");
        let transfer = OfflineCashTransferResultV1::from_statement_against(&statement, request)
            .expect("compact statement carrier");
        OfflineCashPaymentV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            transfer,
            proof: OfflineCashPairedProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_proof: vec![0xA1; 128],
                ep_proof: vec![0xB2; 128],
                eq_carried_lineage: ipa_lineage(1, EQ_FOLDED_GENERATOR_V1),
                ep_carried_lineage: ipa_lineage(17, EP_FOLDED_GENERATOR_V1),
                recursive_pair_binding: recursive_pair_binding(),
            },
            encrypted_credit: vec![0xE5; 128],
        }
    }

    #[test]
    fn payment_size_matrix_for_final_proof_and_lineage_budget() {
        let request = request();
        for (proof_bytes, encrypted_bytes, expected_paired, expected_payment) in [
            (3_072_usize, 0_usize, 7_412_usize, 7_526_usize),
            (3_072, 384, 7_412, 7_911),
            (3_200, 1, 7_668, 7_783),
            (3_200, 384, 7_668, 8_167),
        ] {
            let mut payment = payment(&request);
            payment.proof.eq_proof = vec![0xA1; proof_bytes];
            payment.proof.ep_proof = vec![0xB2; proof_bytes];
            payment.encrypted_credit = vec![0xE5; encrypted_bytes];
            let paired = norito::encode_canonical(&payment.proof).expect("encode paired proof");
            let encoded = norito::encode_canonical(&payment).expect("encode payment");
            assert_eq!(paired.len(), expected_paired);
            assert_eq!(encoded.len(), expected_payment);
        }

        let mut qualification = payment(&request);
        qualification.proof.eq_proof = vec![0xA1; 3_072];
        qualification.proof.ep_proof = vec![0xB2; 3_072];
        qualification.encrypted_credit = vec![0xE5; OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1];
        assert!(qualification.validate_against(&request).is_ok());
        assert_eq!(
            norito::encode_canonical(&qualification)
                .expect("qualification payment")
                .len(),
            7_911
        );
        assert_eq!(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 - 7_911, 25);

        let mut maximum_proofs = payment(&request);
        maximum_proofs.proof.eq_proof = vec![0xA1; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1];
        maximum_proofs.proof.ep_proof = vec![0xB2; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1];
        maximum_proofs.encrypted_credit = vec![0xE5; 1];
        assert!(maximum_proofs.validate_against(&request).is_ok());
        assert_eq!(
            norito::encode_canonical(&maximum_proofs)
                .expect("maximum-proof payment")
                .len(),
            7_783
        );

        let mut outer_oversized = maximum_proofs.clone();
        outer_oversized.encrypted_credit = vec![0xE5; OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1];
        assert_eq!(
            outer_oversized.proof.eq_proof.len(),
            OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        );
        assert_eq!(
            outer_oversized.proof.ep_proof.len(),
            OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        );
        assert_eq!(
            outer_oversized.proof.eq_proof.len() + outer_oversized.proof.ep_proof.len(),
            OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1
        );
        assert!(outer_oversized.proof.validate().is_ok());
        assert!(matches!(
            outer_oversized.validate_against(&request),
            Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                if actual == 8_167 && max == OFFLINE_CASH_PAYMENT_MAX_BYTES_V1
        ));

        let mut parity_oversized = payment(&request);
        parity_oversized.proof.eq_proof = vec![0xA1; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 + 1];
        parity_oversized.proof.ep_proof = vec![0xB2; 1];
        parity_oversized.encrypted_credit = vec![0xE5; 1];
        assert!(matches!(
            parity_oversized.validate_against(&request),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.current"
            })
        ));
        let mut ep_parity_oversized = payment(&request);
        ep_parity_oversized.proof.eq_proof = vec![0xA1; 1];
        ep_parity_oversized.proof.ep_proof = vec![0xB2; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 + 1];
        ep_parity_oversized.encrypted_credit = vec![0xE5; 1];
        assert!(matches!(
            ep_parity_oversized.validate_against(&request),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.current"
            })
        ));
        assert_eq!(
            OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1,
            2 * OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        );
        let mut pair_oversized = maximum_proofs.clone();
        pair_oversized.proof.ep_proof.push(0xB2);
        assert_eq!(
            pair_oversized.proof.eq_proof.len() + pair_oversized.proof.ep_proof.len(),
            OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1 + 1
        );
        assert!(matches!(
            pair_oversized.validate_against(&request),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.proof.current"
            })
        ));

        let mut encrypted_oversized = payment(&request);
        encrypted_oversized.encrypted_credit =
            vec![0xE5; OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1 + 1];
        assert!(matches!(
            encrypted_oversized.validate_against(&request),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.payment.encrypted_credit"
            })
        ));
    }

    #[test]
    fn send_digest_messages_reproduce_public_digests() {
        let request = request();
        let statement = payment(&request)
            .reconstruct_statement(&request)
            .expect("reconstructed statement");
        let transition = statement
            .canonical_transition_digest_message()
            .expect("transition digest message");
        let semantic = statement
            .canonical_semantic_digest_message()
            .expect("semantic digest message");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&transition)),
            statement.transition_digest
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&semantic)),
            statement.canonical_digest().expect("semantic digest")
        );
        let network_frame = norito::encode_canonical(&statement.network_id).expect("network frame");
        let asset_frame = norito::encode_canonical(&statement.asset).expect("asset frame");
        assert_eq!(transition.len(), 441);
        assert_eq!(semantic.len(), 421);
        assert_eq!(network_frame.len(), 72);
        assert_eq!(asset_frame.len(), 72);
        let scale_bytes = statement.scale.to_le_bytes();
        let amount_bytes = statement.amount.to_le_bytes();
        let fields: [(&[u8], usize, usize); 9] = [
            (&statement.release_id, 156, 103),
            (&network_frame[40..], 189, 136),
            (&asset_frame[40..], 222, 169),
            (&scale_bytes, 255, 202),
            (&amount_bytes, 260, 207),
            (&statement.request_digest, 277, 224),
            (&statement.sender_before, 310, 257),
            (&statement.sender_after, 343, 290),
            (&statement.receiver_before, 376, 323),
        ];
        for (field, transition_offset, semantic_offset) in fields {
            assert_eq!(
                &transition[transition_offset..transition_offset + field.len()],
                field
            );
            assert_eq!(
                &semantic[semantic_offset..semantic_offset + field.len()],
                field
            );
        }
        assert_eq!(&transition[409..441], &statement.credit_commitment);
        assert_eq!(&semantic[356..388], &statement.credit_commitment);
        assert_eq!(&semantic[389..421], &statement.transition_digest);
    }

    #[test]
    fn compact_payment_digest_binds_request_and_reconstructed_statement() {
        let request = request();
        let payment = payment(&request);
        let baseline_statement = payment
            .reconstruct_statement(&request)
            .expect("baseline statement");
        let baseline_digest = payment
            .canonical_digest_against(&request)
            .expect("baseline payment digest");

        let mut changed_request = request.clone();
        changed_request.amount += 1;
        changed_request.signature = sign(
            &signing_key(),
            &changed_request
                .canonical_signing_bytes()
                .expect("changed request bytes"),
        );
        let changed_statement = payment
            .reconstruct_statement(&changed_request)
            .expect("changed statement");
        let changed_digest = payment
            .canonical_digest_against(&changed_request)
            .expect("changed payment digest");

        assert_ne!(
            baseline_statement.request_digest,
            changed_statement.request_digest
        );
        assert_ne!(
            baseline_statement.transition_digest,
            changed_statement.transition_digest
        );
        assert_ne!(
            baseline_statement
                .canonical_digest()
                .expect("baseline semantic digest"),
            changed_statement
                .canonical_digest()
                .expect("changed semantic digest")
        );
        assert_ne!(baseline_digest, changed_digest);

        let mut acknowledgement = acknowledgement(&changed_request, &payment);
        acknowledgement.payment_digest = baseline_digest;
        acknowledgement.signature = sign(
            &signing_key(),
            &acknowledgement
                .canonical_signing_bytes()
                .expect("changed acknowledgement bytes"),
        );
        assert!(matches!(
            acknowledgement.validate_against(&changed_request, &payment),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.acknowledgement.binding"
            })
        ));
    }

    #[test]
    fn compact_transfer_rejects_caller_transition_and_request_field_substitution() {
        let request = request();
        let statement = payment(&request)
            .reconstruct_statement(&request)
            .expect("baseline statement");

        let mut caller_digest = statement.clone();
        caller_digest.transition_digest[0] ^= 1;
        assert!(matches!(
            OfflineCashTransferResultV1::from_statement_against(&caller_digest, &request),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.statement.transition_digest"
            })
        ));

        let mut request_field_substitution = statement;
        request_field_substitution.amount += 1;
        request_field_substitution.transition_digest = [0; 32];
        let request_field_substitution = request_field_substitution
            .seal_transition()
            .expect("seal substituted statement");
        assert!(matches!(
            OfflineCashTransferResultV1::from_statement_against(
                &request_field_substitution,
                &request
            ),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_cash.transfer.request_binding"
            })
        ));
    }

    fn acknowledgement(
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
    ) -> OfflineCashAcknowledgementV1 {
        let signing_key = signing_key();
        let mut acknowledgement = OfflineCashAcknowledgementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            release_id: request.release_id,
            request_digest: request.canonical_digest().expect("request digest"),
            payment_digest: payment
                .canonical_digest_against(request)
                .expect("payment digest"),
            receiver_balance_commitment: [11; 32],
            acknowledged_at_ms: request.issued_at_ms + 1,
            signature: sign(&signing_key, b"placeholder"),
        };
        acknowledgement.signature = sign(
            &signing_key,
            &acknowledgement
                .canonical_signing_bytes()
                .expect("acknowledgement bytes"),
        );
        acknowledgement
    }

    #[test]
    fn canonical_session_roundtrips_and_fits_transport_caps() {
        let request = request();
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let raw = validate_offline_cash_session_v1(&request, &payment, &acknowledgement)
            .expect("valid session");
        assert!(raw < OFFLINE_CASH_SESSION_TARGET_BYTES_V1);
        let request_bytes = norito::encode_canonical(&request).expect("encode request");
        let decoded_request = OfflineCashPaymentRequestV1::decode_canonical_exact(&request_bytes)
            .expect("decode request");
        assert_eq!(decoded_request, request);

        let payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
        let decoded_payment =
            OfflineCashPaymentV1::decode_canonical_exact_against(&payment_bytes, &decoded_request)
                .expect("decode payment");
        assert_eq!(decoded_payment, payment);

        let acknowledgement_bytes =
            norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
        let decoded_acknowledgement = OfflineCashAcknowledgementV1::decode_canonical_exact_against(
            &acknowledgement_bytes,
            &decoded_request,
            &decoded_payment,
        )
        .expect("decode acknowledgement");
        assert_eq!(decoded_acknowledgement, acknowledgement);
    }

    #[test]
    fn exact_decoders_reject_outer_cap_before_parsing() {
        let request = request();
        let payment = payment(&request);
        for (result, expected_actual, expected_max) in [
            (
                OfflineCashPaymentRequestV1::decode_canonical_exact(&vec![
                    0;
                    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1
                        + 1
                ])
                .map(|_| ()),
                OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
            ),
            (
                OfflineCashPaymentV1::decode_canonical_exact_against(
                    &vec![0; OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1],
                    &request,
                )
                .map(|_| ()),
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            ),
            (
                OfflineCashAcknowledgementV1::decode_canonical_exact_against(
                    &vec![0; OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1],
                    &request,
                    &payment,
                )
                .map(|_| ()),
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1,
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            ),
        ] {
            assert!(matches!(
                result,
                Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                    if actual == expected_actual && max == expected_max
            ));
        }
    }

    #[test]
    fn exact_decoders_reject_forged_declared_lengths() {
        const NORITO_PAYLOAD_LENGTH_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
        const NORITO_PAYLOAD_LENGTH_END: usize = NORITO_PAYLOAD_LENGTH_OFFSET + 8;

        let request = request();
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);
        let mut noncanonical_request =
            norito::encode_canonical(&request).expect("encode noncanonical request fixture");
        noncanonical_request.push(0);
        assert!(
            OfflineCashPaymentRequestV1::decode_canonical_exact(&noncanonical_request).is_err()
        );
        let mut noncanonical_payment =
            norito::encode_canonical(&payment).expect("encode noncanonical payment fixture");
        noncanonical_payment.push(0);
        assert!(
            OfflineCashPaymentV1::decode_canonical_exact_against(&noncanonical_payment, &request)
                .is_err()
        );
        let mut noncanonical_acknowledgement = norito::encode_canonical(&acknowledgement)
            .expect("encode noncanonical acknowledgement fixture");
        noncanonical_acknowledgement.push(0);
        assert!(
            OfflineCashAcknowledgementV1::decode_canonical_exact_against(
                &noncanonical_acknowledgement,
                &request,
                &payment,
            )
            .is_err()
        );

        let mut request_bytes = norito::encode_canonical(&request).expect("encode request");
        let mut payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
        let mut acknowledgement_bytes =
            norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
        for bytes in [
            &mut request_bytes,
            &mut payment_bytes,
            &mut acknowledgement_bytes,
        ] {
            bytes[NORITO_PAYLOAD_LENGTH_OFFSET..NORITO_PAYLOAD_LENGTH_END]
                .copy_from_slice(&u64::MAX.to_le_bytes());
        }

        assert!(OfflineCashPaymentRequestV1::decode_canonical_exact(&request_bytes).is_err());
        assert!(
            OfflineCashPaymentV1::decode_canonical_exact_against(&payment_bytes, &request).is_err()
        );
        assert!(
            OfflineCashAcknowledgementV1::decode_canonical_exact_against(
                &acknowledgement_bytes,
                &request,
                &payment,
            )
            .is_err()
        );
    }

    #[test]
    fn raw_session_hard_limit_is_distinct_from_qualification_target() {
        assert!(
            validate_offline_cash_raw_session_size_v1(OFFLINE_CASH_SESSION_TARGET_BYTES_V1 + 1)
                .is_ok()
        );
        assert!(
            validate_offline_cash_raw_session_size_v1(OFFLINE_CASH_SESSION_MAX_BYTES_V1).is_ok()
        );
        assert!(matches!(
            validate_offline_cash_raw_session_size_v1(OFFLINE_CASH_SESSION_MAX_BYTES_V1 + 1),
            Err(KagemushaValidationError::EncodedSizeExceeded { actual, max })
                if actual == OFFLINE_CASH_SESSION_MAX_BYTES_V1 + 1
                    && max == OFFLINE_CASH_SESSION_MAX_BYTES_V1
        ));
    }

    #[test]
    fn request_signature_binds_the_current_balance_head() {
        let mut request = request();
        request.receiver_balance_commitment = [0x55; 32];
        assert!(request.validate().is_err());
    }

    #[test]
    fn topology_substitution_and_oversized_proofs_are_rejected() {
        let request = request();
        let mut aliased_proof = payment(&request);
        aliased_proof.proof.ep_proof = aliased_proof.proof.eq_proof.clone();
        assert!(aliased_proof.validate_against(&request).is_err());
        let mut aliased_lineage = payment(&request);
        aliased_lineage.proof.ep_carried_lineage = aliased_lineage.proof.eq_carried_lineage;
        assert!(aliased_lineage.validate_against(&request).is_err());
        let mut substituted = payment(&request);
        substituted.proof.recursive_pair_binding =
            OfflineCashRecursivePairBindingV1::new_guard_bundle([0x81; 32], [0x82; 32])
                .expect("GuardBundle binding");
        assert!(substituted.validate_against(&request).is_err());
        let mut oversized = payment(&request);
        oversized.proof.eq_proof = vec![0xAA; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 + 1];
        assert!(oversized.validate_against(&request).is_err());
    }

    #[test]
    fn acknowledgement_binds_the_persisted_receiver_head() {
        let request = request();
        let payment = payment(&request);
        let mut acknowledgement = acknowledgement(&request, &payment);
        acknowledgement.receiver_balance_commitment = request.receiver_balance_commitment;
        assert!(
            acknowledgement
                .validate_against(&request, &payment)
                .is_err()
        );
    }

    #[test]
    fn peer_adapter_roundtrips_strict_kgm2_text() {
        let adapter = OfflineCashPeerAdapterV1;
        let request = request();
        let payment = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment);

        let request_text = adapter
            .encode_payment_request(&request)
            .expect("encode request text");
        assert!(request_text.starts_with(OFFLINE_CASH_TEXT_PREFIX_V1));
        assert!(!request_text.contains('='));
        let decoded_request = adapter
            .decode_payment_request(&request_text)
            .expect("decode request text");
        assert_eq!(decoded_request, request);

        let payment_text = adapter
            .encode_payment(&request, &payment)
            .expect("encode payment text");
        let decoded_payment = adapter
            .decode_payment(&request, &payment_text)
            .expect("decode payment text");
        assert_eq!(decoded_payment, payment);

        let acknowledgement_text = adapter
            .encode_acknowledgement(&request, &payment, &acknowledgement)
            .expect("encode acknowledgement text");
        let decoded_acknowledgement = adapter
            .decode_acknowledgement(&request, &payment, &acknowledgement_text)
            .expect("decode acknowledgement text");
        assert_eq!(decoded_acknowledgement, acknowledgement);
        assert!(
            request_text.len() + payment_text.len() + acknowledgement_text.len()
                <= OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1
        );

        assert!(
            adapter
                .decode_payment_request(&format!("{request_text}="))
                .is_err()
        );
        assert!(
            adapter
                .decode_payment_request(&format!(" {request_text}"))
                .is_err()
        );
        assert!(
            adapter
                .decode_payment_request(&request_text.replacen("kgm2:", "kgm1:", 1))
                .is_err()
        );
    }

    #[test]
    fn verification_transcript_is_request_release_bound_monotonic_and_replay_safe() {
        let request = request();
        let payment_value = payment(&request);
        let acknowledgement = acknowledgement(&request, &payment_value);
        assert!(
            OfflineCashVerificationTranscriptV1::new(request.clone(), [0xFF; 32], [0xAA; 32],)
                .is_err()
        );
        let mut transcript = OfflineCashVerificationTranscriptV1::new(
            request.clone(),
            request.release_id,
            [0xAA; 32],
        )
        .expect("release-bound structural transcript");
        assert_eq!(transcript.expected_artifact_manifest_sha256(), [0xAA; 32]);
        assert_eq!(
            transcript.state(),
            OfflineCashVerificationTranscriptStateV1::ReceiveRequestValidated
        );
        assert_eq!(
            transcript
                .validate_payment(payment_value.clone())
                .expect("validate payment"),
            OfflineCashVerificationTranscriptEventV1::PaymentValidated
        );
        assert_eq!(
            transcript
                .validate_payment(payment_value.clone())
                .expect("idempotent payment replay"),
            OfflineCashVerificationTranscriptEventV1::PaymentValidationReplay
        );
        let mut conflicting_payment = payment_value.clone();
        conflicting_payment.encrypted_credit.push(0x44);
        assert!(transcript.validate_payment(conflicting_payment).is_err());
        assert_eq!(
            transcript
                .validate_acknowledgement(acknowledgement)
                .expect("validate acknowledgement"),
            OfflineCashVerificationTranscriptEventV1::AcknowledgementValidated
        );
        assert_eq!(
            transcript.state(),
            OfflineCashVerificationTranscriptStateV1::AcknowledgementValidated
        );
        let acknowledgement_replay = transcript
            .validated_acknowledgement()
            .expect("validated acknowledgement")
            .to_owned();
        assert_eq!(
            transcript
                .validate_acknowledgement(acknowledgement_replay)
                .expect("idempotent acknowledgement replay"),
            OfflineCashVerificationTranscriptEventV1::AcknowledgementValidationReplay
        );
        assert!(transcript.validate_payment(payment_value).is_err());
    }

    #[test]
    #[allow(deprecated)]
    fn deprecated_wallet_session_aliases_preserve_non_authorizing_source_compatibility() {
        assert_eq!(
            OfflineCashWalletSessionStateV1::ReceiveRequestReady,
            OfflineCashVerificationTranscriptStateV1::ReceiveRequestValidated
        );
        assert_eq!(
            OfflineCashWalletSessionStateV1::PaymentCommitted,
            OfflineCashVerificationTranscriptStateV1::PaymentValidated
        );
        assert_eq!(
            OfflineCashWalletSessionEventV1::Acknowledged,
            OfflineCashVerificationTranscriptEventV1::AcknowledgementValidated
        );

        let request = request();
        let payment = payment(&request);
        let mut transcript: OfflineCashWalletSessionV1 =
            OfflineCashWalletSessionV1::new(request.clone(), request.release_id, [0xAA; 32])
                .expect("deprecated alias constructs only a structural transcript");
        assert_eq!(
            transcript
                .accept_payment(payment)
                .expect("deprecated method validates only"),
            OfflineCashWalletSessionEventV1::PaymentCommitted
        );
    }

    #[test]
    fn wallet_session_compatibility_names_are_deprecated_transcript_aliases_by_source_contract() {
        let source = include_str!("offline_cash_v1.rs");
        for (alias, target) in [
            (
                "OfflineCashWalletSessionStateV1",
                "OfflineCashVerificationTranscriptStateV1",
            ),
            (
                "OfflineCashWalletSessionEventV1",
                "OfflineCashVerificationTranscriptEventV1",
            ),
            (
                "OfflineCashWalletSessionV1",
                "OfflineCashVerificationTranscriptV1",
            ),
        ] {
            let declaration = format!("pub type {alias} = {target};");
            let attribute_block = source
                .split_once(&declaration)
                .unwrap_or_else(|| panic!("missing compatibility alias `{declaration}`"))
                .0
                .rsplit_once("\n\n")
                .expect("compatibility alias has an attribute boundary")
                .1;
            assert!(
                attribute_block.contains("#[deprecated("),
                "compatibility alias `{alias}` must remain deprecated"
            );
            for forbidden_kind in ["struct", "enum"] {
                let forbidden = format!("pub {forbidden_kind} {alias}");
                assert!(
                    !source.contains(&forbidden),
                    "compatibility name `{alias}` must not declare a runtime {forbidden_kind}"
                );
            }
        }
    }
}

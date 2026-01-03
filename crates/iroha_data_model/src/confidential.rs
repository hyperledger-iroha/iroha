//! Confidential parameter registries and lifecycle helpers.
//!
//! The confidential asset roadmap introduces on-ledger registries that track
//! zero-knowledge verifier metadata together with Pedersen and Poseidon
//! parameter sets. These structures model the governance state transitions
//! (publish → activate → deprecate → withdraw) and advertise the hashes that
//! wallets and validators must verify before accepting an upgrade.

use core::fmt::{self, Display, Formatter};

use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    core::{self as norito_core, DecodeFromSlice, Error as NoritoError},
};

#[cfg(feature = "json")]
use crate::json_helpers::fixed_bytes;

/// Version discriminator for confidential encrypted payloads (v1 layout).
pub const CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1: u8 = 1;

/// AEAD-wrapped note payload for confidential instructions.
///
/// This envelope carries the metadata required to decrypt a shielded note payload:
/// - `version` anchors the on-wire layout. Only `CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` is supported.
/// - `ephemeral_pubkey` conveys the sender's Diffie–Hellman public key (X25519, 32 bytes).
/// - `nonce` is the XChaCha20-Poly1305 nonce used for AEAD encryption (24 bytes).
/// - `ciphertext` stores the encrypted payload with the Poly1305 authentication tag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ConfidentialEncryptedPayload {
    version: u8,
    /// Sender's ephemeral X25519 public key used for ECDH (32 bytes).
    #[cfg_attr(feature = "json", norito(with = "fixed_bytes"))]
    ephemeral_pubkey: [u8; 32],
    /// XChaCha20-Poly1305 nonce (24 bytes).
    #[cfg_attr(feature = "json", norito(with = "fixed_bytes"))]
    nonce: [u8; 24],
    /// Encrypted payload + Poly1305 tag bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    ciphertext: Vec<u8>,
}

fn varint_len(len: usize) -> usize {
    let mut value = len;
    let mut bytes = 0;
    loop {
        bytes += 1;
        if value < 0x80 {
            return bytes;
        }
        value >>= 7;
    }
}

fn write_varint<W: std::io::Write>(writer: &mut W, len: usize) -> Result<(), NoritoError> {
    let mut value = len;
    loop {
        let byte = u8::try_from(value & 0x7F).expect("masked varint chunk must fit within 7 bits");
        value >>= 7;
        if value == 0 {
            writer.write_all(&[byte])?;
            break;
        }
        writer.write_all(&[byte | 0x80])?;
    }
    Ok(())
}

fn read_varint(bytes: &[u8]) -> Result<(usize, usize), NoritoError> {
    let mut value: usize = 0;
    let mut shift = 0usize;
    for (idx, byte) in bytes.iter().enumerate() {
        let chunk = (byte & 0x7F) as usize;
        let shift_u32 = u32::try_from(shift)
            .map_err(|_| NoritoError::Message("ciphertext length overflow".to_owned()))?;
        value |= chunk
            .checked_shl(shift_u32)
            .ok_or_else(|| NoritoError::Message("ciphertext length overflow".to_owned()))?;
        if byte & 0x80 == 0 {
            return Ok((value, idx + 1));
        }
        shift += 7;
        if shift >= usize::BITS as usize {
            return Err(NoritoError::Message("ciphertext length overflow".into()));
        }
    }
    Err(NoritoError::LengthMismatch)
}

impl ConfidentialEncryptedPayload {
    const EPHEMERAL_LEN: usize = 32;
    const NONCE_LEN: usize = 24;
    /// Construct a new envelope using the v1 layout.
    #[must_use]
    pub fn new(ephemeral_pubkey: [u8; 32], nonce: [u8; 24], ciphertext: Vec<u8>) -> Self {
        Self {
            version: CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1,
            ephemeral_pubkey,
            nonce,
            ciphertext,
        }
    }

    /// Construct an envelope when only ciphertext bytes are available.
    ///
    /// This helper preserves compatibility with existing flows that persisted
    /// opaque payloads prior to the v1 layout. It zeros the ephemeral key/nonce
    /// fields so the envelope remains well-formed. Callers should migrate to the
    /// structured [`Self::new`] constructor once wallets expose the full metadata.
    #[must_use]
    pub fn from_ciphertext(ciphertext: Vec<u8>) -> Self {
        Self::new([0; 32], [0; 24], ciphertext)
    }

    /// Return the envelope version recorded on-wire.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Check whether the envelope uses a supported layout.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.version == CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1
    }

    /// Sender's ephemeral public key bytes.
    #[must_use]
    pub const fn ephemeral_pubkey(&self) -> &[u8; 32] {
        &self.ephemeral_pubkey
    }

    /// AEAD nonce bytes.
    #[must_use]
    pub const fn nonce(&self) -> &[u8; 24] {
        &self.nonce
    }

    /// Encrypted payload bytes.
    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    /// Consume the envelope and return the ciphertext.
    #[must_use]
    pub fn into_ciphertext(self) -> Vec<u8> {
        self.ciphertext
    }
}

impl Default for ConfidentialEncryptedPayload {
    fn default() -> Self {
        Self::from_ciphertext(Vec::new())
    }
}

impl norito::NoritoSerialize for ConfidentialEncryptedPayload {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), NoritoError> {
        writer.write_all(&[self.version])?;
        writer.write_all(&self.ephemeral_pubkey)?;
        writer.write_all(&self.nonce)?;
        self::write_varint(&mut writer, self.ciphertext.len())?;
        writer.write_all(&self.ciphertext)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(
            1 + Self::EPHEMERAL_LEN
                + Self::NONCE_LEN
                + self::varint_len(self.ciphertext.len())
                + self.ciphertext.len(),
        )
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::NoritoDeserialize<'de> for ConfidentialEncryptedPayload {
    fn deserialize(archived: &'de norito_core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ConfidentialEncryptedPayload deserialization must succeed for valid archives")
    }

    fn try_deserialize(archived: &'de norito_core::Archived<Self>) -> Result<Self, NoritoError> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let payload = norito_core::payload_slice_from_ptr(ptr)?;
        let (value, _) =
            <ConfidentialEncryptedPayload as DecodeFromSlice>::decode_from_slice(payload)?;
        Ok(value)
    }
}

impl<'a> norito_core::DecodeFromSlice<'a> for ConfidentialEncryptedPayload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        if bytes.len() < 1 + Self::EPHEMERAL_LEN + Self::NONCE_LEN {
            return Err(NoritoError::LengthMismatch);
        }
        let version = bytes[0];
        let mut ephemeral = [0u8; Self::EPHEMERAL_LEN];
        ephemeral.copy_from_slice(&bytes[1..=Self::EPHEMERAL_LEN]);
        let mut nonce = [0u8; Self::NONCE_LEN];
        let nonce_start = 1 + Self::EPHEMERAL_LEN;
        let nonce_end_exclusive = nonce_start + Self::NONCE_LEN;
        let nonce_end_inclusive = nonce_end_exclusive - 1;
        nonce.copy_from_slice(&bytes[nonce_start..=nonce_end_inclusive]);
        let (cipher_len, varint_len) = read_varint(&bytes[nonce_end_exclusive..])?;
        let cipher_start = nonce_end_exclusive + varint_len;
        let cipher_end = cipher_start
            .checked_add(cipher_len)
            .ok_or_else(|| NoritoError::Message("ciphertext length overflow".into()))?;
        if cipher_end > bytes.len() {
            return Err(NoritoError::Message("TruncatedPayload".into()));
        }
        let ciphertext = bytes[cipher_start..cipher_end].to_vec();
        let payload = ConfidentialEncryptedPayload {
            version,
            ephemeral_pubkey: ephemeral,
            nonce,
            ciphertext,
        };
        Ok((payload, cipher_end))
    }
}

impl From<Vec<u8>> for ConfidentialEncryptedPayload {
    fn from(value: Vec<u8>) -> Self {
        Self::from_ciphertext(value)
    }
}

impl From<&[u8]> for ConfidentialEncryptedPayload {
    fn from(value: &[u8]) -> Self {
        Self::from_ciphertext(value.to_vec())
    }
}

/// Status of a confidential registry entry.
///
/// Entries begin in the `Proposed` state once governance publishes new
/// metadata. They become `Active` at the scheduled height and transition
/// to `Withdrawn` once retired, at which point they must not be used by
/// validators or wallets. The lifecycle applies uniformly to verifier keys
/// and parameter sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[repr(u8)]
#[norito(reuse_archived)]
pub enum ConfidentialStatus {
    /// Entry has been published but is not yet active.
    Proposed,
    /// Entry is active and may be used for verification.
    Active,
    /// Entry has been withdrawn and must reject verification attempts.
    Withdrawn,
}

impl ConfidentialStatus {
    /// Returns true if the status permits active use.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, ConfidentialStatus::Active)
    }

    fn from_u8(value: u8) -> Result<Self, NoritoError> {
        match value {
            0 => Ok(Self::Proposed),
            1 => Ok(Self::Active),
            2 => Ok(Self::Withdrawn),
            other => Err(NoritoError::Message(format!(
                "invalid ConfidentialStatus discriminant {other}"
            ))),
        }
    }
}

impl From<ConfidentialStatus> for u8 {
    fn from(status: ConfidentialStatus) -> Self {
        match status {
            ConfidentialStatus::Proposed => 0,
            ConfidentialStatus::Active => 1,
            ConfidentialStatus::Withdrawn => 2,
        }
    }
}

impl<'a> DecodeFromSlice<'a> for ConfidentialStatus {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        let (raw, used) = u8::decode_from_slice(bytes)?;
        ConfidentialStatus::from_u8(raw).map(|status| (status, used))
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ConfidentialStatus {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ConfidentialStatus::Proposed => "Proposed",
            ConfidentialStatus::Active => "Active",
            ConfidentialStatus::Withdrawn => "Withdrawn",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ConfidentialStatus {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Proposed" => Ok(ConfidentialStatus::Proposed),
            "Active" => Ok(ConfidentialStatus::Active),
            "Withdrawn" => Ok(ConfidentialStatus::Withdrawn),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}

/// Digest advertising the active confidential feature set (verifier keys and parameters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct ConfidentialFeatureDigest {
    /// Optional hash summarizing the set of active verifying keys.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub vk_set_hash: Option<[u8; 32]>,
    /// Poseidon parameter set identifier expected by the node.
    pub poseidon_params_id: Option<u32>,
    /// Pedersen parameter set identifier expected by the node.
    pub pedersen_params_id: Option<u32>,
    /// Version of the confidential ruleset encoded in manifests and policies.
    pub conf_rules_version: Option<u32>,
}

impl ConfidentialFeatureDigest {
    /// Construct a new digest from individual components.
    #[must_use]
    pub const fn new(
        vk_set_hash: Option<[u8; 32]>,
        poseidon_params_id: Option<u32>,
        pedersen_params_id: Option<u32>,
        conf_rules_version: Option<u32>,
    ) -> Self {
        Self {
            vk_set_hash,
            poseidon_params_id,
            pedersen_params_id,
            conf_rules_version,
        }
    }

    /// Returns `true` if all fields are `None`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.vk_set_hash.is_none()
            && self.poseidon_params_id.is_none()
            && self.pedersen_params_id.is_none()
            && self.conf_rules_version.is_none()
    }
}

/// Ruleset version embedded into [`ConfidentialFeatureDigest::conf_rules_version`] for v1 networks.
pub const CONFIDENTIAL_RULES_VERSION: u32 = 1;

/// Default digest advertising only the confidential ruleset version.
pub const DEFAULT_CONFIDENTIAL_FEATURE_DIGEST: ConfidentialFeatureDigest =
    ConfidentialFeatureDigest::new(None, None, None, Some(CONFIDENTIAL_RULES_VERSION));

/// Identifier for confidential parameter registries (Pedersen/Poseidon).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct ConfidentialParamsId {
    value: u32,
}

impl ConfidentialParamsId {
    /// Construct a new identifier from a raw integer value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self { value }
    }

    /// Access the underlying integer value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.value
    }
}

impl From<u32> for ConfidentialParamsId {
    fn from(value: u32) -> Self {
        Self { value }
    }
}

impl From<ConfidentialParamsId> for u32 {
    fn from(value: ConfidentialParamsId) -> Self {
        value.value
    }
}

impl Display for ConfidentialParamsId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// Descriptor for a Pedersen parameter set tracked on-ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct PedersenParams {
    /// Identifier referenced by shielded assets and proofs.
    pub params_id: ConfidentialParamsId,
    /// Hash of the curve generators used by the Pedersen commitment scheme.
    #[cfg_attr(feature = "json", norito(with = "fixed_bytes"))]
    pub generators_hash: [u8; 32],
    /// Hash of auxiliary constants (domain separators, blinding hints, etc.).
    #[cfg_attr(feature = "json", norito(with = "fixed_bytes"))]
    pub constants_hash: [u8; 32],
    /// Optional URI (CID) pointing to the canonical parameter bundle documentation.
    pub metadata_uri_cid: Option<String>,
    /// Optional URI (CID) pointing to the binary parameter bundle.
    pub params_cid: Option<String>,
    /// Block height when the parameter set becomes active.
    pub activation_height: Option<u64>,
    /// Block height when the parameter set must be withdrawn.
    pub withdraw_height: Option<u64>,
    /// Lifecycle status of the parameter entry.
    pub status: ConfidentialStatus,
}

impl PedersenParams {
    /// Returns `true` if the parameter set is usable for verification at `height`.
    #[must_use]
    pub fn is_effective_at(&self, height: u64) -> bool {
        match self.status {
            ConfidentialStatus::Active => {
                let activation_ok = self.activation_height.is_none_or(|h| height >= h);
                let withdraw_ok = self.withdraw_height.is_none_or(|limit| height < limit);
                activation_ok && withdraw_ok
            }
            ConfidentialStatus::Proposed | ConfidentialStatus::Withdrawn => false,
        }
    }
}

/// Descriptor for a Poseidon parameter set tracked on-ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize,
        crate::DeriveFastJson
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct PoseidonParams {
    /// Identifier referenced by shielded assets and proofs.
    pub params_id: ConfidentialParamsId,
    /// Hash of the Poseidon round constants.
    #[cfg_attr(feature = "json", norito(with = "fixed_bytes"))]
    pub round_constants_hash: [u8; 32],
    /// Hash of the Poseidon MDS matrix.
    #[cfg_attr(feature = "json", norito(with = "fixed_bytes"))]
    pub mds_matrix_hash: [u8; 32],
    /// Optional URI (CID) pointing to the canonical parameter bundle documentation.
    pub metadata_uri_cid: Option<String>,
    /// Optional URI (CID) pointing to the binary parameter bundle.
    pub params_cid: Option<String>,
    /// Block height when the parameter set becomes active.
    pub activation_height: Option<u64>,
    /// Block height when the parameter set must be withdrawn.
    pub withdraw_height: Option<u64>,
    /// Lifecycle status of the parameter entry.
    pub status: ConfidentialStatus,
}

impl PoseidonParams {
    /// Returns `true` if the parameter set is usable for verification at `height`.
    #[must_use]
    pub fn is_effective_at(&self, height: u64) -> bool {
        match self.status {
            ConfidentialStatus::Active => {
                let activation_ok = self.activation_height.is_none_or(|h| height >= h);
                let withdraw_ok = self.withdraw_height.is_none_or(|limit| height < limit);
                activation_ok && withdraw_ok
            }
            ConfidentialStatus::Proposed | ConfidentialStatus::Withdrawn => false,
        }
    }
}

/// Frequently used confidential registry types.
pub mod prelude {
    pub use super::{ConfidentialParamsId, ConfidentialStatus, PedersenParams, PoseidonParams};
}

#[cfg(test)]
mod tests {
    use norito::codec::{decode_adaptive, encode_adaptive};

    use super::*;

    #[test]
    fn pedersen_roundtrip() {
        let params = PedersenParams {
            params_id: ConfidentialParamsId::new(7),
            generators_hash: [0xA5; 32],
            constants_hash: [0x5A; 32],
            metadata_uri_cid: Some("ipfs://pedersen-docs".into()),
            params_cid: Some("ipfs://pedersen-raw".into()),
            activation_height: Some(10),
            withdraw_height: Some(30),
            status: ConfidentialStatus::Active,
        };
        let bytes = norito::to_bytes(&params).expect("encode pedersen params");
        let decoded: PedersenParams =
            norito::decode_from_bytes(&bytes).expect("decode pedersen params");
        assert_eq!(params, decoded);
        assert!(decoded.is_effective_at(15));
        assert!(!decoded.is_effective_at(35));
    }

    #[test]
    fn poseidon_roundtrip() {
        let params = PoseidonParams {
            params_id: ConfidentialParamsId::new(5),
            round_constants_hash: [0x11; 32],
            mds_matrix_hash: [0x22; 32],
            metadata_uri_cid: None,
            params_cid: Some("ipfs://poseidon".into()),
            activation_height: Some(5),
            withdraw_height: Some(25),
            status: ConfidentialStatus::Active,
        };
        let bytes = norito::to_bytes(&params).expect("encode poseidon params");
        let decoded: PoseidonParams =
            norito::decode_from_bytes(&bytes).expect("decode poseidon params");
        assert_eq!(params, decoded);
        assert!(decoded.is_effective_at(10));
        assert!(!decoded.is_effective_at(30));
    }

    #[test]
    fn encrypted_payload_roundtrips() {
        let payload =
            ConfidentialEncryptedPayload::new([1u8; 32], [2u8; 24], vec![3, 4, 5, 6, 7, 8, 9]);
        let encoded = encode_adaptive(&payload);
        let decoded: ConfidentialEncryptedPayload =
            decode_adaptive(&encoded).expect("decode encrypted payload");
        assert_eq!(decoded, payload);
        assert!(decoded.is_supported());
        assert_eq!(decoded.version(), CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1);
    }

    #[test]
    fn ciphertext_only_helper_zero_fills_metadata() {
        let ciphertext = vec![42u8; 5];
        let payload = ConfidentialEncryptedPayload::from_ciphertext(ciphertext.clone());
        assert_eq!(payload.ephemeral_pubkey(), &[0u8; 32]);
        assert_eq!(payload.nonce(), &[0u8; 24]);
        assert_eq!(payload.ciphertext(), ciphertext.as_slice());
    }
}

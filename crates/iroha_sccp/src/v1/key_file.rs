//! The node-local bridge key file (spec §4.9).
//!
//! Each bridge key lives in one owner-only file `<address-hex>.key` under the attestor's
//! `key_dir`, holding the headered Norito frame of
//! `SccpBridgeKeyFileV1 { secret: [u8; 32], created_at_ms: u64 }`. This module owns the frame
//! format, key generation from the OS CSPRNG, address derivation and signing; directory
//! handling, modes and atomic writes belong to the attestor (`irohad`).
//!
//! The secret is wiped when the value is dropped and never appears in `Debug` output. Wiping is
//! best effort without `unsafe`: the bytes are overwritten behind an optimization barrier.

use core::fmt;

use super::signature::{self, SignatureError, wipe};

unit_error! {
    /// Bridge key file errors.
    pub enum KeyFileError {
        /// The secret is not a valid secp256k1 scalar (`1 ≤ secret < N`).
        InvalidSecret => "bridge key secret is not a valid secp256k1 scalar",
        /// Fresh key material could not be drawn from the OS.
        EntropyUnavailable => "OS randomness is unavailable for bridge key generation",
        /// The Norito frame could not be encoded.
        Encode => "bridge key file frame could not be encoded",
        /// The bytes are not a valid `SccpBridgeKeyFileV1` frame.
        Decode => "bridge key file frame is invalid",
    }
}

/// Suffix of a bridge key file name.
pub const KEY_FILE_SUFFIX: &str = ".key";

/// A node-local secp256k1 bridge key (§4.9).
#[derive(
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::v1::key_file::SccpBridgeKeyFileV1")]
pub struct SccpBridgeKeyFileV1 {
    secret: [u8; 32],
    created_at_ms: u64,
}

impl SccpBridgeKeyFileV1 {
    /// Wrap an existing secret.
    ///
    /// # Errors
    ///
    /// Returns [`KeyFileError::InvalidSecret`] unless `1 ≤ secret < N`.
    pub fn new(mut secret: [u8; 32], created_at_ms: u64) -> Result<Self, KeyFileError> {
        let valid = signature::address_of_secret(&secret).is_ok();
        let key = Self {
            secret,
            created_at_ms,
        };
        wipe(&mut secret);
        if valid {
            Ok(key)
        } else {
            Err(KeyFileError::InvalidSecret)
        }
    }

    /// Generate a fresh key from the OS CSPRNG.
    ///
    /// # Errors
    ///
    /// Returns [`KeyFileError::EntropyUnavailable`] when the OS RNG fails.
    pub fn generate(created_at_ms: u64) -> Result<Self, KeyFileError> {
        let secret = signature::fresh_entropy().map_err(|_| KeyFileError::EntropyUnavailable)?;
        Self::new(secret, created_at_ms)
    }

    /// The raw secret scalar (big-endian).
    #[must_use]
    pub fn secret(&self) -> &[u8; 32] {
        &self.secret
    }

    /// Creation time used to pick the newest registration candidate.
    #[must_use]
    pub fn created_at_ms(&self) -> u64 {
        self.created_at_ms
    }

    /// Compressed secp256k1 public key.
    ///
    /// # Errors
    ///
    /// Returns [`SignatureError::InvalidSecret`] for a corrupted secret.
    pub fn public_key(&self) -> Result<[u8; 33], SignatureError> {
        signature::public_key_of(&self.secret)
    }

    /// Bridge address (§3.8).
    ///
    /// # Errors
    ///
    /// Returns [`SignatureError::InvalidSecret`] for a corrupted secret.
    pub fn address(&self) -> Result<[u8; 20], SignatureError> {
        signature::address_of_secret(&self.secret)
    }

    /// The file name `<address-hex>.key` (40 lowercase hex digits, no prefix).
    ///
    /// # Errors
    ///
    /// Returns [`SignatureError::InvalidSecret`] for a corrupted secret.
    pub fn file_name(&self) -> Result<String, SignatureError> {
        Ok(key_file_name(&self.address()?))
    }

    /// Sign a 32-byte digest (§3.8).
    ///
    /// # Errors
    ///
    /// See [`signature::sign_digest`].
    pub fn sign_digest(&self, digest: &[u8; 32]) -> Result<[u8; 65], SignatureError> {
        signature::sign_digest(&self.secret, digest)
    }

    /// Encode the headered Norito frame written to disk.
    ///
    /// # Errors
    ///
    /// Returns [`KeyFileError::Encode`] when Norito encoding fails.
    pub fn to_frame(&self) -> Result<KeyFileBytes, KeyFileError> {
        norito::to_bytes(self)
            .map(KeyFileBytes)
            .map_err(|_| KeyFileError::Encode)
    }

    /// Decode and validate a headered Norito frame read from disk.
    ///
    /// # Errors
    ///
    /// Returns [`KeyFileError::Decode`] for a malformed frame and
    /// [`KeyFileError::InvalidSecret`] for an invalid scalar.
    pub fn from_frame(bytes: &[u8]) -> Result<Self, KeyFileError> {
        let decoded: Self =
            norito::decode_from_bytes(bytes).map_err(|_| KeyFileError::Decode)?;
        if signature::address_of_secret(&decoded.secret).is_err() {
            return Err(KeyFileError::InvalidSecret);
        }
        Ok(decoded)
    }
}

impl Drop for SccpBridgeKeyFileV1 {
    fn drop(&mut self) {
        wipe(&mut self.secret);
    }
}

impl fmt::Debug for SccpBridgeKeyFileV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SccpBridgeKeyFileV1")
            .field("secret", &"<redacted>")
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}

/// `<address-hex>.key` for a bridge address.
#[must_use]
pub fn key_file_name(address: &[u8; 20]) -> String {
    let mut name: String = address.iter().map(|byte| format!("{byte:02x}")).collect();
    name.push_str(KEY_FILE_SUFFIX);
    name
}

/// Parse `<address-hex>.key` back into an address; `None` for any other name.
#[must_use]
pub fn parse_key_file_name(name: &str) -> Option<[u8; 20]> {
    let hex = name.strip_suffix(KEY_FILE_SUFFIX)?;
    if hex.len() != 40
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut address = [0_u8; 20];
    for (index, slot) in address.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[2 * index..2 * index + 2], 16).ok()?;
    }
    Some(address)
}

/// Encoded key file bytes; wiped on drop.
#[derive(PartialEq, Eq)]
pub struct KeyFileBytes(Vec<u8>);

impl KeyFileBytes {
    /// The encoded frame.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl core::ops::Deref for KeyFileBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for KeyFileBytes {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

impl fmt::Debug for KeyFileBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "KeyFileBytes(<{} redacted bytes>)", self.0.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::{constants::SECP256K1_N, hashes::keccak256};

    fn fixed_secret() -> [u8; 32] {
        keccak256(&[b"sccp-key-file-test"])
    }

    #[test]
    fn frame_roundtrip() {
        let key = SccpBridgeKeyFileV1::new(fixed_secret(), 1_800_000_000_000).unwrap();
        let frame = key.to_frame().unwrap();
        let decoded = SccpBridgeKeyFileV1::from_frame(&frame).unwrap();
        assert_eq!(decoded, key);
        assert_eq!(decoded.secret(), &fixed_secret());
        assert_eq!(decoded.created_at_ms(), 1_800_000_000_000);
        assert_eq!(frame.as_slice(), &*frame);
        // Norito frames carry a header, so the payload is longer than the 40 raw bytes.
        assert!(frame.len() > 40);
    }

    #[test]
    fn invalid_secrets_and_frames_are_rejected() {
        assert_eq!(
            SccpBridgeKeyFileV1::new([0; 32], 1).unwrap_err(),
            KeyFileError::InvalidSecret
        );
        assert_eq!(
            SccpBridgeKeyFileV1::new(SECP256K1_N, 1).unwrap_err(),
            KeyFileError::InvalidSecret
        );
        assert_eq!(
            SccpBridgeKeyFileV1::from_frame(&[1, 2, 3]).unwrap_err(),
            KeyFileError::Decode
        );
        let key = SccpBridgeKeyFileV1::new(fixed_secret(), 5).unwrap();
        let frame = key.to_frame().unwrap();
        let mut truncated = frame.to_vec();
        truncated.pop();
        assert!(SccpBridgeKeyFileV1::from_frame(&truncated).is_err());
        let mut trailing = frame.to_vec();
        trailing.push(0);
        assert!(SccpBridgeKeyFileV1::from_frame(&trailing).is_err());
    }

    #[test]
    fn debug_never_prints_the_secret() {
        let key = SccpBridgeKeyFileV1::new(fixed_secret(), 9).unwrap();
        let debug = format!("{key:?}");
        assert!(debug.contains("<redacted>"));
        let secret_hex: String = fixed_secret().iter().map(|byte| format!("{byte:02x}")).collect();
        assert!(!debug.contains(&secret_hex));
        assert!(!debug.contains(&format!("{:?}", fixed_secret())));
        let frame_debug = format!("{:?}", key.to_frame().unwrap());
        assert!(frame_debug.contains("redacted"));
    }

    #[test]
    fn address_file_name_and_signing() {
        let key = SccpBridgeKeyFileV1::new(fixed_secret(), 1).unwrap();
        let address = key.address().unwrap();
        assert_eq!(
            signature::address_of(&key.public_key().unwrap()).unwrap(),
            address
        );
        let name = key.file_name().unwrap();
        assert_eq!(name.len(), 44);
        assert!(name.ends_with(".key"));
        assert_eq!(parse_key_file_name(&name), Some(address));
        assert_eq!(parse_key_file_name("abc.key"), None);
        assert_eq!(parse_key_file_name(&name.to_uppercase()), None);
        assert_eq!(parse_key_file_name(&name.replace(".key", ".tmp")), None);
        let digest = keccak256(&[b"digest"]);
        let signature = key.sign_digest(&digest).unwrap();
        assert_eq!(signature::recover_address(&digest, &signature), Ok(address));
    }

    #[test]
    fn generated_keys_are_distinct_and_valid() {
        let a = SccpBridgeKeyFileV1::generate(1).unwrap();
        let b = SccpBridgeKeyFileV1::generate(2).unwrap();
        assert_ne!(a.secret(), b.secret());
        assert!(a.address().is_ok());
        assert_eq!(b.created_at_ms(), 2);
    }
}

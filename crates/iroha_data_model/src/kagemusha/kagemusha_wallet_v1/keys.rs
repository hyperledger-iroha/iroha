//! P-256 payment keys and signatures of the wallet protocol (§8).
//!
//! Every wallet key is one canonical uncompressed SEC1 NIST P-256 point and every wallet
//! signature is the fixed-width big-endian ECDSA-P256-SHA256 scalar pair `r || s` with a low
//! `s`. Both values carry no algorithm tag or selector: their Norito payload is exactly the
//! raw key or signature bytes, and every decoder validates the value before it is returned.
//! These checks establish only canonical form; the role a key may sign for is fixed by the
//! object that carries it.

use iroha_schema::IntoSchema;
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey, signature::Verifier as _,
};

use super::{KagemushaWalletValidationErrorV1, WalletResult, invalid_v1};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};

#[cfg(test)]
#[path = "keys_tests.rs"]
mod keys_tests;

/// Exact canonical uncompressed SEC1 P-256 public-key bytes.
pub const KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1: usize = 65;
/// Exact canonical fixed-width P-256 ECDSA signature bytes.
pub const KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1: usize = 64;

/// Hardware-backed P-256 device key: a wallet payment key or a scheme signer key.
///
/// The wire value is exactly one canonical uncompressed SEC1 NIST P-256 point
/// (`0x04 || x || y`). There is no algorithm tag or selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[derive(DeriveJsonSerialize, DeriveJsonDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaDevicePublicKeyV1"
)]
pub struct KagemushaDevicePublicKeyV1([u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]);

/// P-256 device signature.
///
/// The wire value is the fixed-width big-endian ECDSA scalar pair `r || s`.
/// Both scalars must be in `1..n`, and `s` must be low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[derive(DeriveJsonSerialize, DeriveJsonDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaDeviceSignatureV1"
)]
pub struct KagemushaDeviceSignatureV1([u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1]);

impl norito::SerializePayload for KagemushaDevicePublicKeyV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::DeserializePayload<'de> for KagemushaDevicePublicKeyV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("KAGEMUSHA device public key must be canonical SEC1 bytes")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaDevicePublicKeyV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let value = Self::from_sec1_bytes(raw)
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1))
    }
}

impl norito::SerializePayload for KagemushaDeviceSignatureV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.validate()
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        writer.write_all(&self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}

impl<'de> norito::DeserializePayload<'de> for KagemushaDeviceSignatureV1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("KAGEMUSHA device signature must be canonical raw P-256 bytes")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaDeviceSignatureV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        let raw = bytes
            .get(..KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1)
            .ok_or(norito::Error::LengthMismatch)?;
        let value =
            Self::from_raw_bytes(raw).map_err(|error| norito::Error::Message(error.to_string()))?;
        Ok((value, KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1))
    }
}

impl KagemushaDevicePublicKeyV1 {
    /// Parse the canonical uncompressed SEC1 P-256 encoding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaWalletValidationErrorV1::InvalidField`] for the wrong width,
    /// a compressed or invalid point, or a non-canonical encoding.
    pub fn from_sec1_bytes(bytes: &[u8]) -> WalletResult<Self> {
        let raw: [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1] = bytes
            .try_into()
            .map_err(|_| invalid_v1("device_public_key"))?;
        if raw[0] != 0x04 {
            return Err(invalid_v1("device_public_key"));
        }
        let verifying_key =
            P256VerifyingKey::from_sec1_bytes(&raw).map_err(|_| invalid_v1("device_public_key"))?;
        if verifying_key.to_encoded_point(false).as_bytes() != raw {
            return Err(invalid_v1("device_public_key"));
        }
        Ok(Self(raw))
    }

    /// Validate a value obtained through a raw Norito or JSON decoder.
    ///
    /// # Errors
    ///
    /// Returns an error unless the key is one canonical uncompressed P-256 point.
    pub fn validate(&self) -> WalletResult<()> {
        Self::from_sec1_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical uncompressed SEC1 bytes.
    #[must_use]
    pub const fn as_sec1_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1] {
        &self.0
    }

    fn verifying_key(&self) -> WalletResult<P256VerifyingKey> {
        self.validate()?;
        P256VerifyingKey::from_sec1_bytes(&self.0).map_err(|_| invalid_v1("device_public_key"))
    }
}

impl TryFrom<&[u8]> for KagemushaDevicePublicKeyV1 {
    type Error = KagemushaWalletValidationErrorV1;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(value)
    }
}

impl TryFrom<[u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1]> for KagemushaDevicePublicKeyV1 {
    type Error = KagemushaWalletValidationErrorV1;

    fn try_from(
        value: [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V1],
    ) -> Result<Self, Self::Error> {
        Self::from_sec1_bytes(&value)
    }
}

impl AsRef<[u8]> for KagemushaDevicePublicKeyV1 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl KagemushaDeviceSignatureV1 {
    /// Parse a canonical DER ECDSA signature and normalize its S scalar to the low form.
    ///
    /// Android `KeyMint` commonly returns DER from `SHA256withECDSA`; ECDSA's `(r, n - s)`
    /// equivalent is normalized before the fixed-width low-S signature enters Norito.
    /// This conversion is cryptographic framing only, not evidence of hardware enforcement.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaWalletValidationErrorV1::InvalidField`] for malformed or
    /// non-canonical DER or an invalid P-256 signature.
    pub fn from_der_normalizing_low_s(bytes: &[u8]) -> WalletResult<Self> {
        if !(8..=72).contains(&bytes.len()) {
            return Err(invalid_v1("device_signature"));
        }
        let parsed = P256Signature::from_der(bytes).map_err(|_| invalid_v1("device_signature"))?;
        if parsed.to_der().as_bytes() != bytes {
            return Err(invalid_v1("device_signature"));
        }
        let canonical = parsed.normalize_s().unwrap_or(parsed);
        Self::from_raw_bytes(canonical.to_bytes().as_ref())
    }

    /// Parse a canonical fixed-width low-S P-256 ECDSA signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaWalletValidationErrorV1::InvalidField`] for the wrong width,
    /// invalid scalars, or a high-S signature.
    pub fn from_raw_bytes(bytes: &[u8]) -> WalletResult<Self> {
        let raw: [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1] = bytes
            .try_into()
            .map_err(|_| invalid_v1("device_signature"))?;
        let signature =
            P256Signature::from_slice(&raw).map_err(|_| invalid_v1("device_signature"))?;
        if signature.normalize_s().is_some() {
            return Err(invalid_v1("device_signature"));
        }
        Ok(Self(raw))
    }

    /// Validate a value obtained through a raw Norito or JSON decoder.
    ///
    /// # Errors
    ///
    /// Returns an error unless the signature is fixed-width and low-S.
    pub fn validate(&self) -> WalletResult<()> {
        Self::from_raw_bytes(&self.0).map(|_| ())
    }

    /// Return the canonical fixed-width `r || s` bytes.
    #[must_use]
    pub const fn as_raw_bytes(&self) -> &[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1] {
        &self.0
    }

    /// Verify ECDSA-P256-SHA256 over `message` under `public_key`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaWalletValidationErrorV1::InvalidField`] when the key,
    /// signature, or authentication is invalid.
    pub fn verify(
        &self,
        public_key: &KagemushaDevicePublicKeyV1,
        message: &[u8],
    ) -> WalletResult<()> {
        self.validate()?;
        let signature =
            P256Signature::from_slice(&self.0).map_err(|_| invalid_v1("device_signature"))?;
        public_key
            .verifying_key()?
            .verify(message, &signature)
            .map_err(|_| invalid_v1("device_signature"))
    }
}

impl TryFrom<&[u8]> for KagemushaDeviceSignatureV1 {
    type Error = KagemushaWalletValidationErrorV1;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(value)
    }
}

impl TryFrom<[u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1]> for KagemushaDeviceSignatureV1 {
    type Error = KagemushaWalletValidationErrorV1;

    fn try_from(value: [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V1]) -> Result<Self, Self::Error> {
        Self::from_raw_bytes(&value)
    }
}

impl AsRef<[u8]> for KagemushaDeviceSignatureV1 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

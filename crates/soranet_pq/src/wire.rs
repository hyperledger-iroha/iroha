//! Backend-independent post-quantum wire parameters.

use core::fmt;

/// Error returned when backend-independent ML-DSA framing is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsupportedPqcError(&'static str);

impl fmt::Display for UnsupportedPqcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for UnsupportedPqcError {}

/// Supported ML-DSA parameter sets and their canonical wire widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlDsaSuite {
    /// ML-DSA-44.
    MlDsa44,
    /// ML-DSA-65.
    MlDsa65,
    /// ML-DSA-87.
    MlDsa87,
}

impl MlDsaSuite {
    /// Return the numeric identifier used on the FFI surface.
    #[must_use]
    pub const fn suite_id(self) -> u8 {
        match self {
            Self::MlDsa44 => 0,
            Self::MlDsa65 => 1,
            Self::MlDsa87 => 2,
        }
    }

    /// Parse a suite from its stable numeric identifier.
    #[must_use]
    pub const fn from_suite_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::MlDsa44),
            1 => Some(Self::MlDsa65),
            2 => Some(Self::MlDsa87),
            _ => None,
        }
    }

    /// Return the public-key width in bytes.
    #[must_use]
    pub const fn public_key_len(self) -> usize {
        match self {
            Self::MlDsa44 => 1_312,
            Self::MlDsa65 => 1_952,
            Self::MlDsa87 => 2_592,
        }
    }

    /// Return the secret-key width in bytes.
    #[must_use]
    pub const fn secret_key_len(self) -> usize {
        match self {
            Self::MlDsa44 => 2_560,
            Self::MlDsa65 => 4_032,
            Self::MlDsa87 => 4_896,
        }
    }

    /// Return the detached-signature width in bytes.
    #[must_use]
    pub const fn signature_len(self) -> usize {
        match self {
            Self::MlDsa44 => 2_420,
            Self::MlDsa65 => 3_309,
            Self::MlDsa87 => 4_627,
        }
    }

    /// Validate backend-independent public-key framing.
    ///
    /// # Errors
    /// Returns an error for the wrong width or inert all-zero material.
    pub fn validate_public_key(self, bytes: &[u8]) -> Result<(), UnsupportedPqcError> {
        validate_material(bytes, self.public_key_len(), "invalid ML-DSA public key")
    }

    /// Validate backend-independent detached-signature framing.
    ///
    /// # Errors
    /// Returns an error for the wrong width or inert all-zero material.
    pub fn validate_signature(self, bytes: &[u8]) -> Result<(), UnsupportedPqcError> {
        validate_material(bytes, self.signature_len(), "invalid ML-DSA signature")
    }
}

fn validate_material(
    bytes: &[u8],
    expected_len: usize,
    message: &'static str,
) -> Result<(), UnsupportedPqcError> {
    if bytes.len() != expected_len || bytes.iter().all(|byte| *byte == 0) {
        return Err(UnsupportedPqcError(message));
    }
    Ok(())
}

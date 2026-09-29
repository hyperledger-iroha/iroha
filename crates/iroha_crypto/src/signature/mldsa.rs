//! Borrowed ML-DSA envelopes with the existing native verification relation.

use crate::{Error, ML_DSA_65_PUBLIC_KEY_BYTES, error::ParseError};

/// Fixed key-validation diagnostics; ordinary API adapters retain their exact text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyRejection {
    Length,
    AllZero,
    #[cfg(feature = "pqc")]
    Encoding,
}
impl KeyRejection {
    pub(crate) fn into_parse_error(self) -> ParseError {
        ParseError(
            match self {
                Self::Length => "invalid ML-DSA public key length",
                Self::AllZero => "invalid ML-DSA public key: all-zero material",
                #[cfg(feature = "pqc")]
                Self::Encoding => "invalid ML-DSA public key",
            }
            .to_owned(),
        )
    }
}

/// Fixed verification rejection; no allocation is needed to retain a rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rejection {
    Key(KeyRejection),
    Signature,
}
impl Rejection {
    pub(crate) fn into_error(self) -> Error {
        match self {
            Self::Key(reason) => Error::Parse(reason.into_parse_error()),
            Self::Signature => Error::BadSignature,
        }
    }
}

pub(crate) fn validate_public_key(bytes: &[u8]) -> Result<(), KeyRejection> {
    if bytes.len() != ML_DSA_65_PUBLIC_KEY_BYTES {
        return Err(KeyRejection::Length);
    }
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(KeyRejection::AllZero);
    }
    #[cfg(feature = "pqc")]
    {
        use pqcrypto_traits::sign::PublicKey as _;
        pqcrypto_mldsa::mldsa65::PublicKey::from_bytes(bytes)
            .map_err(|_| KeyRejection::Encoding)?;
    }
    Ok(())
}

/// Check the public key before the signature, preserving ordinary API precedence.
/// PQClean's pinned contexts and polynomial buffers are inline; no retained heap-backed key is made.
pub(crate) fn verify(key: &[u8], signature: &[u8], message: &[u8]) -> Result<(), Rejection> {
    validate_public_key(key).map_err(Rejection::Key)?;
    #[cfg(feature = "pqc")]
    {
        use pqcrypto_mldsa::mldsa65;
        use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};
        if signature.len() != mldsa65::signature_bytes() || signature.iter().all(|byte| *byte == 0)
        {
            return Err(Rejection::Signature);
        }
        let signature =
            mldsa65::DetachedSignature::from_bytes(signature).map_err(|_| Rejection::Signature)?;
        let key = mldsa65::PublicKey::from_bytes(key).map_err(|_| Rejection::Signature)?;
        crate::verify_mldsa65_detached(&signature, message, &key).map_err(|_| Rejection::Signature)
    }
    #[cfg(not(feature = "pqc"))]
    {
        let _ = (signature, message);
        Err(Rejection::Signature)
    }
}

#[cfg(test)]
#[path = "mldsa_tests.rs"]
mod tests;

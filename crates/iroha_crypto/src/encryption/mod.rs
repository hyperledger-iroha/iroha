//! A suite of Authenticated Encryption with Associated Data (AEAD) cryptographic ciphers.
//!
//! Each AEAD algorithm provides [`crate::encryption::SymmetricEncryptor::encrypt_easy`] and
//! [`crate::encryption::SymmetricEncryptor::decrypt_easy`] methods which hide the complexity
//! of generating a secure nonce of appropriate size with the ciphertext.
//! The [`crate::encryption::SymmetricEncryptor::encrypt_easy`] prepends the nonce to the front of the ciphertext and
//! [`crate::encryption::SymmetricEncryptor::decrypt_easy`] expects
//! the nonce to be prepended to the front of the ciphertext.
//!
//! More advanced users may use [`crate::encryption::SymmetricEncryptor::encrypt`] and
//! [`crate::encryption::SymmetricEncryptor::decrypt`] directly. These two methods require the
//! caller to supply a nonce with sufficient entropy and should never be reused when encrypting
//! with the same `key`.
//!
//! The convenience struct [`crate::encryption::SymmetricEncryptor`] exists to allow users to easily
//! switch between algorithms by using any algorithm that implements [`aead::Aead`] and [`aead::KeyInit`].
//!
//! [`crate::encryption::ChaCha20Poly1305`] is the only algorithm currently supported,
//! as it is the only one used by the Iroha p2p transport protocol.

use std::{convert::TryFrom, mem, vec::Vec};

use aead::{Aead, AeadCore, AeadInOut, KeyInit, Payload};
pub use chacha20poly1305::ChaCha20Poly1305;
use displaydoc::Display;
use rand::rngs::OsRng;
use rand_core::{OsError, TryRngCore};

use crate::SessionKey;

/// An error that can occur during encryption or decryption
#[derive(thiserror::Error, Display, Debug, Clone, Copy)]
pub enum Error {
    /// Failed to generate nonce for an encryption operation
    NonceGeneration(#[source] OsError),
    /// Failed to encrypt data
    Encryption(aead::Error),
    /// Failed to decrypt data
    Decryption(aead::Error),
    /// Not enough data to decrypt message
    NotEnoughData,
    /// Provided key material has an invalid length
    InvalidKeyLength,
}

fn random_nonce<E: AeadCore>() -> Result<aead::Nonce<E>, Error> {
    let mut value = aead::Nonce::<E>::default();
    OsRng
        .try_fill_bytes(value.as_mut_slice())
        .map_err(Error::NonceGeneration)?;
    Ok(value)
}

fn nonce_from_slice<E: AeadCore>(bytes: &[u8]) -> Result<aead::Nonce<E>, Error> {
    aead::Nonce::<E>::try_from(bytes).map_err(|_| Error::NotEnoughData)
}

/// Helper wrapper around an [`Aead`] implementation that provides convenience
/// helpers for nonce management and envelope layout.
#[derive(Debug, Clone)]
pub struct SymmetricEncryptor<E>
where
    E: Aead + KeyInit,
{
    encryptor: E,
}

impl<E> SymmetricEncryptor<E>
where
    E: Aead + KeyInit,
{
    /// Create a new [`SymmetricEncryptor`] using the provided `encryptor`
    pub fn new(encryptor: E) -> Self {
        Self { encryptor }
    }

    /// Create a new [`SymmetricEncryptor`] from a [`SessionKey`]
    ///
    /// # Errors
    /// Returns [`Error::InvalidKeyLength`] when the session key payload does not match the
    /// expected size for the underlying algorithm.
    pub fn new_from_session_key(key: &SessionKey) -> Result<Self, Error> {
        let encryptor = E::new_from_slice(key.payload()).map_err(|_| Error::InvalidKeyLength)?;
        Ok(Self::new(encryptor))
    }

    /// Create a new [`SymmetricEncryptor`] from key bytes
    ///
    /// # Errors
    /// Returns [`Error::InvalidKeyLength`] when the provided key bytes do not have the correct
    /// length for the algorithm.
    pub fn new_with_key<A: AsRef<[u8]>>(key: A) -> Result<Self, Error> {
        let encryptor = E::new_from_slice(key.as_ref()).map_err(|_| Error::InvalidKeyLength)?;
        Ok(Self::new(encryptor))
    }

    /// Encrypt `plaintext` and integrity protect `aad`. The result is the ciphertext.
    /// This method handles safely generating a `nonce` and prepends it to the ciphertext.
    ///
    /// # Errors
    /// Returns [`Error::NonceGeneration`] if random nonce generation fails or [`Error::Encryption`]
    /// if the cipher rejects the payload.
    pub fn encrypt_easy<A: AsRef<[u8]>>(&self, aad: A, plaintext: A) -> Result<Vec<u8>, Error> {
        let nonce = random_nonce::<E>()?;
        let ciphertext = self
            .encryptor
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.as_ref(),
                    aad: aad.as_ref(),
                },
            )
            .map_err(Error::Encryption)?;

        let nonce_bytes: &[u8] = nonce.as_ref();
        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        result.extend_from_slice(nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Encrypt `plaintext` and integrity protect `aad` into the provided buffer.
    ///
    /// The output buffer is cleared and reused to store the full envelope:
    /// `nonce || ciphertext || tag`. This avoids per-message allocations in hot paths
    /// such as P2P transport.
    ///
    /// # Errors
    /// Returns [`Error::NonceGeneration`] if random nonce generation fails or [`Error::Encryption`]
    /// if the cipher rejects the payload.
    pub fn encrypt_easy_into<'a, A: AsRef<[u8]>>(
        &self,
        aad: A,
        plaintext: A,
        out: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], Error>
    where
        E: AeadInOut,
    {
        let nonce = random_nonce::<E>()?;
        let nonce_bytes: &[u8] = nonce.as_ref();
        let nonce_len = nonce_bytes.len();

        out.clear();
        out.extend_from_slice(plaintext.as_ref());
        self.encryptor
            .encrypt_in_place(&nonce, aad.as_ref(), out)
            .map_err(Error::Encryption)?;

        // Prefix the nonce by shifting ciphertext bytes to the right.
        let ciphertext_len = out.len();
        out.reserve(nonce_len);
        out.resize(ciphertext_len + nonce_len, 0);
        out.copy_within(0..ciphertext_len, nonce_len);
        out[..nonce_len].copy_from_slice(nonce_bytes);
        Ok(out.as_slice())
    }

    /// Encrypt `plaintext` and integrity protect `aad` using the provided `nonce`.
    ///
    /// # Errors
    /// Returns [`Error::NotEnoughData`] when the supplied nonce does not match the algorithm
    /// requirements or [`Error::Encryption`] when encryption fails.
    pub fn encrypt<A: AsRef<[u8]>>(
        &self,
        nonce: A,
        aad: A,
        plaintext: A,
    ) -> Result<Vec<u8>, Error> {
        let nonce = nonce_from_slice::<E>(nonce.as_ref())?;
        self.encryptor
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.as_ref(),
                    aad: aad.as_ref(),
                },
            )
            .map_err(Error::Encryption)
    }

    /// Decrypt `ciphertext` using integrity protected `aad`. The result is the plaintext if successful
    /// or an error if the `ciphertext` cannot be decrypted due to tampering, an incorrect `aad` value,
    /// or incorrect key. `aad` must be the same value used in `encrypt_easy`.
    ///
    /// # Errors
    /// Returns [`Error::NotEnoughData`] when the ciphertext does not contain a nonce and tag or
    /// [`Error::Decryption`] when authentication fails.
    pub fn decrypt_easy<A: AsRef<[u8]>>(&self, aad: A, ciphertext: A) -> Result<Vec<u8>, Error> {
        let data = ciphertext.as_ref();
        let nonce_len = mem::size_of::<aead::Nonce<E>>();
        let tag_len = mem::size_of::<aead::Tag<E>>();
        if data.len() < nonce_len + tag_len {
            return Err(Error::NotEnoughData);
        }
        let (nonce_bytes, ciphertext_bytes) = data.split_at(nonce_len);
        let nonce = nonce_from_slice::<E>(nonce_bytes)?;
        self.decrypt_with_nonce(&nonce, aad.as_ref(), ciphertext_bytes)
    }

    /// Decrypt `ciphertext` using integrity protected `aad` into the provided buffer.
    ///
    /// The output buffer is cleared and reused to store the plaintext to avoid per-message
    /// allocations in hot paths such as P2P message decode.
    ///
    /// # Errors
    /// Returns [`Error::NotEnoughData`] when the ciphertext does not contain a nonce and tag or
    /// [`Error::Decryption`] when authentication fails.
    pub fn decrypt_easy_into<'a, A: AsRef<[u8]>>(
        &self,
        aad: A,
        ciphertext: A,
        out: &'a mut Vec<u8>,
    ) -> Result<&'a [u8], Error>
    where
        E: AeadInOut,
    {
        let data = ciphertext.as_ref();
        let nonce_len = mem::size_of::<aead::Nonce<E>>();
        let tag_len = mem::size_of::<aead::Tag<E>>();
        if data.len() < nonce_len + tag_len {
            return Err(Error::NotEnoughData);
        }
        let (nonce_bytes, ciphertext_bytes) = data.split_at(nonce_len);
        let nonce = nonce_from_slice::<E>(nonce_bytes)?;
        out.clear();
        out.extend_from_slice(ciphertext_bytes);
        self.encryptor
            .decrypt_in_place(&nonce, aad.as_ref(), out)
            .map_err(Error::Decryption)?;
        Ok(out.as_slice())
    }

    /// Decrypt `ciphertext` using integrity protected `aad` and the provided `nonce`.
    ///
    /// # Errors
    /// Returns [`Error::NotEnoughData`] when the nonce size is invalid or [`Error::Decryption`]
    /// when authentication fails.
    pub fn decrypt<A: AsRef<[u8]>>(
        &self,
        nonce: A,
        aad: A,
        ciphertext: A,
    ) -> Result<Vec<u8>, Error> {
        let nonce = nonce_from_slice::<E>(nonce.as_ref())?;
        self.decrypt_with_nonce(&nonce, aad.as_ref(), ciphertext.as_ref())
    }

    fn decrypt_with_nonce(
        &self,
        nonce: &aead::Nonce<E>,
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.encryptor
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(Error::Decryption)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encryptor() -> SymmetricEncryptor<ChaCha20Poly1305> {
        SymmetricEncryptor::new_with_key((0u8..32).collect::<Vec<_>>()).expect("valid key length")
    }

    #[test]
    fn encrypt_easy_roundtrip() {
        let encryptor = encryptor();
        let aad = b"Iroha2";
        let message = b"Hello and Goodbye!";
        let ciphertext = encryptor
            .encrypt_easy(aad.as_ref(), message.as_ref())
            .expect("encryption");
        let plaintext = encryptor
            .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
            .expect("decryption");
        assert_eq!(plaintext.as_slice(), message);
    }

    #[test]
    fn encrypt_with_nonce_roundtrip() {
        let encryptor = encryptor();
        let nonce = random_nonce::<ChaCha20Poly1305>().expect("nonce");
        let aad = b"encrypt test";
        let message = b"Hello and Goodbye!";
        let ciphertext = encryptor
            .encrypt(nonce.as_ref(), aad.as_ref(), message.as_ref())
            .expect("encrypt");
        let plaintext = encryptor
            .decrypt(nonce.as_ref(), aad.as_ref(), ciphertext.as_slice())
            .expect("decrypt");
        assert_eq!(plaintext.as_slice(), message);
    }

    #[test]
    fn decrypt_should_fail_with_tampering() {
        let encryptor = encryptor();
        let aad = b"decrypt should fail";
        let message = b"Hello and Goodbye!";
        let mut ciphertext = encryptor
            .encrypt_easy(aad.as_ref(), message.as_ref())
            .expect("encrypt");

        // wrong AAD fails
        assert!(
            encryptor
                .decrypt_easy(b"different aad".as_ref(), ciphertext.as_slice())
                .is_err()
        );

        // tamper with ciphertext
        ciphertext[0] ^= 0x01;
        assert!(
            encryptor
                .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
                .is_err()
        );
    }

    #[test]
    fn decrypts_empty_message() {
        let encryptor = encryptor();
        let aad = b"Iroha2";
        let ciphertext = encryptor
            .encrypt_easy(aad.as_ref(), b"".as_ref())
            .expect("encrypt");
        let plaintext = encryptor
            .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
            .expect("decrypt");
        assert!(plaintext.is_empty());
    }

    #[test]
    fn decrypt_easy_into_roundtrip() {
        let encryptor = encryptor();
        let aad = b"Iroha2";
        let message = b"Decrypt into buffer!";
        let ciphertext = encryptor
            .encrypt_easy(aad.as_ref(), message.as_ref())
            .expect("encrypt");
        let mut out = Vec::new();
        let plaintext = encryptor
            .decrypt_easy_into(aad.as_ref(), ciphertext.as_slice(), &mut out)
            .expect("decrypt");
        assert_eq!(plaintext, message);
        // Reuse the buffer for a different message length.
        let message2 = b"Short";
        let ciphertext2 = encryptor
            .encrypt_easy(aad.as_ref(), message2.as_ref())
            .expect("encrypt");
        let plaintext2 = encryptor
            .decrypt_easy_into(aad.as_ref(), ciphertext2.as_slice(), &mut out)
            .expect("decrypt");
        assert_eq!(plaintext2, message2);
    }

    #[test]
    fn encrypt_easy_into_roundtrip() {
        let encryptor = encryptor();
        let aad = b"Iroha2";
        let message = b"Encrypt into buffer!";

        let mut out = Vec::new();
        let ciphertext = encryptor
            .encrypt_easy_into(aad.as_ref(), message.as_ref(), &mut out)
            .expect("encrypt");
        let plaintext = encryptor
            .decrypt_easy(aad.as_ref(), ciphertext)
            .expect("decrypt");
        assert_eq!(plaintext.as_slice(), message);
    }
}

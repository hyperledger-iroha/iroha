#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use aead::{
    generic_array::{
        typenum::{U0, U12, U16, U28, U32},
        GenericArray,
    },
    Aead, AeadCore, Error, KeyInit, KeySizeUser, Payload,
};
use chacha20poly1305::ChaCha20Poly1305 as SysChaCha20Poly1305;

use super::Encryptor;

/// `ChaCha20Poly1305` is a symmetric encryption algorithm that uses the `ChaCha20` stream cipher
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ChaCha20Poly1305 {
    key: GenericArray<u8, U32>,
}

impl Encryptor for ChaCha20Poly1305 {
    type MinSize = U28;
}

impl KeySizeUser for ChaCha20Poly1305 {
    type KeySize = U32;
}

impl KeyInit for ChaCha20Poly1305 {
    fn new(key: &GenericArray<u8, Self::KeySize>) -> Self {
        Self { key: *key }
    }
}

impl AeadCore for ChaCha20Poly1305 {
    type NonceSize = U12;
    type TagSize = U16;
    type CiphertextOverhead = U0;
}

// false positives: eliding lifetimes here requires an unstable feature `anonymous_lifetime_in_impl_trait`
#[allow(single_use_lifetimes)]
impl Aead for ChaCha20Poly1305 {
    fn encrypt<'msg, 'aad>(
        &self,
        nonce: &GenericArray<u8, Self::NonceSize>,
        plaintext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error> {
        let aead = SysChaCha20Poly1305::new(&self.key);
        let ciphertext = aead.encrypt(nonce, plaintext)?;
        Ok(ciphertext)
    }

    fn decrypt<'msg, 'aad>(
        &self,
        nonce: &GenericArray<u8, Self::NonceSize>,
        ciphertext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error> {
        let aead = SysChaCha20Poly1305::new(&self.key);
        let plaintext = aead.decrypt(nonce, ciphertext)?;
        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_easy_works() {
        let cipher = ChaCha20Poly1305::new(&ChaCha20Poly1305::key_gen().unwrap());
        let aad = Vec::new();
        let message = b"Hello and Goodbye!".to_vec();
        let ciphertext = cipher.encrypt_easy(&aad, &message).unwrap();
        let decrypted_message = cipher.decrypt_easy(&aad, &ciphertext).unwrap();
        assert_eq!(message, decrypted_message);
    }

    #[test]
    fn encrypt_works() {
        let cipher = ChaCha20Poly1305::new(&ChaCha20Poly1305::key_gen().unwrap());
        let nonce = ChaCha20Poly1305::nonce_gen().unwrap();
        let aad = b"encrypt test".to_vec();
        let message = b"Hello and Goodbye!".to_vec();
        let payload = Payload {
            msg: message.as_slice(),
            aad: aad.as_slice(),
        };
        let ciphertext = cipher.encrypt(&nonce, payload).unwrap();
        let payload = Payload {
            msg: ciphertext.as_slice(),
            aad: aad.as_slice(),
        };
        let decrypted_message = cipher.decrypt(&nonce, payload).unwrap();
        assert_eq!(message, decrypted_message);
    }

    #[test]
    fn decrypt_should_fail() {
        let cipher = ChaCha20Poly1305::new(&ChaCha20Poly1305::key_gen().unwrap());
        let aad = b"decrypt should fail".to_vec();
        let message = b"Hello and Goodbye!".to_vec();
        let res = cipher.encrypt_easy(&aad, &message);
        let mut ciphertext = res.unwrap();

        let aad = b"decrypt should succeed".to_vec();
        // decrypt should fail because if mismatched aad
        cipher.decrypt_easy(&aad, &ciphertext).unwrap_err();

        let aad = b"decrypt should fail".to_vec();
        ciphertext[0] ^= 0x01;
        // decrypt should fail because of tampered ciphertext
        cipher.decrypt_easy(&aad, &ciphertext).unwrap_err();
    }

    #[test]
    fn decrypting_empty_message_should_work() {
        use aead::generic_array::typenum::Unsigned as _;

        let cipher = ChaCha20Poly1305::new(&ChaCha20Poly1305::key_gen().unwrap());
        let aad = b"Iroha2 AAD".to_vec();
        // zero length message, encodes to the message of minimum length (28)
        let message = b"".to_vec();
        let ciphertext = cipher.encrypt_easy(&aad, &message).unwrap();

        assert_eq!(
            ciphertext.len(),
            <ChaCha20Poly1305 as Encryptor>::MinSize::to_usize()
        );

        let decrypted_message = cipher.decrypt_easy(&aad, &ciphertext).unwrap();
        assert_eq!(message, decrypted_message);
    }

    // TODO: this should be tested for, but only after we integrate with secrecy/zeroize
    // #[test]
    // fn zeroed_on_drop() {
    //     let mut aes = ChaCha20Poly1305::new(&ChaCha20Poly1305::key_gen().unwrap());
    //     aes.zeroize();
    //
    //     fn as_bytes<T>(x: &T) -> &[u8] {
    //         use std::{mem, slice};
    //
    //         unsafe { slice::from_raw_parts(x as *const T as *const u8, mem::size_of_val(x)) }
    //     }
    //
    //     assert!(as_bytes(&aes.key).iter().all(|b| *b == 0u8));
    // }
}

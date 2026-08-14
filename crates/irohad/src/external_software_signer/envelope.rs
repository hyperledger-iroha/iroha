//! AEAD key-envelope handling with explicit in-memory secret scrubbing.
use std::fmt;
use iroha_crypto::{
    KeyPair, PrivateKey,
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
};
use norito::codec::{Decode, Encode};
use super::protocol::{
    SIGNER_KEY_MAGIC_V1, SIGNER_MAX_PRIVATE_KEY_BYTES_V1, SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1,
    SIGNER_PROTOCOL_VERSION_V1, SoftwareSignerKeyAlgorithmV1, SoftwareSignerPurposeBindingV1,
    SoftwareSignerRoleV1, digest_canonical, digest_parts, public_key_digest, scrub, valid_identity,
    valid_software_signer_handle,
};
const KEY_ENVELOPE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.external-signer.key-envelope.v1";
const KEY_ENVELOPE_AAD_DOMAIN_V1: &[u8] = b"iroha.external-signer.key-envelope.aad.v1";
const KEY_ENVELOPE_KEK_DOMAIN_V1: &[u8] = b"iroha.external-signer.key-envelope.kek.v1";
const KEY_ENVELOPE_MAX_CIPHERTEXT_BYTES_V1: usize = SIGNER_MAX_PRIVATE_KEY_BYTES_V1 + 1024;
/// Public, authenticated metadata for one encrypted signer key generation.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(super) struct SoftwareSignerKeyEnvelopeAadV1 {
    pub backend: super::protocol::ExternalSignerBackendV1,
    pub handle: String,
    pub service_id: String,
    pub administrator_id: String,
    pub service_uid: u32,
    pub client_uid: u32,
    pub administrator_uid: u32,
    pub role: SoftwareSignerRoleV1,
    pub purpose_binding: SoftwareSignerPurposeBindingV1,
    pub domain: String,
    pub algorithm: SoftwareSignerKeyAlgorithmV1,
    pub key_revision: u64,
    pub policy_revision: u64,
    pub policy_digest: [u8; 32],
    pub public_key: iroha_crypto::PublicKey,
    pub public_key_digest: [u8; 32],
    pub max_request_bytes: u32,
}
impl SoftwareSignerKeyEnvelopeAadV1 {
    pub(super) fn validate(&self) -> Result<(), SoftwareSignerEnvelopeErrorV1> {
        if self.backend != super::protocol::ExternalSignerBackendV1::Software
            || !valid_identity(&self.service_id)
            || !valid_identity(&self.administrator_id)
            || self.service_id == self.administrator_id
            || self.service_uid == self.client_uid
            || self.service_uid == self.administrator_uid
            || self.client_uid == self.administrator_uid
            || !self.purpose_binding.validates_role(self.role)
            || self.domain != self.role.domain()
            || self.key_revision == 0
            || self.policy_revision == 0
            || self.policy_digest == [0; 32]
            || self.max_request_bytes == 0
            || usize::try_from(self.max_request_bytes)
                .ok()
                .is_none_or(|limit| limit > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1)
            || self
                .public_key
                .try_algorithm()
                .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?
                != self.algorithm.algorithm()
            || !self.role.allows_algorithm(self.algorithm)
            || public_key_digest(&self.public_key)
                .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?
                != self.public_key_digest
            || !valid_software_signer_handle(self.role, &self.handle)
        {
            return Err(SoftwareSignerEnvelopeErrorV1::Invalid);
        }
        Ok(())
    }
}
/// Versioned ChaCha20-Poly1305 envelope for one software signing key.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SoftwareSignerKeyEnvelopeV1 {
    /// Exact key-envelope marker.
    pub magic: [u8; 8],
    /// Exact key-envelope version.
    pub version: u16,
    /// Public metadata authenticated as AEAD associated data.
    pub(super) aad: SoftwareSignerKeyEnvelopeAadV1,
    /// `nonce || ciphertext || tag` from the workspace AEAD implementation.
    pub ciphertext: Vec<u8>,
    /// Domain-separated digest covering metadata and ciphertext.
    pub envelope_digest: [u8; 32],
}
impl fmt::Debug for SoftwareSignerKeyEnvelopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoftwareSignerKeyEnvelopeV1")
            .field("version", &self.version)
            .field("role", &self.aad.role)
            .field("key_revision", &self.aad.key_revision)
            .field(
                "public_key_digest",
                &hex::encode(self.aad.public_key_digest),
            )
            .field("ciphertext", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
impl SoftwareSignerKeyEnvelopeV1 {
    pub(super) fn create(
        aad: SoftwareSignerKeyEnvelopeAadV1,
        keypair: &KeyPair,
        wrapping_key: &SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, SoftwareSignerEnvelopeErrorV1> {
        aad.validate()?;
        if keypair.algorithm() != aad.algorithm.algorithm()
            || keypair.public_key() != &aad.public_key
        {
            return Err(SoftwareSignerEnvelopeErrorV1::KeyMismatch);
        }
        let (algorithm, mut private_payload) = keypair
            .private_key()
            .try_to_bytes()
            .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?;
        if private_payload.is_empty() || private_payload.len() > SIGNER_MAX_PRIVATE_KEY_BYTES_V1 {
            scrub(&mut private_payload);
            return Err(SoftwareSignerEnvelopeErrorV1::Invalid);
        }
        let plaintext = SoftwareSignerPrivateKeyPlaintextV1 {
            magic: SIGNER_KEY_MAGIC_V1,
            version: SIGNER_PROTOCOL_VERSION_V1,
            algorithm: SoftwareSignerKeyAlgorithmV1::try_from(algorithm)
                .map_err(|_| SoftwareSignerEnvelopeErrorV1::UnsupportedAlgorithm)?,
            private_payload,
        };
        let mut encoded_plaintext = norito::encode_canonical(&plaintext)
            .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?;
        let aad_bytes = canonical_aad(&aad)?;
        let mut derived_key = wrapping_key.derive_key(&aad_bytes);
        let encryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(derived_key);
        scrub(&mut derived_key);
        let encryptor = encryptor.map_err(|_| SoftwareSignerEnvelopeErrorV1::Unavailable)?;
        let ciphertext = encryptor.encrypt_easy(aad_bytes.as_slice(), encoded_plaintext.as_slice());
        scrub(&mut encoded_plaintext);
        let ciphertext = ciphertext.map_err(|_| SoftwareSignerEnvelopeErrorV1::Unavailable)?;
        if ciphertext.is_empty() || ciphertext.len() > KEY_ENVELOPE_MAX_CIPHERTEXT_BYTES_V1 {
            return Err(SoftwareSignerEnvelopeErrorV1::Invalid);
        }
        let mut envelope = Self {
            magic: SIGNER_KEY_MAGIC_V1,
            version: SIGNER_PROTOCOL_VERSION_V1,
            aad,
            ciphertext,
            envelope_digest: [0; 32],
        };
        envelope.envelope_digest = envelope.compute_digest()?;
        Ok(envelope)
    }
    pub(super) fn open(
        &self,
        wrapping_key: &SoftwareSignerWrappingKeyV1,
    ) -> Result<KeyPair, SoftwareSignerEnvelopeErrorV1> {
        self.validate_public()?;
        let aad_bytes = canonical_aad(&self.aad)?;
        let mut derived_key = wrapping_key.derive_key(&aad_bytes);
        let decryptor = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(derived_key);
        scrub(&mut derived_key);
        let decryptor = decryptor.map_err(|_| SoftwareSignerEnvelopeErrorV1::Unavailable)?;
        let mut plaintext_bytes = decryptor
            .decrypt_easy(aad_bytes.as_slice(), self.ciphertext.as_slice())
            .map_err(|_| SoftwareSignerEnvelopeErrorV1::AuthenticationFailed)?;
        if plaintext_bytes.is_empty()
            || plaintext_bytes.len() > SIGNER_MAX_PRIVATE_KEY_BYTES_V1 + 512
        {
            scrub(&mut plaintext_bytes);
            return Err(SoftwareSignerEnvelopeErrorV1::Invalid);
        }
        let plaintext: SoftwareSignerPrivateKeyPlaintextV1 =
            norito::decode_canonical(&plaintext_bytes).map_err(|_| {
                scrub(&mut plaintext_bytes);
                SoftwareSignerEnvelopeErrorV1::Invalid
            })?;
        scrub(&mut plaintext_bytes);
        if plaintext.magic != SIGNER_KEY_MAGIC_V1
            || plaintext.version != SIGNER_PROTOCOL_VERSION_V1
            || plaintext.algorithm != self.aad.algorithm
            || plaintext.private_payload.is_empty()
            || plaintext.private_payload.len() > SIGNER_MAX_PRIVATE_KEY_BYTES_V1
        {
            return Err(SoftwareSignerEnvelopeErrorV1::Invalid);
        }
        let private_key = PrivateKey::from_bytes(
            plaintext.algorithm.algorithm(),
            plaintext.private_payload.as_slice(),
        )
        .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?;
        let keypair = KeyPair::from_private_key(private_key)
            .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?;
        if keypair.public_key() != &self.aad.public_key {
            return Err(SoftwareSignerEnvelopeErrorV1::KeyMismatch);
        }
        Ok(keypair)
    }
    pub(super) fn validate_public(&self) -> Result<(), SoftwareSignerEnvelopeErrorV1> {
        if self.magic != SIGNER_KEY_MAGIC_V1
            || self.version != SIGNER_PROTOCOL_VERSION_V1
            || self.ciphertext.is_empty()
            || self.ciphertext.len() > KEY_ENVELOPE_MAX_CIPHERTEXT_BYTES_V1
            || self.envelope_digest == [0; 32]
        {
            return Err(SoftwareSignerEnvelopeErrorV1::Invalid);
        }
        self.aad.validate()?;
        if self.compute_digest()? != self.envelope_digest {
            return Err(SoftwareSignerEnvelopeErrorV1::AuthenticationFailed);
        }
        Ok(())
    }
    pub(super) fn compute_digest(&self) -> Result<[u8; 32], SoftwareSignerEnvelopeErrorV1> {
        digest_canonical(
            KEY_ENVELOPE_DIGEST_DOMAIN_V1,
            &(
                self.magic,
                self.version,
                self.aad.clone(),
                self.ciphertext.clone(),
            ),
        )
        .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)
    }
    pub(super) const fn aad(&self) -> &SoftwareSignerKeyEnvelopeAadV1 {
        &self.aad
    }
}
#[derive(Decode, Encode)]
struct SoftwareSignerPrivateKeyPlaintextV1 {
    magic: [u8; 8],
    version: u16,
    algorithm: SoftwareSignerKeyAlgorithmV1,
    private_payload: Vec<u8>,
}
impl Drop for SoftwareSignerPrivateKeyPlaintextV1 {
    fn drop(&mut self) {
        scrub(&mut self.private_payload);
    }
}
/// Runtime-only 256-bit key used to open signer key envelopes.
pub struct SoftwareSignerWrappingKeyV1 {
    bytes: [u8; 32],
}
impl SoftwareSignerWrappingKeyV1 {
    /// Construct a wrapping key from exactly 32 runtime-supplied bytes.
    ///
    /// # Errors
    ///
    /// Rejects all-zero key material.
    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, SoftwareSignerEnvelopeErrorV1> {
        if bytes == [0; 32] {
            return Err(SoftwareSignerEnvelopeErrorV1::InvalidWrappingKey);
        }
        Ok(Self { bytes })
    }
    fn derive_key(&self, aad: &[u8]) -> [u8; 32] {
        let context = digest_parts(KEY_ENVELOPE_KEK_DOMAIN_V1, &[aad]);
        *blake3::keyed_hash(&self.bytes, &context).as_bytes()
    }
}
impl fmt::Debug for SoftwareSignerWrappingKeyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SoftwareSignerWrappingKeyV1([REDACTED])")
    }
}
impl Drop for SoftwareSignerWrappingKeyV1 {
    fn drop(&mut self) {
        scrub(&mut self.bytes);
    }
}
fn canonical_aad(
    aad: &SoftwareSignerKeyEnvelopeAadV1,
) -> Result<Vec<u8>, SoftwareSignerEnvelopeErrorV1> {
    aad.validate()?;
    let payload =
        norito::encode_canonical(aad).map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)?;
    let digest = digest_parts(KEY_ENVELOPE_AAD_DOMAIN_V1, &[&payload]);
    norito::encode_canonical(&(
        SIGNER_KEY_MAGIC_V1,
        SIGNER_PROTOCOL_VERSION_V1,
        digest,
        payload,
    ))
    .map_err(|_| SoftwareSignerEnvelopeErrorV1::Invalid)
}
/// Payload-free key-envelope failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareSignerEnvelopeErrorV1 {
    /// Envelope structure or public metadata is invalid.
    Invalid,
    /// Wrapping key is an inert all-zero value.
    InvalidWrappingKey,
    /// Only Ed25519 and ML-DSA are supported.
    UnsupportedAlgorithm,
    /// Public and private key material do not match.
    KeyMismatch,
    /// AEAD authentication or the outer digest failed.
    AuthenticationFailed,
    /// Cryptographic randomness or the AEAD implementation was unavailable.
    Unavailable,
}
#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use super::*;
    use crate::external_software_signer::protocol::ExternalSignerBackendV1;
    fn wrapping(byte: u8) -> SoftwareSignerWrappingKeyV1 {
        SoftwareSignerWrappingKeyV1::try_from_bytes([byte; 32]).expect("fixture wrapping key")
    }
    fn fixture() -> (KeyPair, SoftwareSignerKeyEnvelopeAadV1) {
        let keypair =
            KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519).expect("fixture signer key");
        let aad = SoftwareSignerKeyEnvelopeAadV1 {
            backend: ExternalSignerBackendV1::Software,
            handle: "software://sorafs/promotion/primary".to_owned(),
            service_id: "promotion-signer-primary".to_owned(),
            administrator_id: "release-security-primary".to_owned(),
            service_uid: 4101,
            client_uid: 4102,
            administrator_uid: 4103,
            role: SoftwareSignerRoleV1::Promotion,
            purpose_binding: SoftwareSignerPurposeBindingV1::NativeOrPromotion,
            domain: SoftwareSignerRoleV1::Promotion.domain().to_owned(),
            algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x42; 32],
            public_key: keypair.public_key().clone(),
            public_key_digest: public_key_digest(keypair.public_key()).expect("public key digest"),
            max_request_bytes: 4096,
        };
        (keypair, aad)
    }
    #[test]
    fn wrong_wrapping_key_aad_and_ciphertext_fail_aead_authentication() {
        let (keypair, aad) = fixture();
        let envelope = SoftwareSignerKeyEnvelopeV1::create(aad, &keypair, &wrapping(0x43))
            .expect("create fixture envelope");
        assert!(matches!(
            envelope.open(&wrapping(0x44)),
            Err(SoftwareSignerEnvelopeErrorV1::AuthenticationFailed)
        ));
        let mut wrong_aad = envelope.clone();
        wrong_aad.aad.policy_digest[0] ^= 1;
        wrong_aad.envelope_digest = wrong_aad.compute_digest().expect("outer digest");
        assert!(matches!(
            wrong_aad.open(&wrapping(0x43)),
            Err(SoftwareSignerEnvelopeErrorV1::AuthenticationFailed)
        ));
        let mut corrupt_ciphertext = envelope;
        corrupt_ciphertext.ciphertext[0] ^= 1;
        corrupt_ciphertext.envelope_digest =
            corrupt_ciphertext.compute_digest().expect("outer digest");
        assert!(matches!(
            corrupt_ciphertext.open(&wrapping(0x43)),
            Err(SoftwareSignerEnvelopeErrorV1::AuthenticationFailed)
        ));
    }
}

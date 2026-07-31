//! Privacy ticket payloads used by the `SoraNet` anonymity layer.
//!
//! The types defined here mirror the specification captured in
//! `specs/soranet/zk_ticket_plan.md`. They intentionally keep the schema
//! lean so Halo2 commitments can be layered on top without frequent changes.
//! The cryptographic proof plumbing (commitments, nullifier checks, Halo2
//! verification) lives in the host runtime and the `iroha_zkp_halo2` crate.

use iroha_crypto::{PrivateKey, PublicKey, Signature, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::Digest32;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{account::AccountId, metadata::Metadata};

const LEAF_TAG_BLINDED_CID: &[u8] = b"soranet.ticket.body.blinded_cid.v1";
const LEAF_TAG_SCOPE: &[u8] = b"soranet.ticket.body.scope.v1";
const LEAF_TAG_MAX_USES: &[u8] = b"soranet.ticket.body.max_uses.v1";
const LEAF_TAG_VALID_AFTER: &[u8] = b"soranet.ticket.body.valid_after.v1";
const LEAF_TAG_VALID_UNTIL: &[u8] = b"soranet.ticket.body.valid_until.v1";
const LEAF_TAG_ISSUER_ID: &[u8] = b"soranet.ticket.body.issuer_id.v1";
const LEAF_TAG_SALT_EPOCH: &[u8] = b"soranet.ticket.body.salt_epoch.v1";
const LEAF_TAG_POLICY_FLAGS: &[u8] = b"soranet.ticket.body.policy_flags.v1";
const LEAF_TAG_METADATA: &[u8] = b"soranet.ticket.body.metadata.v1";
const NODE_DOMAIN: &[u8] = b"soranet.ticket.body.node.v1";

/// Errors raised during ticket commitment verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TicketCommitmentError {
    /// Embedded commitment does not match the canonical Merkle commitment derived from the body.
    #[error("ticket commitment mismatch")]
    Mismatch {
        /// Commitment computed from the body fields.
        expected: Digest32,
        /// Commitment supplied within the envelope.
        actual: Digest32,
    },
}

/// Errors raised during ticket signature verification.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TicketSignatureError {
    /// The embedded ticket commitment does not match the body.
    #[error(transparent)]
    Commitment(#[from] TicketCommitmentError),
    /// Ticket issuer account is not single-signatory, so the embedded signature has no verifier.
    #[error("ticket issuer account has no single signatory")]
    MissingIssuerSignatory,
    /// The issuer signature is malformed or does not verify.
    #[error("ticket signature verification failed: {0}")]
    Signature(#[from] iroha_crypto::Error),
}

/// Ticket capability scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "scope", content = "value"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[allow(clippy::exhaustive_enums)]
pub enum TicketScopeV1 {
    /// Ticket permits read-only access.
    Read,
    /// Ticket permits write operations (e.g., upload, mutate).
    Write,
    /// Ticket grants administrative operations (e.g., revoke, delegate).
    Admin,
}

/// Canonical ticket payload describing the blinded CID and policy window.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct TicketBodyV1 {
    /// Blinded content identifier protected by the `SoraNet` salt schedule.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub blinded_cid: Digest32,
    /// Capability scope granted to the holder.
    pub scope: TicketScopeV1,
    /// Maximum number of times the ticket may be redeemed.
    pub max_uses: u16,
    /// Timestamp (seconds since Unix epoch) after which the ticket becomes valid.
    pub valid_after: u64,
    /// Timestamp (seconds since Unix epoch) when the ticket expires.
    pub valid_until: u64,
    /// Issuer account authorised to mint tickets.
    pub issuer_id: AccountId,
    /// Salt epoch expected by relays (ties ticket to blinded CID derivation).
    pub salt_epoch: u32,
    /// Policy flags (bitfield) controlling optional behaviours (e.g., deterministic nonce).
    pub policy_flags: u32,
    /// Optional metadata surfaced to relays/gateways (audience, region, etc.).
    #[norito(default)]
    pub metadata: Metadata,
}

impl TicketBodyV1 {
    /// Compute the canonical BLAKE3 Merkle commitment for this ticket body.
    #[must_use]
    pub fn compute_commitment(&self) -> Digest32 {
        compute_ticket_commitment(self)
    }
}

/// Ticket envelope bundling the body, cryptographic commitment, proof, and signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct TicketEnvelopeV1 {
    /// Canonical ticket body.
    pub body: TicketBodyV1,
    /// Commitment over ticket fields (exact hash computed in host runtime).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub commitment: Digest32,
    /// Halo2 proof bytes verifying the commitment/nullifier constraints.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub zk_proof: Vec<u8>,
    /// Issuer signature sealing the ticket body + commitment.
    pub signature: Signature,
    /// Nullifier used to detect replay; actual validation occurs host-side.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub nullifier: Digest32,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode)]
struct TicketSignaturePayloadV1 {
    body: TicketBodyV1,
    commitment: Digest32,
}

impl<'a> norito::core::DecodeFromSlice<'a> for TicketBodyV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for TicketEnvelopeV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

impl TicketEnvelopeV1 {
    /// Build and sign a ticket envelope with a commitment derived from `body`.
    ///
    /// # Errors
    ///
    /// Returns [`iroha_crypto::Error::Other`] when the body issuer does not match the signing key,
    /// or a signing error when the cryptographic backend rejects the private key.
    pub fn try_sign(
        body: TicketBodyV1,
        zk_proof: Vec<u8>,
        nullifier: Digest32,
        private_key: &PrivateKey,
    ) -> Result<Self, iroha_crypto::Error> {
        let issuer_public_key = PublicKey::from(private_key.clone());
        if body.issuer_id.try_signatory() != Some(&issuer_public_key) {
            return Err(iroha_crypto::Error::Other(
                "ticket issuer_id does not match signing private key".to_owned(),
            ));
        }
        let commitment = body.compute_commitment();
        let signature_payload = TicketSignaturePayloadV1 {
            body: body.clone(),
            commitment,
        };
        let signature = SignatureOf::try_new(private_key, &signature_payload)?.into();
        Ok(Self {
            body,
            commitment,
            zk_proof,
            signature,
            nullifier,
        })
    }

    /// Returns `true` if the ticket has expired relative to `timestamp`.
    #[must_use]
    pub fn is_expired(&self, timestamp: u64) -> bool {
        timestamp >= self.body.valid_until
    }

    /// Returns `true` if the ticket is not yet active at `timestamp`.
    #[must_use]
    pub fn is_not_yet_active(&self, timestamp: u64) -> bool {
        timestamp < self.body.valid_after
    }

    /// Returns a display hint describing the ticket scope.
    #[must_use]
    pub fn scope_label(&self) -> &'static str {
        match self.body.scope {
            TicketScopeV1::Read => "read",
            TicketScopeV1::Write => "write",
            TicketScopeV1::Admin => "admin",
        }
    }

    /// Verifies that the embedded commitment matches the canonical Merkle commitment over the body.
    ///
    /// This helper provides a lightweight host-side check that ensures the Halo2 proof payload can
    /// be validated against the ticket metadata without embedding the proving stack in the data
    /// model crate.
    ///
    /// # Errors
    ///
    /// Returns [`TicketCommitmentError::Mismatch`] when the commitment stored in the envelope does
    /// not match the value derived from the ticket body fields.
    pub fn verify_commitment(&self) -> Result<(), TicketCommitmentError> {
        let expected = self.body.compute_commitment();
        if expected == self.commitment {
            Ok(())
        } else {
            Err(TicketCommitmentError::Mismatch {
                expected,
                actual: self.commitment,
            })
        }
    }

    /// Returns `true` when the commitment embedded in the envelope is missing or does not match the
    /// canonical commitment derived from the body.
    #[must_use]
    pub fn commitment_is_placeholder(&self) -> bool {
        self.verify_commitment().is_err()
    }

    /// Verifies the issuer signature over the canonical body plus commitment payload.
    ///
    /// # Errors
    ///
    /// Returns [`TicketSignatureError`] when the commitment is stale, the issuer account is not
    /// single-signatory, the signature payload is malformed, or signature verification fails.
    pub fn verify_signature(&self) -> Result<(), TicketSignatureError> {
        self.verify_commitment()?;
        let issuer_public_key = self
            .body
            .issuer_id
            .try_signatory()
            .ok_or(TicketSignatureError::MissingIssuerSignatory)?;
        let signature =
            super::signature_for_public_key_algorithm(issuer_public_key, &self.signature)?;
        SignatureOf::<TicketSignaturePayloadV1>::from_signature(signature)
            .verify(issuer_public_key, &self.signature_payload())
            .map_err(TicketSignatureError::Signature)
    }

    fn signature_payload(&self) -> TicketSignaturePayloadV1 {
        TicketSignaturePayloadV1 {
            body: self.body.clone(),
            commitment: self.commitment,
        }
    }
}

fn compute_ticket_commitment(body: &TicketBodyV1) -> Digest32 {
    let mut leaves = Vec::with_capacity(9);

    leaves.push(hash_leaf(LEAF_TAG_BLINDED_CID, body.blinded_cid.as_slice()));

    let scope_bytes = body.scope.encode();
    leaves.push(hash_leaf(LEAF_TAG_SCOPE, scope_bytes.as_slice()));

    let max_uses_bytes = body.max_uses.to_le_bytes();
    leaves.push(hash_leaf(LEAF_TAG_MAX_USES, &max_uses_bytes));

    let valid_after_bytes = body.valid_after.to_le_bytes();
    leaves.push(hash_leaf(LEAF_TAG_VALID_AFTER, &valid_after_bytes));

    let valid_until_bytes = body.valid_until.to_le_bytes();
    leaves.push(hash_leaf(LEAF_TAG_VALID_UNTIL, &valid_until_bytes));

    let issuer_bytes = body.issuer_id.encode();
    leaves.push(hash_leaf(LEAF_TAG_ISSUER_ID, issuer_bytes.as_slice()));

    let salt_epoch_bytes = body.salt_epoch.to_le_bytes();
    leaves.push(hash_leaf(LEAF_TAG_SALT_EPOCH, &salt_epoch_bytes));

    let policy_flags_bytes = body.policy_flags.to_le_bytes();
    leaves.push(hash_leaf(LEAF_TAG_POLICY_FLAGS, &policy_flags_bytes));

    let metadata_bytes = body.metadata.encode();
    leaves.push(hash_leaf(LEAF_TAG_METADATA, metadata_bytes.as_slice()));

    merkle_root(leaves)
}

fn hash_leaf(domain: &[u8], value: &[u8]) -> Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    let len = u32::try_from(value.len()).expect("ticket leaf exceeds u32 length");
    hasher.update(&len.to_le_bytes());
    hasher.update(value);
    finalize_hash(&hasher)
}

fn hash_node(left: &Digest32, right: &Digest32) -> Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NODE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    finalize_hash(&hasher)
}

fn merkle_root(mut leaves: Vec<Digest32>) -> Digest32 {
    debug_assert!(!leaves.is_empty());

    while leaves.len() > 1 {
        let mut next = Vec::with_capacity(leaves.len().div_ceil(2));
        for chunk in leaves.chunks(2) {
            let combined = match chunk {
                [left, right] => hash_node(left, right),
                [single] => hash_node(single, single),
                _ => unreachable!("chunks() never yields empty slice"),
            };
            next.push(combined);
        }
        leaves = next;
    }

    leaves[0]
}

fn finalize_hash(hasher: &blake3::Hasher) -> Digest32 {
    let mut out = [0_u8; 32];
    let clone = hasher.clone();
    out.copy_from_slice(clone.finalize().as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::codec::{Decode, Encode};

    use super::*;
    use crate::{account::AccountId, domain::DomainId};

    const SMALL_ORDER_ED25519_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_ED25519_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn sample_issuer_keypair() -> KeyPair {
        KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("derive checked Soranet ticket fixture issuer keypair")
    }

    fn sample_body() -> TicketBodyV1 {
        let _domain = DomainId::try_new("wonderland", "universal").expect("static domain is valid");
        let issuer_key = sample_issuer_keypair();
        let issuer_id = AccountId::new(issuer_key.public_key().clone());
        TicketBodyV1 {
            blinded_cid: [0xBA; 32],
            scope: TicketScopeV1::Read,
            max_uses: 3,
            valid_after: 10,
            valid_until: 20,
            issuer_id,
            salt_epoch: 42,
            policy_flags: 0,
            metadata: Metadata::default(),
        }
    }

    #[test]
    fn ticket_envelope_roundtrip() {
        let envelope = sample_envelope();
        let bytes = envelope.encode();
        let decoded =
            TicketEnvelopeV1::decode(&mut bytes.as_slice()).expect("decode ticket envelope");
        assert_eq!(envelope, decoded);
    }

    fn sample_envelope() -> TicketEnvelopeV1 {
        let body = sample_body();
        let commitment = body.compute_commitment();
        TicketEnvelopeV1 {
            body,
            commitment,
            zk_proof: Vec::new(),
            signature: Signature::try_from_bytes(&[0x42u8; 64])
                .expect("nonzero ticket signature fixture"),
            nullifier: [0u8; 32],
        }
    }

    fn signed_sample_envelope() -> TicketEnvelopeV1 {
        let issuer_key = sample_issuer_keypair();
        TicketEnvelopeV1::try_sign(
            sample_body(),
            Vec::new(),
            [0u8; 32],
            issuer_key.private_key(),
        )
        .expect("sign checked SoraNet ticket envelope")
    }

    fn signature_with_malformed_ed25519_r(
        signature: &Signature,
        replacement_r: &[u8; 32],
    ) -> Signature {
        let mut payload = signature.payload().to_vec();
        payload[..replacement_r.len()].copy_from_slice(replacement_r);
        Signature::from_bytes(&payload)
    }

    #[test]
    fn detects_expiration_boundaries() {
        let ticket = sample_envelope();
        assert!(!ticket.is_expired(19));
        assert!(ticket.is_expired(20));
        assert!(ticket.is_expired(25));
    }

    #[test]
    fn detects_activation_window() {
        let ticket = sample_envelope();
        assert!(ticket.is_not_yet_active(5));
        assert!(!ticket.is_not_yet_active(10));
        assert!(!ticket.is_not_yet_active(15));
    }

    #[test]
    fn scope_labels_match_variants() {
        let mut ticket = sample_envelope();
        assert_eq!(ticket.scope_label(), "read");
        ticket.body.scope = TicketScopeV1::Write;
        assert_eq!(ticket.scope_label(), "write");
        ticket.body.scope = TicketScopeV1::Admin;
        assert_eq!(ticket.scope_label(), "admin");
    }

    #[test]
    fn commitment_verification_succeeds_for_matching_body() {
        let mut ticket = sample_envelope();
        assert_eq!(ticket.verify_commitment(), Ok(()));
        assert!(!ticket.commitment_is_placeholder());

        ticket.commitment[0] ^= 0xFF;
        assert!(ticket.verify_commitment().is_err());
        assert!(ticket.commitment_is_placeholder());
    }

    #[test]
    fn commitment_changes_with_body_mutation() {
        let body = sample_body();
        let commitment = body.compute_commitment();
        assert_ne!(commitment, [0u8; 32]);

        let mut tweaked = body.clone();
        tweaked.max_uses += 1;
        assert_ne!(tweaked.compute_commitment(), commitment);
    }

    #[test]
    fn ticket_signature_verifies_body_and_commitment() {
        signed_sample_envelope()
            .verify_signature()
            .expect("checked ticket signature verifies");
    }

    #[test]
    fn ticket_signature_rejects_stale_commitment() {
        let mut ticket = signed_sample_envelope();
        ticket.body.max_uses += 1;

        assert!(matches!(
            ticket.verify_signature(),
            Err(TicketSignatureError::Commitment(
                TicketCommitmentError::Mismatch { .. }
            ))
        ));
    }

    #[test]
    fn ticket_try_sign_rejects_issuer_private_key_mismatch() {
        let wrong_key = KeyPair::try_from_seed(vec![0xA6; 32], Algorithm::Ed25519)
            .expect("derive checked alternate ticket keypair");
        let err = TicketEnvelopeV1::try_sign(
            sample_body(),
            Vec::new(),
            [0u8; 32],
            wrong_key.private_key(),
        )
        .expect_err("issuer mismatch must reject before signing");

        assert!(matches!(err, iroha_crypto::Error::Other(_)));
    }

    #[test]
    fn ticket_signature_rejects_all_zero_signature_material() {
        let mut ticket = signed_sample_envelope();
        ticket.signature = Signature::from_bytes(&[0u8; 64]);

        assert!(matches!(
            ticket.verify_signature(),
            Err(TicketSignatureError::Signature(_))
        ));
    }

    #[test]
    fn ticket_signature_rejects_malformed_ed25519_signature_r() {
        let valid_signature = signed_sample_envelope().signature;

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_R),
            ("noncanonical", NONCANONICAL_ED25519_R),
        ] {
            let mut ticket = signed_sample_envelope();
            ticket.signature = signature_with_malformed_ed25519_r(&valid_signature, &replacement_r);

            assert!(
                matches!(
                    ticket.verify_signature(),
                    Err(TicketSignatureError::Signature(
                        iroha_crypto::Error::BadSignature
                    ))
                ),
                "{label} ticket signature R must fail Ed25519 admission"
            );
        }
    }
}

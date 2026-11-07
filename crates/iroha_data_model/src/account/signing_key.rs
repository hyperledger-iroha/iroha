//! Off-chain signing keys of accounts (`specs/account_signing_keys.md`).
//!
//! An account's controller (its single transaction key, or the members of its multisig policy)
//! signs only transactions. Everything else the account signs off chain, such as an Iroha
//! Connect raw request or a Torii canonical request, is signed by a separate signing key the
//! account registers on chain with
//! [`SetAccountSigningKeyV1`](crate::isi::account_signing_key::SetAccountSigningKeyV1).
//!
//! Every off-chain signature covers the [`offchain_signing_message_v1`] envelope. It names the
//! signature's purpose, the network, the account, the key generation and a domain tag, and it
//! starts with a fixed prefix and is longer than any 32-byte hash. So no off-chain signature is
//! ever a valid transaction or query signature, and a signature made for one purpose, network,
//! account or key generation is never valid for another.

use crate::{
    NetworkId,
    account::{AccountAddress, AccountAddressError, AccountId},
};
use iroha_crypto::{PrivateKey, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{string::String, vec::Vec};

/// Prefix of every off-chain signing message.
pub const OFFCHAIN_SIGNING_DOMAIN_V1: &[u8] = b"iroha:offchain:v1\0";

/// Domain tag of Torii canonical-request authentication, signed with
/// [`OffchainSignaturePurposeV1::ToriiCanonicalRequest`].
pub const TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1: &str = "iroha:torii:canonical-request:v1";

/// What an off-chain signature authorises.
///
/// The signer chooses the purpose from what it signs; a requester never chooses it. A wallet
/// signs an Iroha Connect raw request as [`Self::ConnectRaw`] unless the request carries
/// [`TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1`], in which case it must check that the bytes are a
/// Torii canonical request and show it before signing it as [`Self::ToriiCanonicalRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OffchainSignaturePurposeV1 {
    /// Opaque bytes a dApp asked the wallet to sign through Iroha Connect `SignRequestRaw`.
    ConnectRaw,
    /// Torii canonical-request authentication (`X-Iroha-Signature`).
    ToriiCanonicalRequest,
}

impl OffchainSignaturePurposeV1 {
    /// The byte that identifies the purpose in the envelope.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::ConnectRaw => 1,
            Self::ToriiCanonicalRequest => 2,
        }
    }

    /// The purpose a wallet signs a Connect raw request with `domain_tag` under.
    #[must_use]
    pub fn for_connect_raw_tag(domain_tag: &str) -> Self {
        if domain_tag == TORII_CANONICAL_REQUEST_DOMAIN_TAG_V1 {
            Self::ToriiCanonicalRequest
        } else {
            Self::ConnectRaw
        }
    }
}

/// The off-chain signing key an account registered, as verifiers read it.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::account::signing_key::AccountSigningKeyV1")]
pub struct AccountSigningKeyV1 {
    /// Public half of the signing key.
    pub public_key: PublicKey,
    /// Generation of the key: the number of installs, rotations and revocations so far.
    pub generation: u32,
}

/// World record of an account's off-chain signing key.
///
/// `generation` counts every install, rotation and revocation, so a signature names the exact
/// key it was made with and a revoked or replaced key never verifies again.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::account::signing_key::AccountSigningKeyRecordV1")]
pub struct AccountSigningKeyRecordV1 {
    /// Generation of the last install, rotation or revocation.
    pub generation: u32,
    /// The active signing key, or `None` once revoked.
    #[norito(required)]
    pub public_key: Option<PublicKey>,
}

impl AccountSigningKeyRecordV1 {
    /// The active key and its generation, if the account has one.
    #[must_use]
    pub fn active(&self) -> Option<AccountSigningKeyV1> {
        self.public_key
            .as_ref()
            .map(|public_key| AccountSigningKeyV1 {
                public_key: public_key.clone(),
                generation: self.generation,
            })
    }
}

/// World reservation of a key that is, or once was, an account's off-chain signing key.
///
/// The World keys it by the single-key `AccountId` the signing key would control. A reserved
/// key never becomes another account's signing key, is never installed again, and stays
/// reserved after its account rotates or revokes it.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::account::signing_key::AccountSigningKeyReservationV1")]
pub struct AccountSigningKeyReservationV1 {
    /// The account that installed the key.
    pub account: AccountId,
    /// Generation at which the key was installed.
    pub generation: u32,
    /// Whether the key is still the account's active signing key.
    pub active: bool,
}

/// Why an off-chain signing message could not be built or a signature did not verify.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OffchainSigningError {
    /// The account has no canonical address bytes.
    #[error("account has no canonical address: {0}")]
    Account(String),
    /// The domain tag is empty.
    #[error("off-chain signing domain tag must not be empty")]
    EmptyDomainTag,
    /// A field is too long to frame.
    #[error("off-chain signing message field is too long")]
    TooLong,
    /// The key could not sign the message.
    #[error("off-chain signing failed: {0}")]
    Signing(String),
    /// The signature does not verify under the account's signing key.
    #[error("off-chain signature does not verify under the account signing key")]
    Verification,
}

impl From<AccountAddressError> for OffchainSigningError {
    fn from(error: AccountAddressError) -> Self {
        Self::Account(error.to_string())
    }
}

/// Build the message an account's signing key signs off chain.
///
/// The layout is fixed and framed, so every SDK builds it without a codec:
///
/// ```text
/// "iroha:offchain:v1\0"
/// purpose                u8   (1 ConnectRaw, 2 ToriiCanonicalRequest)
/// network_id             [u8; 32] (genesis hash)
/// account_len, account   u32 LE, canonical account-address bytes
/// generation             u32 LE
/// tag_len, domain_tag    u32 LE, UTF-8
/// bytes_len, bytes       u64 LE, the signed bytes
/// ```
///
/// # Errors
/// The account has no canonical address, the tag is empty, or a field cannot be framed.
pub fn offchain_signing_message_v1(
    purpose: OffchainSignaturePurposeV1,
    network_id: &NetworkId,
    account: &AccountId,
    generation: u32,
    domain_tag: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, OffchainSigningError> {
    if domain_tag.is_empty() {
        return Err(OffchainSigningError::EmptyDomainTag);
    }
    let account = AccountAddress::from_account_id(account)?.canonical_bytes()?;
    let account_len = u32::try_from(account.len()).map_err(|_| OffchainSigningError::TooLong)?;
    let tag_len = u32::try_from(domain_tag.len()).map_err(|_| OffchainSigningError::TooLong)?;
    let bytes_len = u64::try_from(bytes.len()).map_err(|_| OffchainSigningError::TooLong)?;
    let network = network_id.as_bytes();
    let capacity = [
        OFFCHAIN_SIGNING_DOMAIN_V1.len(),
        1,
        network.len(),
        core::mem::size_of::<u32>(),
        account.len(),
        core::mem::size_of::<u32>(),
        core::mem::size_of::<u32>(),
        domain_tag.len(),
        core::mem::size_of::<u64>(),
        bytes.len(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or(OffchainSigningError::TooLong)?;
    let mut message = Vec::new();
    message
        .try_reserve_exact(capacity)
        .map_err(|_| OffchainSigningError::TooLong)?;
    message.extend_from_slice(OFFCHAIN_SIGNING_DOMAIN_V1);
    message.push(purpose.code());
    message.extend_from_slice(network);
    message.extend_from_slice(&account_len.to_le_bytes());
    message.extend_from_slice(&account);
    message.extend_from_slice(&generation.to_le_bytes());
    message.extend_from_slice(&tag_len.to_le_bytes());
    message.extend_from_slice(domain_tag.as_bytes());
    message.extend_from_slice(&bytes_len.to_le_bytes());
    message.extend_from_slice(bytes);
    Ok(message)
}

/// The context an off-chain signature is made in and verified against.
#[derive(Debug, Clone, Copy)]
pub struct OffchainSigningContextV1<'a> {
    /// What the signature authorises.
    pub purpose: OffchainSignaturePurposeV1,
    /// The exact network.
    pub network_id: &'a NetworkId,
    /// The account whose signing key signs.
    pub account: &'a AccountId,
    /// Generation of the account's signing key.
    pub generation: u32,
    /// Domain tag of the request.
    pub domain_tag: &'a str,
}

impl OffchainSigningContextV1<'_> {
    /// The message the signing key signs for `bytes` in this context.
    ///
    /// # Errors
    /// As [`offchain_signing_message_v1`].
    pub fn message(&self, bytes: &[u8]) -> Result<Vec<u8>, OffchainSigningError> {
        offchain_signing_message_v1(
            self.purpose,
            self.network_id,
            self.account,
            self.generation,
            self.domain_tag,
            bytes,
        )
    }

    /// Sign `bytes` with the account's signing key.
    ///
    /// The caller must hold the signing key, never the account's transaction key.
    ///
    /// # Errors
    /// The message cannot be built or the key cannot sign.
    pub fn sign(
        &self,
        signing_key: &PrivateKey,
        bytes: &[u8],
    ) -> Result<Signature, OffchainSigningError> {
        let message = self.message(bytes)?;
        Signature::try_new(signing_key, &message)
            .map_err(|error| OffchainSigningError::Signing(error.to_string()))
    }

    /// Verify a signature over `bytes` against the account's registered signing key.
    ///
    /// `signing_key` must be the key the account registered on chain at `self.generation`,
    /// read from committed state; a key the requester supplies proves nothing.
    ///
    /// # Errors
    /// The message cannot be built or the signature does not verify.
    pub fn verify(
        &self,
        signing_key: &PublicKey,
        signature: &Signature,
        bytes: &[u8],
    ) -> Result<(), OffchainSigningError> {
        let message = self.message(bytes)?;
        signature
            .verify(signing_key, &message)
            .map_err(|_| OffchainSigningError::Verification)
    }
}

#[cfg(test)]
#[path = "signing_key_tests.rs"]
mod tests;

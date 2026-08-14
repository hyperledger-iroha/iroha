//! Canonical Anonymous-PGC encrypted account-state root derivation.
use iroha_data_model::privacy::{
    ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1, PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
    PrivacyNamespaceV1, PrivacyPgcAccountV1, PrivacyProtocolIdV1, PrivacyRootV1,
};
use thiserror::Error;
/// Failure deriving one canonical Anonymous-PGC encrypted account-state root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyPgcAccountStateRootErrorV1 {
    /// The supplied namespace is not structurally valid.
    #[error("privacy PGC account-root namespace is invalid")]
    InvalidNamespace,
    /// The namespace selects a protocol other than Anonymous-PGC.
    #[error("privacy PGC account-root namespace has the wrong protocol")]
    WrongProtocol,
    /// Epoch zero is reserved and cannot identify authoritative state.
    #[error("privacy PGC account-root epoch must be non-zero")]
    ZeroEpoch,
    /// The bootstrapped public supply must be positive.
    #[error("privacy PGC account-root total supply must be non-zero")]
    ZeroTotalSupply,
    /// The complete table does not have one of the closed first-release sizes.
    #[error("privacy PGC account-root count is not a closed first-release size")]
    InvalidAccountCount,
    /// A public key or ciphertext component uses the reserved zero encoding.
    #[error("privacy PGC account-root contains a zero point")]
    ZeroPoint,
    /// Account public keys are not in strict canonical order.
    #[error("privacy PGC account-root keys are not strictly increasing")]
    KeysNotStrictlyIncreasing,
    /// A public key or ciphertext component is not a canonical P-256 point.
    #[error("privacy PGC account-root contains an invalid canonical P-256 point")]
    InvalidPoint,
    /// The canonical namespace could not be encoded.
    #[error("privacy PGC namespace encoding failed")]
    NamespaceEncoding,
    /// The canonical namespace encoding length cannot be represented.
    #[error("privacy PGC namespace encoding length overflow")]
    NamespaceEncodingLength,
    /// Root derivation reached the reserved all-zero digest.
    #[error("privacy PGC account-root derivation produced the reserved zero root")]
    ZeroRoot,
}
/// Deterministically derive one complete PGC encrypted account-state root.
///
/// This is the public wallet/validator boundary for predicting the exact
/// successor root committed by an Anonymous-PGC payment. `accounts` must be
/// the complete table selected by `namespace`, in strict public-key order.
/// The closed first-release cardinalities bound the operation to at most 64
/// entries; no partial-table or caller-selected root form exists.
pub fn derive_privacy_pgc_account_state_root_v1(
    namespace: PrivacyNamespaceV1,
    epoch: u64,
    total_supply: u32,
    accounts: &[PrivacyPgcAccountV1],
) -> Result<PrivacyRootV1, PrivacyPgcAccountStateRootErrorV1> {
    namespace
        .validate()
        .map_err(|_| PrivacyPgcAccountStateRootErrorV1::InvalidNamespace)?;
    if namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
        return Err(PrivacyPgcAccountStateRootErrorV1::WrongProtocol);
    }
    if epoch == 0 {
        return Err(PrivacyPgcAccountStateRootErrorV1::ZeroEpoch);
    }
    if total_supply == 0 {
        return Err(PrivacyPgcAccountStateRootErrorV1::ZeroTotalSupply);
    }
    let count = u32::try_from(accounts.len())
        .map_err(|_| PrivacyPgcAccountStateRootErrorV1::InvalidAccountCount)?;
    if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&count) {
        return Err(PrivacyPgcAccountStateRootErrorV1::InvalidAccountCount);
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.public_key.is_zero()
            || account.encrypted_balance.left.is_zero()
            || account.encrypted_balance.right.is_zero()
        {
            return Err(PrivacyPgcAccountStateRootErrorV1::ZeroPoint);
        }
        if index > 0 && accounts[index - 1].public_key >= account.public_key {
            return Err(PrivacyPgcAccountStateRootErrorV1::KeysNotStrictlyIncreasing);
        }
        for point in [
            account.public_key,
            account.encrypted_balance.left,
            account.encrypted_balance.right,
        ] {
            crate::privacy_engines::p256::CompressedPointV1::from_slice(point.as_bytes())
                .map_err(|_| PrivacyPgcAccountStateRootErrorV1::InvalidPoint)?;
        }
    }
    let namespace_bytes = norito::to_bytes(&namespace)
        .map_err(|_| PrivacyPgcAccountStateRootErrorV1::NamespaceEncoding)?;
    let namespace_len = u64::try_from(namespace_bytes.len())
        .map_err(|_| PrivacyPgcAccountStateRootErrorV1::NamespaceEncodingLength)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1);
    hasher.update(&namespace_len.to_le_bytes());
    hasher.update(&namespace_bytes);
    hasher.update(&epoch.to_le_bytes());
    hasher.update(&total_supply.to_le_bytes());
    hasher.update(&count.to_le_bytes());
    for account in accounts {
        hasher.update(account.public_key.as_bytes());
        hasher.update(account.encrypted_balance.left.as_bytes());
        hasher.update(account.encrypted_balance.right.as_bytes());
    }
    let root = PrivacyRootV1::new(*hasher.finalize().as_bytes());
    if root.is_zero() {
        return Err(PrivacyPgcAccountStateRootErrorV1::ZeroRoot);
    }
    Ok(root)
}
pub(crate) fn compute_privacy_pgc_account_state_root_v1(
    namespace: PrivacyNamespaceV1,
    epoch: u64,
    total_supply: u32,
    accounts: &[PrivacyPgcAccountV1],
) -> Result<PrivacyRootV1, &'static str> {
    derive_privacy_pgc_account_state_root_v1(namespace, epoch, total_supply, accounts)
        .map_err(|_| "privacy PGC account-state root derivation failed")
}

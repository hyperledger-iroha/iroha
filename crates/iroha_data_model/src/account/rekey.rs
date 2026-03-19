//! Stable account rekey metadata for tracking alias-backed account continuity.

use std::{io::Cursor, vec::Vec};

use iroha_crypto::PublicKey;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{Account, AccountId, DomainId, Name};

/// Stable account label that survives signatory rotation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AccountLabel {
    /// Domain in which the account lives.
    pub domain: DomainId,
    /// Human-readable label unique within the domain.
    pub label: Name,
}

impl AccountLabel {
    /// Create a new account label.
    pub fn new(domain: DomainId, label: Name) -> Self {
        Self { domain, label }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AccountLabel {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = Cursor::new(bytes);
        let value: Self = norito::codec::Decode::decode(&mut cursor)?;
        let used =
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
        Ok((value, used))
    }
}

/// Record that tracks the active concrete account behind a stable account label.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AccountRekeyRecord {
    /// Stable label under which the account is addressed.
    pub label: AccountLabel,
    /// Current concrete account id behind the stable label.
    pub active_account_id: AccountId,
    /// Historical concrete account ids retained for continuity and audit trails.
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    pub previous_account_ids: Vec<AccountId>,
    /// Current single-key signatory when the active account is directly key-controlled.
    ///
    /// Multisig-controlled accounts do not expose a single signatory, so this remains `None`
    /// for alias-backed multisig identities.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub active_signatory: Option<PublicKey>,
    /// Historical single-key signatories retained for audit trails.
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    pub previous_signatories: Vec<PublicKey>,
}

impl AccountRekeyRecord {
    /// Bootstrap a rekey record from an existing account using its canonical label.
    ///
    /// Returns [`None`] when the account has not yet been assigned a stable label.
    #[must_use]
    pub fn from_account(account: &Account) -> Option<Self> {
        let label = account.label()?.clone();
        Some(Self::new(label, account.id.clone()))
    }

    /// Bootstrap a rekey record for an arbitrary alias binding.
    #[must_use]
    pub fn new(label: AccountLabel, active_account_id: AccountId) -> Self {
        Self {
            label,
            active_signatory: active_account_id.try_signatory().cloned(),
            active_account_id,
            previous_account_ids: Vec::new(),
            previous_signatories: Vec::new(),
        }
    }

    /// Repoint the stable label to a new concrete account and retain the previous controller ids.
    #[must_use]
    pub fn repoint_to_account(&self, next_account_id: AccountId) -> Self {
        if self.active_account_id == next_account_id {
            return self.clone();
        }

        let mut previous_account_ids = self.previous_account_ids.clone();
        previous_account_ids.push(self.active_account_id.clone());

        let mut previous_signatories = self.previous_signatories.clone();
        if let Some(active_signatory) = self.active_signatory.as_ref() {
            previous_signatories.push(active_signatory.clone());
        }

        Self {
            label: self.label.clone(),
            active_account_id: next_account_id.clone(),
            previous_account_ids,
            active_signatory: next_account_id.try_signatory().cloned(),
            previous_signatories,
        }
    }

    /// Plan a rotation to a new signatory-backed account, returning the staged record.
    #[must_use]
    pub fn rotate_to(&self, next_signatory: PublicKey) -> Self {
        self.repoint_to_account(AccountId::new(next_signatory))
    }
}

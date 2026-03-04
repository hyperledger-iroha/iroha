//! Prototype helpers for decoupling logical account identity from the active signatory.
//!
//! These types are not wired into the runtime yet—they capture the migration surface for
//! implementing `RotateAccountSignatory` without breaking existing account identifiers.
//!
//! See `docs/source/isi_extension_plan.md` for the execution plan tied to these types.

use std::{io::Cursor, vec::Vec};

use iroha_crypto::PublicKey;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{Account, DomainId, Name};

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

/// Prototype record that tracks an account label and its active signatory.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AccountRekeyRecord {
    /// Stable label under which the account is addressed.
    pub label: AccountLabel,
    /// Current signatory public key.
    pub active_signatory: PublicKey,
    /// Historical signatories retained for audit trails.
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
        let signatory = account.try_signatory()?.clone();
        Some(Self {
            label,
            active_signatory: signatory,
            previous_signatories: Vec::new(),
        })
    }

    /// Plan a rotation to a new signatory, returning the staged record.
    #[must_use]
    pub fn rotate_to(&self, next_signatory: PublicKey) -> Self {
        let mut previous = self.previous_signatories.clone();
        previous.push(self.active_signatory.clone());
        Self {
            label: self.label.clone(),
            active_signatory: next_signatory,
            previous_signatories: previous,
        }
    }
}

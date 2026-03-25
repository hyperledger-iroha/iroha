//! Stable account rekey metadata for tracking alias-backed account continuity.

use std::{io::Cursor, string::String, vec::Vec};

use iroha_crypto::PublicKey;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{Account, AccountId, DomainId, Name};
use crate::{
    error::ParseError,
    nexus::{DataSpaceCatalog, DataSpaceId},
};

/// Stable account label that survives signatory rotation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AccountLabel {
    /// Human-readable label unique within the alias namespace.
    pub label: Name,
    /// Optional concrete domain scope for the alias.
    #[norito(default)]
    pub domain: Option<DomainId>,
    /// Dataspace in which the alias is registered.
    #[norito(default)]
    pub dataspace: DataSpaceId,
}

impl AccountLabel {
    /// Create a new account label in the default `universal` dataspace.
    pub fn new(domain: DomainId, label: Name) -> Self {
        Self::new_in_dataspace(label, Some(domain), DataSpaceId::GLOBAL)
    }

    /// Create a new domainless account label in the provided dataspace.
    #[must_use]
    pub fn domainless(label: Name, dataspace: DataSpaceId) -> Self {
        Self::new_in_dataspace(label, None, dataspace)
    }

    /// Create a new account label from explicit alias components.
    #[must_use]
    pub fn new_in_dataspace(label: Name, domain: Option<DomainId>, dataspace: DataSpaceId) -> Self {
        Self {
            label,
            domain,
            dataspace,
        }
    }

    /// Parse a canonical account alias literal.
    ///
    /// Supported forms are `label@domain.dataspace` and `label@dataspace`.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the literal is malformed or the dataspace alias is unknown.
    pub fn from_literal(input: &str, catalog: &DataSpaceCatalog) -> Result<Self, ParseError> {
        let canonical = canonicalize_literal(input)?;
        let segments = split_alias_segments(&canonical)?;
        let label = segments
            .label
            .parse()
            .map_err(|_| ParseError::new("account alias label segment is invalid"))?;
        let domain = segments
            .domain
            .map(|domain| {
                domain
                    .parse()
                    .map_err(|_| ParseError::new("account alias domain segment is invalid"))
            })
            .transpose()?;
        let dataspace = catalog
            .by_alias(segments.dataspace)
            .map(|entry| entry.id)
            .ok_or_else(|| ParseError::new("unknown dataspace alias in account alias"))?;
        Ok(Self::new_in_dataspace(label, domain, dataspace))
    }

    /// Render the canonical account alias literal using dataspace aliases from the catalog.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the dataspace identifier is not present in the catalog.
    pub fn to_literal(&self, catalog: &DataSpaceCatalog) -> Result<String, ParseError> {
        let dataspace = catalog
            .by_id(self.dataspace)
            .ok_or_else(|| ParseError::new("unknown dataspace id for account alias"))?;
        let label = self.label.as_ref().to_ascii_lowercase();
        let dataspace_alias = dataspace.alias.to_ascii_lowercase();
        Ok(self.domain.as_ref().map_or_else(
            || format!("{label}@{dataspace_alias}"),
            |domain| {
                format!(
                "{label}@{}.{}",
                domain.to_string().to_ascii_lowercase(),
                dataspace_alias
            )
            },
        ))
    }
}

fn canonicalize_literal(input: &str) -> Result<String, ParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseError::new("account alias must not be empty"));
    }
    if trimmed != input {
        return Err(ParseError::new(
            "account alias must not contain leading or trailing whitespace",
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ParseError::new(
            "account alias must not contain control characters",
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

struct AliasSegments<'a> {
    label: &'a str,
    domain: Option<&'a str>,
    dataspace: &'a str,
}

fn split_alias_segments(input: &str) -> Result<AliasSegments<'_>, ParseError> {
    let (label, right) = input.split_once('@').ok_or_else(|| {
        ParseError::new(
            "account alias must use `label@domain.dataspace` or `label@dataspace` format",
        )
    })?;
    if right.contains('@') {
        return Err(ParseError::new(
            "account alias must contain exactly one `@` separator",
        ));
    }

    if label.is_empty() {
        return Err(ParseError::new(
            "account alias label segment must not be empty",
        ));
    }
    if right.is_empty() {
        return Err(ParseError::new(
            "account alias dataspace segment must not be empty",
        ));
    }

    let dot_count = right.bytes().filter(|byte| *byte == b'.').count();
    if dot_count == 1 {
        let (domain, dataspace) = right.split_once('.').expect("counted dot");
        if domain.is_empty() || dataspace.is_empty() {
            return Err(ParseError::new(
                "account alias domain and dataspace segments must not be empty",
            ));
        }
        return Ok(AliasSegments {
            label,
            domain: Some(domain),
            dataspace,
        });
    }
    if dot_count > 1 {
        return Err(ParseError::new(
            "account alias must contain at most one `.` after `@`",
        ));
    }
    Ok(AliasSegments {
        label,
        domain: None,
        dataspace: right,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::DataSpaceMetadata;

    fn catalog() -> DataSpaceCatalog {
        DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "retail".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog")
    }

    #[test]
    fn account_label_parses_domainful_literal() {
        let label =
            AccountLabel::from_literal("Treasury@Banking.Retail", &catalog()).expect("valid alias");
        assert_eq!(label.label.as_ref(), "treasury");
        assert_eq!(
            label.domain,
            Some("banking".parse::<DomainId>().expect("domain id"))
        );
        assert_eq!(label.dataspace, DataSpaceId::new(7));
    }

    #[test]
    fn account_label_parses_domainless_literal() {
        let label = AccountLabel::from_literal("primary@retail", &catalog()).expect("valid alias");
        assert_eq!(label.label.as_ref(), "primary");
        assert_eq!(label.domain, None);
        assert_eq!(label.dataspace, DataSpaceId::new(7));
    }

    #[test]
    fn account_label_roundtrips_canonical_literal() {
        let catalog = catalog();
        let label =
            AccountLabel::from_literal("Treasury@Banking.Retail", &catalog).expect("valid alias");
        assert_eq!(
            label.to_literal(&catalog).expect("literal"),
            "treasury@banking.retail"
        );
    }

    #[test]
    fn account_label_rejects_unknown_dataspace_alias() {
        let err = AccountLabel::from_literal("primary@banking.missing", &catalog())
            .expect_err("unknown dataspace must fail");
        assert!(err.to_string().contains("unknown dataspace alias"));
    }

    #[test]
    fn account_label_rejects_invalid_literals() {
        for raw in [
            "",
            " ",
            "primary",
            "primary@",
            "@retail",
            "primary@@retail",
            "primary@banking.retail.extra",
            "primary@banking.",
            "primary@.retail",
        ] {
            assert!(
                AccountLabel::from_literal(raw, &catalog()).is_err(),
                "must fail: {raw}"
            );
        }
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
        let active_signatory = next_account_id.try_signatory().cloned();

        let mut previous_account_ids = self.previous_account_ids.clone();
        previous_account_ids.push(self.active_account_id.clone());

        let mut previous_signatories = self.previous_signatories.clone();
        if let Some(active_signatory) = self.active_signatory.as_ref() {
            previous_signatories.push(active_signatory.clone());
        }

        Self {
            label: self.label.clone(),
            active_account_id: next_account_id,
            previous_account_ids,
            active_signatory,
            previous_signatories,
        }
    }

    /// Plan a rotation to a new signatory-backed account, returning the staged record.
    #[must_use]
    pub fn rotate_to(&self, next_signatory: PublicKey) -> Self {
        self.repoint_to_account(AccountId::new(next_signatory))
    }
}

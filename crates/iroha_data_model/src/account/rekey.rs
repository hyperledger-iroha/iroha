//! Stable account rekey metadata for tracking alias-backed account continuity.
use super::{Account, AccountId, Name};
use crate::{
    alias_setup::AccountAliasName,
    domain::DomainId,
    error::ParseError,
    nexus::{DataSpaceCatalog, DataSpaceId},
};
use core::fmt;
use iroha_crypto::PublicKey;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{io::Cursor, str::FromStr, string::String, vec::Vec};
use thiserror::Error;
/// Dataspace-scoped alias-domain segment used only inside account aliases.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[repr(transparent)]
#[norito(decode_from_slice)]
pub struct AccountAliasDomain(pub Name);
impl AccountAliasDomain {
    /// Construct an alias-domain segment from its canonical name.
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self(name)
    }
    /// Borrow the underlying alias-domain segment name.
    #[must_use]
    pub fn name(&self) -> &Name {
        &self.0
    }
}
impl fmt::Display for AccountAliasDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl From<Name> for AccountAliasDomain {
    fn from(value: Name) -> Self {
        Self(value)
    }
}
impl From<AccountAliasDomain> for Name {
    fn from(value: AccountAliasDomain) -> Self {
        value.0
    }
}
impl FromStr for AccountAliasDomain {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Name>()
            .map(Self)
            .map_err(|_| ParseError::new("account alias domain segment is invalid"))
    }
}
/// Stable on-chain account alias that survives signatory rotation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct AccountAlias {
    /// Human-readable alias label unique within the alias namespace.
    pub label: Name,
    /// Optional alias-domain scope for the alias, unique only within its dataspace.
    #[norito(required)]
    pub domain: Option<AccountAliasDomain>,
    /// Dataspace in which the alias is registered.
    pub dataspace: DataSpaceId,
}
impl AccountAlias {
    /// Create a new account alias from explicit alias components.
    #[must_use]
    pub fn new(label: Name, domain: Option<AccountAliasDomain>, dataspace: DataSpaceId) -> Self {
        Self {
            label,
            domain,
            dataspace,
        }
    }
    /// Create a new domainless account alias in the provided dataspace.
    #[must_use]
    pub fn domainless(label: Name, dataspace: DataSpaceId) -> Self {
        Self::new(label, None, dataspace)
    }
    /// Create a new account alias from explicit alias components.
    #[must_use]
    pub fn new_in_dataspace(
        label: Name,
        domain: Option<AccountAliasDomain>,
        dataspace: DataSpaceId,
    ) -> Self {
        Self::new(label, domain, dataspace)
    }
    /// Parse a canonical account alias literal.
    ///
    /// Supported forms are `name@domain.dataspace` and `name@dataspace`.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the literal is malformed or the dataspace alias is unknown.
    pub fn from_literal(input: &str, catalog: &DataSpaceCatalog) -> Result<Self, ParseError> {
        let name = input.parse::<AccountAliasName>()?;
        let dataspace = catalog
            .by_alias(name.dataspace.as_ref())
            .map(|entry| entry.id)
            .ok_or_else(|| ParseError::new("unknown dataspace alias in account alias"))?;
        Ok(Self::new_in_dataspace(
            name.label,
            name.domain.map(AccountAliasDomain::new),
            dataspace,
        ))
    }
    /// Render the canonical account alias literal using dataspace aliases from the catalog.
    ///
    /// # Errors
    /// Returns [`ParseError`] when the dataspace identifier is not present in the catalog.
    pub fn to_literal(&self, catalog: &DataSpaceCatalog) -> Result<String, ParseError> {
        let dataspace = catalog
            .by_id(self.dataspace)
            .ok_or_else(|| ParseError::new("unknown dataspace id for account alias"))?;
        AccountAliasName::try_new(
            self.label.as_ref(),
            self.domain.as_ref().map(|domain| domain.name().as_ref()),
            dataspace.alias.as_str(),
        )
        .map(|name| name.to_string())
    }
    /// Resolve the alias-domain scope into a dataspace-qualified [`DomainId`].
    ///
    /// # Errors
    /// Returns [`ParseError`] when the alias references an unknown dataspace identifier.
    pub fn domain_id(&self, catalog: &DataSpaceCatalog) -> Result<Option<DomainId>, ParseError> {
        let Some(domain) = self.domain.as_ref() else {
            return Ok(None);
        };
        let dataspace = catalog
            .by_id(self.dataspace)
            .ok_or_else(|| ParseError::new("unknown dataspace id for account alias"))?;
        let dataspace_alias = dataspace
            .alias
            .parse::<Name>()
            .map_err(|_| ParseError::new("dataspace alias in catalog is invalid"))?;
        Ok(Some(DomainId::try_new(domain.name(), &dataspace_alias)?))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for AccountAlias {
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
            AccountAlias::from_literal("Treasury@Banking.Retail", &catalog()).expect("valid alias");
        assert_eq!(label.label.as_ref(), "treasury");
        assert_eq!(
            label.domain,
            Some(
                "banking"
                    .parse::<AccountAliasDomain>()
                    .expect("alias domain")
            )
        );
        assert_eq!(label.dataspace, DataSpaceId::new(7));
    }
    #[test]
    fn account_label_parses_domainless_literal() {
        let label = AccountAlias::from_literal("primary@retail", &catalog()).expect("valid alias");
        assert_eq!(label.label.as_ref(), "primary");
        assert_eq!(label.domain, None);
        assert_eq!(label.dataspace, DataSpaceId::new(7));
    }
    #[test]
    fn account_label_roundtrips_canonical_literal() {
        let catalog = catalog();
        let label =
            AccountAlias::from_literal("Treasury@Banking.Retail", &catalog).expect("valid alias");
        assert_eq!(
            label.to_literal(&catalog).expect("literal"),
            "treasury@banking.retail"
        );
    }
    #[test]
    fn account_label_resolves_domain_id_with_dataspace_alias() {
        let alias =
            AccountAlias::from_literal("Treasury@Banking.Retail", &catalog()).expect("valid alias");
        assert_eq!(
            alias.domain_id(&catalog()).expect("resolve domain"),
            Some(DomainId::parse_fully_qualified("banking.retail").expect("domain id"))
        );
    }
    #[test]
    fn account_label_rejects_unknown_dataspace_alias() {
        let err = AccountAlias::from_literal("primary@banking.missing", &catalog())
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
                AccountAlias::from_literal(raw, &catalog()).is_err(),
                "must fail: {raw}"
            );
        }
    }
}
/// Provenance for one account-id transition retained by an [`AccountRekeyRecord`].
///
/// Entries are positional: entry `i` describes the transition from
/// `previous_account_ids[i]` to the next account id in the record. Only an
/// explicit [`Self::AccountIdRekey`] transition can carry controller continuity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum AccountRekeyTransitionProvenance {
    /// The stable alias was assigned to a different, independently controlled account.
    #[codec(index = 0)]
    AliasReassignment,
    /// The canonical account-id rekey operation retired the predecessor controller.
    #[codec(index = 1)]
    AccountIdRekey,
}
/// Structural failures in an account rekey record's transition history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AccountRekeyRecordError {
    /// The provenance vector is not aligned with the retained account-id history.
    #[error(
        "account rekey transition provenance has {provenance_count} entries for {account_id_count} previous account ids"
    )]
    TransitionProvenanceLength {
        /// Number of retained previous account ids.
        account_id_count: usize,
        /// Number of typed provenance entries.
        provenance_count: usize,
    },
}
/// Record that tracks the active concrete account behind a stable account label.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AccountRekeyRecord {
    /// Stable alias under which the account is addressed.
    pub label: AccountAlias,
    /// Current concrete account id behind the stable label.
    pub active_account_id: AccountId,
    /// Historical concrete account ids retained for continuity and audit trails.
    pub previous_account_ids: Vec<AccountId>,
    /// Current single-key signatory when the active account is directly key-controlled.
    ///
    /// Multisig-controlled accounts do not expose a single signatory, so this remains `None`
    /// for alias-backed multisig identities.
    #[norito(required)]
    pub active_signatory: Option<PublicKey>,
    /// Historical single-key signatories retained for audit trails.
    pub previous_signatories: Vec<PublicKey>,
    /// Typed provenance for every transition in `previous_account_ids`.
    ///
    /// Its length must exactly match `previous_account_ids`; records without history carry an
    /// explicit empty vector.
    pub transition_provenance: Vec<AccountRekeyTransitionProvenance>,
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
    pub fn new(label: AccountAlias, active_account_id: AccountId) -> Self {
        Self {
            label,
            active_signatory: active_account_id.try_signatory().cloned(),
            active_account_id,
            previous_account_ids: Vec::new(),
            previous_signatories: Vec::new(),
            transition_provenance: Vec::new(),
        }
    }
    fn validate_transition_provenance(&self) -> Result<(), AccountRekeyRecordError> {
        if self.transition_provenance.len() != self.previous_account_ids.len() {
            return Err(AccountRekeyRecordError::TransitionProvenanceLength {
                account_id_count: self.previous_account_ids.len(),
                provenance_count: self.transition_provenance.len(),
            });
        }
        Ok(())
    }
    fn repoint_to_account_with_provenance(
        &self,
        next_account_id: AccountId,
        provenance: AccountRekeyTransitionProvenance,
    ) -> Result<Self, AccountRekeyRecordError> {
        if self.active_account_id == next_account_id {
            self.validate_transition_provenance()?;
            return Ok(self.clone());
        }
        self.validate_transition_provenance()?;
        let active_signatory = next_account_id.try_signatory().cloned();
        let mut previous_account_ids = self.previous_account_ids.clone();
        previous_account_ids.push(self.active_account_id.clone());
        let mut transition_provenance = self.transition_provenance.clone();
        transition_provenance.push(provenance);
        let mut previous_signatories = self.previous_signatories.clone();
        if let Some(active_signatory) = self.active_signatory.as_ref() {
            previous_signatories.push(active_signatory.clone());
        }
        Ok(Self {
            label: self.label.clone(),
            active_account_id: next_account_id,
            previous_account_ids,
            active_signatory,
            previous_signatories,
            transition_provenance,
        })
    }
    /// Assign the stable alias to another independently controlled account.
    ///
    /// Alias reassignment retains audit history but deliberately breaks controller continuity.
    ///
    /// # Errors
    /// Returns an error when the existing typed history is structurally malformed.
    pub fn reassign_alias_to_account(
        &self,
        next_account_id: AccountId,
    ) -> Result<Self, AccountRekeyRecordError> {
        self.repoint_to_account_with_provenance(
            next_account_id,
            AccountRekeyTransitionProvenance::AliasReassignment,
        )
    }
    /// Record a canonical account-id rekey that retired the previous controller.
    ///
    /// This constructor records only provenance. Core must additionally enforce retirement and
    /// move state atomically at the canonical account-id mutation point.
    ///
    /// # Errors
    /// Returns an error when the existing typed history is structurally malformed.
    pub fn repoint_for_account_id_rekey(
        &self,
        next_account_id: AccountId,
    ) -> Result<Self, AccountRekeyRecordError> {
        self.repoint_to_account_with_provenance(
            next_account_id,
            AccountRekeyTransitionProvenance::AccountIdRekey,
        )
    }
    /// Return the consecutive, explicitly proven account-id rekey predecessors of the active id.
    ///
    /// An alias-reassignment entry breaks the active lineage. The returned slice is therefore
    /// always the maximal `AccountIdRekey` suffix ending at `active_account_id`.
    ///
    /// # Errors
    /// Returns an error when the provenance vector is not aligned with the account-id history.
    pub fn active_account_id_rekey_predecessors(
        &self,
    ) -> Result<&[AccountId], AccountRekeyRecordError> {
        self.validate_transition_provenance()?;
        let suffix_start = self
            .transition_provenance
            .iter()
            .rposition(|provenance| *provenance != AccountRekeyTransitionProvenance::AccountIdRekey)
            .map_or(0, |index| index + 1);
        Ok(&self.previous_account_ids[suffix_start..])
    }
    /// Plan a rotation to a new signatory-backed account, returning the staged record.
    ///
    /// # Errors
    /// Returns an error when the existing typed history is structurally malformed.
    pub fn rotate_to(&self, next_signatory: PublicKey) -> Result<Self, AccountRekeyRecordError> {
        self.repoint_for_account_id_rekey(AccountId::new(next_signatory))
    }
}
#[cfg(test)]
mod rekey_record_tests {
    use super::*;
    use iroha_crypto::KeyPair;
    use norito::codec::{DecodeAll, Encode};
    fn account_id() -> AccountId {
        AccountId::new(
            KeyPair::try_random()
                .expect("generate rekey fixture keypair")
                .public_key()
                .clone(),
        )
    }
    fn alias() -> AccountAlias {
        AccountAlias::domainless(
            "wire".parse().expect("account alias label"),
            crate::nexus::DataSpaceId::UNIVERSAL,
        )
    }
    #[cfg(feature = "json")]
    #[test]
    fn account_alias_json_requires_explicit_scope_fields() {
        let alias = alias();
        let value = norito::json::to_value(&alias).expect("serialize current account alias");
        let object = value.as_object().expect("account alias JSON object");
        assert!(object.contains_key("domain"));
        assert!(object.contains_key("dataspace"));
        for field in ["domain", "dataspace"] {
            let mut missing = value.clone();
            missing
                .as_object_mut()
                .expect("account alias JSON object")
                .remove(field);
            assert!(
                norito::json::from_value::<AccountAlias>(missing).is_err(),
                "current account alias JSON must reject missing `{field}`"
            );
        }
    }
    #[test]
    fn transition_provenance_wire_tags_are_first_release_exact() {
        for (provenance, tag) in [
            (AccountRekeyTransitionProvenance::AliasReassignment, 0_u32),
            (AccountRekeyTransitionProvenance::AccountIdRekey, 1_u32),
        ] {
            let encoded = provenance.encode();
            assert_eq!(encoded, tag.to_le_bytes());
            let mut bytes = encoded.as_slice();
            assert_eq!(
                AccountRekeyTransitionProvenance::decode_all(&mut bytes)
                    .expect("decode current provenance tag"),
                provenance
            );
        }
        let retired_tag = 2_u32.to_le_bytes();
        let mut bytes = retired_tag.as_slice();
        assert!(
            AccountRekeyTransitionProvenance::decode_all(&mut bytes).is_err(),
            "non-canonical provenance tag must reject"
        );
    }
    #[test]
    fn transition_history_requires_explicit_provenance() {
        let first = account_id();
        let active = account_id();
        let mut record = AccountRekeyRecord::new(alias(), first)
            .repoint_for_account_id_rekey(active)
            .expect("canonical account-id rekey");
        record.transition_provenance.clear();
        let expected = AccountRekeyRecordError::TransitionProvenanceLength {
            account_id_count: 1,
            provenance_count: 0,
        };
        assert_eq!(
            record
                .active_account_id_rekey_predecessors()
                .expect_err("missing provenance must reject lineage"),
            expected
        );
        assert_eq!(
            record
                .reassign_alias_to_account(account_id())
                .expect_err("missing provenance must reject another transition"),
            expected
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn rekey_record_json_requires_all_first_release_fields() {
        let record = AccountRekeyRecord::new(alias(), account_id());
        let value = norito::json::to_value(&record).expect("serialize current rekey record");
        let object = value.as_object().expect("rekey record JSON object");
        assert!(object.contains_key("active_signatory"));
        assert_eq!(
            object
                .get("transition_provenance")
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        for field in ["active_signatory", "transition_provenance"] {
            let mut missing = value.clone();
            missing
                .as_object_mut()
                .expect("rekey record JSON object")
                .remove(field);
            assert!(
                norito::json::from_value::<AccountRekeyRecord>(missing).is_err(),
                "current rekey record JSON must reject missing `{field}`"
            );
        }
    }
    #[test]
    fn alias_reassignment_breaks_the_active_account_id_rekey_suffix() {
        let first = account_id();
        let second = account_id();
        let active = account_id();
        let record = AccountRekeyRecord::new(alias(), first.clone())
            .reassign_alias_to_account(second.clone())
            .expect("alias reassignment")
            .repoint_for_account_id_rekey(active.clone())
            .expect("canonical account-id rekey");
        assert_eq!(record.previous_account_ids, vec![first, second.clone()]);
        assert_eq!(
            record.transition_provenance,
            vec![
                AccountRekeyTransitionProvenance::AliasReassignment,
                AccountRekeyTransitionProvenance::AccountIdRekey,
            ]
        );
        assert_eq!(
            record
                .active_account_id_rekey_predecessors()
                .expect("well-formed transition history"),
            &[second]
        );
        assert_eq!(record.active_account_id, active);
    }
}

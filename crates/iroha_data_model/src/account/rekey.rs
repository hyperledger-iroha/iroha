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
    #[norito(default)]
    pub domain: Option<AccountAliasDomain>,
    /// Dataspace in which the alias is registered.
    #[norito(default)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum AccountRekeyTransitionProvenance {
    /// Transition decoded from state written before provenance was recorded.
    ///
    /// Legacy history is retained for audit, but permanently remains non-authorizing.
    #[codec(index = 0)]
    #[default]
    LegacyUnspecified,
    /// The stable alias was assigned to a different, independently controlled account.
    #[codec(index = 1)]
    AliasReassignment,
    /// The canonical account-id rekey operation retired the predecessor controller.
    #[codec(index = 2)]
    AccountIdRekey,
}
/// Structural failures in an account rekey record's transition history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AccountRekeyRecordError {
    /// A non-legacy provenance vector is not aligned with the retained account-id history.
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
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub active_signatory: Option<PublicKey>,
    /// Historical single-key signatories retained for audit trails.
    pub previous_signatories: Vec<PublicKey>,
    /// Typed provenance for every transition in `previous_account_ids`.
    ///
    /// This trailing defaulted field preserves decoding of legacy persisted records. An empty
    /// vector paired with non-empty history is normalized deterministically to
    /// [`AccountRekeyTransitionProvenance::LegacyUnspecified`] during state rebuild and never
    /// authorizes controller continuity.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
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
    fn normalized_transition_provenance(
        &self,
    ) -> Result<Vec<AccountRekeyTransitionProvenance>, AccountRekeyRecordError> {
        if self.transition_provenance.is_empty() {
            return Ok(vec![
                AccountRekeyTransitionProvenance::LegacyUnspecified;
                self.previous_account_ids.len()
            ]);
        }
        if self.transition_provenance.len() != self.previous_account_ids.len() {
            return Err(AccountRekeyRecordError::TransitionProvenanceLength {
                account_id_count: self.previous_account_ids.len(),
                provenance_count: self.transition_provenance.len(),
            });
        }
        Ok(self.transition_provenance.clone())
    }
    fn repoint_to_account_with_provenance(
        &self,
        next_account_id: AccountId,
        provenance: AccountRekeyTransitionProvenance,
    ) -> Result<Self, AccountRekeyRecordError> {
        if self.active_account_id == next_account_id {
            self.normalized_transition_provenance()?;
            return Ok(self.clone());
        }
        let active_signatory = next_account_id.try_signatory().cloned();
        let mut previous_account_ids = self.previous_account_ids.clone();
        previous_account_ids.push(self.active_account_id.clone());
        let mut transition_provenance = self.normalized_transition_provenance()?;
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
    /// Normalize a decoded legacy history to explicit non-authorizing provenance.
    ///
    /// This is intended for the canonical state rebuild path. Ordinary decoding leaves missing
    /// trailing provenance untouched so historical wire bytes retain their original meaning.
    ///
    /// # Errors
    /// Returns an error for a partially populated, non-legacy provenance vector.
    pub fn normalize_legacy_transition_provenance(
        &mut self,
    ) -> Result<(), AccountRekeyRecordError> {
        self.transition_provenance = self.normalized_transition_provenance()?;
        Ok(())
    }
    /// Return the consecutive, explicitly proven account-id rekey predecessors of the active id.
    ///
    /// A legacy or alias-reassignment entry breaks the active lineage. The returned slice is
    /// therefore always the maximal `AccountIdRekey` suffix ending at `active_account_id`.
    ///
    /// # Errors
    /// Returns an error for a partially populated provenance vector. A completely missing legacy
    /// vector is accepted as an all-legacy, non-authorizing history.
    pub fn active_account_id_rekey_predecessors(
        &self,
    ) -> Result<&[AccountId], AccountRekeyRecordError> {
        if self.transition_provenance.is_empty() {
            return Ok(&self.previous_account_ids[self.previous_account_ids.len()..]);
        }
        if self.transition_provenance.len() != self.previous_account_ids.len() {
            return Err(AccountRekeyRecordError::TransitionProvenanceLength {
                account_id_count: self.previous_account_ids.len(),
                provenance_count: self.transition_provenance.len(),
            });
        }
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
    #[test]
    fn legacy_wire_defaults_transition_provenance_without_authorizing_it() {
        #[derive(Encode)]
        struct LegacyAccountRekeyRecord {
            label: AccountAlias,
            active_account_id: AccountId,
            previous_account_ids: Vec<AccountId>,
            active_signatory: Option<PublicKey>,
            previous_signatories: Vec<PublicKey>,
        }
        let previous = account_id();
        let active = account_id();
        let legacy = LegacyAccountRekeyRecord {
            label: alias(),
            active_account_id: active.clone(),
            previous_account_ids: vec![previous],
            active_signatory: active.try_signatory().cloned(),
            previous_signatories: Vec::new(),
        };
        let encoded = legacy.encode();
        let mut bytes = encoded.as_slice();
        let mut decoded = AccountRekeyRecord::decode_all(&mut bytes)
            .expect("legacy account rekey record must decode");
        assert!(bytes.is_empty());
        assert!(decoded.transition_provenance.is_empty());
        assert!(
            decoded
                .active_account_id_rekey_predecessors()
                .expect("missing legacy provenance is structurally valid")
                .is_empty(),
            "legacy history must remain non-authorizing before rebuild"
        );
        decoded
            .normalize_legacy_transition_provenance()
            .expect("rebuild normalization");
        assert_eq!(
            decoded.transition_provenance,
            vec![AccountRekeyTransitionProvenance::LegacyUnspecified]
        );
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

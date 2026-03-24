//! Ledger-backed SNS storage helpers.
//!
//! This module provides a minimal authoritative view over SNS name records
//! persisted in `World.smart_contract_state`. The higher-level mutation API is
//! still being wired through ISI/Torii, but ownership-sensitive checks use this
//! module as the single read path so lifecycle handling and selector hashing stay
//! consistent.

use std::str::FromStr;

use hex;
use iroha_data_model::{
    account::{AccountId, rekey::AccountLabel},
    domain::DomainId,
    name::Name,
    nexus::{DataSpaceCatalog, DataSpaceId},
    sns::{
        NameRecordV1, NameSelectorError, NameSelectorV1, NameStatus, NameTombstoneStateV1, SuffixId,
    },
};
use mv::storage::StorageReadOnly;
use norito::codec::Decode as _;

use crate::state::WorldReadOnly;

/// Fixed SNS suffix id for full account-alias lease records.
pub const ACCOUNT_ALIAS_SUFFIX_ID: SuffixId = 0x1001;
/// Fixed SNS suffix id for domain-name lease records.
pub const DOMAIN_NAME_SUFFIX_ID: SuffixId = 0x1002;
/// Fixed SNS suffix id for dataspace-alias lease records.
pub const DATASPACE_ALIAS_SUFFIX_ID: SuffixId = 0x1003;

/// Reserved dataspace alias that must stay permanently defined.
pub const RESERVED_UNIVERSAL_DATASPACE_ALIAS: &str = "universal";

/// SNS namespaces used by the authoritative name-record storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnsNamespace {
    /// Full account-alias key (`label@domain.dataspace` or `label@dataspace`).
    AccountAlias,
    /// Canonical domain literal.
    Domain,
    /// Canonical dataspace alias.
    Dataspace,
}

impl SnsNamespace {
    /// Stable suffix identifier assigned to this namespace.
    #[must_use]
    pub const fn suffix_id(self) -> SuffixId {
        match self {
            Self::AccountAlias => ACCOUNT_ALIAS_SUFFIX_ID,
            Self::Domain => DOMAIN_NAME_SUFFIX_ID,
            Self::Dataspace => DATASPACE_ALIAS_SUFFIX_ID,
        }
    }
}

/// Compute the durable smart-contract-state key for a SNS record selector.
#[must_use]
pub fn record_storage_key(selector: &NameSelectorV1) -> Name {
    Name::from_str(&format!(
        "sns/records/{}/{}",
        selector.suffix_id,
        hex::encode(selector.name_hash())
    ))
    .expect("static SNS storage key format is a valid Name")
}

/// Build the selector used for a full account-alias lease record.
pub fn selector_for_account_alias(
    alias: &AccountLabel,
    catalog: &DataSpaceCatalog,
) -> Result<NameSelectorV1, iroha_data_model::error::ParseError> {
    Ok(NameSelectorV1 {
        version: NameSelectorV1::VERSION,
        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
        // `AccountLabel::to_literal` already canonicalizes and lowercases the
        // alias boundary format, so we can store the literal directly here even
        // though `NameSelectorV1::new` rejects `@`.
        label: alias.to_literal(catalog)?,
    })
}

/// Build the selector used for a canonical domain-name lease record.
pub fn selector_for_domain(domain: &DomainId) -> Result<NameSelectorV1, NameSelectorError> {
    NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain.name().as_ref())
}

/// Build the selector used for a canonical dataspace-alias lease record.
pub fn selector_for_dataspace_alias(alias: &str) -> Result<NameSelectorV1, NameSelectorError> {
    NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, alias)
}

/// Decode a SNS record from world state for the supplied selector.
#[must_use]
pub fn record_by_selector(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
) -> Option<NameRecordV1> {
    let key = record_storage_key(selector);
    let bytes = world.smart_contract_state().get(&key)?;
    let mut slice = bytes.as_slice();
    NameRecordV1::decode(&mut slice).ok()
}

/// Compute the effective lifecycle for `record` using deterministic ledger time.
#[must_use]
pub fn effective_status(record: &NameRecordV1, now_ms: u64) -> NameStatus {
    if matches!(record.status, NameStatus::Tombstoned(_)) {
        return record.status.clone();
    }
    if let NameStatus::Frozen(frozen) = &record.status
        && now_ms < frozen.until_ms
    {
        return record.status.clone();
    }

    if now_ms >= record.redemption_expires_at_ms {
        NameStatus::Tombstoned(NameTombstoneStateV1 {
            reason: "expired".to_owned(),
        })
    } else if now_ms >= record.grace_expires_at_ms {
        NameStatus::Redemption
    } else if now_ms >= record.expires_at_ms {
        NameStatus::GracePeriod
    } else {
        NameStatus::Active
    }
}

/// Return the active owner for a SNS selector when the record lifecycle is `Active`.
#[must_use]
pub fn active_owner_by_selector(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
    now_ms: u64,
) -> Option<AccountId> {
    let record = record_by_selector(world, selector)?;
    matches!(effective_status(&record, now_ms), NameStatus::Active).then_some(record.owner)
}

/// Return the active owner for a full account-alias lease record.
#[must_use]
pub fn active_account_alias_owner(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountLabel,
    now_ms: u64,
) -> Option<AccountId> {
    let selector = selector_for_account_alias(alias, catalog).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
}

/// Return the active owner for a domain-name lease record.
#[must_use]
pub fn active_domain_owner(
    world: &impl WorldReadOnly,
    domain: &DomainId,
    now_ms: u64,
) -> Option<AccountId> {
    let selector = selector_for_domain(domain).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
}

/// Return the active owner for a canonical dataspace alias.
#[must_use]
pub fn active_dataspace_owner_by_alias(
    world: &impl WorldReadOnly,
    alias: &str,
    now_ms: u64,
) -> Option<AccountId> {
    let selector = selector_for_dataspace_alias(alias).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
}

/// Resolve the active owner for the dataspace id using the current catalog alias.
#[must_use]
pub fn active_dataspace_owner_by_id(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    dataspace_id: DataSpaceId,
    now_ms: u64,
) -> Option<AccountId> {
    let alias = catalog.by_id(dataspace_id)?.alias.as_str();
    active_dataspace_owner_by_alias(world, alias, now_ms)
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        account::{AccountAddress, AccountId},
        metadata::Metadata,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
        sns::{NameControllerV1, NameRecordV1, NameSelectorV1, NameStatus},
    };

    use super::*;
    use crate::state::World;

    fn owner() -> AccountId {
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
    }

    fn dataspace_catalog() -> DataSpaceCatalog {
        DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "banking".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("catalog")
    }

    #[test]
    fn account_alias_selector_uses_canonical_literal() {
        let catalog = dataspace_catalog();
        let alias =
            AccountLabel::domainless("treasury".parse().expect("label"), DataSpaceId::new(7));

        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");

        assert_eq!(selector.suffix_id, ACCOUNT_ALIAS_SUFFIX_ID);
        assert_eq!(selector.label, "treasury@banking");
    }

    #[test]
    fn active_dataspace_owner_reads_from_world_storage() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let owner = owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );

        let mut world = World::default();
        world.smart_contract_state_mut_for_testing().insert(
            record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
        let view = world.view();

        assert_eq!(
            active_dataspace_owner_by_id(&view, &catalog, DataSpaceId::new(7), 50),
            Some(owner)
        );
    }

    #[test]
    fn active_owner_rejects_non_active_lifecycle_states() {
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, "banking").expect("selector");
        let owner = owner();
        let record = NameRecordV1::new(
            selector,
            owner.clone(),
            Vec::new(),
            0,
            10,
            20,
            30,
            40,
            Metadata::default(),
        );

        assert!(matches!(effective_status(&record, 10), NameStatus::Active));
        assert!(matches!(
            effective_status(&record, 25),
            NameStatus::GracePeriod
        ));
        assert!(matches!(
            effective_status(&record, 35),
            NameStatus::Redemption
        ));
        assert!(matches!(
            effective_status(&record, 45),
            NameStatus::Tombstoned(_)
        ));
    }
}

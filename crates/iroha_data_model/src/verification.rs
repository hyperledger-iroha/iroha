//! Formal verification helpers for the Iroha data model.

use std::{
    collections::btree_map::{BTreeMap, Entry},
    format,
    string::String,
    vec::Vec,
};

use iroha_primitives::numeric::Numeric;

use crate::{
    account::{Account, AccountId},
    asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
    domain::{Domain, DomainId},
};

const INVARIANT_DOMAIN_UNIQUE: &str = "domain.unique_id";
const INVARIANT_DOMAIN_OWNER_EXISTS: &str = "domain.owner_exists";
const INVARIANT_DOMAIN_OWNER_MATCH: &str = "domain.owner_domain_matches";
const INVARIANT_ACCOUNT_UNIQUE: &str = "account.unique_id";
const INVARIANT_ACCOUNT_DOMAIN: &str = "account.domain_exists";
const INVARIANT_ASSET_DEF_UNIQUE: &str = "asset_definition.unique_id";
const INVARIANT_ASSET_DEF_DOMAIN: &str = "asset_definition.domain_exists";
const INVARIANT_ASSET_DEF_OWNER_EXISTS: &str = "asset_definition.owner_exists";
const INVARIANT_ASSET_DEF_OWNER_MATCH: &str = "asset_definition.owner_domain_matches";
const INVARIANT_ASSET_DEF_TOTAL_MATCH: &str = "asset_definition.total_quantity_matches_assets";
const INVARIANT_ASSET_UNIQUE: &str = "asset.unique_id";
const INVARIANT_ASSET_ACCOUNT_EXISTS: &str = "asset.account_exists";
const INVARIANT_ASSET_DEF_EXISTS: &str = "asset.definition_exists";
const INVARIANT_ASSET_VALUE_SPEC: &str = "asset.value_matches_spec";
const INVARIANT_ASSET_TOTAL_OVERFLOW: &str = "asset.total_quantity_accumulates";

type DomainIndex<'a> = BTreeMap<DomainId, &'a Domain>;
type AccountIndex<'a> = BTreeMap<AccountId, &'a Account>;
type AssetDefinitionIndex<'a> = BTreeMap<AssetDefinitionId, &'a AssetDefinition>;

/// Verification failure for a specific invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Identifier of the invariant that failed.
    pub invariant: &'static str,
    /// Human-readable description of the failure.
    pub detail: String,
}

/// Accumulates the outcome of executing verification checks.
#[derive(Debug, Clone, Default)]
pub struct VerificationReport {
    total_checks: usize,
    violations: Vec<Violation>,
}

impl VerificationReport {
    /// Create an empty report.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_checks: 0,
            violations: Vec::new(),
        }
    }

    /// Record a successful invariant evaluation.
    pub fn ok(&mut self, invariant: &'static str) {
        let _ = invariant;
        self.total_checks = self.total_checks.saturating_add(1);
    }

    /// Record a failed invariant evaluation with context.
    pub fn violation(&mut self, invariant: &'static str, detail: impl Into<String>) {
        self.total_checks = self.total_checks.saturating_add(1);
        self.violations.push(Violation {
            invariant,
            detail: detail.into(),
        });
    }

    /// Returns `true` when no violations were observed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.violations.is_empty()
    }

    /// Total number of invariant evaluations completed.
    #[must_use]
    pub fn total_checks(&self) -> usize {
        self.total_checks
    }

    /// Collected verification failures.
    #[must_use]
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }
}

/// Borrowed snapshot of the world state required for verification.
#[derive(Debug, Clone, Copy)]
pub struct WorldSnapshot<'a> {
    /// Domains registered in the snapshot.
    pub domains: &'a [Domain],
    /// Accounts registered in the snapshot.
    pub accounts: &'a [Account],
    /// Asset definitions registered in the snapshot.
    pub asset_definitions: &'a [AssetDefinition],
    /// Assets held in the snapshot.
    pub assets: &'a [Asset],
}

impl WorldSnapshot<'_> {
    /// Execute all built-in verification checks on the snapshot.
    #[must_use]
    pub fn verify(self) -> VerificationReport {
        let mut report = VerificationReport::new();

        let domains = index_domains(self.domains, &mut report);
        let accounts = index_accounts(self.accounts, &domains, &mut report);
        let definitions =
            index_asset_definitions(self.asset_definitions, &domains, &accounts, &mut report);
        let totals = index_assets(self.assets, &accounts, &definitions, &mut report);

        verify_domain_owners(&domains, &accounts, &mut report);
        verify_asset_totals(&definitions, &totals, &mut report);

        report
    }
}

fn index_domains<'a>(domains: &'a [Domain], report: &mut VerificationReport) -> DomainIndex<'a> {
    let mut index = DomainIndex::new();

    for domain in domains {
        let id = domain.id.clone();
        match index.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(domain);
                report.ok(INVARIANT_DOMAIN_UNIQUE);
            }
            Entry::Occupied(_) => {
                report.violation(
                    INVARIANT_DOMAIN_UNIQUE,
                    format!("duplicate domain identifier `{id}`"),
                );
            }
        }
    }

    index
}

fn index_accounts<'a>(
    accounts: &'a [Account],
    domains: &DomainIndex<'a>,
    report: &mut VerificationReport,
) -> AccountIndex<'a> {
    let mut index = AccountIndex::new();

    for account in accounts {
        let id = account.id.clone();
        match index.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(account);
                report.ok(INVARIANT_ACCOUNT_UNIQUE);
            }
            Entry::Occupied(_) => {
                report.violation(
                    INVARIANT_ACCOUNT_UNIQUE,
                    format!("duplicate account identifier `{id}`"),
                );
            }
        }

        if domains.contains_key(account.id.domain()) {
            report.ok(INVARIANT_ACCOUNT_DOMAIN);
        } else {
            report.violation(
                INVARIANT_ACCOUNT_DOMAIN,
                format!(
                    "account `{}` references unknown domain `{}`",
                    id,
                    account.id.domain()
                ),
            );
        }
    }

    index
}

fn index_asset_definitions<'a>(
    definitions: &'a [AssetDefinition],
    domains: &DomainIndex<'a>,
    accounts: &AccountIndex<'a>,
    report: &mut VerificationReport,
) -> AssetDefinitionIndex<'a> {
    let mut index = AssetDefinitionIndex::new();

    for definition in definitions {
        let id = definition.id.clone();
        match index.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(definition);
                report.ok(INVARIANT_ASSET_DEF_UNIQUE);
            }
            Entry::Occupied(_) => {
                report.violation(
                    INVARIANT_ASSET_DEF_UNIQUE,
                    format!("duplicate asset definition identifier `{id}`"),
                );
            }
        }

        if domains.contains_key(id.domain()) {
            report.ok(INVARIANT_ASSET_DEF_DOMAIN);
        } else {
            report.violation(
                INVARIANT_ASSET_DEF_DOMAIN,
                format!(
                    "asset definition `{id}` references unknown domain `{domain}`",
                    domain = id.domain()
                ),
            );
        }

        if accounts.contains_key(definition.owned_by()) {
            report.ok(INVARIANT_ASSET_DEF_OWNER_EXISTS);
        } else {
            report.violation(
                INVARIANT_ASSET_DEF_OWNER_EXISTS,
                format!(
                    "asset definition `{id}` references unknown owner `{owner}`",
                    owner = definition.owned_by()
                ),
            );
        }

        if let Some(owner) = accounts.get(definition.owned_by()) {
            if owner.id.domain() == id.domain() {
                report.ok(INVARIANT_ASSET_DEF_OWNER_MATCH);
            } else {
                report.violation(
                    INVARIANT_ASSET_DEF_OWNER_MATCH,
                    format!(
                        "asset definition `{id}` owner `{owner}` belongs to domain `{domain}`",
                        owner = definition.owned_by(),
                        domain = owner.id.domain()
                    ),
                );
            }
        }
    }

    index
}

fn index_assets<'a>(
    assets: &'a [Asset],
    accounts: &AccountIndex<'a>,
    definitions: &AssetDefinitionIndex<'a>,
    report: &mut VerificationReport,
) -> BTreeMap<AssetDefinitionId, Numeric> {
    let mut index: BTreeMap<AssetId, &Asset> = BTreeMap::new();
    let mut totals: BTreeMap<AssetDefinitionId, Numeric> = BTreeMap::new();

    for asset in assets {
        let id = asset.id.clone();
        match index.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(asset);
                report.ok(INVARIANT_ASSET_UNIQUE);
            }
            Entry::Occupied(_) => {
                report.violation(
                    INVARIANT_ASSET_UNIQUE,
                    format!("duplicate asset identifier `{id}`"),
                );
            }
        }

        if accounts.contains_key(id.account()) {
            report.ok(INVARIANT_ASSET_ACCOUNT_EXISTS);
        } else {
            report.violation(
                INVARIANT_ASSET_ACCOUNT_EXISTS,
                format!(
                    "asset `{id}` references unknown account `{account}`",
                    account = id.account()
                ),
            );
        }

        if let Some(definition) = definitions.get(id.definition()) {
            report.ok(INVARIANT_ASSET_DEF_EXISTS);

            let spec = definition.spec();
            match spec.check(asset.value()) {
                Ok(()) => report.ok(INVARIANT_ASSET_VALUE_SPEC),
                Err(err) => report.violation(
                    INVARIANT_ASSET_VALUE_SPEC,
                    format!(
                        "asset `{id}` value `{}` violates spec `{spec}`: {err}",
                        asset.value()
                    ),
                ),
            }

            let value = asset.value().clone();
            let entry = totals
                .entry(id.definition().clone())
                .or_insert_with(Numeric::zero);
            let current = entry.clone();
            if let Some(sum) = current.checked_add(value.clone()) {
                *entry = sum;
                report.ok(INVARIANT_ASSET_TOTAL_OVERFLOW);
            } else {
                report.violation(
                    INVARIANT_ASSET_TOTAL_OVERFLOW,
                    format!("accumulating asset `{id}` with value `{value}` overflowed totals"),
                );
            }
        } else {
            report.violation(
                INVARIANT_ASSET_DEF_EXISTS,
                format!(
                    "asset `{id}` references unknown definition `{definition}`",
                    definition = id.definition()
                ),
            );
        }
    }

    totals
}

fn verify_domain_owners<'a>(
    domains: &DomainIndex<'a>,
    accounts: &AccountIndex<'a>,
    report: &mut VerificationReport,
) {
    for (id, domain) in domains {
        if let Some(owner) = accounts.get(domain.owned_by()) {
            report.ok(INVARIANT_DOMAIN_OWNER_EXISTS);
            if owner.id.domain() == id {
                report.ok(INVARIANT_DOMAIN_OWNER_MATCH);
            } else {
                report.violation(
                    INVARIANT_DOMAIN_OWNER_MATCH,
                    format!(
                        "domain `{id}` owner `{}` belongs to domain `{}`",
                        domain.owned_by(),
                        owner.id.domain()
                    ),
                );
            }
        } else {
            report.violation(
                INVARIANT_DOMAIN_OWNER_EXISTS,
                format!(
                    "domain `{id}` references unknown owner `{}`",
                    domain.owned_by()
                ),
            );
        }
    }
}

fn verify_asset_totals(
    definitions: &AssetDefinitionIndex<'_>,
    totals: &BTreeMap<AssetDefinitionId, Numeric>,
    report: &mut VerificationReport,
) {
    for (id, definition) in definitions {
        let observed = totals.get(id).cloned().unwrap_or_else(Numeric::zero);
        let expected_total = definition.total_quantity().clone();
        if observed == expected_total {
            report.ok(INVARIANT_ASSET_DEF_TOTAL_MATCH);
        } else {
            report.violation(
                INVARIANT_ASSET_DEF_TOTAL_MATCH,
                format!(
                    "asset definition `{id}` tracks total `{expected_total}` but observed `{observed}`"
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iroha_crypto::KeyPair;
    use iroha_primitives::numeric::NumericSpec;

    use super::*;
    use crate::{
        asset::{Mintable, definition::AssetConfidentialPolicy},
        metadata::Metadata,
    };

    #[test]
    fn valid_snapshot_passes_formal_verification() {
        let domain_id: DomainId = "wonderland".parse().expect("valid domain id");
        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());

        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: account_id.clone(),
        };

        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };

        let definition_id: AssetDefinitionId = "rose#wonderland"
            .parse()
            .expect("valid asset definition id");
        let asset_definition = AssetDefinition {
            id: definition_id.clone(),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            owned_by: account_id.clone(),
            total_quantity: Numeric::new(10, 0),
            confidential_policy: AssetConfidentialPolicy::default(),
        };

        let asset = Asset {
            id: AssetId::new(definition_id.clone(), account_id.clone()),
            value: Numeric::new(10, 0),
        };

        let domains = vec![domain];
        let accounts = vec![account];
        let definitions = vec![asset_definition];
        let assets = vec![asset];

        let snapshot = WorldSnapshot {
            domains: &domains,
            accounts: &accounts,
            asset_definitions: &definitions,
            assets: &assets,
        };

        let report = snapshot.verify();
        assert!(report.is_success());
        assert_eq!(report.violations().len(), 0);
        assert!(report.total_checks() > 0);
    }

    #[test]
    fn snapshot_with_inconsistencies_reports_violations() {
        let domain_id: DomainId = "wonderland".parse().expect("valid domain id");
        let missing_owner_key = KeyPair::random();
        let missing_owner =
            AccountId::new(domain_id.clone(), missing_owner_key.public_key().clone());
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: missing_owner.clone(),
        };

        let foreign_domain: DomainId = "elsewhere".parse().expect("valid domain id");
        let account_key = KeyPair::random();
        let account_id = AccountId::new(foreign_domain.clone(), account_key.public_key().clone());
        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };

        let definition_id: AssetDefinitionId = "rose#wonderland"
            .parse()
            .expect("valid asset definition id");
        let asset_definition = AssetDefinition {
            id: definition_id.clone(),
            spec: NumericSpec::integer(),
            mintable: Mintable::Once,
            logo: None,
            metadata: Metadata::default(),
            owned_by: missing_owner,
            total_quantity: Numeric::new(5, 0),
            confidential_policy: AssetConfidentialPolicy::default(),
        };

        let asset = Asset {
            id: AssetId::new(definition_id.clone(), account_id.clone()),
            value: Numeric::new(5, 1),
        };

        let domains = vec![domain];
        let accounts = vec![account];
        let definitions = vec![asset_definition];
        let assets = vec![asset];

        let snapshot = WorldSnapshot {
            domains: &domains,
            accounts: &accounts,
            asset_definitions: &definitions,
            assets: &assets,
        };

        let report = snapshot.verify();
        assert!(!report.is_success());

        let invariants: BTreeSet<_> = report
            .violations()
            .iter()
            .map(|violation| violation.invariant)
            .collect();

        assert!(invariants.contains(&INVARIANT_DOMAIN_OWNER_EXISTS));
        assert!(invariants.contains(&INVARIANT_ACCOUNT_DOMAIN));
        assert!(invariants.contains(&INVARIANT_ASSET_VALUE_SPEC));
        assert!(invariants.contains(&INVARIANT_ASSET_DEF_TOTAL_MATCH));
    }
}

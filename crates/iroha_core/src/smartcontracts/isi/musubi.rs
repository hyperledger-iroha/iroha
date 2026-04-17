//! Musubi package registry instruction and query handlers.

use std::{collections::BTreeMap, str::FromStr};

use iroha_data_model::{
    domain::DomainId,
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        musubi::{
            AssertMusubiReleaseExists, PublishMusubiRelease, SetMusubiShortAlias, YankMusubiRelease,
        },
    },
    musubi::{
        MusubiPackageId, MusubiPackageRef, MusubiPackageSummary, MusubiRelease,
        MusubiReleaseStatus, MusubiReleaseSummary, MusubiVersion,
    },
    name::Name,
    query::{error::QueryExecutionFail, musubi::prelude::*},
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};

use super::prelude::*;
use crate::{
    prelude::ValidSingularQuery,
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

const RELEASE_KEY_PREFIX: &str = "musubi_release_";
const SHORT_ALIAS_KEY_PREFIX: &str = "musubi_alias_";
const PACKAGE_RELEASE_INDEX_PREFIX: &str = "musubi_release_index_";
const PACKAGE_CATALOG_KEY: &str = "musubi_package_catalog";
const SEARCH_LIMIT_CAP: usize = 1_000;

impl Execute for PublishMusubiRelease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut release = self.release;
        release.published_by = authority.clone();
        release.published_at_ms = state_transaction.block_unix_timestamp_ms();
        release.status = MusubiReleaseStatus::Active;
        release
            .validate_publishable()
            .map_err(|err| invalid_parameter(err.reason()))?;
        ensure_namespace_authority(&release.package.package, authority, state_transaction)?;
        ensure_dependencies_exist(&release, state_transaction)?;
        ensure_sorafs_pin_active(&release, state_transaction)?;
        ensure_dapp_contracts_exist(&release, state_transaction)?;

        let key = release_key(&release.package);
        if state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .is_some()
        {
            return Err(Error::InvariantViolation(
                format!("Musubi release `{}` already exists", release.package).into(),
            ));
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(key, release.encode());
        index_published_release(&release, state_transaction);
        Ok(())
    }
}

impl Execute for YankMusubiRelease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_namespace_authority(&self.package.package, authority, state_transaction)?;
        let key = release_key(&self.package);
        let bytes = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                Error::InvariantViolation(
                    format!("Musubi release `{}` not found", self.package).into(),
                )
            })?;
        let mut release = decode_release_for_instruction(&bytes)?;
        if !release.status.is_active() {
            return Err(Error::InvariantViolation(
                format!("Musubi release `{}` is already yanked", self.package).into(),
            ));
        }
        release.yank(self.reason, state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .smart_contract_state
            .insert(key, release.encode());
        Ok(())
    }
}

impl Execute for SetMusubiShortAlias {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_permission(state_transaction, authority, "CanSetMusubiShortAlias")?;
        if package_releases_in_world(state_transaction.world(), &self.alias.target, false)
            .is_empty()
        {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi short alias `{}` targets package `{}` with no active releases",
                    self.alias.alias, self.alias.target
                )
                .into(),
            ));
        }
        let key = short_alias_key(&self.alias.alias);
        if let Some(existing) = state_transaction.world.smart_contract_state.get(&key) {
            let existing = decode_package_id_for_instruction(existing)?;
            if existing != self.alias.target {
                return Err(Error::InvariantViolation(
                    format!(
                        "Musubi short alias `{}` already targets `{}`",
                        self.alias.alias, existing
                    )
                    .into(),
                ));
            }
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(key, self.alias.target.encode());
        Ok(())
    }
}

impl Execute for AssertMusubiReleaseExists {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let package_ref = MusubiPackageRef::new(self.package, self.version);
        if state_transaction
            .world
            .smart_contract_state
            .get(&release_key(&package_ref))
            .is_some()
        {
            Ok(())
        } else {
            Err(Error::InvariantViolation(
                format!("Musubi release `{package_ref}` not found").into(),
            ))
        }
    }
}

impl ValidSingularQuery for FindMusubiReleaseByRef {
    fn execute(&self, state_ro: &impl StateReadOnly) -> Result<MusubiRelease, QueryExecutionFail> {
        let key = release_key(&self.package);
        let bytes = state_ro
            .world()
            .smart_contract_state()
            .get(&key)
            .ok_or(QueryExecutionFail::NotFound)?;
        decode_release_for_query(bytes)
    }
}

impl ValidSingularQuery for FindMusubiPackageVersions {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<Vec<MusubiVersion>, QueryExecutionFail> {
        let mut versions = package_versions_in_world(state_ro.world(), &self.package);
        versions.sort();
        versions.dedup();
        Ok(versions)
    }
}

impl ValidSingularQuery for FindMusubiPackageReleases {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<Vec<MusubiReleaseSummary>, QueryExecutionFail> {
        let releases =
            package_releases_in_world(state_ro.world(), &self.package, self.include_yanked);
        Ok(releases
            .iter()
            .map(MusubiReleaseSummary::from_release)
            .collect())
    }
}

impl ValidSingularQuery for SearchMusubiPackages {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<Vec<MusubiPackageSummary>, QueryExecutionFail> {
        let mut packages = package_summaries_in_world(
            state_ro.world(),
            self.namespace.as_ref(),
            &self.query,
            self.include_yanked,
        );
        packages.sort_by(|left, right| left.package.cmp(&right.package));
        let offset = usize::try_from(self.offset).unwrap_or(usize::MAX);
        let limit = musubi_search_limit(self.limit);
        Ok(packages.into_iter().skip(offset).take(limit).collect())
    }
}

fn musubi_search_limit(limit: u32) -> usize {
    usize::try_from(limit)
        .unwrap_or(usize::MAX)
        .min(SEARCH_LIMIT_CAP)
}

impl ValidSingularQuery for FindMusubiShortAliasByName {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiPackageId, QueryExecutionFail> {
        let key = short_alias_key(&self.alias);
        let bytes = state_ro
            .world()
            .smart_contract_state()
            .get(&key)
            .ok_or(QueryExecutionFail::NotFound)?;
        decode_package_id_for_query(bytes)
    }
}

fn ensure_dependencies_exist(
    release: &MusubiRelease,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let mut selected_versions = BTreeMap::<MusubiPackageId, MusubiVersion>::new();
    for dependency in &release.dependencies {
        if dependency.package.package == release.package.package {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi release `{}` cannot depend on its own package id",
                    release.package
                )
                .into(),
            ));
        }
        if let Some(previous) = selected_versions.insert(
            dependency.package.package.clone(),
            dependency.package.version.clone(),
        ) && previous != dependency.package.version
        {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi release `{}` selects multiple versions of `{}`",
                    release.package, dependency.package.package
                )
                .into(),
            ));
        }
        let key = release_key(&dependency.package);
        let bytes = state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .ok_or_else(|| {
                Error::InvariantViolation(
                    format!(
                        "Musubi dependency `{}` is not published",
                        dependency.package
                    )
                    .into(),
                )
            })?;
        let dependency_release = decode_release_for_instruction(bytes)?;
        if !dependency_release.status.is_active() {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi dependency `{}` is yanked and cannot be selected by new releases",
                    dependency.package
                )
                .into(),
            ));
        }
    }
    Ok(())
}

fn ensure_sorafs_pin_active(
    release: &MusubiRelease,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let digest = release.archive.sorafs_manifest;
    let record = state_transaction
        .world
        .pin_manifests
        .get(&digest)
        .ok_or_else(|| {
            Error::InvariantViolation(
                format!(
                    "Musubi release `{}` references unregistered SoraFS manifest {}",
                    release.package,
                    hex::encode(digest.as_bytes())
                )
                .into(),
            )
        })?;
    if record.status.is_active() {
        Ok(())
    } else {
        Err(Error::InvariantViolation(
            format!(
                "Musubi release `{}` references inactive SoraFS manifest {}",
                release.package,
                hex::encode(digest.as_bytes())
            )
            .into(),
        ))
    }
}

fn ensure_dapp_contracts_exist(
    release: &MusubiRelease,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let Some(dapp) = release.dapp.as_ref() else {
        return Ok(());
    };
    let now_ms = state_transaction.block_unix_timestamp_ms();
    for alias in &dapp.contracts {
        if state_transaction
            .world
            .contract_address_by_alias_at(alias, now_ms)
            .is_none()
        {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi dapp link for `{}` references unknown or expired contract alias `{alias}`",
                    release.package
                )
                .into(),
            ));
        }
    }
    Ok(())
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> bool {
    state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|perms| perms.iter().any(|perm| perm.name() == permission))
}

fn require_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> Result<(), Error> {
    if has_permission(state_transaction, authority, permission) {
        Ok(())
    } else {
        Err(Error::InvariantViolation(
            format!("permission {permission} required for Musubi registry operation").into(),
        ))
    }
}

fn ensure_namespace_authority(
    package: &MusubiPackageId,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let namespace = &package.namespace;
    if let Some(domain) = namespace.domain_segment() {
        let domain_id = DomainId::try_new(domain, namespace.dataspace_segment())
            .map_err(|err| invalid_parameter(err.reason()))?;
        let domain = state_transaction
            .world
            .domains()
            .get(&domain_id)
            .ok_or_else(|| {
                Error::InvariantViolation(
                    format!("Musubi namespace domain `{domain_id}` is not registered").into(),
                )
            })?;
        if domain.owned_by() == authority {
            Ok(())
        } else {
            Err(Error::InvariantViolation(
                format!("authority `{authority}` does not own Musubi namespace `{namespace}`")
                    .into(),
            ))
        }
    } else {
        let owner = crate::sns::active_dataspace_owner_by_alias(
            state_transaction.world(),
            namespace.dataspace_segment(),
            state_transaction.block_unix_timestamp_ms(),
        )
        .ok_or_else(|| {
            Error::InvariantViolation(
                format!(
                    "Musubi namespace dataspace `{}` has no active SNS owner",
                    namespace.dataspace_segment()
                )
                .into(),
            )
        })?;
        if owner == *authority {
            Ok(())
        } else {
            Err(Error::InvariantViolation(
                format!("authority `{authority}` does not own Musubi namespace `{namespace}`")
                    .into(),
            ))
        }
    }
}

fn package_versions_in_world(
    world: &impl WorldReadOnly,
    package: &MusubiPackageId,
) -> Vec<MusubiVersion> {
    package_releases_in_world(world, package, true)
        .into_iter()
        .map(|release| release.package.version)
        .collect()
}

fn package_releases_in_world(
    world: &impl WorldReadOnly,
    package: &MusubiPackageId,
    include_yanked: bool,
) -> Vec<MusubiRelease> {
    let mut releases = package_release_refs_in_world(world, package)
        .into_iter()
        .filter_map(|package| {
            world
                .smart_contract_state()
                .get(&release_key(&package))
                .and_then(|bytes| decode_release_lossy(bytes))
        })
        .filter(|release| include_yanked || release.status.is_active())
        .collect::<Vec<_>>();
    if releases.is_empty() {
        releases = world
            .smart_contract_state()
            .iter()
            .filter(|(key, _)| key.as_ref().starts_with(RELEASE_KEY_PREFIX))
            .filter_map(|(_, bytes)| decode_release_lossy(bytes))
            .filter(|release| release.package.package == *package)
            .filter(|release| include_yanked || release.status.is_active())
            .collect::<Vec<_>>();
    }
    releases.sort_by(|left, right| {
        left.package
            .version
            .precedence_cmp(&right.package.version)
            .unwrap_or_else(|_| left.package.version.cmp(&right.package.version))
    });
    releases
}

fn package_summaries_in_world(
    world: &impl WorldReadOnly,
    namespace: Option<&iroha_data_model::musubi::MusubiNamespace>,
    query: &str,
    include_yanked: bool,
) -> Vec<MusubiPackageSummary> {
    let mut packages = BTreeMap::<MusubiPackageId, (Option<MusubiVersion>, u32, u32, bool)>::new();
    let catalog = package_catalog_in_world(world);
    if catalog.is_empty() {
        for release in world
            .smart_contract_state()
            .iter()
            .filter(|(key, _)| key.as_ref().starts_with(RELEASE_KEY_PREFIX))
            .filter_map(|(_, bytes)| decode_release_lossy(bytes))
        {
            if namespace.is_some_and(|namespace| &release.package.package.namespace != namespace) {
                continue;
            }
            if !query.is_empty() && !release.package.package.to_string().contains(query) {
                continue;
            }
            add_release_to_summary(&mut packages, release);
        }
    } else {
        for package in catalog {
            if namespace.is_some_and(|namespace| &package.namespace != namespace) {
                continue;
            }
            if !query.is_empty() && !package.to_string().contains(query) {
                continue;
            }
            for release in package_releases_in_world(world, &package, true) {
                add_release_to_summary(&mut packages, release);
            }
        }
    }
    packages
        .into_iter()
        .filter(|(_, (_, _, _, has_active))| include_yanked || *has_active)
        .map(
            |(package, (latest_active, release_count, yanked_count, _))| {
                MusubiPackageSummary::new(package, latest_active, release_count, yanked_count)
            },
        )
        .collect()
}

fn add_release_to_summary(
    packages: &mut BTreeMap<MusubiPackageId, (Option<MusubiVersion>, u32, u32, bool)>,
    release: MusubiRelease,
) {
    let is_active = release.status.is_active();
    let entry = packages
        .entry(release.package.package.clone())
        .or_insert((None, 0, 0, false));
    entry.1 = entry.1.saturating_add(1);
    if is_active {
        entry.3 = true;
        let replace_latest = entry.0.as_ref().is_none_or(|latest| {
            latest
                .precedence_cmp(&release.package.version)
                .is_ok_and(|ordering| ordering.is_lt())
        });
        if replace_latest {
            entry.0 = Some(release.package.version);
        }
    } else {
        entry.2 = entry.2.saturating_add(1);
    }
}

fn package_release_refs_in_world(
    world: &impl WorldReadOnly,
    package: &MusubiPackageId,
) -> Vec<MusubiPackageRef> {
    world
        .smart_contract_state()
        .get(&package_release_index_key(package))
        .and_then(|bytes| decode_release_index_lossy(bytes))
        .unwrap_or_default()
}

fn package_catalog_in_world(world: &impl WorldReadOnly) -> Vec<MusubiPackageId> {
    let key = Name::from_str(PACKAGE_CATALOG_KEY).expect("Musubi catalog key is valid");
    world
        .smart_contract_state()
        .get(&key)
        .and_then(|bytes| decode_package_catalog_lossy(bytes))
        .unwrap_or_default()
}

fn index_published_release(
    release: &MusubiRelease,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    let index_key = package_release_index_key(&release.package.package);
    let mut releases = state_transaction
        .world
        .smart_contract_state
        .get(&index_key)
        .and_then(|bytes| decode_release_index_lossy(bytes))
        .unwrap_or_default();
    releases.push(release.package.clone());
    releases.sort();
    releases.dedup();
    state_transaction
        .world
        .smart_contract_state
        .insert(index_key, releases.encode());

    let catalog_key = Name::from_str(PACKAGE_CATALOG_KEY).expect("Musubi catalog key is valid");
    let mut catalog = state_transaction
        .world
        .smart_contract_state
        .get(&catalog_key)
        .and_then(|bytes| decode_package_catalog_lossy(bytes))
        .unwrap_or_default();
    catalog.push(release.package.package.clone());
    catalog.sort();
    catalog.dedup();
    state_transaction
        .world
        .smart_contract_state
        .insert(catalog_key, catalog.encode());
}

fn package_release_index_key(package: &MusubiPackageId) -> Name {
    storage_key(
        PACKAGE_RELEASE_INDEX_PREFIX,
        package.canonical_name().as_bytes(),
    )
}

fn release_key(package: &MusubiPackageRef) -> Name {
    storage_key(RELEASE_KEY_PREFIX, package.canonical_ref().as_bytes())
}

fn short_alias_key(alias: &Name) -> Name {
    storage_key(SHORT_ALIAS_KEY_PREFIX, alias.as_ref().as_bytes())
}

fn storage_key(prefix: &str, payload: &[u8]) -> Name {
    let digest = blake3::hash(payload);
    Name::from_str(&format!("{prefix}{}", hex::encode(digest.as_bytes())))
        .expect("Musubi registry storage keys are valid names")
}

fn decode_release_lossy(bytes: &[u8]) -> Option<MusubiRelease> {
    let mut cursor = bytes;
    let release = MusubiRelease::decode(&mut cursor).ok()?;
    cursor.is_empty().then_some(release)
}

fn decode_release_index_lossy(bytes: &[u8]) -> Option<Vec<MusubiPackageRef>> {
    let mut cursor = bytes;
    let releases = Vec::<MusubiPackageRef>::decode(&mut cursor).ok()?;
    cursor.is_empty().then_some(releases)
}

fn decode_package_catalog_lossy(bytes: &[u8]) -> Option<Vec<MusubiPackageId>> {
    let mut cursor = bytes;
    let packages = Vec::<MusubiPackageId>::decode(&mut cursor).ok()?;
    cursor.is_empty().then_some(packages)
}

fn decode_release_for_instruction(bytes: &[u8]) -> Result<MusubiRelease, Error> {
    decode_release_lossy(bytes).ok_or_else(|| {
        Error::InvariantViolation("stored Musubi release record is malformed".into())
    })
}

fn decode_package_id_for_instruction(bytes: &[u8]) -> Result<MusubiPackageId, Error> {
    let mut cursor = bytes;
    let package = MusubiPackageId::decode(&mut cursor)
        .map_err(|_| Error::InvariantViolation("stored Musubi short alias is malformed".into()))?;
    if cursor.is_empty() {
        Ok(package)
    } else {
        Err(Error::InvariantViolation(
            "stored Musubi short alias has trailing bytes".into(),
        ))
    }
}

fn decode_release_for_query(bytes: &[u8]) -> Result<MusubiRelease, QueryExecutionFail> {
    decode_release_lossy(bytes).ok_or_else(|| {
        QueryExecutionFail::Conversion("stored Musubi release record is malformed".to_owned())
    })
}

fn decode_package_id_for_query(bytes: &[u8]) -> Result<MusubiPackageId, QueryExecutionFail> {
    let mut cursor = bytes;
    let package = MusubiPackageId::decode(&mut cursor).map_err(|err| {
        QueryExecutionFail::Conversion(format!("stored Musubi short alias is malformed: {err}"))
    })?;
    if cursor.is_empty() {
        Ok(package)
    } else {
        Err(QueryExecutionFail::Conversion(
            "stored Musubi short alias has trailing bytes".to_owned(),
        ))
    }
}

fn invalid_parameter(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into().into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        musubi::{MusubiArchiveRef, MusubiDappLink, MusubiShortAlias},
        nexus::DataSpaceId,
        smart_contract::{ContractAddress, ContractAlias},
        sorafs::pin_registry::ManifestDigest,
    };
    use iroha_executor_data_model::permission::musubi::CanSetMusubiShortAlias;
    use nonzero_ext::nonzero;

    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[test]
    fn storage_key_is_stable_and_name_safe() {
        let package: MusubiPackageRef = "dex.universal/swap-core@1.2.3".parse().expect("package");

        assert_eq!(release_key(&package), release_key(&package));
        assert!(
            release_key(&package)
                .as_ref()
                .starts_with(RELEASE_KEY_PREFIX)
        );
    }

    #[test]
    fn malformed_release_decode_is_rejected() {
        let err = decode_release_for_query(b"not a release").expect_err("malformed");

        assert!(err.to_string().contains("malformed"));
    }

    #[test]
    fn publishable_release_roundtrip_decodes_losslessly() {
        let package: MusubiPackageRef = "dex.universal/swap-core@1.2.3".parse().expect("package");
        let keypair = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        let publisher = AccountId::new(keypair.public_key().clone());
        let release = MusubiRelease::new(
            package,
            iroha_data_model::musubi::MusubiArchiveRef::new(
                ManifestDigest::new([1; 32]),
                [2; 32],
                10,
                1,
            ),
            Vec::new(),
            vec!["quote".parse().expect("export")],
            None,
            publisher,
            0,
        );

        assert_eq!(decode_release_lossy(&release.encode()), Some(release));
    }

    #[test]
    fn short_alias_retarget_to_different_package_is_rejected() {
        let state = test_state();
        let authority = publisher();
        let first = sample_release("dex.universal/swap-core@1.0.0");
        let second = sample_release("dex.universal/router@1.0.0");
        let alias: Name = "swap".parse().expect("alias");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        grant_short_alias_permission(&mut tx, &authority);
        seed_published_release(&mut tx, first.clone());
        seed_published_release(&mut tx, second.clone());

        SetMusubiShortAlias::new(MusubiShortAlias::new(
            alias.clone(),
            first.package.package.clone(),
        ))
        .execute(&authority, &mut tx)
        .expect("initial alias set");
        let err =
            SetMusubiShortAlias::new(MusubiShortAlias::new(alias, second.package.package.clone()))
                .execute(&authority, &mut tx)
                .expect_err("retarget rejected");

        assert!(err.to_string().contains("already targets"));
    }

    #[test]
    fn short_alias_requires_active_release() {
        let state = test_state();
        let authority = publisher();
        let mut release = sample_release("dex.universal/legacy@0.9.0");
        release.yank("superseded", 42);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        grant_short_alias_permission(&mut tx, &authority);
        seed_published_release(&mut tx, release.clone());

        let err = SetMusubiShortAlias::new(MusubiShortAlias::new(
            "legacy".parse().expect("alias"),
            release.package.package,
        ))
        .execute(&authority, &mut tx)
        .expect_err("inactive release rejected");

        assert!(err.to_string().contains("no active releases"));
    }

    #[test]
    fn dapp_link_rejects_missing_contract_alias() {
        let state = test_state();
        let alias: ContractAlias = "router::dex.universal".parse().expect("alias");
        let release = sample_dapp_release("dex.universal/swap-core@1.0.0", alias);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_000, 0);
        let mut block = state.block(header);
        let tx = block.transaction();

        let err = ensure_dapp_contracts_exist(&release, &tx).expect_err("missing alias rejected");

        assert!(err.to_string().contains("unknown or expired"));
    }

    #[test]
    fn dapp_link_accepts_active_contract_alias() {
        let state = test_state();
        let authority = publisher();
        let alias: ContractAlias = "router::dex.universal".parse().expect("alias");
        let release = sample_dapp_release("dex.universal/swap-core@1.0.0", alias.clone());
        let contract_address =
            ContractAddress::derive(0, &authority, 0, DataSpaceId::GLOBAL).expect("address");

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1_000, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        tx.world
            .bind_contract_alias(&contract_address, alias, None, None, 1_000)
            .expect("bind contract alias");

        ensure_dapp_contracts_exist(&release, &tx).expect("active alias accepted");
    }

    #[test]
    fn search_limit_cap_allows_large_registry_pages() {
        assert_eq!(musubi_search_limit(1_500), SEARCH_LIMIT_CAP);
        assert_eq!(musubi_search_limit(250), 250);
    }

    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn publisher() -> AccountId {
        let keypair = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn grant_short_alias_permission(
        tx: &mut crate::state::StateTransaction<'_, '_>,
        authority: &AccountId,
    ) {
        tx.world
            .add_account_permission(authority, CanSetMusubiShortAlias.into());
    }

    fn seed_published_release(
        tx: &mut crate::state::StateTransaction<'_, '_>,
        release: MusubiRelease,
    ) {
        tx.world
            .smart_contract_state
            .insert(release_key(&release.package), release.encode());
        index_published_release(&release, tx);
    }

    fn sample_release(raw: &str) -> MusubiRelease {
        MusubiRelease::new(
            raw.parse().expect("package"),
            MusubiArchiveRef::new(ManifestDigest::new([1; 32]), [2; 32], 10, 1),
            Vec::new(),
            vec!["quote".parse().expect("export")],
            None,
            publisher(),
            0,
        )
    }

    fn sample_dapp_release(raw: &str, alias: ContractAlias) -> MusubiRelease {
        let package: MusubiPackageRef = raw.parse().expect("package");
        let dapp =
            MusubiDappLink::new(package.package.namespace.clone(), vec![alias]).expect("dapp link");
        MusubiRelease::new(
            package,
            MusubiArchiveRef::new(ManifestDigest::new([3; 32]), [4; 32], 10, 1),
            Vec::new(),
            vec!["quote".parse().expect("export")],
            Some(dapp),
            publisher(),
            0,
        )
    }
}

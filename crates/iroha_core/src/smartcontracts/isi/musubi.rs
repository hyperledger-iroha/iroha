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
        MusubiPackageId, MusubiPackageRef, MusubiRelease, MusubiReleaseStatus, MusubiVersion,
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
        ensure_namespace_authority(&self.alias.target, authority, state_transaction)?;
        if package_versions_in_world(state_transaction.world(), &self.alias.target).is_empty() {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi short alias `{}` targets package `{}` with no releases",
                    self.alias.alias, self.alias.target
                )
                .into(),
            ));
        }
        // TODO: replace this namespace-owner fallback with the governance permission
        // token once the registry authority model is wired into permissions.
        state_transaction.world.smart_contract_state.insert(
            short_alias_key(&self.alias.alias),
            self.alias.target.encode(),
        );
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
        if state_transaction
            .world
            .smart_contract_state
            .get(&key)
            .is_none()
        {
            return Err(Error::InvariantViolation(
                format!(
                    "Musubi dependency `{}` is not published",
                    dependency.package
                )
                .into(),
            ));
        }
    }
    Ok(())
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
    world
        .smart_contract_state()
        .iter()
        .filter(|(key, _)| key.as_ref().starts_with(RELEASE_KEY_PREFIX))
        .filter_map(|(_, bytes)| decode_release_lossy(bytes))
        .filter(|release| release.package.package == *package)
        .map(|release| release.package.version)
        .collect()
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

fn decode_release_for_instruction(bytes: &[u8]) -> Result<MusubiRelease, Error> {
    decode_release_lossy(bytes).ok_or_else(|| {
        Error::InvariantViolation("stored Musubi release record is malformed".into())
    })
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
    use iroha_data_model::{account::AccountId, sorafs::pin_registry::ManifestDigest};

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
}

//! Musubi package registry data types for Kotodama source packages.
//!
//! Musubi uses canonical package names of the form `namespace/package` and exact
//! release references of the form `namespace/package@version`. The namespace is
//! intentionally the same suffix format used by Kotodama dapp contract aliases:
//! `<dataspace>` or `<domain>.<dataspace>`.

use std::{fmt, str::FromStr, string::String, vec::Vec};

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::AccountId, error::ParseError, name::Name, smart_contract::ContractAlias,
    sorafs::pin_registry::ManifestDigest,
};

/// Canonical namespace for Musubi packages and Kotodama dapp links.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct MusubiNamespace(String);

impl MusubiNamespace {
    /// Parse and canonicalize a namespace literal.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the namespace is not `<dataspace>` or
    /// `<domain>.<dataspace>`.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return the canonical namespace literal.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Optional domain segment for `<domain>.<dataspace>` namespaces.
    #[must_use]
    pub fn domain_segment(&self) -> Option<&str> {
        self.0.split_once('.').map(|(domain, _)| domain)
    }

    /// Dataspace segment for both namespace shapes.
    #[must_use]
    pub fn dataspace_segment(&self) -> &str {
        self.0
            .rsplit_once('.')
            .map_or(self.0.as_str(), |(_, dataspace)| dataspace)
    }

    /// Build a contract alias in this namespace.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the contract name is invalid.
    pub fn contract_alias(&self, contract_name: &str) -> Result<ContractAlias, ParseError> {
        ContractAlias::from_components(
            contract_name,
            self.domain_segment(),
            self.dataspace_segment(),
        )
    }
}

impl AsRef<str> for MusubiNamespace {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for MusubiNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MusubiNamespace {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_clean_literal(raw, "musubi namespace")?;
        if raw.contains('/') || raw.contains('@') || raw.contains(':') {
            return Err(ParseError::new(
                "musubi namespace must not contain `/`, `@`, or `:`",
            ));
        }

        let mut segments = raw.split('.');
        let first = segments
            .next()
            .ok_or_else(|| ParseError::new("musubi namespace must not be empty"))?;
        let second = segments.next();
        if segments.next().is_some() {
            return Err(ParseError::new(
                "musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`",
            ));
        }
        let first = parse_namespace_segment(first)?;
        let canonical = if let Some(second) = second {
            let second = parse_namespace_segment(second)?;
            format!("{first}.{second}")
        } else {
            first.to_string()
        };
        Ok(Self(canonical))
    }
}

/// Canonical Musubi package name segment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct MusubiPackageName(String);

impl MusubiPackageName {
    /// Parse and canonicalize a package name.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the name contains separators or invalid name
    /// characters.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return the canonical package name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for MusubiPackageName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for MusubiPackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MusubiPackageName {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_clean_literal(raw, "musubi package name")?;
        if raw.contains('/') || raw.contains('@') || raw.contains(':') || raw.contains('.') {
            return Err(ParseError::new(
                "musubi package name must not contain `/`, `@`, `:`, or `.`",
            ));
        }
        let name = Name::from_str(raw)
            .map_err(|_| ParseError::new("musubi package name segment is invalid"))?;
        Ok(Self(name.as_ref().to_owned()))
    }
}

/// Canonical Musubi package identifier, without a release version.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageId {
    /// Namespace that owns the package.
    pub namespace: MusubiNamespace,
    /// Package name within the namespace.
    pub name: MusubiPackageName,
}

impl MusubiPackageId {
    /// Construct a package id from validated components.
    #[must_use]
    pub const fn new(namespace: MusubiNamespace, name: MusubiPackageName) -> Self {
        Self { namespace, name }
    }

    /// Parse components and construct a canonical package id.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when either component is invalid.
    pub fn from_parts(namespace: &str, name: &str) -> Result<Self, ParseError> {
        Ok(Self {
            namespace: namespace.parse()?,
            name: name.parse()?,
        })
    }

    /// Format the canonical `namespace/package` literal.
    #[must_use]
    pub fn canonical_name(&self) -> String {
        format!("{}/{}", self.namespace, self.name)
    }
}

impl fmt::Display for MusubiPackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_name())
    }
}

impl FromStr for MusubiPackageId {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_clean_literal(raw, "musubi package id")?;
        if raw.starts_with('@') {
            return Err(ParseError::new(
                "musubi package id must not use a leading `@` namespace",
            ));
        }
        if raw.contains('@') {
            return Err(ParseError::new(
                "musubi package id must not include a version; use `namespace/package@version` for a release",
            ));
        }
        let (namespace, name) = raw.split_once('/').ok_or_else(|| {
            ParseError::new("musubi package id must use `namespace/package` format")
        })?;
        if name.contains('/') {
            return Err(ParseError::new(
                "musubi package id must contain exactly one `/` separator",
            ));
        }
        Self::from_parts(namespace, name)
    }
}

/// Exact semantic version for a Musubi release.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct MusubiVersion(String);

impl MusubiVersion {
    /// Parse and validate a semantic version.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the version is not `MAJOR.MINOR.PATCH` with
    /// optional prerelease or build metadata.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return the canonical version string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for MusubiVersion {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for MusubiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MusubiVersion {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_clean_literal(raw, "musubi version")?;
        validate_semver(raw)?;
        Ok(Self(raw.to_owned()))
    }
}

/// Exact Musubi package release reference.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageRef {
    /// Package id without a version.
    pub package: MusubiPackageId,
    /// Exact release version.
    pub version: MusubiVersion,
}

impl MusubiPackageRef {
    /// Construct a package release reference.
    #[must_use]
    pub const fn new(package: MusubiPackageId, version: MusubiVersion) -> Self {
        Self { package, version }
    }

    /// Format the canonical `namespace/package@version` literal.
    #[must_use]
    pub fn canonical_ref(&self) -> String {
        format!("{}@{}", self.package, self.version)
    }
}

impl fmt::Display for MusubiPackageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_ref())
    }
}

impl FromStr for MusubiPackageRef {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_clean_literal(raw, "musubi package reference")?;
        if raw.starts_with('@') {
            return Err(ParseError::new(
                "musubi package reference must not use a leading `@` namespace",
            ));
        }
        let (package, version) = raw.rsplit_once('@').ok_or_else(|| {
            ParseError::new("musubi package reference must use `namespace/package@version` format")
        })?;
        if package.contains('@') {
            return Err(ParseError::new(
                "musubi package reference must contain exactly one `@` version separator",
            ));
        }
        Ok(Self {
            package: package.parse()?,
            version: version.parse()?,
        })
    }
}

/// Exact source archive reference stored outside the chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArchiveRef {
    /// Canonical SoraFS manifest digest for the source archive.
    pub sorafs_manifest: ManifestDigest,
    /// BLAKE3-256 hash of the canonical source archive payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub archive_hash_blake3_256: [u8; 32],
}

impl MusubiArchiveRef {
    /// Construct a source archive reference.
    #[must_use]
    pub const fn new(sorafs_manifest: ManifestDigest, archive_hash_blake3_256: [u8; 32]) -> Self {
        Self {
            sorafs_manifest,
            archive_hash_blake3_256,
        }
    }
}

/// Source-library dependency pinned to an exact package release.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiDependency {
    /// Import alias used by Kotodama source code.
    pub alias: Name,
    /// Exact package release imported by the alias.
    pub package: MusubiPackageRef,
}

impl MusubiDependency {
    /// Construct a source dependency.
    #[must_use]
    pub const fn new(alias: Name, package: MusubiPackageRef) -> Self {
        Self { alias, package }
    }
}

/// Link between a package namespace and dapp contract aliases in the same namespace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiDappLink {
    /// Shared package and dapp namespace.
    pub namespace: MusubiNamespace,
    /// Contract aliases that belong to the namespace.
    pub contracts: Vec<ContractAlias>,
}

impl MusubiDappLink {
    /// Construct a dapp link and verify that every contract alias belongs to the namespace.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when any contract alias uses a different namespace.
    pub fn new(
        namespace: MusubiNamespace,
        contracts: Vec<ContractAlias>,
    ) -> Result<Self, ParseError> {
        if contracts
            .iter()
            .all(|alias| contract_alias_namespace(alias) == namespace.as_str())
        {
            Ok(Self {
                namespace,
                contracts,
            })
        } else {
            Err(ParseError::new(
                "musubi dapp link contract aliases must use the package namespace",
            ))
        }
    }
}

/// Lifecycle status of a Musubi release.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
pub enum MusubiReleaseStatus {
    /// Release can be selected by new lockfiles.
    Active,
    /// Release is hidden from new resolver output but remains immutable for existing lockfiles.
    Yanked(MusubiYankInfo),
}

impl MusubiReleaseStatus {
    /// Returns true when the release can be selected by new resolution.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Metadata attached when a Musubi release is yanked.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiYankInfo {
    /// Human-readable reason recorded by the namespace owner.
    pub reason: String,
    /// Ledger timestamp in milliseconds when the release was yanked.
    pub yanked_at_ms: u64,
}

/// Immutable registry record for a single package release.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiRelease {
    /// Exact package release identifier.
    pub package: MusubiPackageRef,
    /// Off-chain source archive reference.
    pub archive: MusubiArchiveRef,
    /// Source-library dependencies imported by this release.
    pub dependencies: Vec<MusubiDependency>,
    /// Exported Kotodama functions available to downstream packages.
    pub exports: Vec<Name>,
    /// Optional dapp namespace link for contract aliases.
    pub dapp: Option<MusubiDappLink>,
    /// Account that published the release.
    pub published_by: AccountId,
    /// Ledger timestamp in milliseconds when the release was published.
    pub published_at_ms: u64,
    /// Current release lifecycle state.
    pub status: MusubiReleaseStatus,
}

impl MusubiRelease {
    /// Construct an active release record.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        package: MusubiPackageRef,
        archive: MusubiArchiveRef,
        dependencies: Vec<MusubiDependency>,
        exports: Vec<Name>,
        dapp: Option<MusubiDappLink>,
        published_by: AccountId,
        published_at_ms: u64,
    ) -> Self {
        Self {
            package,
            archive,
            dependencies,
            exports,
            dapp,
            published_by,
            published_at_ms,
            status: MusubiReleaseStatus::Active,
        }
    }

    /// Mark the release as yanked while preserving the immutable archive record.
    pub fn yank(&mut self, reason: impl Into<String>, yanked_at_ms: u64) {
        self.status = MusubiReleaseStatus::Yanked(MusubiYankInfo {
            reason: reason.into(),
            yanked_at_ms,
        });
    }
}

/// Curated global short alias pointing at a canonical package id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiShortAlias {
    /// Human-friendly alias with no namespace prefix.
    pub alias: Name,
    /// Canonical package id selected by governance.
    pub target: MusubiPackageId,
}

impl MusubiShortAlias {
    /// Construct a short alias binding.
    #[must_use]
    pub const fn new(alias: Name, target: MusubiPackageId) -> Self {
        Self { alias, target }
    }
}

fn validate_clean_literal(raw: &str, label: &'static str) -> Result<(), ParseError> {
    if raw.is_empty() {
        return Err(match label {
            "musubi namespace" => ParseError::new("musubi namespace must not be empty"),
            "musubi package name" => ParseError::new("musubi package name must not be empty"),
            "musubi package id" => ParseError::new("musubi package id must not be empty"),
            "musubi package reference" => {
                ParseError::new("musubi package reference must not be empty")
            }
            "musubi version" => ParseError::new("musubi version must not be empty"),
            _ => ParseError::new("musubi literal must not be empty"),
        });
    }
    if raw.trim() != raw {
        return Err(ParseError::new(
            "musubi literals must not contain leading or trailing whitespace",
        ));
    }
    if raw.chars().any(char::is_control) {
        return Err(ParseError::new(
            "musubi literals must not contain control characters",
        ));
    }
    Ok(())
}

fn parse_namespace_segment(raw: &str) -> Result<Name, ParseError> {
    if raw.is_empty() {
        return Err(ParseError::new(
            "musubi namespace segments must not be empty",
        ));
    }
    Name::from_str(raw).map_err(|_| ParseError::new("musubi namespace segment is invalid"))
}

fn validate_semver(raw: &str) -> Result<(), ParseError> {
    let (without_build, build) = raw
        .split_once('+')
        .map_or((raw, None), |(left, right)| (left, Some(right)));
    if without_build.contains('+') || build.is_some_and(str::is_empty) {
        return Err(ParseError::new(
            "musubi version build metadata must be non-empty and use one `+` separator",
        ));
    }
    if let Some(build) = build {
        validate_semver_identifiers(build, "musubi version build metadata")?;
    }

    let (core, prerelease) = without_build
        .split_once('-')
        .map_or((without_build, None), |(left, right)| (left, Some(right)));
    if core.contains('-') || prerelease.is_some_and(str::is_empty) {
        return Err(ParseError::new(
            "musubi version prerelease must be non-empty and use one `-` separator",
        ));
    }
    if let Some(prerelease) = prerelease {
        validate_semver_identifiers(prerelease, "musubi version prerelease")?;
    }

    let mut core_parts = core.split('.');
    let major = core_parts.next();
    let minor = core_parts.next();
    let patch = core_parts.next();
    if core_parts.next().is_some() || major.is_none() || minor.is_none() || patch.is_none() {
        return Err(ParseError::new(
            "musubi version must use `MAJOR.MINOR.PATCH` format",
        ));
    }
    for part in [major, minor, patch].into_iter().flatten() {
        validate_numeric_identifier(part)?;
    }
    Ok(())
}

fn validate_numeric_identifier(raw: &str) -> Result<(), ParseError> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ParseError::new(
            "musubi version core identifiers must be numeric",
        ));
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return Err(ParseError::new(
            "musubi version core identifiers must not contain leading zeroes",
        ));
    }
    Ok(())
}

fn validate_semver_identifiers(raw: &str, label: &'static str) -> Result<(), ParseError> {
    for part in raw.split('.') {
        if part.is_empty()
            || !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(match label {
                "musubi version prerelease" => ParseError::new(
                    "musubi version prerelease identifiers must be ASCII alphanumeric or `-`",
                ),
                "musubi version build metadata" => ParseError::new(
                    "musubi version build identifiers must be ASCII alphanumeric or `-`",
                ),
                _ => ParseError::new("musubi version identifiers are invalid"),
            });
        }
    }
    Ok(())
}

fn contract_alias_namespace(alias: &ContractAlias) -> String {
    alias.domain_segment().map_or_else(
        || alias.dataspace_segment().to_owned(),
        |domain| format!("{}.{}", domain, alias.dataspace_segment()),
    )
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{Decode, Encode};

    #[test]
    fn package_reference_uses_namespace_slash_name_at_version() {
        let reference: MusubiPackageRef = "dex.universal/swap-core@1.2.3"
            .parse()
            .expect("valid package ref");

        assert_eq!(reference.package.namespace.as_str(), "dex.universal");
        assert_eq!(reference.package.name.as_str(), "swap-core");
        assert_eq!(reference.version.as_str(), "1.2.3");
        assert_eq!(reference.to_string(), "dex.universal/swap-core@1.2.3");
    }

    #[test]
    fn package_reference_rejects_leading_at_namespace() {
        let err = "@dex.universal/swap-core@1.2.3"
            .parse::<MusubiPackageRef>()
            .expect_err("leading @ rejected");

        assert_eq!(
            err.reason(),
            "musubi package reference must not use a leading `@` namespace"
        );
    }

    #[test]
    fn namespace_builds_matching_contract_alias() {
        let namespace: MusubiNamespace = "dex.universal".parse().expect("namespace");
        let alias = namespace.contract_alias("router").expect("alias");

        assert_eq!(alias.as_ref(), "router::dex.universal");
    }

    #[test]
    fn dapp_link_requires_matching_contract_namespace() {
        let namespace: MusubiNamespace = "dex.universal".parse().expect("namespace");
        let matching = "router::dex.universal"
            .parse::<ContractAlias>()
            .expect("matching alias");
        let other = "router::other.universal"
            .parse::<ContractAlias>()
            .expect("other alias");

        assert!(MusubiDappLink::new(namespace.clone(), vec![matching]).is_ok());
        assert!(MusubiDappLink::new(namespace, vec![other]).is_err());
    }

    #[test]
    fn version_validation_rejects_ambiguous_core_numbers() {
        assert!("1.2.3-alpha.1+build-7".parse::<MusubiVersion>().is_ok());

        let err = "1.02.3"
            .parse::<MusubiVersion>()
            .expect_err("leading zero rejected");
        assert_eq!(
            err.reason(),
            "musubi version core identifiers must not contain leading zeroes"
        );
    }

    #[test]
    fn release_roundtrips_through_norito() {
        let package = "dex.universal/swap-core@1.2.3"
            .parse::<MusubiPackageRef>()
            .expect("package ref");
        let archive = MusubiArchiveRef::new(ManifestDigest::new([1; 32]), [2; 32]);
        let dependency = MusubiDependency::new(
            "math".parse().expect("alias"),
            "std.universal/math@1.0.0".parse().expect("dep"),
        );
        let dapp = MusubiDappLink::new(
            "dex.universal".parse().expect("namespace"),
            vec!["router::dex.universal".parse().expect("contract alias")],
        )
        .expect("dapp link");
        let keypair = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
        let publisher = AccountId::new(keypair.public_key().clone());
        let mut release = MusubiRelease::new(
            package,
            archive,
            vec![dependency],
            vec!["quote".parse().expect("export")],
            Some(dapp),
            publisher,
            42,
        );
        release.yank("superseded", 84);

        let bytes = release.encode();
        let mut cursor = bytes.as_slice();
        let decoded = MusubiRelease::decode(&mut cursor).expect("decode release");

        assert!(cursor.is_empty());
        assert_eq!(decoded, release);
        assert!(!decoded.status.is_active());
    }
}

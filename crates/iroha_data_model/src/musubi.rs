//! Musubi package registry data types for Kotodama source packages.
//!
//! Musubi uses canonical package names of the form `namespace/package`, release
//! references of the form `namespace/package@version`, and local manifest
//! requirements such as `^1.2.3`. The namespace is intentionally the same
//! suffix format used by Kotodama dapp contract aliases: `<dataspace>` or
//! `<domain>.<dataspace>`.

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

    /// Compare this version with another using semantic-version precedence.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if a validated version cannot be interpreted for
    /// precedence comparison.
    pub fn precedence_cmp(&self, other: &Self) -> Result<core::cmp::Ordering, ParseError> {
        compare_semver(self.as_str(), other.as_str())
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

/// Version requirement accepted by Musubi manifests and resolved into exact releases.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct MusubiVersionReq(String);

impl MusubiVersionReq {
    /// Parse and validate a version requirement.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the requirement is not an exact, caret,
    /// tilde, wildcard, or comparator-list requirement.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return the canonical requirement string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return an exact version when this requirement pins a single release.
    #[must_use]
    pub fn exact_version(&self) -> Option<MusubiVersion> {
        let raw = self.0.strip_prefix('=').unwrap_or(self.0.as_str());
        MusubiVersion::new(raw).ok()
    }

    /// Returns true when `version` satisfies this requirement.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if a validated requirement or version cannot be
    /// interpreted for precedence comparison.
    pub fn matches(&self, version: &MusubiVersion) -> Result<bool, ParseError> {
        version_req_matches(self.as_str(), version.as_str())
    }
}

impl AsRef<str> for MusubiVersionReq {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for MusubiVersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MusubiVersionReq {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_clean_literal(raw, "musubi version requirement")?;
        validate_version_req(raw)?;
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
    /// Total bytes included in the canonical source archive.
    pub source_bytes: u64,
    /// Total regular files included in the canonical source archive.
    pub source_file_count: u32,
}

impl MusubiArchiveRef {
    /// Construct a source archive reference.
    #[must_use]
    pub const fn new(
        sorafs_manifest: ManifestDigest,
        archive_hash_blake3_256: [u8; 32],
        source_bytes: u64,
        source_file_count: u32,
    ) -> Self {
        Self {
            sorafs_manifest,
            archive_hash_blake3_256,
            source_bytes,
            source_file_count,
        }
    }

    /// Returns true when this archive can back a first-class registry release.
    #[must_use]
    pub const fn is_non_empty(&self) -> bool {
        self.source_bytes > 0 && self.source_file_count > 0
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

/// Exported Kotodama library symbols for a Musubi release.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiExportSet {
    /// Functions callable by downstream packages as `alias::function(...)`.
    pub functions: Vec<Name>,
}

impl MusubiExportSet {
    /// Construct an export set from function names.
    #[must_use]
    pub fn new(mut functions: Vec<Name>) -> Self {
        functions.sort();
        functions.dedup();
        Self { functions }
    }

    /// Returns true if no source-library symbols are exported.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
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

/// Compact release metadata returned by package listing queries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseSummary {
    /// Exact release identifier.
    pub package: MusubiPackageRef,
    /// Off-chain archive commitment.
    pub archive: MusubiArchiveRef,
    /// Current lifecycle status.
    pub status: MusubiReleaseStatus,
    /// Exported library functions.
    pub exports: Vec<Name>,
    /// Account that published the release.
    pub published_by: AccountId,
    /// Ledger timestamp in milliseconds when the release was published.
    pub published_at_ms: u64,
}

impl MusubiReleaseSummary {
    /// Build a summary from a full release record.
    #[must_use]
    pub fn from_release(release: &MusubiRelease) -> Self {
        Self {
            package: release.package.clone(),
            archive: release.archive,
            status: release.status.clone(),
            exports: release.exports.clone(),
            published_by: release.published_by.clone(),
            published_at_ms: release.published_at_ms,
        }
    }
}

/// Compact package metadata returned by package search.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageSummary {
    /// Canonical package identifier.
    pub package: MusubiPackageId,
    /// Highest active version, if one exists.
    pub latest_active: Option<MusubiVersion>,
    /// Total release count, including yanked releases.
    pub release_count: u32,
    /// Yanked release count.
    pub yanked_count: u32,
}

impl MusubiPackageSummary {
    /// Construct a package summary.
    #[must_use]
    pub const fn new(
        package: MusubiPackageId,
        latest_active: Option<MusubiVersion>,
        release_count: u32,
        yanked_count: u32,
    ) -> Self {
        Self {
            package,
            latest_active,
            release_count,
            yanked_count,
        }
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
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
            exports: MusubiExportSet::new(exports).functions,
            dapp,
            published_by,
            published_at_ms,
            status: MusubiReleaseStatus::Active,
        }
    }

    /// Validate this release as a publishable registry record.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when anti-squatting release requirements fail.
    pub fn validate_publishable(&self) -> Result<(), ParseError> {
        if !self.archive.is_non_empty() {
            return Err(ParseError::new(
                "musubi release archive must contain at least one source file and one byte",
            ));
        }
        if self.exports.is_empty() {
            return Err(ParseError::new(
                "musubi release must export at least one Kotodama function",
            ));
        }
        if let Some(dapp) = &self.dapp {
            MusubiDappLink::new(dapp.namespace.clone(), dapp.contracts.clone())?;
        }
        Ok(())
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
            "musubi version requirement" => {
                ParseError::new("musubi version requirement must not be empty")
            }
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

fn validate_version_req(raw: &str) -> Result<(), ParseError> {
    if raw == "*" {
        return Ok(());
    }
    if let Some(rest) = raw.strip_prefix('^').or_else(|| raw.strip_prefix('~')) {
        validate_semver(rest)?;
        return Ok(());
    }
    if raw.ends_with(".*") {
        validate_wildcard_req(raw)?;
        return Ok(());
    }
    if raw.contains(',') || is_comparator_req(raw) {
        for comparator in raw.split(',') {
            validate_comparator_req(comparator.trim())?;
        }
        return Ok(());
    }
    validate_semver(raw.strip_prefix('=').unwrap_or(raw))
}

fn validate_wildcard_req(raw: &str) -> Result<(), ParseError> {
    let prefix = raw
        .strip_suffix(".*")
        .ok_or_else(|| ParseError::new("musubi wildcard requirement must end in `.*`"))?;
    let parts = prefix.split('.').collect::<Vec<_>>();
    match parts.as_slice() {
        [major] => validate_numeric_identifier(major),
        [major, minor] => {
            validate_numeric_identifier(major)?;
            validate_numeric_identifier(minor)
        }
        _ => Err(ParseError::new(
            "musubi wildcard requirement must be `MAJOR.*` or `MAJOR.MINOR.*`",
        )),
    }
}

fn is_comparator_req(raw: &str) -> bool {
    [">=", "<=", ">", "<", "="]
        .iter()
        .any(|prefix| raw.starts_with(prefix))
}

fn validate_comparator_req(raw: &str) -> Result<(), ParseError> {
    let version = raw
        .strip_prefix(">=")
        .or_else(|| raw.strip_prefix("<="))
        .or_else(|| raw.strip_prefix('>'))
        .or_else(|| raw.strip_prefix('<'))
        .or_else(|| raw.strip_prefix('='))
        .ok_or_else(|| {
            ParseError::new("musubi comparator requirement must start with >=, <=, >, <, or =")
        })?;
    validate_semver(version)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SemverPrecedence<'a> {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Option<&'a str>,
}

fn parse_semver_precedence(raw: &str) -> Result<SemverPrecedence<'_>, ParseError> {
    validate_semver(raw)?;
    let without_build = raw.split_once('+').map_or(raw, |(left, _)| left);
    let (core, prerelease) = without_build
        .split_once('-')
        .map_or((without_build, None), |(left, right)| (left, Some(right)));
    let mut parts = core.split('.');
    let major = parse_u64_identifier(parts.next().expect("validated major"))?;
    let minor = parse_u64_identifier(parts.next().expect("validated minor"))?;
    let patch = parse_u64_identifier(parts.next().expect("validated patch"))?;
    Ok(SemverPrecedence {
        major,
        minor,
        patch,
        prerelease,
    })
}

fn parse_u64_identifier(raw: &str) -> Result<u64, ParseError> {
    raw.parse::<u64>()
        .map_err(|_| ParseError::new("musubi version core identifiers exceed u64"))
}

fn version_req_matches(req: &str, version: &str) -> Result<bool, ParseError> {
    if req == "*" {
        return Ok(true);
    }
    if let Some(base) = req.strip_prefix('^') {
        return range_matches(version, base, caret_upper_bound(base)?);
    }
    if let Some(base) = req.strip_prefix('~') {
        return range_matches(version, base, tilde_upper_bound(base)?);
    }
    if req.ends_with(".*") {
        return wildcard_matches(req, version);
    }
    if req.contains(',') || is_comparator_req(req) {
        for comparator in req.split(',') {
            if !comparator_matches(comparator.trim(), version)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    compare_semver(version, req.strip_prefix('=').unwrap_or(req)).map(|ordering| ordering.is_eq())
}

fn range_matches(
    version: &str,
    lower: &str,
    upper: SemverPrecedence<'static>,
) -> Result<bool, ParseError> {
    Ok(compare_semver(version, lower)?.is_ge()
        && compare_precedence(&parse_semver_precedence(version)?, &upper).is_lt())
}

fn caret_upper_bound(base: &str) -> Result<SemverPrecedence<'static>, ParseError> {
    let base = parse_semver_precedence(base)?;
    let (major, minor, patch) = if base.major > 0 {
        (base.major + 1, 0, 0)
    } else if base.minor > 0 {
        (0, base.minor + 1, 0)
    } else {
        (0, 0, base.patch + 1)
    };
    Ok(SemverPrecedence {
        major,
        minor,
        patch,
        prerelease: None,
    })
}

fn tilde_upper_bound(base: &str) -> Result<SemverPrecedence<'static>, ParseError> {
    let base = parse_semver_precedence(base)?;
    Ok(SemverPrecedence {
        major: base.major,
        minor: base.minor + 1,
        patch: 0,
        prerelease: None,
    })
}

fn wildcard_matches(req: &str, version: &str) -> Result<bool, ParseError> {
    let version = parse_semver_precedence(version)?;
    let prefix = req.trim_end_matches(".*");
    let parts = prefix.split('.').collect::<Vec<_>>();
    match parts.as_slice() {
        [major] => Ok(version.major == parse_u64_identifier(major)?),
        [major, minor] => Ok(version.major == parse_u64_identifier(major)?
            && version.minor == parse_u64_identifier(minor)?),
        _ => Err(ParseError::new(
            "musubi wildcard requirement must be `MAJOR.*` or `MAJOR.MINOR.*`",
        )),
    }
}

fn comparator_matches(req: &str, version: &str) -> Result<bool, ParseError> {
    let (operator, expected) = if let Some(version) = req.strip_prefix(">=") {
        (">=", version)
    } else if let Some(version) = req.strip_prefix("<=") {
        ("<=", version)
    } else if let Some(version) = req.strip_prefix('>') {
        (">", version)
    } else if let Some(version) = req.strip_prefix('<') {
        ("<", version)
    } else if let Some(version) = req.strip_prefix('=') {
        ("=", version)
    } else {
        return Err(ParseError::new(
            "musubi comparator requirement must start with >=, <=, >, <, or =",
        ));
    };
    let ordering = compare_semver(version, expected)?;
    Ok(match operator {
        ">=" => ordering.is_ge(),
        "<=" => ordering.is_le(),
        ">" => ordering.is_gt(),
        "<" => ordering.is_lt(),
        "=" => ordering.is_eq(),
        _ => false,
    })
}

fn compare_semver(left: &str, right: &str) -> Result<core::cmp::Ordering, ParseError> {
    Ok(compare_precedence(
        &parse_semver_precedence(left)?,
        &parse_semver_precedence(right)?,
    ))
}

fn compare_precedence(
    left: &SemverPrecedence<'_>,
    right: &SemverPrecedence<'_>,
) -> core::cmp::Ordering {
    left.major
        .cmp(&right.major)
        .then_with(|| left.minor.cmp(&right.minor))
        .then_with(|| left.patch.cmp(&right.patch))
        .then_with(|| compare_prerelease(left.prerelease, right.prerelease))
}

fn compare_prerelease(left: Option<&str>, right: Option<&str>) -> core::cmp::Ordering {
    match (left, right) {
        (None, None) => core::cmp::Ordering::Equal,
        (None, Some(_)) => core::cmp::Ordering::Greater,
        (Some(_), None) => core::cmp::Ordering::Less,
        (Some(left), Some(right)) => compare_prerelease_parts(left, right),
    }
}

fn compare_prerelease_parts(left: &str, right: &str) -> core::cmp::Ordering {
    for (left, right) in left.split('.').zip(right.split('.')) {
        let left_num = left.parse::<u64>().ok();
        let right_num = right.parse::<u64>().ok();
        let ordering = match (left_num, right_num) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => core::cmp::Ordering::Less,
            (None, Some(_)) => core::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        };
        if !ordering.is_eq() {
            return ordering;
        }
    }
    left.split('.').count().cmp(&right.split('.').count())
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
    fn version_requirements_match_supported_cargo_like_forms() {
        let version = "1.4.2".parse::<MusubiVersion>().expect("version");

        assert!(
            "^1.2.0"
                .parse::<MusubiVersionReq>()
                .expect("caret")
                .matches(&version)
                .expect("match")
        );
        assert!(
            "~1.4.0"
                .parse::<MusubiVersionReq>()
                .expect("tilde")
                .matches(&version)
                .expect("match")
        );
        assert!(
            "1.*"
                .parse::<MusubiVersionReq>()
                .expect("wildcard")
                .matches(&version)
                .expect("match")
        );
        assert!(
            ">=1.2.0,<2.0.0"
                .parse::<MusubiVersionReq>()
                .expect("comparators")
                .matches(&version)
                .expect("match")
        );
        assert!(
            !"^2.0.0"
                .parse::<MusubiVersionReq>()
                .expect("caret")
                .matches(&version)
                .expect("match")
        );
    }

    #[test]
    fn prerelease_precedence_is_lower_than_release() {
        let prerelease = "1.2.3-alpha.1"
            .parse::<MusubiVersion>()
            .expect("prerelease");
        let release = "1.2.3".parse::<MusubiVersion>().expect("release");

        assert!(
            ">=1.2.3-alpha.1,<1.2.3"
                .parse::<MusubiVersionReq>()
                .expect("range")
                .matches(&prerelease)
                .expect("match")
        );
        assert!(
            ">=1.2.3"
                .parse::<MusubiVersionReq>()
                .expect("range")
                .matches(&release)
                .expect("match")
        );
    }

    #[test]
    fn release_roundtrips_through_norito() {
        let package = "dex.universal/swap-core@1.2.3"
            .parse::<MusubiPackageRef>()
            .expect("package ref");
        let archive = MusubiArchiveRef::new(ManifestDigest::new([1; 32]), [2; 32], 128, 2);
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

    #[test]
    fn release_validation_rejects_empty_archive_or_exports() {
        let package = "dex.universal/swap-core@1.2.3"
            .parse::<MusubiPackageRef>()
            .expect("package ref");
        let keypair = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
        let publisher = AccountId::new(keypair.public_key().clone());
        let release = MusubiRelease::new(
            package,
            MusubiArchiveRef::new(ManifestDigest::new([1; 32]), [2; 32], 0, 0),
            Vec::new(),
            Vec::new(),
            None,
            publisher,
            42,
        );

        let err = release.validate_publishable().expect_err("empty archive");
        assert!(err.reason().contains("archive"));
    }

    #[test]
    fn export_set_sorts_and_deduplicates_functions() {
        let exports = MusubiExportSet::new(vec![
            "quote".parse().expect("name"),
            "swap".parse().expect("name"),
            "quote".parse().expect("name"),
        ]);

        assert_eq!(exports.functions.len(), 2);
        assert_eq!(exports.functions[0].as_ref(), "quote");
        assert_eq!(exports.functions[1].as_ref(), "swap");
    }
}

//! First-release Musubi registry wire model.
//!
//! Musubi package identity is structural and stable: a package is keyed by its
//! home [`DataSpaceId`], package scope, and package name.  Human-facing
//! `namespace/package` selectors are resolved through immutable namespace
//! bindings before they enter releases, resolver rows, or lock graphs.

use core::cmp::Ordering;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
    string::String,
    vec::Vec,
};

use iroha_crypto::{Hash, HashOf, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::{AccountController, AccountId},
    error::ParseError,
    id::ChainId,
    name::Name,
    nexus::DataSpaceId,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid,
            ProviderIngestCompletionAuthorityV1, ProviderIngestFinalizedAnchorV1,
            ReplicationOrderId,
        },
    },
};

/// Musubi registry schema version shipped by the first release.
pub const MUSUBI_REGISTRY_VERSION_V1: u8 = 1;
/// Typed artifact-descriptor schema version shipped by the first release.
pub const MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1: u16 = 1;
/// Kotodama/IVM ABI version supported by Musubi V1.
pub const MUSUBI_IVM_ABI_VERSION_V1: u16 = 1;
/// Maximum canonical namespace length in UTF-8 bytes.
pub const MUSUBI_MAX_NAMESPACE_BYTES_V1: usize = 255;
/// Maximum canonical package-name length in ASCII bytes.
pub const MUSUBI_MAX_PACKAGE_NAME_BYTES_V1: usize = 64;
/// Maximum prerelease identifiers in a version.
pub const MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1: usize = 16;
/// Maximum bytes in one alphanumeric prerelease identifier.
pub const MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1: usize = 64;
/// Maximum comparators in a version requirement.
pub const MUSUBI_MAX_VERSION_COMPARATORS_V1: usize = 16;
/// Maximum source bytes committed by one archive.
pub const MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum CAR bytes committed by one archive.
pub const MUSUBI_MAX_CAR_BYTES_V1: u64 = 96 * 1024 * 1024;
/// Maximum regular files committed by one archive.
pub const MUSUBI_MAX_FILES_V1: u32 = 4_096;
/// Maximum chunks committed by one archive.
pub const MUSUBI_MAX_CHUNKS_V1: u32 = 16_384;
/// Maximum normal dependencies in a published release.
pub const MUSUBI_MAX_DEPENDENCIES_V1: usize = 256;
/// Maximum exported interface names in a published release.
pub const MUSUBI_MAX_EXPORTS_V1: usize = 1_024;
/// Maximum nodes in an exact publication proof or verification lock.
pub const MUSUBI_MAX_RESOLUTION_NODES_V1: usize = 1_024;
/// Maximum dependency depth in an exact publication proof.
pub const MUSUBI_MAX_RESOLUTION_DEPTH_V1: u16 = 64;
/// Maximum archive locations attached to an archive.
pub const MUSUBI_MAX_ARCHIVE_LOCATIONS_V1: usize = 4;
/// Minimum distinct healthy replicas required for fresh resolution.
pub const MUSUBI_MIN_HEALTHY_REPLICAS_V1: u16 = 3;
/// Maximum providers recorded for one archive location.
pub const MUSUBI_MAX_LOCATION_PROVIDERS_V1: usize = 64;
/// Maximum package owners.
pub const MUSUBI_MAX_PACKAGE_OWNERS_V1: usize = 64;
/// Maximum signatures carried by a namespace delegation approval set.
pub const MUSUBI_MAX_NAMESPACE_DELEGATION_APPROVALS_V1: usize = 64;
/// Maximum controller approvals on a publication staging receipt or provider attestation.
pub const MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1: usize = 64;
/// Maximum lifetime of an authenticated seed-ingress receipt.
pub const MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1: u64 = 24 * 60 * 60 * 1_000;
/// Maximum accepted maintainers.
pub const MUSUBI_MAX_PACKAGE_MAINTAINERS_V1: usize = 256;
/// Maximum accepted package members, including owners and non-owner maintainers.
pub const MUSUBI_MAX_PACKAGE_MEMBERS_V1: usize =
    MUSUBI_MAX_PACKAGE_OWNERS_V1 + MUSUBI_MAX_PACKAGE_MAINTAINERS_V1;
/// Maximum package keywords.
pub const MUSUBI_MAX_KEYWORDS_V1: usize = 32;
/// Default registry page size.
pub const MUSUBI_DEFAULT_PAGE_SIZE_V1: u32 = 50;
/// Consensus maximum registry page size.
pub const MUSUBI_MAX_PAGE_SIZE_V1: usize = 100;
/// Maximum global alias length.
pub const MUSUBI_MAX_ALIAS_BYTES_V1: usize = 32;
/// Maximum query cursor key length.
pub const MUSUBI_MAX_CURSOR_KEY_BYTES_V1: usize = 512;

/// Domain used to derive an [`ArchiveId`] from canonical Norito bytes.
pub const MUSUBI_ARCHIVE_ID_DOMAIN_V1: &[u8] = b"iroha.musubi.archive-id.v1";
/// Domain used to derive immutable release digests.
pub const MUSUBI_RELEASE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.musubi.release-digest.v1";
/// Domain used to commit the archive-independent semantic release manifest.
pub const MUSUBI_SEMANTIC_RELEASE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.musubi.semantic-release-digest.v1";
/// Domain used to derive normalized verification-lock digests.
pub const MUSUBI_VERIFICATION_LOCK_DIGEST_DOMAIN_V1: &[u8] = b"iroha.musubi.verification-lock.v1";
/// Domain used to bind immutable namespace records.
pub const MUSUBI_NAMESPACE_BINDING_DIGEST_DOMAIN_V1: &[u8] = b"iroha.musubi.namespace-binding.v1";
/// Domain used to authorize one generation-bound namespace delegation.
pub const MUSUBI_NAMESPACE_DELEGATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.musubi.namespace-delegation.signature.v1";
/// Domain used to sign an authenticated SoraFS seed-ingress receipt.
pub const MUSUBI_SEED_INGRESS_RECEIPT_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.musubi.seed-ingress-receipt.signature.v1";
/// Domain used when a provider attests that it parsed and verified a Musubi bundle.
pub const MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.musubi.provider-bundle-attestation.signature.v1";

fn parse_clean(raw: &str, empty: &'static str, invalid: &'static str) -> Result<(), ParseError> {
    if raw.is_empty() {
        return Err(ParseError::new(empty));
    }
    if raw.trim() != raw || raw.chars().any(char::is_control) {
        return Err(ParseError::new(invalid));
    }
    Ok(())
}

fn parse_u64_identifier(raw: &str) -> Result<u64, ParseError> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ParseError::new(
            "Musubi version identifiers must be numeric",
        ));
    }
    if raw.len() > 1 && raw.starts_with('0') {
        return Err(ParseError::new(
            "Musubi numeric version identifiers must not have leading zeroes",
        ));
    }
    raw.parse()
        .map_err(|_| ParseError::new("Musubi numeric version identifier overflows u64"))
}

fn digest_is_zero(bytes: &[u8; 32]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn domain_hash(domain: &[u8], encoded: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(
        &u64::try_from(domain.len())
            .expect("Musubi hash domain length fits u64")
            .to_le_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        &u64::try_from(encoded.len())
            .expect("Musubi encoded payload length fits u64")
            .to_le_bytes(),
    );
    hasher.update(encoded);
    *hasher.finalize().as_bytes()
}

fn domain_signing_hash<T: Encode>(domain: &[u8], payload: &T) -> HashOf<T> {
    let encoded = payload.encode();
    let domain_len = u64::try_from(domain.len())
        .expect("Musubi signature domain length fits u64")
        .to_le_bytes();
    let encoded_len = u64::try_from(encoded.len())
        .expect("Musubi signed payload length fits u64")
        .to_le_bytes();
    HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
        &domain_len,
        domain,
        &encoded_len,
        &encoded,
    ]))
}

fn validate_ascii_kebab(raw: &str, maximum: usize, label: &'static str) -> Result<(), ParseError> {
    parse_clean(raw, label, label)?;
    if raw.len() > maximum
        || raw.starts_with('-')
        || raw.ends_with('-')
        || raw.contains("--")
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ParseError::new(label));
    }
    Ok(())
}

/// Canonical human-facing namespace text resolved through a namespace binding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiNamespaceV1(String);

impl MusubiNamespaceV1 {
    /// Parse a dataspace-root or domain-qualified namespace.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return canonical namespace text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate namespace text obtained through decoding.
    pub fn validate(&self) -> Result<(), ParseError> {
        Self::from_str(&self.0).map(|_| ())
    }

    /// Return the optional domain segment.
    #[must_use]
    pub fn domain_segment(&self) -> Option<&str> {
        self.0.split_once('.').map(|(domain, _)| domain)
    }

    /// Return the human-facing dataspace segment.
    #[must_use]
    pub fn dataspace_segment(&self) -> &str {
        self.0
            .rsplit_once('.')
            .map_or(self.0.as_str(), |(_, dataspace)| dataspace)
    }
}

impl FromStr for MusubiNamespaceV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        parse_clean(
            raw,
            "Musubi namespace must not be empty",
            "Musubi namespace is not canonical",
        )?;
        if raw.len() > MUSUBI_MAX_NAMESPACE_BYTES_V1 || raw.contains(['/', '@', ':']) {
            return Err(ParseError::new("Musubi namespace is not canonical"));
        }
        let segments = raw.split('.').collect::<Vec<_>>();
        if !(1..=2).contains(&segments.len()) {
            return Err(ParseError::new(
                "Musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`",
            ));
        }
        let canonical = segments
            .into_iter()
            .map(Name::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ParseError::new("Musubi namespace segment is invalid"))?
            .into_iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
            .join(".");
        if canonical != raw {
            return Err(ParseError::new("Musubi namespace is not canonical"));
        }
        Ok(Self(canonical))
    }
}

impl fmt::Display for MusubiNamespaceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Structural package scope within the stable home dataspace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiPackageScopeV1 {
    /// Package belongs to the dataspace root.
    DataspaceRoot,
    /// Package belongs to a domain in the dataspace.
    Domain(Name),
}

/// Immutable binding from public namespace text to stable structural identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiNamespaceBindingV1 {
    /// Canonical public namespace.
    pub namespace: MusubiNamespaceV1,
    /// Stable home dataspace.
    pub home_dataspace: DataSpaceId,
    /// Dataspace-root or domain scope.
    pub scope: MusubiPackageScopeV1,
    /// Monotonic namespace-owner generation used to bind delegations.
    pub generation: u64,
}

impl MusubiNamespaceBindingV1 {
    /// Validate that namespace text and structural scope agree.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.namespace.validate()?;
        if self.generation == 0 {
            return Err(ParseError::new(
                "Musubi namespace binding generation must be non-zero",
            ));
        }
        match (&self.scope, self.namespace.domain_segment()) {
            (MusubiPackageScopeV1::DataspaceRoot, None) => Ok(()),
            (MusubiPackageScopeV1::Domain(domain), Some(text)) if domain.as_ref() == text => Ok(()),
            _ => Err(ParseError::new(
                "Musubi namespace binding text and scope disagree",
            )),
        }
    }

    /// Validate this binding against the authoritative SNS/domain ownership generation.
    ///
    /// Core calls this when registering the immutable binding. Later ownership changes do not
    /// rewrite the binding; package-claim authorization instead compares delegations against the
    /// then-current authoritative generation.
    pub fn validate_authority_generation(
        &self,
        authoritative_generation: u64,
    ) -> Result<(), ParseError> {
        self.validate()?;
        if authoritative_generation == 0 || self.generation != authoritative_generation {
            return Err(ParseError::new(
                "Musubi namespace binding ownership generation is stale",
            ));
        }
        Ok(())
    }

    /// Domain-separated digest of the immutable binding.
    #[must_use]
    pub fn digest(&self) -> MusubiNamespaceBindingDigestV1 {
        MusubiNamespaceBindingDigestV1(domain_hash(
            MUSUBI_NAMESPACE_BINDING_DIGEST_DOMAIN_V1,
            &self.encode(),
        ))
    }
}

/// Canonical package-name segment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageNameV1(String);

impl MusubiPackageNameV1 {
    /// Parse a lowercase ASCII kebab package name.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return the canonical name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate package-name text obtained through decoding.
    pub fn validate(&self) -> Result<(), ParseError> {
        Self::from_str(&self.0).map(|_| ())
    }
}

impl FromStr for MusubiPackageNameV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_ascii_kebab(
            raw,
            MUSUBI_MAX_PACKAGE_NAME_BYTES_V1,
            "Musubi package name must be lowercase ASCII kebab text",
        )?;
        Ok(Self(raw.to_owned()))
    }
}

impl fmt::Display for MusubiPackageNameV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable structural package identifier; namespace aliases are not embedded.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageIdV1 {
    /// Stable home dataspace.
    pub home_dataspace: DataSpaceId,
    /// Dataspace-root or domain scope.
    pub scope: MusubiPackageScopeV1,
    /// Package name inside the scope.
    pub name: MusubiPackageNameV1,
}

impl MusubiPackageIdV1 {
    /// Construct a structural package identifier.
    #[must_use]
    pub const fn new(
        home_dataspace: DataSpaceId,
        scope: MusubiPackageScopeV1,
        name: MusubiPackageNameV1,
    ) -> Self {
        Self {
            home_dataspace,
            scope,
            name,
        }
    }

    /// Validate the structural package identity recursively.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.name.validate()
    }
}

impl fmt::Display for MusubiPackageIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scope {
            MusubiPackageScopeV1::DataspaceRoot => {
                write!(formatter, "{}/{}", self.home_dataspace, self.name)
            }
            MusubiPackageScopeV1::Domain(domain) => write!(
                formatter,
                "{}.{}//{}",
                domain, self.home_dataspace, self.name
            ),
        }
    }
}

/// User-facing package selector resolved through an immutable namespace binding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageSelectorV1 {
    /// Canonical public namespace text.
    pub namespace: MusubiNamespaceV1,
    /// Package name in that namespace.
    pub name: MusubiPackageNameV1,
}

impl MusubiPackageSelectorV1 {
    /// Validate both canonical selector components.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.namespace.validate()?;
        self.name.validate()
    }
}

impl FromStr for MusubiPackageSelectorV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let (namespace, name) = raw.split_once('/').ok_or_else(|| {
            ParseError::new("Musubi package selector must use `namespace/package`")
        })?;
        if name.contains('/') {
            return Err(ParseError::new(
                "Musubi package selector must contain one slash",
            ));
        }
        Ok(Self {
            namespace: namespace.parse()?,
            name: name.parse()?,
        })
    }
}

impl fmt::Display for MusubiPackageSelectorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.namespace, self.name)
    }
}

/// One canonical SemVer prerelease identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiPrereleaseIdentifierV1 {
    /// Numeric identifier without leading zeroes.
    Numeric(u64),
    /// ASCII alphanumeric/hyphen identifier containing a non-digit.
    AlphaNumeric(String),
}

impl MusubiPrereleaseIdentifierV1 {
    fn parse(raw: &str) -> Result<Self, ParseError> {
        if raw.is_empty() || raw.len() > MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi prerelease identifier is out of bounds",
            ));
        }
        if raw.bytes().all(|byte| byte.is_ascii_digit()) {
            return parse_u64_identifier(raw).map(Self::Numeric);
        }
        if !raw
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(ParseError::new(
                "Musubi prerelease identifiers must be ASCII alphanumeric or hyphen",
            ));
        }
        Ok(Self::AlphaNumeric(raw.to_owned()))
    }

    /// Validate a decoded prerelease identifier recursively.
    pub fn validate(&self) -> Result<(), ParseError> {
        match self {
            Self::Numeric(_) => Ok(()),
            Self::AlphaNumeric(raw) => {
                if raw.is_empty()
                    || raw.len() > MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1
                    || raw.bytes().all(|byte| byte.is_ascii_digit())
                    || !raw
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                {
                    Err(ParseError::new(
                        "Musubi prerelease identifier is noncanonical or out of bounds",
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Ord for MusubiPrereleaseIdentifierV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numeric(left), Self::Numeric(right)) => left.cmp(right),
            (Self::Numeric(_), Self::AlphaNumeric(_)) => Ordering::Less,
            (Self::AlphaNumeric(_), Self::Numeric(_)) => Ordering::Greater,
            (Self::AlphaNumeric(left), Self::AlphaNumeric(right)) => left.cmp(right),
        }
    }
}

impl PartialOrd for MusubiPrereleaseIdentifierV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for MusubiPrereleaseIdentifierV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric(value) => write!(formatter, "{value}"),
            Self::AlphaNumeric(value) => formatter.write_str(value),
        }
    }
}

/// Structured canonical semantic version. Build metadata is forbidden in V1.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiVersionV1 {
    /// Major component.
    pub major: u64,
    /// Minor component.
    pub minor: u64,
    /// Patch component.
    pub patch: u64,
    /// Ordered prerelease identifiers.
    pub prerelease: Vec<MusubiPrereleaseIdentifierV1>,
}

impl MusubiVersionV1 {
    /// Construct and validate a structured version.
    pub fn new(
        major: u64,
        minor: u64,
        patch: u64,
        prerelease: Vec<MusubiPrereleaseIdentifierV1>,
    ) -> Result<Self, ParseError> {
        let value = Self {
            major,
            minor,
            patch,
            prerelease,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate consensus bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.prerelease.len() > MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1 {
            return Err(ParseError::new(
                "Musubi version has too many prerelease identifiers",
            ));
        }
        self.prerelease
            .iter()
            .try_for_each(MusubiPrereleaseIdentifierV1::validate)
    }

    /// Whether this is a prerelease version.
    #[must_use]
    pub fn is_prerelease(&self) -> bool {
        !self.prerelease.is_empty()
    }

    fn same_core(&self, other: &Self) -> bool {
        (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)
    }
}

impl FromStr for MusubiVersionV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        parse_clean(
            raw,
            "Musubi version must not be empty",
            "Musubi version is not canonical",
        )?;
        if raw.contains('+') {
            return Err(ParseError::new(
                "Musubi V1 versions do not permit build metadata",
            ));
        }
        let (core, prerelease) = raw
            .split_once('-')
            .map_or((raw, None), |(core, pre)| (core, Some(pre)));
        let components = core.split('.').collect::<Vec<_>>();
        if components.len() != 3 {
            return Err(ParseError::new(
                "Musubi version must use `MAJOR.MINOR.PATCH`",
            ));
        }
        let prerelease = match prerelease {
            None => Vec::new(),
            Some("") => return Err(ParseError::new("Musubi prerelease must not be empty")),
            Some(raw) => raw
                .split('.')
                .map(MusubiPrereleaseIdentifierV1::parse)
                .collect::<Result<Vec<_>, _>>()?,
        };
        Self::new(
            parse_u64_identifier(components[0])?,
            parse_u64_identifier(components[1])?,
            parse_u64_identifier(components[2])?,
            prerelease,
        )
    }
}

impl fmt::Display for MusubiVersionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(first) = self.prerelease.first() {
            write!(formatter, "-{first}")?;
            for identifier in &self.prerelease[1..] {
                write!(formatter, ".{identifier}")?;
            }
        }
        Ok(())
    }
}

impl Ord for MusubiVersionV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(
                || match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                    (true, true) | (false, false) => self.prerelease.cmp(&other.prerelease),
                    (true, false) => Ordering::Greater,
                    (false, true) => Ordering::Less,
                },
            )
    }
}

impl PartialOrd for MusubiVersionV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Comparator operator used by a canonical comma-separated requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiComparatorOpV1 {
    /// Greater than.
    Greater,
    /// Greater than or equal.
    GreaterOrEqual,
    /// Less than.
    Less,
    /// Less than or equal.
    LessOrEqual,
    /// Equal.
    Equal,
}

/// One exact comparator in a canonical requirement AST.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiVersionComparatorV1 {
    /// Comparator operator.
    pub op: MusubiComparatorOpV1,
    /// Complete structured version.
    pub version: MusubiVersionV1,
}

/// Payload of a `MAJOR.MINOR.*` wildcard requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiMinorWildcardV1 {
    /// Required major component.
    pub major: u64,
    /// Required minor component.
    pub minor: u64,
}

/// Canonical Cargo-style version requirement AST.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiVersionReqV1 {
    /// Any stable release (`*`); prereleases still require explicit eligibility.
    Any,
    /// Cargo-compatible bare/caret requirement.
    Caret(MusubiVersionV1),
    /// Tilde requirement.
    Tilde(MusubiVersionV1),
    /// `MAJOR.*` wildcard.
    MajorWildcard(u64),
    /// `MAJOR.MINOR.*` wildcard.
    MinorWildcard(MusubiMinorWildcardV1),
    /// Exact `=VERSION` requirement.
    Exact(MusubiVersionV1),
    /// Canonical conjunction of comma-separated comparators.
    Comparators(Vec<MusubiVersionComparatorV1>),
}

impl MusubiVersionReqV1 {
    /// Parse and canonicalize a requirement.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Validate AST bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        match self {
            Self::Caret(version) | Self::Tilde(version) | Self::Exact(version) => {
                version.validate()?
            }
            Self::Comparators(comparators) => {
                if comparators.is_empty()
                    || comparators.len() > MUSUBI_MAX_VERSION_COMPARATORS_V1
                    || comparators.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return Err(ParseError::new(
                        "Musubi comparator list is empty, oversized, or noncanonical",
                    ));
                }
                for comparator in comparators {
                    comparator.version.validate()?;
                }
                let exacts = comparators
                    .iter()
                    .filter(|item| item.op == MusubiComparatorOpV1::Equal)
                    .map(|item| &item.version)
                    .collect::<BTreeSet<_>>();
                if exacts.len() > 1 {
                    return Err(ParseError::new(
                        "Musubi comparator list contains contradictory exact versions",
                    ));
                }
            }
            Self::Any | Self::MajorWildcard(_) | Self::MinorWildcard(_) => {}
        }
        Ok(())
    }

    /// Return whether a release satisfies this requirement using Cargo prerelease eligibility.
    #[must_use]
    pub fn matches(&self, version: &MusubiVersionV1) -> bool {
        if version.is_prerelease() && !self.prerelease_eligible(version) {
            return false;
        }
        match self {
            Self::Any => true,
            Self::Caret(base) => {
                version >= base && caret_upper_bound(base).is_none_or(|upper| version < &upper)
            }
            Self::Tilde(base) => {
                version >= base && tilde_upper_bound(base).is_none_or(|upper| version < &upper)
            }
            Self::MajorWildcard(major) => version.major == *major,
            Self::MinorWildcard(wildcard) => {
                version.major == wildcard.major && version.minor == wildcard.minor
            }
            Self::Exact(expected) => version == expected,
            Self::Comparators(comparators) => comparators.iter().all(|comparator| {
                let ordering = version.cmp(&comparator.version);
                match comparator.op {
                    MusubiComparatorOpV1::Greater => ordering.is_gt(),
                    MusubiComparatorOpV1::GreaterOrEqual => ordering.is_ge(),
                    MusubiComparatorOpV1::Less => ordering.is_lt(),
                    MusubiComparatorOpV1::LessOrEqual => ordering.is_le(),
                    MusubiComparatorOpV1::Equal => ordering.is_eq(),
                }
            }),
        }
    }

    fn prerelease_eligible(&self, candidate: &MusubiVersionV1) -> bool {
        let explicitly_names_core =
            |version: &MusubiVersionV1| version.is_prerelease() && version.same_core(candidate);
        match self {
            Self::Caret(version) | Self::Tilde(version) | Self::Exact(version) => {
                explicitly_names_core(version)
            }
            Self::Comparators(comparators) => comparators
                .iter()
                .any(|comparator| explicitly_names_core(&comparator.version)),
            Self::Any | Self::MajorWildcard(_) | Self::MinorWildcard(_) => false,
        }
    }
}

impl FromStr for MusubiVersionReqV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let raw = raw.trim();
        parse_clean(
            raw,
            "Musubi version requirement must not be empty",
            "Musubi version requirement is invalid",
        )?;
        if raw == "*" {
            return Ok(Self::Any);
        }
        if let Some(version) = raw.strip_prefix('=')
            && !raw.contains(',')
        {
            return Ok(Self::Exact(version.parse()?));
        }
        if raw.contains(',') || starts_comparator(raw) {
            let mut comparators = raw
                .split(',')
                .map(str::trim)
                .map(parse_comparator)
                .collect::<Result<Vec<_>, _>>()?;
            comparators.sort();
            comparators.dedup();
            let requirement = Self::Comparators(comparators);
            requirement.validate()?;
            return Ok(requirement);
        }
        if let Some(version) = raw.strip_prefix('^') {
            return Ok(Self::Caret(version.parse()?));
        }
        if let Some(version) = raw.strip_prefix('~') {
            return Ok(Self::Tilde(version.parse()?));
        }
        if let Some(prefix) = raw.strip_suffix(".*") {
            let parts = prefix.split('.').collect::<Vec<_>>();
            return match parts.as_slice() {
                [major] => Ok(Self::MajorWildcard(parse_u64_identifier(major)?)),
                [major, minor] => Ok(Self::MinorWildcard(MusubiMinorWildcardV1 {
                    major: parse_u64_identifier(major)?,
                    minor: parse_u64_identifier(minor)?,
                })),
                _ => Err(ParseError::new(
                    "Musubi wildcard must be `MAJOR.*` or `MAJOR.MINOR.*`",
                )),
            };
        }
        // Cargo treats a bare complete version as a caret requirement.
        Ok(Self::Caret(raw.parse()?))
    }
}

impl fmt::Display for MusubiVersionReqV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => formatter.write_str("*"),
            Self::Caret(version) => write!(formatter, "^{version}"),
            Self::Tilde(version) => write!(formatter, "~{version}"),
            Self::MajorWildcard(major) => write!(formatter, "{major}.*"),
            Self::MinorWildcard(wildcard) => {
                write!(formatter, "{}.{}.*", wildcard.major, wildcard.minor)
            }
            Self::Exact(version) => write!(formatter, "={version}"),
            Self::Comparators(comparators) => {
                for (index, comparator) in comparators.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(",")?;
                    }
                    let operator = match comparator.op {
                        MusubiComparatorOpV1::Greater => ">",
                        MusubiComparatorOpV1::GreaterOrEqual => ">=",
                        MusubiComparatorOpV1::Less => "<",
                        MusubiComparatorOpV1::LessOrEqual => "<=",
                        MusubiComparatorOpV1::Equal => "=",
                    };
                    write!(formatter, "{operator}{}", comparator.version)?;
                }
                Ok(())
            }
        }
    }
}

fn starts_comparator(raw: &str) -> bool {
    raw.starts_with(['>', '<', '='])
}

fn parse_comparator(raw: &str) -> Result<MusubiVersionComparatorV1, ParseError> {
    let (op, version) = if let Some(version) = raw.strip_prefix(">=") {
        (MusubiComparatorOpV1::GreaterOrEqual, version)
    } else if let Some(version) = raw.strip_prefix("<=") {
        (MusubiComparatorOpV1::LessOrEqual, version)
    } else if let Some(version) = raw.strip_prefix('>') {
        (MusubiComparatorOpV1::Greater, version)
    } else if let Some(version) = raw.strip_prefix('<') {
        (MusubiComparatorOpV1::Less, version)
    } else if let Some(version) = raw.strip_prefix('=') {
        (MusubiComparatorOpV1::Equal, version)
    } else {
        return Err(ParseError::new(
            "Musubi comparator must start with >, >=, <, <=, or =",
        ));
    };
    Ok(MusubiVersionComparatorV1 {
        op,
        version: version.parse()?,
    })
}

fn caret_upper_bound(base: &MusubiVersionV1) -> Option<MusubiVersionV1> {
    let core = if base.major > 0 {
        (base.major.checked_add(1)?, 0, 0)
    } else if base.minor > 0 {
        (0, base.minor.checked_add(1)?, 0)
    } else {
        (0, 0, base.patch.checked_add(1)?)
    };
    MusubiVersionV1::new(core.0, core.1, core.2, Vec::new()).ok()
}

fn tilde_upper_bound(base: &MusubiVersionV1) -> Option<MusubiVersionV1> {
    let (major, minor) = base
        .minor
        .checked_add(1)
        .map_or((base.major.checked_add(1)?, 0), |minor| (base.major, minor));
    MusubiVersionV1::new(major, minor, 0, Vec::new()).ok()
}

macro_rules! digest_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Encode,
            Decode,
            IntoSchema,
        )]
        #[repr(transparent)]
        #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
        pub struct $name(
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub  [u8; 32],
        );

        impl $name {
            /// Construct from exact digest bytes.
            #[must_use]
            pub const fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Access exact digest bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Whether this is the forbidden all-zero sentinel.
            #[must_use]
            pub fn is_zero(&self) -> bool {
                digest_is_zero(&self.0)
            }
        }
    };
}

digest_type!(
    ArchiveId,
    "Domain-separated identity of a canonical Musubi archive commitment."
);
digest_type!(
    MusubiContentDigestV1,
    "BLAKE3-256 content commitment used by Musubi V1."
);
digest_type!(
    MusubiReleaseDigestV1,
    "Domain-separated digest of an immutable release manifest."
);
digest_type!(
    MusubiSemanticReleaseDigestV1,
    "Domain-separated digest of an archive-independent semantic release manifest."
);
digest_type!(
    MusubiVerificationLockDigestV1,
    "Digest of a normalized exact verification lock."
);
digest_type!(
    MusubiNamespaceBindingDigestV1,
    "Digest of an immutable namespace binding."
);
digest_type!(
    MusubiArchiveLocationIdV1,
    "Stable identity of an archive location record."
);
digest_type!(
    MusubiInviteIdV1,
    "Stable identity of a package governance invitation."
);
digest_type!(
    MusubiGovernanceActionDigestV1,
    "Digest binding an enacted Parliament action."
);
digest_type!(MusubiQueryHashV1, "Digest of canonical query parameters.");

/// Complete source-archive commitment whose domain-separated Norito hash is [`ArchiveId`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArchiveCommitmentV1 {
    /// Canonical SoraFS root CID.
    pub root_cid: ManifestRootCid,
    /// Registered SoraFS chunker profile.
    pub chunker: ChunkerProfileHandle,
    /// Digest of the canonical ordered chunk plan.
    pub chunk_plan_digest: MusubiContentDigestV1,
    /// Proof-of-retrievability commitment root.
    pub por_root: MusubiContentDigestV1,
    /// Uncompressed source payload length.
    pub content_length: u64,
    /// Digest of canonical CAR bytes.
    pub car_digest: MusubiContentDigestV1,
    /// Canonical CAR byte length.
    pub car_size: u64,
    /// Digest of the complete canonical bundle.
    pub bundle_digest: MusubiContentDigestV1,
    /// Digest of the normalized source tree.
    pub source_tree_digest: MusubiContentDigestV1,
    /// Digest of the typed artifact descriptor.
    pub descriptor_digest: MusubiContentDigestV1,
    /// Number of regular source files.
    pub file_count: u32,
    /// Number of chunks.
    pub chunk_count: u32,
}

impl MusubiArchiveCommitmentV1 {
    /// Validate first-release archive bounds and non-inert commitments.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.content_length == 0 || self.content_length > MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi archive source length is out of bounds",
            ));
        }
        if self.car_size == 0 || self.car_size > MUSUBI_MAX_CAR_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi archive CAR length is out of bounds",
            ));
        }
        if self.file_count == 0 || self.file_count > MUSUBI_MAX_FILES_V1 {
            return Err(ParseError::new(
                "Musubi archive file count is out of bounds",
            ));
        }
        if self.chunk_count == 0 || self.chunk_count > MUSUBI_MAX_CHUNKS_V1 {
            return Err(ParseError::new(
                "Musubi archive chunk count is out of bounds",
            ));
        }
        if self.chunker.to_handle().len() > 128
            || [
                self.chunk_plan_digest,
                self.por_root,
                self.car_digest,
                self.bundle_digest,
                self.source_tree_digest,
                self.descriptor_digest,
            ]
            .iter()
            .any(MusubiContentDigestV1::is_zero)
        {
            return Err(ParseError::new(
                "Musubi archive contains an invalid or inert commitment",
            ));
        }
        Ok(())
    }

    /// Compute the domain-separated ArchiveId from canonical Norito bytes.
    #[must_use]
    pub fn archive_id(&self) -> ArchiveId {
        ArchiveId(domain_hash(MUSUBI_ARCHIVE_ID_DOMAIN_V1, &self.encode()))
    }
}

/// Typed descriptor parsed and verified by every provider before serving a bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArtifactDescriptorV1 {
    /// Must equal [`MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1`].
    pub version: u16,
    /// Domain-separated semantic release-manifest digest.
    pub semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
    /// Digest of the normalized source tree.
    pub source_tree_digest: MusubiContentDigestV1,
    /// Digest of the normalized verification lock.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
    /// Total bytes in the positive selected source set.
    pub source_bytes: u64,
    /// Number of regular files in the positive selected source set.
    pub source_file_count: u32,
}

impl MusubiArtifactDescriptorV1 {
    /// Construct and validate a first-release artifact descriptor.
    pub fn new(
        semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
        source_tree_digest: MusubiContentDigestV1,
        verification_lock_digest: MusubiVerificationLockDigestV1,
        source_bytes: u64,
        source_file_count: u32,
    ) -> Result<Self, ParseError> {
        let descriptor = Self {
            version: MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1,
            semantic_release_manifest_digest,
            source_tree_digest,
            verification_lock_digest,
            source_bytes,
            source_file_count,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Validate descriptor version, digest bindings, and first-release source bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.version != MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1
            || self.semantic_release_manifest_digest.is_zero()
            || self.source_tree_digest.is_zero()
            || self.verification_lock_digest.is_zero()
            || self.source_bytes == 0
            || self.source_bytes > MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1
            || self.source_file_count == 0
            || self.source_file_count > MUSUBI_MAX_FILES_V1
        {
            return Err(ParseError::new(
                "Musubi artifact descriptor is invalid or out of bounds",
            ));
        }
        Ok(())
    }
}

/// Authoritative archive registration independent of any renewable location.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRecordV1 {
    /// Derived archive identity.
    pub archive_id: ArchiveId,
    /// Complete immutable commitment.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Authenticated, expiring receipt that admitted the exact CAR body through seed ingress.
    pub staging_receipt: MusubiSeedIngressReceiptV1,
    /// Account that registered the archive.
    pub registered_by: AccountId,
    /// Finalized block height of registration.
    pub registered_at_height: u64,
    /// Monotonic location-set revision.
    pub location_revision: u64,
    /// Sorted identities of current non-retired locations for exact bounded lookup.
    pub location_ids: Vec<MusubiArchiveLocationIdV1>,
}

impl MusubiArchiveRecordV1 {
    /// Validate the commitment and its derived identity.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.commitment.validate()?;
        self.staging_receipt.validate()?;
        if self.archive_id != self.commitment.archive_id()
            || self.staging_receipt.payload.binding.archive_id != self.archive_id
            || self.staging_receipt.payload.binding.car_body_digest != self.commitment.car_digest
            || self.staging_receipt.payload.binding.car_body_length != self.commitment.car_size
            || self.staging_receipt.payload.binding.publisher != self.registered_by
            || self.registered_at_height == 0
            || self.location_revision == 0
            || self.location_ids.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || self
                .location_ids
                .iter()
                .any(MusubiArchiveLocationIdV1::is_zero)
            || self.location_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi archive record identity, staging receipt, or revision is invalid",
            ));
        }
        Ok(())
    }
}

/// Lifecycle of one renewable SoraFS archive location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiArchiveLocationStateV1 {
    /// Registered but not yet finalized at replication quorum.
    Pending,
    /// Finalized and healthy.
    Healthy,
    /// Finalized but below the registry replica target.
    Degraded,
    /// Retired from future reads.
    Retired,
}

/// Canonical ordered key for one renewable archive location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArchiveLocationKeyV1 {
    /// Archive served by the location.
    pub archive_id: ArchiveId,
    /// Stable identity within the archive's bounded location set.
    pub location_id: MusubiArchiveLocationIdV1,
}

impl MusubiArchiveLocationKeyV1 {
    /// Construct the canonical ordered location key.
    #[must_use]
    pub const fn new(archive_id: ArchiveId, location_id: MusubiArchiveLocationIdV1) -> Self {
        Self {
            archive_id,
            location_id,
        }
    }
}

/// Fixed-size reverse-index value from one SoraFS pin manifest to one Musubi location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiPinLocationReferenceV1 {
    /// Exact pin-manifest key duplicated for snapshot consistency validation.
    pub pin_manifest: ManifestDigest,
    /// Uniquely bound Musubi archive location.
    pub location: MusubiArchiveLocationKeyV1,
    /// Whether this is the location's current pin rather than an immutable reuse tombstone.
    pub active: bool,
}

impl MusubiPinLocationReferenceV1 {
    /// Validate non-inert pin and location identities.
    pub fn validate(&self) -> Result<(), ParseError> {
        if digest_is_zero(self.pin_manifest.as_bytes())
            || self.location.archive_id.is_zero()
            || self.location.location_id.is_zero()
        {
            return Err(ParseError::new(
                "Musubi pin-to-location reverse reference is invalid",
            ));
        }
        Ok(())
    }
}

/// Fixed-size reverse-index value from one SoraFS replication order to one Musubi location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiReplicationOrderLocationReferenceV1 {
    /// Exact replication-order key duplicated for snapshot consistency validation.
    pub replication_order: ReplicationOrderId,
    /// Uniquely bound Musubi archive location.
    pub location: MusubiArchiveLocationKeyV1,
    /// Whether this is the location's current order rather than an immutable reuse tombstone.
    pub active: bool,
}

impl MusubiReplicationOrderLocationReferenceV1 {
    /// Validate non-inert order and location identities.
    pub fn validate(&self) -> Result<(), ParseError> {
        if digest_is_zero(self.replication_order.as_bytes())
            || self.location.archive_id.is_zero()
            || self.location.location_id.is_zero()
        {
            return Err(ParseError::new(
                "Musubi replication-order-to-location reverse reference is invalid",
            ));
        }
        Ok(())
    }
}

/// Ordered provider/location composite key for exact provider-prefix lifecycle refreshes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderLocationKeyV1 {
    /// Provider whose loss or restoration affects the location.
    pub provider_id: ProviderId,
    /// Musubi location containing verified evidence from this provider.
    pub location: MusubiArchiveLocationKeyV1,
}

impl MusubiProviderLocationKeyV1 {
    /// Construct an exact provider/location reverse-index key.
    #[must_use]
    pub const fn new(provider_id: ProviderId, location: MusubiArchiveLocationKeyV1) -> Self {
        Self {
            provider_id,
            location,
        }
    }

    /// Return inclusive ordered bounds covering only one provider's location references.
    #[must_use]
    pub fn provider_range(provider_id: ProviderId) -> std::ops::RangeInclusive<Self> {
        let location = |fill| {
            MusubiArchiveLocationKeyV1::new(
                ArchiveId::new([fill; 32]),
                MusubiArchiveLocationIdV1::new([fill; 32]),
            )
        };
        Self::new(provider_id, location(0))..=Self::new(provider_id, location(u8::MAX))
    }

    /// Validate non-inert provider and location identities.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.provider_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.location.archive_id.is_zero()
            || self.location.location_id.is_zero()
        {
            return Err(ParseError::new(
                "Musubi provider-to-location reverse key is invalid",
            ));
        }
        Ok(())
    }
}

/// Renewable SoraFS pin and replication-order binding for an archive.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveLocationV1 {
    /// Stable location identity.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Archive served by this location.
    pub archive_id: ArchiveId,
    /// Registry-grade pin manifest.
    pub pin_manifest: ManifestDigest,
    /// SoraFS replication order.
    pub replication_order: ReplicationOrderId,
    /// Distinct providers whose completions were finalized.
    pub providers: Vec<ProviderId>,
    /// Provider-signed evidence that each finalized completion parsed and verified the bundle.
    pub provider_attestations: Vec<MusubiProviderBundleVerificationAttestationV1>,
    /// Earliest epoch at which the location should be renewed.
    pub renew_after_epoch: u64,
    /// Epoch after which this location is no longer valid.
    pub expires_at_epoch: u64,
    /// Finalized block height of the latest state transition.
    pub finalized_height: u64,
    /// Compare-and-set revision.
    pub revision: u64,
    /// Current location state.
    pub state: MusubiArchiveLocationStateV1,
}

impl MusubiArchiveLocationV1 {
    /// Return the canonical ordered storage key.
    #[must_use]
    pub const fn key(&self) -> MusubiArchiveLocationKeyV1 {
        MusubiArchiveLocationKeyV1::new(self.archive_id, self.location_id)
    }

    /// Validate provider, renewal, and revision bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.location_id.is_zero()
            || self.archive_id.is_zero()
            || self.providers.is_empty()
            || self.providers.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
            || self.provider_attestations.len() != self.providers.len()
            || self.renew_after_epoch >= self.expires_at_epoch
            || self.finalized_height == 0
            || self.revision == 0
        {
            return Err(ParseError::new("Musubi archive location is invalid"));
        }
        if self.providers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ParseError::new(
                "Musubi archive location providers must be sorted and distinct",
            ));
        }
        for (provider, attestation) in self.providers.iter().zip(&self.provider_attestations) {
            attestation.validate()?;
            let binding = &attestation.payload.binding;
            if &binding.provider_id != provider
                || binding.archive_id != self.archive_id
                || binding.replication_order != self.replication_order
            {
                return Err(ParseError::new(
                    "Musubi archive location attestation does not match its provider or order",
                ));
            }
        }
        if self
            .provider_attestations
            .windows(2)
            .any(|pair| pair[0].payload.binding.provider_id >= pair[1].payload.binding.provider_id)
        {
            return Err(ParseError::new(
                "Musubi archive location attestations must be sorted by distinct provider",
            ));
        }
        if self.provider_attestations.windows(2).any(|pair| {
            let left = &pair[0].payload.binding;
            let right = &pair[1].payload.binding;
            left.chain_id != right.chain_id
                || left.genesis_block_hash != right.genesis_block_hash
                || left.archive_id != right.archive_id
                || left.bundle_digest != right.bundle_digest
                || left.descriptor_digest != right.descriptor_digest
                || left.semantic_release_manifest_digest != right.semantic_release_manifest_digest
                || left.verification_lock_digest != right.verification_lock_digest
                || left.source_tree_digest != right.source_tree_digest
        }) {
            return Err(ParseError::new(
                "Musubi archive location attestations disagree on bundle commitments",
            ));
        }
        Ok(())
    }
}

/// Fresh-selection availability distinct from yank and Parliament takedown state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiStorageAvailabilityV1 {
    /// Archive has finalized replication quorum and is fresh-selectable.
    Selectable,
    /// Archive is below quorum; locked fetches may use remaining locations.
    BelowQuorum,
    /// No healthy location is currently known.
    Unavailable,
}

/// Finalized aggregate availability projection for an archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArchiveAvailabilityV1 {
    /// Archive identity.
    pub archive_id: ArchiveId,
    /// Aggregate location state.
    pub availability: MusubiStorageAvailabilityV1,
    /// Number of healthy distinct replicas.
    pub healthy_replicas: u16,
    /// Number of non-retired locations with active matching pin/order evidence.
    pub active_locations: u8,
    /// Finalized anchor height.
    pub finalized_height: u64,
    /// Finalized block hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub finalized_block_hash: [u8; 32],
    /// Universal resolver-index revision.
    pub index_revision: u64,
}

impl MusubiArchiveAvailabilityV1 {
    /// Validate aggregate consistency and first-release bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero()
            || usize::from(self.active_locations) > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || self.index_revision == 0
            || digest_is_zero(&self.finalized_block_hash)
        {
            return Err(ParseError::new(
                "Musubi archive availability record is invalid",
            ));
        }
        if matches!(self.availability, MusubiStorageAvailabilityV1::Selectable)
            && self.healthy_replicas < MUSUBI_MIN_HEALTHY_REPLICAS_V1
        {
            return Err(ParseError::new(
                "Musubi selectable archive is below replication quorum",
            ));
        }
        Ok(())
    }
}

/// Bounded universal reverse references from one archive to exact published releases.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArchiveReverseReferencesV1 {
    /// Referenced archive identity.
    pub archive_id: ArchiveId,
    /// Sorted exact releases whose immutable manifests reference the archive.
    pub releases: Vec<MusubiReleaseIdV1>,
}

impl MusubiArchiveReverseReferencesV1 {
    /// Validate identity, cardinality, and canonical exact-release order.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero()
            || self.releases.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self.releases.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi archive reverse references are invalid or noncanonical",
            ));
        }
        self.releases
            .iter()
            .try_for_each(MusubiReleaseIdV1::validate)
    }
}

/// Exact structural release identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseIdV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Exact structured version.
    pub version: MusubiVersionV1,
}

impl MusubiReleaseIdV1 {
    /// Construct an exact release identifier.
    #[must_use]
    pub const fn new(package: MusubiPackageIdV1, version: MusubiVersionV1) -> Self {
        Self { package, version }
    }

    /// Validate package identity and structured version recursively.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.version.validate()
    }
}

impl fmt::Display for MusubiReleaseIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.package, self.version)
    }
}

/// Kotodama source edition accepted by Musubi V1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiKotodamaEditionV1 {
    /// First-release Kotodama edition.
    V1,
}

/// Exact IVM ABI binding embedded in every release and lock node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiAbiBindingV1 {
    /// Must equal [`MUSUBI_IVM_ABI_VERSION_V1`].
    pub abi_version: u16,
    /// Canonical IVM ABI hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub abi_hash: [u8; 32],
}

impl MusubiAbiBindingV1 {
    /// Construct the only first-release ABI binding.
    pub fn new(abi_hash: [u8; 32]) -> Result<Self, ParseError> {
        if digest_is_zero(&abi_hash) {
            return Err(ParseError::new("Musubi ABI hash must not be zero"));
        }
        Ok(Self {
            abi_version: MUSUBI_IVM_ABI_VERSION_V1,
            abi_hash,
        })
    }

    /// Validate the fixed ABI version and non-inert hash.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.abi_version != MUSUBI_IVM_ABI_VERSION_V1 || digest_is_zero(&self.abi_hash) {
            return Err(ParseError::new(
                "Musubi release has an invalid IVM ABI binding",
            ));
        }
        Ok(())
    }
}

/// Normal dependency requirement in a published manifest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiDependencyReqV1 {
    /// Parent-local import alias.
    pub alias: Name,
    /// Stable dependency package identity.
    pub package: MusubiPackageIdV1,
    /// Published SemVer range.
    pub requirement: MusubiVersionReqV1,
}

impl MusubiDependencyReqV1 {
    /// Validate structural identity and the canonical version requirement.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.requirement.validate()
    }
}

/// Dependency kind recorded in consumer-owned exact locks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiDependencyKindV1 {
    /// Published normal dependency.
    Normal,
    /// Root-local development dependency; never propagates transitively.
    Development,
}

macro_rules! bounded_text_type {
    ($name:ident, $maximum:expr, $doc:literal, $error:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema,
        )]
        #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
        pub struct $name(String);

        impl $name {
            /// Parse canonical bounded text.
            pub fn new(raw: &str) -> Result<Self, ParseError> {
                parse_clean(raw, $error, $error)?;
                if raw.len() > $maximum {
                    return Err(ParseError::new($error));
                }
                Ok(Self(raw.to_owned()))
            }

            /// Return the validated text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Validate text obtained through decoding.
            pub fn validate(&self) -> Result<(), ParseError> {
                Self::new(&self.0).map(|_| ())
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(raw: &str) -> Result<Self, Self::Err> {
                Self::new(raw)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

bounded_text_type!(
    MusubiDescriptionV1,
    4_096,
    "Bounded immutable release or mutable package description.",
    "Musubi description is empty, noncanonical, or exceeds 4096 bytes"
);
bounded_text_type!(
    MusubiDocumentRefV1,
    2_048,
    "Bounded readme, license, or repository reference.",
    "Musubi document reference is empty, noncanonical, or exceeds 2048 bytes"
);
bounded_text_type!(
    MusubiReasonV1,
    1_024,
    "Bounded governance, yank, or takedown reason.",
    "Musubi reason is empty, noncanonical, or exceeds 1024 bytes"
);

/// Canonical lowercase keyword.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiKeywordV1(String);

impl MusubiKeywordV1 {
    /// Validate keyword text obtained through decoding.
    pub fn validate(&self) -> Result<(), ParseError> {
        Self::from_str(&self.0).map(|_| ())
    }
}

impl FromStr for MusubiKeywordV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_ascii_kebab(raw, 64, "Musubi keyword must be lowercase ASCII kebab text")?;
        Ok(Self(raw.to_owned()))
    }
}

impl fmt::Display for MusubiKeywordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Immutable descriptive metadata committed by a release digest.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseMetadataV1 {
    /// Human description.
    pub description: Option<MusubiDescriptionV1>,
    /// Packaged readme path or identifier.
    pub readme: Option<MusubiDocumentRefV1>,
    /// SPDX expression or packaged license path.
    pub license: Option<MusubiDocumentRefV1>,
    /// Public source repository.
    pub repository: Option<MusubiDocumentRefV1>,
    /// Sorted unique keywords.
    pub keywords: Vec<MusubiKeywordV1>,
}

impl MusubiReleaseMetadataV1 {
    /// Canonicalize keyword set order.
    pub fn canonicalize(&mut self) {
        self.keywords.sort();
        self.keywords.dedup();
    }

    /// Validate keyword bounds and canonical ordering.
    pub fn validate(&self) -> Result<(), ParseError> {
        if let Some(description) = &self.description {
            description.validate()?;
        }
        if let Some(readme) = &self.readme {
            readme.validate()?;
        }
        if let Some(license) = &self.license {
            license.validate()?;
        }
        if let Some(repository) = &self.repository {
            repository.validate()?;
        }
        if self.keywords.len() > MUSUBI_MAX_KEYWORDS_V1
            || self.keywords.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi keywords exceed their bound or are not sorted and unique",
            ));
        }
        self.keywords.iter().try_for_each(MusubiKeywordV1::validate)
    }
}

/// Finalized universal registry snapshot used by a resolution graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiRegistrySnapshotV1 {
    /// Finalized block height.
    pub finalized_height: u64,
    /// Finalized block hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub finalized_block_hash: [u8; 32],
    /// Resolver sparse-index revision.
    pub index_revision: u64,
}

impl MusubiRegistrySnapshotV1 {
    /// Validate a non-inert finalized anchor and revision.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.finalized_height == 0
            || self.index_revision == 0
            || digest_is_zero(&self.finalized_block_hash)
        {
            return Err(ParseError::new("Musubi registry snapshot is invalid"));
        }
        Ok(())
    }
}

/// Parent-local exact edge in a publication proof or verification lock.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiExactDependencyEdgeV1 {
    /// Parent-local import alias.
    pub alias: Name,
    /// Dependency kind.
    pub kind: MusubiDependencyKindV1,
    /// Published package/range requirement.
    pub package: MusubiPackageIdV1,
    /// Published range.
    pub requirement: MusubiVersionReqV1,
    /// Exact selected release.
    pub selected: MusubiReleaseIdV1,
}

impl MusubiExactDependencyEdgeV1 {
    /// Validate structural identity and requirement satisfaction.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.selected.validate()?;
        self.requirement.validate()?;
        if self.selected.package != self.package
            || !self.requirement.matches(&self.selected.version)
        {
            return Err(ParseError::new(
                "Musubi exact dependency does not satisfy its package requirement",
            ));
        }
        Ok(())
    }
}

/// Exact immutable dependency node used in publication verification.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiVerificationNodeV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
    /// Immutable release digest.
    pub release_digest: MusubiReleaseDigestV1,
    /// Source archive identity.
    pub archive_id: ArchiveId,
    /// Normalized source-tree digest.
    pub source_digest: MusubiContentDigestV1,
    /// Typed-interface digest.
    pub interface_digest: MusubiContentDigestV1,
    /// ABI binding.
    pub abi: MusubiAbiBindingV1,
    /// Sorted parent-local exact edges.
    pub dependencies: Vec<MusubiExactDependencyEdgeV1>,
}

impl MusubiVerificationNodeV1 {
    /// Validate node commitments, dependency bounds, and edge order.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        if self.release_digest.is_zero()
            || self.archive_id.is_zero()
            || self.source_digest.is_zero()
            || self.interface_digest.is_zero()
            || self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi verification node is invalid or noncanonical",
            ));
        }
        self.dependencies
            .iter()
            .try_for_each(MusubiExactDependencyEdgeV1::validate)
    }
}

/// Normalized, secret-free exact verification lock packaged with a release.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiVerificationLockV1 {
    /// Lock schema identifier; must equal `musubi-verification-lock`.
    pub schema: String,
    /// Schema version; must equal one.
    pub version: u8,
    /// Root release whose dependencies are proven.
    pub root: MusubiReleaseIdV1,
    /// Sorted exact selections for every direct normal dependency of the root.
    pub root_dependencies: Vec<MusubiExactDependencyEdgeV1>,
    /// Sorted exact dependency nodes; the root itself is not included.
    pub nodes: Vec<MusubiVerificationNodeV1>,
}

impl MusubiVerificationLockV1 {
    /// Fixed verification-lock schema label.
    pub const SCHEMA: &'static str = "musubi-verification-lock";

    /// Canonicalize all set-like vectors.
    pub fn canonicalize(&mut self) {
        self.root_dependencies.sort();
        self.root_dependencies.dedup();
        for node in &mut self.nodes {
            node.dependencies.sort();
            node.dependencies.dedup();
        }
        self.nodes
            .sort_by(|left, right| left.release.cmp(&right.release));
        self.nodes
            .dedup_by(|left, right| left.release == right.release);
    }

    /// Validate schema, graph bounds, uniqueness, cycles, and depth.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.root.validate()?;
        if self.schema != Self::SCHEMA
            || self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.root_dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self
                .root_dependencies
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.nodes.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self
                .nodes
                .windows(2)
                .any(|pair| pair[0].release >= pair[1].release)
        {
            return Err(ParseError::new(
                "Musubi verification lock is invalid or noncanonical",
            ));
        }
        let nodes = self
            .nodes
            .iter()
            .map(|node| (&node.release, node))
            .collect::<BTreeMap<_, _>>();
        for dependency in &self.root_dependencies {
            dependency.validate()?;
            if dependency.kind != MusubiDependencyKindV1::Normal
                || !nodes.contains_key(&dependency.selected)
            {
                return Err(ParseError::new(
                    "Musubi root dependency must be normal and select an exact proof node",
                ));
            }
        }
        for node in &self.nodes {
            node.validate()?;
        }
        validate_exact_graph(&self.nodes)
    }

    /// Compute the normalized lock digest.
    #[must_use]
    pub fn digest(&self) -> MusubiVerificationLockDigestV1 {
        MusubiVerificationLockDigestV1(domain_hash(
            MUSUBI_VERIFICATION_LOCK_DIGEST_DOMAIN_V1,
            &self.encode(),
        ))
    }
}

fn validate_exact_graph(nodes: &[MusubiVerificationNodeV1]) -> Result<(), ParseError> {
    let by_release = nodes
        .iter()
        .map(|node| (&node.release, node))
        .collect::<BTreeMap<_, _>>();
    let mut complete = BTreeSet::new();
    let mut visiting = BTreeSet::new();

    fn visit<'a>(
        release: &'a MusubiReleaseIdV1,
        depth: u16,
        by_release: &BTreeMap<&'a MusubiReleaseIdV1, &'a MusubiVerificationNodeV1>,
        visiting: &mut BTreeSet<&'a MusubiReleaseIdV1>,
        complete: &mut BTreeSet<&'a MusubiReleaseIdV1>,
    ) -> Result<(), ParseError> {
        if depth > MUSUBI_MAX_RESOLUTION_DEPTH_V1 {
            return Err(ParseError::new(
                "Musubi verification graph exceeds maximum depth",
            ));
        }
        if complete.contains(release) {
            return Ok(());
        }
        if !visiting.insert(release) {
            return Err(ParseError::new(
                "Musubi verification graph contains a cycle",
            ));
        }
        let node = by_release.get(release).ok_or_else(|| {
            ParseError::new("Musubi verification graph references a missing node")
        })?;
        for edge in &node.dependencies {
            if edge.kind == MusubiDependencyKindV1::Normal {
                visit(
                    &edge.selected,
                    depth.saturating_add(1),
                    by_release,
                    visiting,
                    complete,
                )?;
            }
        }
        visiting.remove(release);
        complete.insert(release);
        Ok(())
    }

    for release in by_release.keys().copied() {
        visit(release, 1, &by_release, &mut visiting, &mut complete)?;
    }
    Ok(())
}

/// Bounded exact resolution proof supplied at publication.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiResolutionProofV1 {
    /// Finalized universal-index snapshot used by the resolver.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Normalized exact verification lock.
    pub lock: MusubiVerificationLockV1,
}

impl MusubiResolutionProofV1 {
    /// Validate the finalized anchor and exact graph.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        self.lock.validate()
    }
}

/// Canonical archive-independent release semantics embedded in the Musubi bundle.
///
/// The archive identity is deliberately absent: the canonical bundle embeds this
/// payload, so including [`ArchiveId`] here would create an impossible hash cycle
/// between the bundle digest and the archive commitment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSemanticReleaseManifestV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
    /// Kotodama source edition.
    pub edition: MusubiKotodamaEditionV1,
    /// Exact IVM ABI V1 binding.
    pub abi: MusubiAbiBindingV1,
    /// Sorted normal dependency ranges.
    pub dependencies: Vec<MusubiDependencyReqV1>,
    /// Sorted exported Kotodama interface names.
    pub exports: Vec<Name>,
    /// Typed-interface digest.
    pub interface_digest: MusubiContentDigestV1,
    /// Immutable descriptive metadata.
    pub metadata: MusubiReleaseMetadataV1,
    /// Digest of the packaged normalized verification lock.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
}

impl MusubiSemanticReleaseManifestV1 {
    /// Canonicalize every set-like semantic field before packaging.
    pub fn canonicalize(&mut self) {
        self.dependencies.sort();
        self.dependencies.dedup();
        self.exports.sort();
        self.exports.dedup();
        self.metadata.canonicalize();
    }

    /// Validate archive-independent release semantics and canonical ordering.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        self.metadata.validate()?;
        if self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.exports.len() > MUSUBI_MAX_EXPORTS_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || self.exports.windows(2).any(|pair| pair[0] >= pair[1])
            || self.interface_digest.is_zero()
            || self.verification_lock_digest.is_zero()
        {
            return Err(ParseError::new(
                "Musubi semantic release manifest is invalid or noncanonical",
            ));
        }
        for dependency in &self.dependencies {
            dependency.validate()?;
            if dependency.package == self.release.package {
                return Err(ParseError::new(
                    "Musubi release cannot depend on its own package",
                ));
            }
        }
        Ok(())
    }

    /// Domain-separated digest used inside bundles, staging receipts, and provider attestations.
    #[must_use]
    pub fn semantic_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        MusubiSemanticReleaseDigestV1(domain_hash(
            MUSUBI_SEMANTIC_RELEASE_DIGEST_DOMAIN_V1,
            &self.encode(),
        ))
    }
}

/// Immutable registry release manifest binding semantic content to one source archive.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiReleaseManifestV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
    /// Kotodama source edition.
    pub edition: MusubiKotodamaEditionV1,
    /// Exact IVM ABI V1 binding.
    pub abi: MusubiAbiBindingV1,
    /// Sorted normal dependency ranges.
    pub dependencies: Vec<MusubiDependencyReqV1>,
    /// Sorted exported Kotodama interface names.
    pub exports: Vec<Name>,
    /// Typed-interface digest.
    pub interface_digest: MusubiContentDigestV1,
    /// Immutable descriptive metadata.
    pub metadata: MusubiReleaseMetadataV1,
    /// Canonical source archive.
    pub archive_id: ArchiveId,
    /// Digest of the packaged normalized verification lock.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
}

impl MusubiReleaseManifestV1 {
    /// Canonicalize set-like fields before publication.
    pub fn canonicalize(&mut self) {
        self.dependencies.sort();
        self.dependencies.dedup();
        self.exports.sort();
        self.exports.dedup();
        self.metadata.canonicalize();
    }

    /// Project the canonical archive-independent manifest embedded in the bundle.
    #[must_use]
    pub fn semantic_manifest(&self) -> MusubiSemanticReleaseManifestV1 {
        MusubiSemanticReleaseManifestV1 {
            release: self.release.clone(),
            edition: self.edition,
            abi: self.abi,
            dependencies: self.dependencies.clone(),
            exports: self.exports.clone(),
            interface_digest: self.interface_digest,
            metadata: self.metadata.clone(),
            verification_lock_digest: self.verification_lock_digest,
        }
    }

    /// Compute the archive-independent bundle/receipt/provider-attestation digest.
    #[must_use]
    pub fn semantic_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        self.semantic_manifest().semantic_digest()
    }

    /// Explicit alias for [`Self::semantic_digest`].
    #[must_use]
    pub fn semantic_release_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        self.semantic_digest()
    }

    /// Validate first-release release-manifest invariants.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.semantic_manifest().validate()?;
        if self.archive_id.is_zero() {
            return Err(ParseError::new(
                "Musubi registry release manifest has an invalid archive identity",
            ));
        }
        Ok(())
    }

    /// Domain-separated immutable release digest.
    #[must_use]
    pub fn release_digest(&self) -> MusubiReleaseDigestV1 {
        MusubiReleaseDigestV1(domain_hash(MUSUBI_RELEASE_DIGEST_DOMAIN_V1, &self.encode()))
    }
}

/// Publication payload that binds a release to its independently validated exact proof.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPublicationV1 {
    /// Immutable release manifest.
    pub manifest: MusubiReleaseManifestV1,
    /// Exact dependency proof and packaged verification lock.
    pub resolution: MusubiResolutionProofV1,
}

impl MusubiPublicationV1 {
    /// Validate release, proof root, lock digest, and direct dependency selections.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.manifest.validate()?;
        self.resolution.validate()?;
        if self.resolution.lock.root != self.manifest.release
            || self.resolution.lock.digest() != self.manifest.verification_lock_digest
        {
            return Err(ParseError::new(
                "Musubi publication proof does not bind the release manifest",
            ));
        }
        if self.manifest.dependencies.len() != self.resolution.lock.root_dependencies.len() {
            return Err(ParseError::new(
                "Musubi publication proof direct dependency count is inconsistent",
            ));
        }
        for (manifest, exact) in self
            .manifest
            .dependencies
            .iter()
            .zip(&self.resolution.lock.root_dependencies)
        {
            if exact.kind != MusubiDependencyKindV1::Normal
                || exact.alias != manifest.alias
                || exact.package != manifest.package
                || exact.requirement != manifest.requirement
            {
                return Err(ParseError::new(
                    "Musubi publication proof does not exactly bind a direct dependency",
                ));
            }
        }
        Ok(())
    }
}

/// Exact, replay-resistant request binding accepted by authenticated SoraFS seed ingress.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSeedIngressReceiptBindingV1 {
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact committed genesis block hash for the selected chain.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_block_hash: [u8; 32],
    /// Account publishing the package release.
    pub publisher: AccountId,
    /// Authenticated seed-ingress broker whose controller signs the receipt.
    pub ingress_broker: AccountId,
    /// SoraFS provider selected by the ingress broker.
    pub seed_provider: ProviderId,
    /// Domain-separated digest of the semantic release manifest.
    pub semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
    /// Exact archive commitment staged by this request.
    pub archive_id: ArchiveId,
    /// Digest of the exact CAR request body received by ingress.
    pub car_body_digest: MusubiContentDigestV1,
    /// Length of the exact CAR request body received by ingress.
    pub car_body_length: u64,
    /// Unpredictable operation nonce preventing receipt replay across attempts.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; 32],
}

impl MusubiSeedIngressReceiptBindingV1 {
    /// Validate every exact deployment, actor, commitment, and anti-replay binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        if digest_is_zero(&self.genesis_block_hash)
            || self.seed_provider.as_bytes().iter().all(|byte| *byte == 0)
            || self.semantic_release_manifest_digest.is_zero()
            || self.archive_id.is_zero()
            || self.car_body_digest.is_zero()
            || self.car_body_length == 0
            || self.car_body_length > MUSUBI_MAX_CAR_BYTES_V1
            || digest_is_zero(&self.nonce)
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt binding is invalid",
            ));
        }
        Ok(())
    }
}

/// Canonical expiring statement signed by an authenticated SoraFS seed-ingress broker.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSeedIngressReceiptPayloadV1 {
    /// Receipt schema version; must equal one.
    pub version: u8,
    /// Exact ingress request and deployment binding.
    pub binding: MusubiSeedIngressReceiptBindingV1,
    /// Broker issuance time in Unix milliseconds.
    pub issued_at_ms: u64,
    /// Inclusive receipt expiry in Unix milliseconds.
    pub expires_at_ms: u64,
}

impl MusubiSeedIngressReceiptPayloadV1 {
    /// Validate the closed schema, exact request binding, and bounded positive lifetime.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.binding.validate()?;
        let lifetime = self
            .expires_at_ms
            .checked_sub(self.issued_at_ms)
            .filter(|lifetime| *lifetime > 0)
            .ok_or_else(|| ParseError::new("Musubi seed-ingress receipt lifetime is invalid"))?;
        if self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.issued_at_ms == 0
            || lifetime > MUSUBI_MAX_SEED_INGRESS_RECEIPT_LIFETIME_MS_V1
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt lifetime or version is invalid",
            ));
        }
        Ok(())
    }

    /// Compute the domain-separated typed hash signed by the ingress broker controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        domain_signing_hash(MUSUBI_SEED_INGRESS_RECEIPT_SIGNATURE_DOMAIN_V1, self)
    }
}

/// One ingress-broker controller approval over an exact staging receipt payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSeedIngressReceiptApprovalV1 {
    /// Controller key that produced the approval.
    pub public_key: PublicKey,
    /// Signature over [`MusubiSeedIngressReceiptPayloadV1::signing_hash`].
    pub signature: SignatureOf<MusubiSeedIngressReceiptPayloadV1>,
}

/// Signed, expiring SoraFS seed-ingress receipt used by resumable publication.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSeedIngressReceiptV1 {
    /// Exact signed receipt statement.
    pub payload: MusubiSeedIngressReceiptPayloadV1,
    /// Canonically ordered approvals from the ingress broker controller.
    pub approvals: Vec<MusubiSeedIngressReceiptApprovalV1>,
}

impl MusubiSeedIngressReceiptV1 {
    /// Validate the payload and bounded, strictly ordered controller approval set.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.payload.validate()?;
        if self.approvals.is_empty()
            || self.approvals.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt approvals must be bounded, sorted, and unique",
            ));
        }
        Ok(())
    }

    /// Verify the exact request binding, receipt validity window, and broker controller quorum.
    pub fn verify(
        &self,
        expected_binding: &MusubiSeedIngressReceiptBindingV1,
        current_time_ms: u64,
    ) -> Result<(), ParseError> {
        self.validate()?;
        if &self.payload.binding != expected_binding
            || current_time_ms < self.payload.issued_at_ms
            || current_time_ms > self.payload.expires_at_ms
        {
            return Err(ParseError::new(
                "Musubi seed-ingress receipt binding or validity window does not match",
            ));
        }

        let signing_hash = self.payload.signing_hash();
        match self.payload.binding.ingress_broker.controller() {
            AccountController::Single(public_key) => {
                let [approval] = self.approvals.as_slice() else {
                    return Err(ParseError::new(
                        "Musubi single-key ingress broker requires exactly one approval",
                    ));
                };
                if &approval.public_key != public_key {
                    return Err(ParseError::new(
                        "Musubi seed-ingress receipt approval is not a broker key",
                    ));
                }
                approval
                    .signature
                    .verify_hash(public_key, signing_hash)
                    .map_err(|_| ParseError::new("Musubi seed-ingress receipt signature failed"))
            }
            AccountController::Multisig(policy) => {
                let mut approved_weight = 0_u32;
                for approval in &self.approvals {
                    let Some(member) = policy
                        .members()
                        .iter()
                        .find(|member| member.public_key() == &approval.public_key)
                    else {
                        return Err(ParseError::new(
                            "Musubi seed-ingress receipt approval is not a broker key",
                        ));
                    };
                    approval
                        .signature
                        .verify_hash(&approval.public_key, signing_hash)
                        .map_err(|_| {
                            ParseError::new("Musubi seed-ingress receipt signature failed")
                        })?;
                    approved_weight = approved_weight
                        .checked_add(u32::from(member.weight()))
                        .ok_or_else(|| {
                            ParseError::new("Musubi seed-ingress receipt weight overflows")
                        })?;
                }
                if approved_weight < u32::from(policy.threshold()) {
                    return Err(ParseError::new(
                        "Musubi seed-ingress receipt does not meet broker threshold",
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Exact parsed-bundle and finalized-replication completion bound by one provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleVerificationBindingV1 {
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact committed genesis block hash for the selected chain.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_block_hash: [u8; 32],
    /// Provider that parsed and verified the canonical bundle.
    pub provider_id: ProviderId,
    /// Registered provider owner that finalized the replication completion.
    pub completed_by: AccountId,
    /// Exact governed completion authority accepted by the chain.
    pub completion_authority: ProviderIngestCompletionAuthorityV1,
    /// Replication order whose completion is being attested.
    pub replication_order: ReplicationOrderId,
    /// Exact order-scoped assignment revision accepted at completion.
    pub assignment_revision: u64,
    /// Epoch at which this provider completed ingestion.
    pub completion_epoch: u64,
    /// Finalized committed-chain anchor used by the completion.
    pub finalized_anchor: ProviderIngestFinalizedAnchorV1,
    /// Exact archive whose bundle was parsed and verified.
    pub archive_id: ArchiveId,
    /// Digest of the complete canonical bundle that was parsed and verified.
    pub bundle_digest: MusubiContentDigestV1,
    /// Digest of the typed artifact descriptor parsed from the bundle.
    pub descriptor_digest: MusubiContentDigestV1,
    /// Domain-separated digest of the semantic release manifest parsed from the bundle.
    pub semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1,
    /// Digest of the normalized verification lock parsed from the bundle.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
    /// Digest of the normalized source tree parsed from the bundle.
    pub source_tree_digest: MusubiContentDigestV1,
}

impl MusubiProviderBundleVerificationBindingV1 {
    /// Validate exact provider authority, finalized completion, and parsed bundle commitments.
    pub fn validate(&self) -> Result<(), ParseError> {
        if digest_is_zero(&self.genesis_block_hash)
            || self.provider_id.as_bytes().iter().all(|byte| *byte == 0)
            || self.completed_by != self.completion_authority.provider_owner
            || !self.completion_authority.is_valid()
            || self
                .replication_order
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.assignment_revision == 0
            || self.completion_epoch == 0
            || !self.finalized_anchor.is_valid()
            || self.archive_id.is_zero()
            || self.bundle_digest.is_zero()
            || self.descriptor_digest.is_zero()
            || self.semantic_release_manifest_digest.is_zero()
            || self.verification_lock_digest.is_zero()
            || self.source_tree_digest.is_zero()
        {
            return Err(ParseError::new(
                "Musubi provider bundle verification binding is invalid",
            ));
        }
        Ok(())
    }
}

/// Canonical statement that a provider parsed and verified a bundle before finalized completion.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleVerificationPayloadV1 {
    /// Attestation schema version; must equal one.
    pub version: u8,
    /// Exact deployment, bundle, provider, and finalized completion binding.
    pub binding: MusubiProviderBundleVerificationBindingV1,
}

impl MusubiProviderBundleVerificationPayloadV1 {
    /// Validate the closed schema and every exact attestation binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.version != MUSUBI_REGISTRY_VERSION_V1 {
            return Err(ParseError::new(
                "Musubi provider bundle verification version is invalid",
            ));
        }
        self.binding.validate()
    }

    /// Compute the domain-separated typed hash signed by the provider-owner controller.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        domain_signing_hash(MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SIGNATURE_DOMAIN_V1, self)
    }
}

/// One provider-owner controller approval over an exact bundle verification payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleVerificationApprovalV1 {
    /// Provider-owner controller key that produced the approval.
    pub public_key: PublicKey,
    /// Signature over [`MusubiProviderBundleVerificationPayloadV1::signing_hash`].
    pub signature: SignatureOf<MusubiProviderBundleVerificationPayloadV1>,
}

/// Signed provider proof that the canonical bundle was parsed and verified before completion.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleVerificationAttestationV1 {
    /// Exact signed parsed-bundle and finalized-completion statement.
    pub payload: MusubiProviderBundleVerificationPayloadV1,
    /// Canonically ordered approvals from the provider-owner controller.
    pub approvals: Vec<MusubiProviderBundleVerificationApprovalV1>,
}

impl MusubiProviderBundleVerificationAttestationV1 {
    /// Validate the payload and bounded, strictly ordered controller approval set.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.payload.validate()?;
        if self.approvals.is_empty()
            || self.approvals.len() > MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(ParseError::new(
                "Musubi provider bundle approvals must be bounded, sorted, and unique",
            ));
        }
        Ok(())
    }

    /// Verify the exact finalized completion binding and provider-owner controller quorum.
    pub fn verify(
        &self,
        expected_binding: &MusubiProviderBundleVerificationBindingV1,
    ) -> Result<(), ParseError> {
        self.validate()?;
        if &self.payload.binding != expected_binding {
            return Err(ParseError::new(
                "Musubi provider bundle verification binding does not match",
            ));
        }

        let signing_hash = self.payload.signing_hash();
        match self
            .payload
            .binding
            .completion_authority
            .provider_owner
            .controller()
        {
            AccountController::Single(public_key) => {
                let [approval] = self.approvals.as_slice() else {
                    return Err(ParseError::new(
                        "Musubi single-key provider owner requires exactly one approval",
                    ));
                };
                if &approval.public_key != public_key {
                    return Err(ParseError::new(
                        "Musubi provider bundle approval is not a provider-owner key",
                    ));
                }
                approval
                    .signature
                    .verify_hash(public_key, signing_hash)
                    .map_err(|_| ParseError::new("Musubi provider bundle signature failed"))
            }
            AccountController::Multisig(policy) => {
                let mut approved_weight = 0_u32;
                for approval in &self.approvals {
                    let Some(member) = policy
                        .members()
                        .iter()
                        .find(|member| member.public_key() == &approval.public_key)
                    else {
                        return Err(ParseError::new(
                            "Musubi provider bundle approval is not a provider-owner key",
                        ));
                    };
                    approval
                        .signature
                        .verify_hash(&approval.public_key, signing_hash)
                        .map_err(|_| ParseError::new("Musubi provider bundle signature failed"))?;
                    approved_weight = approved_weight
                        .checked_add(u32::from(member.weight()))
                        .ok_or_else(|| {
                            ParseError::new("Musubi provider bundle approval weight overflows")
                        })?;
                }
                if approved_weight < u32::from(policy.threshold()) {
                    return Err(ParseError::new(
                        "Musubi provider bundle approvals do not meet provider-owner threshold",
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Canonical, domain-separated payload authorized by a namespace owner.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiNamespaceDelegationPayloadV1 {
    /// Delegation payload schema version.
    pub version: u8,
    /// Immutable namespace binding being delegated.
    pub namespace_binding: MusubiNamespaceBindingDigestV1,
    /// Authoritative SNS/domain ownership generation at signing time.
    pub owner_generation: u64,
    /// Exact authoritative namespace owner that issued the delegation.
    pub owner: AccountId,
    /// Delegated account.
    pub delegate: AccountId,
    /// Last block height at which the delegation may claim a package.
    pub expires_at_height: u64,
}

impl MusubiNamespaceDelegationPayloadV1 {
    /// Validate the closed V1 payload shape.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.namespace_binding.is_zero()
            || self.owner_generation == 0
            || self.expires_at_height == 0
        {
            return Err(ParseError::new(
                "Musubi namespace delegation payload is invalid",
            ));
        }
        Ok(())
    }

    /// Compute the canonical domain-separated hash signed by every owner approval.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<Self> {
        domain_signing_hash(MUSUBI_NAMESPACE_DELEGATION_SIGNATURE_DOMAIN_V1, self)
    }
}

/// One owner-controller approval of a namespace delegation payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiNamespaceDelegationApprovalV1 {
    /// Controller key that produced the signature.
    pub public_key: PublicKey,
    /// Signature of [`MusubiNamespaceDelegationPayloadV1::signing_hash`].
    pub signature: SignatureOf<MusubiNamespaceDelegationPayloadV1>,
}

/// Generation-bound authority to claim an absent package in one namespace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiNamespaceDelegationV1 {
    /// Exact immutable authorization payload.
    pub payload: MusubiNamespaceDelegationPayloadV1,
    /// Canonically ordered owner-controller approvals.
    pub approvals: Vec<MusubiNamespaceDelegationApprovalV1>,
}

impl MusubiNamespaceDelegationV1 {
    /// Validate the payload and the bounded, strictly ordered approval set.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.payload.validate()?;
        if self.approvals.is_empty()
            || self.approvals.len() > MUSUBI_MAX_NAMESPACE_DELEGATION_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(ParseError::new(
                "Musubi namespace delegation approvals must be bounded, sorted, and unique",
            ));
        }
        Ok(())
    }

    /// Verify a delegation against current authoritative ownership and the claiming account.
    ///
    /// The authoritative owner and generation must come from the live SNS dataspace record or
    /// domain record selected by the immutable namespace binding. Single-key and weighted
    /// multisignature account controllers are both enforced exactly.
    pub fn verify(
        &self,
        binding: &MusubiNamespaceBindingV1,
        authoritative_owner: &AccountId,
        authoritative_owner_generation: u64,
        claiming_authority: &AccountId,
        current_height: u64,
    ) -> Result<(), ParseError> {
        self.validate()?;
        if self.payload.namespace_binding != binding.digest()
            || &self.payload.owner != authoritative_owner
            || self.payload.owner_generation != authoritative_owner_generation
            || &self.payload.delegate != claiming_authority
            || current_height > self.payload.expires_at_height
        {
            return Err(ParseError::new(
                "Musubi namespace delegation does not match current authority",
            ));
        }

        let signing_hash = self.payload.signing_hash();
        match authoritative_owner.controller() {
            AccountController::Single(public_key) => {
                let [approval] = self.approvals.as_slice() else {
                    return Err(ParseError::new(
                        "Musubi single-key namespace owner requires exactly one approval",
                    ));
                };
                if &approval.public_key != public_key {
                    return Err(ParseError::new(
                        "Musubi namespace delegation approval is not an owner key",
                    ));
                }
                approval
                    .signature
                    .verify_hash(public_key, signing_hash)
                    .map_err(|_| ParseError::new("Musubi namespace delegation signature failed"))
            }
            AccountController::Multisig(policy) => {
                let mut approved_weight = 0_u32;
                for approval in &self.approvals {
                    let Some(member) = policy
                        .members()
                        .iter()
                        .find(|member| member.public_key() == &approval.public_key)
                    else {
                        return Err(ParseError::new(
                            "Musubi namespace delegation approval is not an owner key",
                        ));
                    };
                    approval
                        .signature
                        .verify_hash(&approval.public_key, signing_hash)
                        .map_err(|_| {
                            ParseError::new("Musubi namespace delegation signature failed")
                        })?;
                    approved_weight = approved_weight
                        .checked_add(u32::from(member.weight()))
                        .ok_or_else(|| {
                            ParseError::new("Musubi namespace delegation weight overflows")
                        })?;
                }
                if approved_weight < u32::from(policy.threshold()) {
                    return Err(ParseError::new(
                        "Musubi namespace delegation does not meet owner threshold",
                    ));
                }
                Ok(())
            }
        }
    }
}

/// Independent package governance revisions used for compare-and-set mutations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageRevisionsV1 {
    /// Owner, member, and invitation revision.
    pub governance: u64,
    /// Mutable package metadata revision.
    pub metadata: u64,
    /// Archive-location revision.
    pub archive_locations: u64,
}

impl MusubiPackageRevisionsV1 {
    /// All first-release revisions begin at one.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.governance == 0 || self.metadata == 0 || self.archive_locations == 0 {
            return Err(ParseError::new("Musubi package revisions must be non-zero"));
        }
        Ok(())
    }
}

/// Authoritative package record stored in the stable home dataspace.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageRecordV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Canonical namespace text under which this absent package was first claimed.
    pub claimed_namespace: MusubiNamespaceV1,
    /// Namespace binding under which the absent package was first claimed.
    pub claimed_namespace_binding: MusubiNamespaceBindingDigestV1,
    /// Sorted non-empty package-owner set.
    pub owners: Vec<AccountId>,
    /// Sorted accepted owner and maintainer accounts for exact package-local lookup.
    pub member_accounts: Vec<AccountId>,
    /// Finalized height of the first publication/claim.
    pub claimed_at_height: u64,
    /// Compare-and-set revisions.
    pub revisions: MusubiPackageRevisionsV1,
}

impl MusubiPackageRecordV1 {
    /// Validate the last-owner invariant, bounds, ordering, and revisions.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.claimed_namespace.validate()?;
        self.revisions.validate()?;
        let namespace_scope_matches =
            match (&self.package.scope, self.claimed_namespace.domain_segment()) {
                (MusubiPackageScopeV1::DataspaceRoot, None) => true,
                (MusubiPackageScopeV1::Domain(domain), Some(text)) => domain.as_ref() == text,
                _ => false,
            };
        if self.claimed_namespace_binding.is_zero()
            || !namespace_scope_matches
            || self.claimed_at_height == 0
            || self.owners.is_empty()
            || self.owners.len() > MUSUBI_MAX_PACKAGE_OWNERS_V1
            || self.owners.windows(2).any(|pair| pair[0] >= pair[1])
            || self.member_accounts.is_empty()
            || self.member_accounts.len() > MUSUBI_MAX_PACKAGE_MEMBERS_V1
            || self
                .member_accounts
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || !self
                .owners
                .iter()
                .all(|owner| self.member_accounts.binary_search(owner).is_ok())
        {
            return Err(ParseError::new(
                "Musubi package record violates ownership invariants",
            ));
        }
        Ok(())
    }
}

/// Independent permissions granted to an accepted package maintainer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiMaintainerPermissionsV1 {
    /// May publish a new immutable release.
    pub publish: bool,
    /// May yank and unyank releases.
    pub yank: bool,
    /// May update package metadata.
    pub metadata: bool,
    /// May add, renew, and retire archive locations.
    pub archive_locations: bool,
}

impl MusubiMaintainerPermissionsV1 {
    /// Whether the role grants at least one capability.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.publish && !self.yank && !self.metadata && !self.archive_locations
    }
}

/// Accepted package member role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiPackageRoleV1 {
    /// Package owner with governance authority.
    Owner,
    /// Maintainer with explicitly independent permissions.
    Maintainer(MusubiMaintainerPermissionsV1),
}

/// Canonical package-local ordered key for an accepted member.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageMemberKeyV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Accepted member account.
    pub account: AccountId,
}

impl MusubiPackageMemberKeyV1 {
    /// Construct the canonical ordered member key.
    #[must_use]
    pub const fn new(package: MusubiPackageIdV1, account: AccountId) -> Self {
        Self { package, account }
    }
}

/// Accepted package member record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageMemberV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Accepted member account.
    pub account: AccountId,
    /// Current role.
    pub role: MusubiPackageRoleV1,
    /// Finalized acceptance height.
    pub accepted_at_height: u64,
    /// Governance revision that created or last changed the member.
    pub governance_revision: u64,
}

impl MusubiPackageMemberV1 {
    /// Return the canonical ordered storage key.
    #[must_use]
    pub fn key(&self) -> MusubiPackageMemberKeyV1 {
        MusubiPackageMemberKeyV1::new(self.package.clone(), self.account.clone())
    }

    /// Validate role and revision.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.accepted_at_height == 0
            || self.governance_revision == 0
            || matches!(self.role, MusubiPackageRoleV1::Maintainer(role) if role.is_empty())
        {
            return Err(ParseError::new("Musubi package member record is invalid"));
        }
        Ok(())
    }
}

/// Invitation lifecycle; only acceptance creates package authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiInvitationStateV1 {
    /// Awaiting the invited account's acceptance.
    Pending,
    /// Accepted exactly once.
    Accepted,
    /// Revoked by package governance.
    Revoked,
    /// Expired before acceptance.
    Expired,
}

/// Package owner/maintainer invitation bound to a governance revision.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiMaintainerInvitationV1 {
    /// Stable invitation identity.
    pub invite_id: MusubiInviteIdV1,
    /// Package being governed.
    pub package: MusubiPackageIdV1,
    /// Inviting owner.
    pub invited_by: AccountId,
    /// Account that alone may accept.
    pub invited_account: AccountId,
    /// Offered role.
    pub role: MusubiPackageRoleV1,
    /// Expected governance revision for compare-and-set.
    pub expected_governance_revision: u64,
    /// Final block height at which acceptance is valid.
    pub expires_at_height: u64,
    /// Invitation lifecycle.
    pub state: MusubiInvitationStateV1,
}

impl MusubiMaintainerInvitationV1 {
    /// Validate identity, role, and compare-and-set bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.invite_id.is_zero()
            || self.expected_governance_revision == 0
            || self.expires_at_height == 0
            || matches!(self.role, MusubiPackageRoleV1::Maintainer(role) if role.is_empty())
        {
            return Err(ParseError::new("Musubi package invitation is invalid"));
        }
        Ok(())
    }
}

/// Mutable package metadata record, separate from immutable release metadata.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiPackageMetadataRecordV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Current metadata projection.
    pub metadata: MusubiReleaseMetadataV1,
    /// Compare-and-set metadata revision.
    pub revision: u64,
    /// Account that applied the revision.
    pub changed_by: AccountId,
    /// Finalized change height.
    pub changed_at_height: u64,
}

impl MusubiPackageMetadataRecordV1 {
    /// Validate metadata and revision.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.metadata.validate()?;
        if self.revision == 0 || self.changed_at_height == 0 {
            return Err(ParseError::new("Musubi package metadata record is invalid"));
        }
        Ok(())
    }
}

/// Reversible release-yank state, separate from immutable release content.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseYankV1 {
    /// Exact release.
    pub release: MusubiReleaseIdV1,
    /// `true` for yanked, `false` for unyanked.
    pub yanked: bool,
    /// Required reason for the transition.
    pub reason: MusubiReasonV1,
    /// Account applying the transition.
    pub changed_by: AccountId,
    /// Finalized transition height.
    pub changed_at_height: u64,
    /// Compare-and-set yank revision.
    pub revision: u64,
}

impl MusubiReleaseYankV1 {
    /// Validate transition anchor and revision.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.changed_at_height == 0 || self.revision == 0 {
            return Err(ParseError::new("Musubi release yank record is invalid"));
        }
        Ok(())
    }
}

/// Payload of an enacted artifact takedown.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiArtifactTakedownV1 {
    /// Enacted action digest.
    pub action_digest: MusubiGovernanceActionDigestV1,
    /// Public bounded reason.
    pub reason: MusubiReasonV1,
    /// Finalized enactment height.
    pub enacted_at_height: u64,
}

/// Governed artifact availability, independent of yank and replication health.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiArtifactGovernanceStateV1 {
    /// No enacted takedown applies.
    Available,
    /// Parliament has enacted an action-digest-bound takedown.
    TakenDown(MusubiArtifactTakedownV1),
}

impl MusubiArtifactGovernanceStateV1 {
    /// Validate any governed takedown binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        if let Self::TakenDown(takedown) = self
            && (takedown.action_digest.is_zero() || takedown.enacted_at_height == 0)
        {
            return Err(ParseError::new(
                "Musubi artifact takedown record is invalid",
            ));
        }
        Ok(())
    }
}

/// Complete resolver selection state for one exact release.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseSelectionStateV1 {
    /// Reversible yank state.
    pub yank: MusubiReleaseYankV1,
    /// Finalized storage availability.
    pub storage: MusubiArchiveAvailabilityV1,
    /// Parliament takedown state.
    pub governance: MusubiArtifactGovernanceStateV1,
}

impl MusubiReleaseSelectionStateV1 {
    /// Whether a fresh resolver may select this release.
    #[must_use]
    pub fn fresh_selectable(&self) -> bool {
        !self.yank.yanked
            && self.storage.availability == MusubiStorageAvailabilityV1::Selectable
            && matches!(self.governance, MusubiArtifactGovernanceStateV1::Available)
    }

    /// Validate all independent state components.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.yank.validate()?;
        self.storage.validate()?;
        self.governance.validate()
    }
}

/// Independent compare-and-set revisions for mutable release projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseRevisionsV1 {
    /// Reversible yank-state revision.
    pub yank: u64,
    /// Parliament artifact-governance revision.
    pub artifact_governance: u64,
}

impl MusubiReleaseRevisionsV1 {
    /// First-release revisions are always non-zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.yank == 0 || self.artifact_governance == 0 {
            return Err(ParseError::new("Musubi release revisions must be non-zero"));
        }
        Ok(())
    }
}

/// Authoritative release record; storage health remains a separate universal projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiReleaseRecordV1 {
    /// Immutable semantic manifest.
    pub manifest: MusubiReleaseManifestV1,
    /// Domain-separated immutable manifest digest.
    pub release_digest: MusubiReleaseDigestV1,
    /// Account that published the release.
    pub published_by: AccountId,
    /// Finalized publication height.
    pub published_at_height: u64,
    /// Reversible yank projection.
    pub yank: MusubiReleaseYankV1,
    /// Parliament takedown projection.
    pub artifact_governance: MusubiArtifactGovernanceStateV1,
    /// Compare-and-set revisions for mutable projections.
    pub revisions: MusubiReleaseRevisionsV1,
}

impl MusubiReleaseRecordV1 {
    /// Validate immutable identity and all mutable projections recursively.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.manifest.validate()?;
        self.yank.validate()?;
        self.artifact_governance.validate()?;
        self.revisions.validate()?;
        if self.release_digest != self.manifest.release_digest()
            || self.yank.release != self.manifest.release
            || self.yank.revision != self.revisions.yank
            || self.published_at_height == 0
        {
            return Err(ParseError::new(
                "Musubi release record is internally inconsistent",
            ));
        }
        Ok(())
    }
}

/// Canonical permanent global alias name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiAliasNameV1(String);

impl MusubiAliasNameV1 {
    /// Return canonical alias text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate alias text obtained through decoding.
    pub fn validate(&self) -> Result<(), ParseError> {
        Self::from_str(&self.0).map(|_| ())
    }
}

impl FromStr for MusubiAliasNameV1 {
    type Err = ParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        validate_ascii_kebab(
            raw,
            MUSUBI_MAX_ALIAS_BYTES_V1,
            "Musubi alias must be 1-32 lowercase ASCII kebab characters",
        )?;
        Ok(Self(raw.to_owned()))
    }
}

impl fmt::Display for MusubiAliasNameV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Prospective global-alias price policy denominated in whole XOR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiAliasPricingPolicyV1 {
    /// Monotonic pricing revision.
    pub revision: u64,
    /// One-character alias price.
    pub length_1_xor: u64,
    /// Two-character alias price.
    pub length_2_xor: u64,
    /// Three-character alias price.
    pub length_3_xor: u64,
    /// Four-character alias price.
    pub length_4_xor: u64,
    /// Five-to-thirty-two-character alias price.
    pub length_5_to_32_xor: u64,
}

impl MusubiAliasPricingPolicyV1 {
    /// Genesis first-release prices.
    pub const GENESIS: Self = Self {
        revision: 1,
        length_1_xor: 1_000,
        length_2_xor: 200,
        length_3_xor: 40,
        length_4_xor: 8,
        length_5_to_32_xor: 1,
    };

    /// Price for a validated alias.
    #[must_use]
    pub fn price_for(&self, alias: &MusubiAliasNameV1) -> u64 {
        match alias.as_str().len() {
            1 => self.length_1_xor,
            2 => self.length_2_xor,
            3 => self.length_3_xor,
            4 => self.length_4_xor,
            _ => self.length_5_to_32_xor,
        }
    }

    /// Validate a prospective non-zero policy.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.revision == 0
            || [
                self.length_1_xor,
                self.length_2_xor,
                self.length_3_xor,
                self.length_4_xor,
                self.length_5_to_32_xor,
            ]
            .contains(&0)
        {
            return Err(ParseError::new("Musubi alias pricing policy is invalid"));
        }
        Ok(())
    }
}

/// Permanent global alias registration.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiAliasRecordV1 {
    /// Permanent alias name.
    pub alias: MusubiAliasNameV1,
    /// Current structural package target.
    pub target: MusubiPackageIdV1,
    /// Registering package owner.
    pub registered_by: AccountId,
    /// Pricing-policy revision used atomically with payment.
    pub pricing_revision: u64,
    /// Whole XOR paid.
    pub paid_xor: u64,
    /// Finalized registration height.
    pub registered_at_height: u64,
    /// Monotonic history revision; registrations begin at one.
    pub history_revision: u64,
}

impl MusubiAliasRecordV1 {
    /// Validate pricing/payment and immutable registration fields.
    pub fn validate(&self, policy: &MusubiAliasPricingPolicyV1) -> Result<(), ParseError> {
        self.alias.validate()?;
        self.target.validate()?;
        policy.validate()?;
        if self.pricing_revision != policy.revision
            || self.paid_xor != policy.price_for(&self.alias)
            || self.registered_at_height == 0
            || self.history_revision == 0
        {
            return Err(ParseError::new("Musubi alias registration is invalid"));
        }
        Ok(())
    }
}

/// Permanent alias history action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiAliasHistoryActionV1 {
    /// Initial paid registration.
    Registered,
    /// Parliament recovery retarget; normal owners cannot retarget.
    ParliamentRetarget,
}

/// Canonical permanent-alias history key ordered by alias and revision.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiAliasHistoryKeyV1 {
    /// Permanent alias whose history is addressed.
    pub alias: MusubiAliasNameV1,
    /// Monotonic history revision.
    pub revision: u64,
}

impl MusubiAliasHistoryKeyV1 {
    /// Construct the canonical ordered alias-history key.
    #[must_use]
    pub const fn new(alias: MusubiAliasNameV1, revision: u64) -> Self {
        Self { alias, revision }
    }
}

/// One immutable alias-history entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiAliasHistoryEntryV1 {
    /// Alias.
    pub alias: MusubiAliasNameV1,
    /// History revision.
    pub revision: u64,
    /// Transition kind.
    pub action: MusubiAliasHistoryActionV1,
    /// Previous target; absent only for registration.
    pub previous_target: Option<MusubiPackageIdV1>,
    /// New/current target.
    pub target: MusubiPackageIdV1,
    /// Enacted action digest for Parliament retargets.
    pub governance_action: Option<MusubiGovernanceActionDigestV1>,
    /// Finalized transition height.
    pub finalized_height: u64,
}

impl MusubiAliasHistoryEntryV1 {
    /// Return the canonical ordered storage key.
    #[must_use]
    pub fn key(&self) -> MusubiAliasHistoryKeyV1 {
        MusubiAliasHistoryKeyV1::new(self.alias.clone(), self.revision)
    }

    /// Validate revision and action-specific fields.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.alias.validate()?;
        self.target.validate()?;
        if let Some(previous_target) = &self.previous_target {
            previous_target.validate()?;
        }
        let action_valid = match self.action {
            MusubiAliasHistoryActionV1::Registered => {
                self.revision == 1
                    && self.previous_target.is_none()
                    && self.governance_action.is_none()
            }
            MusubiAliasHistoryActionV1::ParliamentRetarget => {
                self.revision > 1
                    && self.previous_target.is_some()
                    && self
                        .governance_action
                        .is_some_and(|digest| !digest.is_zero())
            }
        };
        if !action_valid || self.finalized_height == 0 {
            return Err(ParseError::new("Musubi alias history entry is invalid"));
        }
        Ok(())
    }
}

/// Enacted Parliament decision binding an exact recovery action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiGovernanceDecisionV1 {
    /// Unique enacted decision identifier for replay protection.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub decision_id: [u8; 32],
    /// Digest of the exact action payload.
    pub action_digest: MusubiGovernanceActionDigestV1,
    /// Enactment height.
    pub enacted_at_height: u64,
    /// Existing mandatory execution-delay boundary.
    pub execute_after_height: u64,
}

impl MusubiGovernanceDecisionV1 {
    /// Validate replay and delay anchors.
    pub fn validate(&self) -> Result<(), ParseError> {
        if digest_is_zero(&self.decision_id)
            || self.action_digest.is_zero()
            || self.enacted_at_height == 0
            || self.execute_after_height <= self.enacted_at_height
        {
            return Err(ParseError::new("Musubi governance decision is invalid"));
        }
        Ok(())
    }
}

/// Payload for Parliament package-owner recovery.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiRecoverPackageOwnersV1 {
    /// Package being recovered.
    pub package: MusubiPackageIdV1,
    /// Sorted replacement owners.
    pub owners: Vec<AccountId>,
    /// Expected governance revision.
    pub expected_revision: u64,
}

/// Payload for Parliament alias recovery.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiRetargetAliasV1 {
    /// Alias being recovered.
    pub alias: MusubiAliasNameV1,
    /// New structural target.
    pub target: MusubiPackageIdV1,
    /// Expected history revision.
    pub expected_revision: u64,
}

/// Payload for Parliament artifact takedown.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiTakedownArtifactActionV1 {
    /// Exact release.
    pub release: MusubiReleaseIdV1,
    /// Bounded public reason.
    pub reason: MusubiReasonV1,
    /// Current artifact-governance revision required by compare-and-set.
    pub expected_artifact_governance_revision: u64,
}

/// Payload for an enacted Musubi registry-policy replacement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiSetRegistryPolicyActionV1 {
    /// Complete replacement policy.
    pub policy: MusubiRegistryPolicyV1,
    /// Current policy revision required by compare-and-set.
    pub expected_revision: u64,
}

/// Parliament-only package/alias/artifact recovery action.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiParliamentActionV1 {
    /// Replace package owners while preserving the last-owner invariant.
    RecoverPackageOwners(MusubiRecoverPackageOwnersV1),
    /// Retarget a permanent global alias while retaining history.
    RetargetAlias(MusubiRetargetAliasV1),
    /// Make one immutable artifact unavailable.
    TakedownArtifact(MusubiTakedownArtifactActionV1),
    /// Prospectively replace registry admission and alias-pricing policy.
    SetRegistryPolicy(MusubiSetRegistryPolicyActionV1),
}

impl MusubiParliamentActionV1 {
    /// Validate action-specific bounds and compare-and-set revisions.
    pub fn validate(&self) -> Result<(), ParseError> {
        match self {
            Self::RecoverPackageOwners(recovery) => {
                if recovery.owners.is_empty()
                    || recovery.owners.len() > MUSUBI_MAX_PACKAGE_OWNERS_V1
                    || recovery.owners.windows(2).any(|pair| pair[0] >= pair[1])
                    || recovery.expected_revision == 0
                {
                    return Err(ParseError::new(
                        "Musubi Parliament owner recovery is invalid",
                    ));
                }
            }
            Self::RetargetAlias(recovery) if recovery.expected_revision == 0 => {
                return Err(ParseError::new(
                    "Musubi Parliament alias retarget revision is invalid",
                ));
            }
            Self::SetRegistryPolicy(replacement) => {
                replacement.policy.validate()?;
                if replacement.expected_revision == 0
                    || replacement.expected_revision.checked_add(1)
                        != Some(replacement.policy.revision)
                {
                    return Err(ParseError::new(
                        "Musubi Parliament policy replacement revision is invalid",
                    ));
                }
            }
            Self::TakedownArtifact(takedown) => {
                takedown.release.validate()?;
                takedown.reason.validate()?;
                if takedown.expected_artifact_governance_revision == 0 {
                    return Err(ParseError::new(
                        "Musubi Parliament artifact takedown revision is invalid",
                    ));
                }
            }
            Self::RetargetAlias(_) => {}
        }
        Ok(())
    }

    /// Domain-separated digest used by the enacted decision.
    #[must_use]
    pub fn action_digest(&self) -> MusubiGovernanceActionDigestV1 {
        MusubiGovernanceActionDigestV1(domain_hash(
            b"iroha.musubi.parliament-action.v1",
            &self.encode(),
        ))
    }
}

/// Admission mode for new archives, releases, and aliases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiRegistryAdmissionModeV1 {
    /// Reject new archives, releases, and aliases.
    Closed,
    /// Admit only allowlisted stable dataspaces.
    Allowlisted,
    /// Public admission subject to normal ownership and payment checks.
    Open,
}

/// Versioned first-release Musubi registry policy.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiRegistryPolicyV1 {
    /// Must equal one.
    pub version: u8,
    /// Compare-and-set policy revision.
    pub revision: u64,
    /// Admission mode.
    pub mode: MusubiRegistryAdmissionModeV1,
    /// Sorted stable dataspaces used only by allowlisted mode.
    pub allowlisted_dataspaces: Vec<DataSpaceId>,
    /// Prospective alias prices.
    pub alias_pricing: MusubiAliasPricingPolicyV1,
}

impl Default for MusubiRegistryPolicyV1 {
    fn default() -> Self {
        Self {
            version: MUSUBI_REGISTRY_VERSION_V1,
            revision: 1,
            mode: MusubiRegistryAdmissionModeV1::Open,
            allowlisted_dataspaces: Vec::new(),
            alias_pricing: MusubiAliasPricingPolicyV1::GENESIS,
        }
    }
}

impl MusubiRegistryPolicyV1 {
    /// Validate version, bounds, ordering, and mode-specific allowlist use.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.alias_pricing.validate()?;
        if self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.revision == 0
            || self.allowlisted_dataspaces.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self
                .allowlisted_dataspaces
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || (!matches!(self.mode, MusubiRegistryAdmissionModeV1::Allowlisted)
                && !self.allowlisted_dataspaces.is_empty())
        {
            return Err(ParseError::new(
                "Musubi registry policy is invalid or noncanonical",
            ));
        }
        Ok(())
    }
}

/// Compact universal sparse-index row used by exact resolution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiResolverReleaseRowV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
    /// Immutable release digest.
    pub release_digest: MusubiReleaseDigestV1,
    /// Archive identity.
    pub archive_id: ArchiveId,
    /// Source-tree digest.
    pub source_digest: MusubiContentDigestV1,
    /// Typed-interface digest.
    pub interface_digest: MusubiContentDigestV1,
    /// ABI binding.
    pub abi: MusubiAbiBindingV1,
    /// Sorted normal dependency ranges.
    pub dependencies: Vec<MusubiDependencyReqV1>,
    /// Independent selection state.
    pub selection: MusubiReleaseSelectionStateV1,
    /// Universal index revision.
    pub index_revision: u64,
}

impl MusubiResolverReleaseRowV1 {
    /// Validate compact resolver commitments and canonical dependency order.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        self.selection.validate()?;
        if self.release_digest.is_zero()
            || self.archive_id.is_zero()
            || self.source_digest.is_zero()
            || self.interface_digest.is_zero()
            || self.index_revision == 0
            || self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || self.selection.yank.release != self.release
            || self.selection.storage.archive_id != self.archive_id
        {
            return Err(ParseError::new(
                "Musubi resolver row is invalid or noncanonical",
            ));
        }
        self.dependencies
            .iter()
            .try_for_each(MusubiDependencyReqV1::validate)
    }
}

/// Finalized cursor binding its exact query, last key, index revision, and optional caller.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiFinalizedCursorV1 {
    /// Finalized registry snapshot.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Canonical query hash.
    pub query_hash: MusubiQueryHashV1,
    /// Last returned ordered key.
    pub last_key: String,
    /// Caller binding for authorization-sensitive queries.
    pub caller: Option<AccountId>,
}

impl MusubiFinalizedCursorV1 {
    /// Validate all cursor bindings.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        if self.query_hash.is_zero()
            || self.last_key.is_empty()
            || self.last_key.len() > MUSUBI_MAX_CURSOR_KEY_BYTES_V1
            || self.last_key.chars().any(char::is_control)
        {
            return Err(ParseError::new("Musubi finalized cursor is invalid"));
        }
        Ok(())
    }
}

/// Explicit stale-cursor classification returned instead of silently restarting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiCursorFailureV1 {
    /// Finalized height/hash no longer matches the requested snapshot.
    FinalizedAnchorMismatch,
    /// Query hash differs.
    QueryMismatch,
    /// Universal sparse-index revision differs.
    IndexRevisionMismatch,
    /// Caller binding differs.
    CallerMismatch,
    /// Last key is absent from the requested ordered index.
    LastKeyStale,
}

macro_rules! musubi_page_type {
    ($name:ident, $item:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
        #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
        pub struct $name {
            /// Ordered result items.
            pub items: Vec<$item>,
            /// Cursor for the next page, absent at the end.
            pub next_cursor: Option<MusubiFinalizedCursorV1>,
            /// Finalized snapshot shared by every item.
            pub snapshot: MusubiRegistrySnapshotV1,
        }

        impl $name {
            /// Validate page size, snapshot, and cursor.
            pub fn validate(&self) -> Result<(), ParseError> {
                self.snapshot.validate()?;
                if self.items.len() > MUSUBI_MAX_PAGE_SIZE_V1 {
                    return Err(ParseError::new("Musubi query page exceeds its item bound"));
                }
                if let Some(cursor) = &self.next_cursor {
                    cursor.validate()?;
                    if cursor.snapshot != self.snapshot {
                        return Err(ParseError::new(
                            "Musubi query page cursor uses a different finalized snapshot",
                        ));
                    }
                }
                Ok(())
            }
        }
    };
}

musubi_page_type!(
    MusubiPackagePageV1,
    MusubiPackageRecordV1,
    "Ordered page of exact package records."
);
musubi_page_type!(
    MusubiReleasePageV1,
    MusubiReleaseRecordV1,
    "Ordered page of release records with yank, takedown, and revision projections."
);
musubi_page_type!(
    MusubiVersionPageV1,
    MusubiVersionV1,
    "Ordered page of structured package versions."
);
musubi_page_type!(
    MusubiMaintainerPageV1,
    MusubiPackageMemberV1,
    "Ordered page of accepted package members."
);
musubi_page_type!(
    MusubiArchiveLocationPageV1,
    MusubiArchiveLocationV1,
    "Ordered page of renewable archive locations."
);
musubi_page_type!(
    MusubiAliasHistoryPageV1,
    MusubiAliasHistoryEntryV1,
    "Ordered page of permanent alias history."
);
/// Ordered page of universal resolver-index rows with authoritative lock identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverIndexPageV1 {
    /// Deployment-selected chain identity used by generated lockfiles.
    pub chain_id: ChainId,
    /// Hash of the first finalized block used as the genesis identity.
    pub genesis_hash: [u8; 32],
    /// Ordered universal resolver-index rows.
    pub items: Vec<MusubiResolverReleaseRowV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiResolverIndexPageV1 {
    /// Validate lock identity, page size, snapshot, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.chain_id.as_str().is_empty()
            || self.genesis_hash.iter().all(|byte| *byte == 0)
            || self.items.len() > MUSUBI_MAX_PAGE_SIZE_V1
        {
            return Err(ParseError::new(
                "Musubi resolver page has an invalid chain identity or item bound",
            ));
        }
        self.snapshot.validate()?;
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if cursor.snapshot != self.snapshot {
                return Err(ParseError::new(
                    "Musubi resolver page cursor uses a different finalized snapshot",
                ));
            }
        }
        Ok(())
    }
}

/// Compact ordered directory row; rich fuzzy search may rebuild this projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiOrderedPackageEntryV1 {
    /// Human-facing namespace/package selector.
    pub selector: MusubiPackageSelectorV1,
    /// Structural package identity stored in manifests and locks.
    pub package: MusubiPackageIdV1,
    /// Highest fresh-selectable version, if any.
    pub latest_selectable: Option<MusubiVersionV1>,
    /// Package metadata revision projected into the directory.
    pub metadata_revision: u64,
    /// Universal directory revision.
    pub index_revision: u64,
}

impl MusubiOrderedPackageEntryV1 {
    /// Validate non-zero revisions and any structured version.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.selector.validate()?;
        self.package.validate()?;
        if self.metadata_revision == 0 || self.index_revision == 0 {
            return Err(ParseError::new(
                "Musubi ordered package entry has an invalid revision",
            ));
        }
        self.latest_selectable
            .as_ref()
            .map_or(Ok(()), MusubiVersionV1::validate)
    }
}

/// Ordered package-directory response with authoritative lock identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiOrderedPackagePageV1 {
    /// Deployment-selected chain identity used by generated lockfiles.
    pub chain_id: ChainId,
    /// Hash of the first finalized block used as the genesis identity.
    pub genesis_hash: [u8; 32],
    /// Ordered public-directory entries.
    pub items: Vec<MusubiOrderedPackageEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiOrderedPackagePageV1 {
    /// Validate lock identity, page size, snapshot, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.chain_id.as_str().is_empty()
            || self.genesis_hash.iter().all(|byte| *byte == 0)
            || self.items.len() > MUSUBI_MAX_PAGE_SIZE_V1
        {
            return Err(ParseError::new(
                "Musubi directory page has an invalid chain identity or item bound",
            ));
        }
        self.snapshot.validate()?;
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if cursor.snapshot != self.snapshot {
                return Err(ParseError::new(
                    "Musubi directory page cursor uses a different finalized snapshot",
                ));
            }
        }
        Ok(())
    }
}

/// Exact package lookup request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiExactPackageQueryV1 {
    /// Structural package identity.
    pub package: MusubiPackageIdV1,
}

/// Exact release lookup request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiExactReleaseQueryV1 {
    /// Exact release identity.
    pub release: MusubiReleaseIdV1,
}

/// Bounded ordered-prefix selector for deterministic directory/index queries.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiOrderedPrefixV1(String);

impl MusubiOrderedPrefixV1 {
    /// Parse a non-empty portable ordered prefix.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        parse_clean(
            raw,
            "Musubi ordered prefix must not be empty",
            "Musubi ordered prefix is invalid",
        )?;
        if raw.len() > MUSUBI_MAX_CURSOR_KEY_BYTES_V1 {
            return Err(ParseError::new("Musubi ordered prefix exceeds its bound"));
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return prefix text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate prefix text obtained through decoding.
    pub fn validate(&self) -> Result<(), ParseError> {
        Self::new(&self.0).map(|_| ())
    }
}

/// Shared finalized page request for versions, members, locations, aliases, and prefix scans.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiPageRequestV1 {
    /// Requested count; zero selects [`MUSUBI_DEFAULT_PAGE_SIZE_V1`].
    pub limit: u32,
    /// Continuation cursor.
    pub cursor: Option<MusubiFinalizedCursorV1>,
}

impl MusubiPageRequestV1 {
    /// Effective page size capped by the consensus maximum.
    #[must_use]
    pub fn effective_limit(&self) -> usize {
        let requested = if self.limit == 0 {
            MUSUBI_DEFAULT_PAGE_SIZE_V1
        } else {
            self.limit
        };
        usize::try_from(requested)
            .unwrap_or(usize::MAX)
            .min(MUSUBI_MAX_PAGE_SIZE_V1)
    }

    /// Validate a supplied cursor.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.cursor
            .as_ref()
            .map_or(Ok(()), MusubiFinalizedCursorV1::validate)
    }
}

/// Resolver-index range request; exact resolution never uses fuzzy search.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverIndexQueryV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Optional SemVer filtering requirement.
    pub requirement: Option<MusubiVersionReqV1>,
    /// Page controls and finalized cursor.
    pub page: MusubiPageRequestV1,
}

/// Package-scoped versions/members query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiPackagePageQueryV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Page controls.
    pub page: MusubiPageRequestV1,
}

/// Archive-location query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveLocationQueryV1 {
    /// Archive identity.
    pub archive_id: ArchiveId,
    /// Page controls.
    pub page: MusubiPageRequestV1,
}

/// Exact alias lookup or history query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiAliasQueryV1 {
    /// Permanent alias.
    pub alias: MusubiAliasNameV1,
    /// Page controls used by history; ignored by exact lookup.
    pub page: MusubiPageRequestV1,
}

/// Ordered-prefix registry query.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiOrderedPrefixQueryV1 {
    /// Canonical structural index prefix.
    pub prefix: MusubiOrderedPrefixV1,
    /// Page controls.
    pub page: MusubiPageRequestV1,
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::codec::DecodeAll as _;

    use super::*;
    use crate::{
        account::{MultisigMember, MultisigPolicy},
        sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1,
    };

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives a checked keypair");
        AccountId::new(keypair.public_key().clone())
    }

    fn package(name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
            name.parse().expect("package name"),
        )
    }

    fn release(name: &str, version: &str) -> MusubiReleaseIdV1 {
        MusubiReleaseIdV1::new(package(name), version.parse().expect("version"))
    }

    fn snapshot() -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: 42,
            finalized_block_hash: [0x42; 32],
            index_revision: 3,
        }
    }

    fn archive_commitment() -> MusubiArchiveCommitmentV1 {
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
            por_root: MusubiContentDigestV1::new([3; 32]),
            content_length: 1_024,
            car_digest: MusubiContentDigestV1::new([4; 32]),
            car_size: 2_048,
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: 2,
            chunk_count: 4,
        }
    }

    fn seed_ingress_binding(broker: AccountId) -> MusubiSeedIngressReceiptBindingV1 {
        let commitment = archive_commitment();
        MusubiSeedIngressReceiptBindingV1 {
            chain_id: ChainId::from("musubi-publish-test"),
            genesis_block_hash: [0x15; 32],
            publisher: account(20),
            ingress_broker: broker,
            seed_provider: ProviderId::new([0x16; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x17; 32]),
            archive_id: commitment.archive_id(),
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [0x18; 32],
        }
    }

    fn provider_completion_authority(owner: AccountId) -> ProviderIngestCompletionAuthorityV1 {
        ProviderIngestCompletionAuthorityV1::new(
            owner,
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [0x21; 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [0x22; 32],
            },
        )
    }

    fn provider_bundle_binding(owner: AccountId) -> MusubiProviderBundleVerificationBindingV1 {
        MusubiProviderBundleVerificationBindingV1 {
            chain_id: ChainId::from("musubi-publish-test"),
            genesis_block_hash: [0x23; 32],
            provider_id: ProviderId::new([0x24; 32]),
            completed_by: owner.clone(),
            completion_authority: provider_completion_authority(owner),
            replication_order: ReplicationOrderId::new([0x25; 32]),
            assignment_revision: 3,
            completion_epoch: 9,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: 77,
                block_hash: [0x26; 32],
            },
            archive_id: archive_commitment().archive_id(),
            bundle_digest: MusubiContentDigestV1::new([0x27; 32]),
            descriptor_digest: MusubiContentDigestV1::new([0x28; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x29; 32]),
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0x2A; 32]),
            source_tree_digest: MusubiContentDigestV1::new([0x2B; 32]),
        }
    }

    fn verification_lock(root: MusubiReleaseIdV1) -> MusubiVerificationLockV1 {
        MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root,
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        }
    }

    fn release_manifest() -> MusubiReleaseManifestV1 {
        let release = release("swap-core", "1.2.3");
        let lock = verification_lock(release.clone());
        MusubiReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
            dependencies: Vec::new(),
            exports: vec!["quote".parse().expect("export")],
            interface_digest: MusubiContentDigestV1::new([9; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            archive_id: archive_commitment().archive_id(),
            verification_lock_digest: lock.digest(),
        }
    }

    #[test]
    fn namespace_binding_uses_stable_dataspace_scope_and_generation() {
        let binding = MusubiNamespaceBindingV1 {
            namespace: "dex.universal".parse().expect("namespace"),
            home_dataspace: DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
            generation: 4,
        };
        binding.validate().expect("valid binding");
        assert!(!binding.digest().is_zero());

        let mut invalid = binding.clone();
        invalid.generation = 0;
        assert!(invalid.validate().is_err());
        invalid.generation = 1;
        invalid.scope = MusubiPackageScopeV1::DataspaceRoot;
        assert!(invalid.validate().is_err());

        let selector: MusubiPackageSelectorV1 =
            "dex.universal/swap-core".parse().expect("selector");
        assert_eq!(selector.to_string(), "dex.universal/swap-core");
    }

    #[test]
    fn namespace_delegation_authenticates_owner_generation_and_delegate() {
        let owner_keypair = KeyPair::try_from_seed(vec![41; 32], Algorithm::Ed25519)
            .expect("owner fixture keypair");
        let owner = AccountId::new(owner_keypair.public_key().clone());
        let delegate = account(42);
        let binding = MusubiNamespaceBindingV1 {
            namespace: "dex.universal".parse().expect("namespace"),
            home_dataspace: DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
            generation: 4,
        };
        binding
            .validate_authority_generation(4)
            .expect("binding generation is current at registration");
        let payload = MusubiNamespaceDelegationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            namespace_binding: binding.digest(),
            owner_generation: 4,
            owner: owner.clone(),
            delegate: delegate.clone(),
            expires_at_height: 100,
        };
        let approval = MusubiNamespaceDelegationApprovalV1 {
            public_key: owner_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                owner_keypair.private_key(),
                payload.signing_hash(),
            )
            .expect("sign delegation"),
        };
        let delegation = MusubiNamespaceDelegationV1 {
            payload,
            approvals: vec![approval],
        };
        delegation
            .verify(&binding, &owner, 4, &delegate, 100)
            .expect("current signed delegation verifies");
        assert!(
            delegation
                .verify(&binding, &owner, 5, &delegate, 100)
                .is_err()
        );
        assert!(
            delegation
                .verify(&binding, &owner, 4, &account(43), 100)
                .is_err()
        );
        assert!(
            delegation
                .verify(&binding, &owner, 4, &delegate, 101)
                .is_err()
        );
    }

    #[test]
    fn seed_ingress_receipt_rejects_expiry_replay_and_commitment_substitution() {
        let broker_keypair = KeyPair::try_from_seed(vec![51; 32], Algorithm::Ed25519)
            .expect("broker fixture keypair");
        let broker = AccountId::new(broker_keypair.public_key().clone());
        let binding = seed_ingress_binding(broker);
        let payload = MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: binding.clone(),
            issued_at_ms: 1_000,
            expires_at_ms: 2_000,
        };
        let receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_keypair.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_keypair.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign seed-ingress receipt"),
            }],
            payload,
        };

        receipt
            .verify(&binding, 1_500)
            .expect("current exact receipt verifies");
        assert!(receipt.verify(&binding, 999).is_err());
        assert!(receipt.verify(&binding, 2_001).is_err());

        let mut replayed = binding.clone();
        replayed.chain_id = ChainId::from("musubi-other-chain");
        assert!(receipt.verify(&replayed, 1_500).is_err());
        let mut substituted = binding.clone();
        substituted.archive_id = ArchiveId::new([0xEE; 32]);
        assert!(receipt.verify(&substituted, 1_500).is_err());

        let mut tampered = receipt.clone();
        tampered.payload.binding.car_body_digest = MusubiContentDigestV1::new([0xEF; 32]);
        let tampered_binding = tampered.payload.binding.clone();
        assert!(tampered.verify(&tampered_binding, 1_500).is_err());

        let decoded = MusubiSeedIngressReceiptV1::decode_all(&mut receipt.encode().as_slice())
            .expect("receipt Norito roundtrip");
        assert_eq!(decoded, receipt);
    }

    #[test]
    fn provider_bundle_attestation_requires_controller_quorum_and_exact_finalized_completion() {
        let first = KeyPair::try_from_seed(vec![61; 32], Algorithm::Ed25519)
            .expect("first provider keypair");
        let second = KeyPair::try_from_seed(vec![62; 32], Algorithm::Ed25519)
            .expect("second provider keypair");
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(first.public_key().clone(), 1).expect("first member"),
                MultisigMember::new(second.public_key().clone(), 1).expect("second member"),
            ],
        )
        .expect("provider owner policy");
        let owner = AccountId::new_multisig(policy);
        let binding = provider_bundle_binding(owner);
        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: binding.clone(),
        };
        let signing_hash = payload.signing_hash();
        let mut approvals = vec![
            MusubiProviderBundleVerificationApprovalV1 {
                public_key: first.public_key().clone(),
                signature: SignatureOf::try_from_hash(first.private_key(), signing_hash)
                    .expect("first provider approval"),
            },
            MusubiProviderBundleVerificationApprovalV1 {
                public_key: second.public_key().clone(),
                signature: SignatureOf::try_from_hash(second.private_key(), signing_hash)
                    .expect("second provider approval"),
            },
        ];
        approvals.sort_by(|left, right| left.public_key.cmp(&right.public_key));
        let attestation = MusubiProviderBundleVerificationAttestationV1 { payload, approvals };

        attestation
            .verify(&binding)
            .expect("provider controller quorum verifies exact bundle and completion");
        let mut below_quorum = attestation.clone();
        below_quorum.approvals.pop();
        assert!(below_quorum.verify(&binding).is_err());

        let mut replayed_completion = binding.clone();
        replayed_completion.finalized_anchor.block_hash = [0xED; 32];
        assert!(attestation.verify(&replayed_completion).is_err());
        let mut substituted = binding.clone();
        substituted.verification_lock_digest = MusubiVerificationLockDigestV1::new([0xEC; 32]);
        assert!(attestation.verify(&substituted).is_err());

        let mut tampered = attestation.clone();
        tampered.payload.binding.source_tree_digest = MusubiContentDigestV1::new([0xEB; 32]);
        let tampered_binding = tampered.payload.binding.clone();
        assert!(tampered.verify(&tampered_binding).is_err());

        let decoded = MusubiProviderBundleVerificationAttestationV1::decode_all(
            &mut attestation.encode().as_slice(),
        )
        .expect("provider attestation Norito roundtrip");
        assert_eq!(decoded, attestation);
    }

    #[test]
    fn structured_version_rejects_build_metadata_overflow_and_leading_zeroes() {
        assert!("1.2.3-alpha.1".parse::<MusubiVersionV1>().is_ok());
        assert!("1.2.3+local".parse::<MusubiVersionV1>().is_err());
        assert!(
            "18446744073709551616.0.0"
                .parse::<MusubiVersionV1>()
                .is_err()
        );
        assert!("1.02.3".parse::<MusubiVersionV1>().is_err());
        assert!("1.2.3-alpha.01".parse::<MusubiVersionV1>().is_err());

        let too_many = vec![
            MusubiPrereleaseIdentifierV1::Numeric(1);
            MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1 + 1
        ];
        assert!(MusubiVersionV1::new(1, 0, 0, too_many).is_err());
    }

    #[test]
    fn version_order_is_semver_order() {
        let alpha: MusubiVersionV1 = "1.0.0-alpha.2".parse().expect("alpha");
        let beta: MusubiVersionV1 = "1.0.0-alpha.10".parse().expect("beta");
        let release: MusubiVersionV1 = "1.0.0".parse().expect("release");
        assert!(alpha < beta);
        assert!(beta < release);
    }

    #[test]
    fn requirements_parse_to_one_canonical_ast() {
        let bare: MusubiVersionReqV1 = "1.2.3".parse().expect("bare");
        assert_eq!(bare.to_string(), "^1.2.3");
        assert!(matches!(
            "=1.2.3".parse::<MusubiVersionReqV1>().expect("exact"),
            MusubiVersionReqV1::Exact(_)
        ));

        let ordered: MusubiVersionReqV1 = " <2.0.0, >=1.0.0,>=1.0.0 ".parse().expect("range");
        assert_eq!(ordered.to_string(), ">=1.0.0,<2.0.0");

        assert!(
            ">=1.0.0,=1.0.0,=1.1.0"
                .parse::<MusubiVersionReqV1>()
                .is_err()
        );
    }

    #[test]
    fn requirements_apply_cargo_prerelease_eligibility() {
        let prerelease: MusubiVersionV1 = "1.2.3-beta.1".parse().expect("prerelease");
        let stable: MusubiVersionV1 = "1.2.3".parse().expect("stable");
        assert!(
            !"*".parse::<MusubiVersionReqV1>()
                .expect("any")
                .matches(&prerelease)
        );
        assert!(
            !"^1.2.0"
                .parse::<MusubiVersionReqV1>()
                .expect("caret")
                .matches(&prerelease)
        );
        assert!(
            "^1.2.3-alpha.1"
                .parse::<MusubiVersionReqV1>()
                .expect("prerelease caret")
                .matches(&prerelease)
        );
        assert!(
            "^1.2.3-alpha.1"
                .parse::<MusubiVersionReqV1>()
                .expect("prerelease caret")
                .matches(&stable)
        );
    }

    #[test]
    fn requirement_validation_recurses_into_decoded_fields() {
        let invalid = MusubiVersionReqV1::Caret(MusubiVersionV1 {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: vec![MusubiPrereleaseIdentifierV1::AlphaNumeric("01".to_owned())],
        });
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn archive_id_binds_every_canonical_commitment_field() {
        let archive = archive_commitment();
        archive.validate().expect("valid archive");
        let original = archive.archive_id();
        let mut changed = archive.clone();
        changed.car_size += 1;
        assert_ne!(original, changed.archive_id());

        let mut oversized = archive;
        oversized.content_length = MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1 + 1;
        assert!(oversized.validate().is_err());
    }

    #[test]
    fn archive_commitment_roundtrips_through_norito() {
        let archive = archive_commitment();
        let bytes = archive.encode();
        let mut cursor = bytes.as_slice();
        let decoded = MusubiArchiveCommitmentV1::decode(&mut cursor).expect("decode archive");
        assert!(cursor.is_empty());
        assert_eq!(decoded, archive);
        decoded.validate().expect("decoded archive validates");
    }

    #[test]
    fn release_manifest_and_publication_proof_are_bound() {
        let manifest = release_manifest();
        manifest.validate().expect("valid manifest");
        let lock = verification_lock(manifest.release.clone());
        let publication = MusubiPublicationV1 {
            manifest: manifest.clone(),
            resolution: MusubiResolutionProofV1 {
                snapshot: snapshot(),
                lock,
            },
        };
        publication.validate().expect("valid publication");

        let bytes = manifest.encode();
        let mut cursor = bytes.as_slice();
        let decoded = MusubiReleaseManifestV1::decode(&mut cursor).expect("decode release");
        assert!(cursor.is_empty());
        assert_eq!(decoded, manifest);
        assert_eq!(decoded.release_digest(), manifest.release_digest());

        let semantic = manifest.semantic_manifest();
        semantic.validate().expect("semantic projection validates");
        assert_eq!(semantic.semantic_digest(), manifest.semantic_digest());
        let mut different_archive = manifest.clone();
        different_archive.archive_id = ArchiveId::new([0xFE; 32]);
        assert_eq!(
            different_archive.semantic_digest(),
            manifest.semantic_digest()
        );
        assert_ne!(
            different_archive.release_digest(),
            manifest.release_digest()
        );
    }

    #[test]
    fn publication_binds_each_root_requirement_to_one_exact_node() {
        let dependency_package = package("codec");
        let selected = MusubiReleaseIdV1::new(
            dependency_package.clone(),
            "1.2.0".parse().expect("selected version"),
        );
        let parallel = MusubiReleaseIdV1::new(
            dependency_package.clone(),
            "1.3.0".parse().expect("parallel version"),
        );
        let node = |release: MusubiReleaseIdV1, fill: u8| MusubiVerificationNodeV1 {
            release,
            release_digest: MusubiReleaseDigestV1::new([fill; 32]),
            archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
            source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
            interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
            abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
            dependencies: Vec::new(),
        };
        let dependency = MusubiDependencyReqV1 {
            alias: "codec".parse().expect("alias"),
            package: dependency_package.clone(),
            requirement: "^1.0.0".parse().expect("requirement"),
        };
        let exact = MusubiExactDependencyEdgeV1 {
            alias: dependency.alias.clone(),
            kind: MusubiDependencyKindV1::Normal,
            package: dependency.package.clone(),
            requirement: dependency.requirement.clone(),
            selected: selected.clone(),
        };
        let mut lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release("swap-core", "1.2.3"),
            root_dependencies: vec![exact],
            nodes: vec![node(parallel, 20), node(selected, 10)],
        };
        lock.canonicalize();
        lock.validate().expect("exact root selection validates");

        let mut manifest = release_manifest();
        manifest.dependencies = vec![dependency];
        manifest.verification_lock_digest = lock.digest();
        let mut publication = MusubiPublicationV1 {
            manifest,
            resolution: MusubiResolutionProofV1 {
                snapshot: snapshot(),
                lock,
            },
        };
        publication
            .validate()
            .expect("one exact direct selection is unambiguous");

        publication.manifest.dependencies[0].alias = "renamed".parse().expect("alias");
        assert!(publication.validate().is_err());
    }

    #[test]
    fn exact_graph_rejects_cycles() {
        let first = release("first", "1.0.0");
        let second = release("second", "1.0.0");
        let edge = |alias: &str, selected: MusubiReleaseIdV1| MusubiExactDependencyEdgeV1 {
            alias: alias.parse().expect("alias"),
            kind: MusubiDependencyKindV1::Normal,
            package: selected.package.clone(),
            requirement: "^1.0.0".parse().expect("requirement"),
            selected,
        };
        let node = |release: MusubiReleaseIdV1, dependency: MusubiExactDependencyEdgeV1| {
            MusubiVerificationNodeV1 {
                release,
                release_digest: MusubiReleaseDigestV1::new([1; 32]),
                archive_id: ArchiveId::new([2; 32]),
                source_digest: MusubiContentDigestV1::new([3; 32]),
                interface_digest: MusubiContentDigestV1::new([4; 32]),
                abi: MusubiAbiBindingV1::new([5; 32]).expect("ABI"),
                dependencies: vec![dependency],
            }
        };
        let mut lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: 1,
            root: release("root", "1.0.0"),
            root_dependencies: Vec::new(),
            nodes: vec![
                node(first.clone(), edge("second", second.clone())),
                node(second, edge("first", first)),
            ],
        };
        lock.canonicalize();
        assert!(lock.validate().is_err());
    }

    #[test]
    fn release_record_keeps_yank_and_takedown_outside_immutable_digest() {
        let manifest = release_manifest();
        let publisher = account(7);
        let record = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            yank: MusubiReleaseYankV1 {
                release: manifest.release.clone(),
                yanked: false,
                reason: "initial publication".parse().expect("reason"),
                changed_by: publisher.clone(),
                changed_at_height: 42,
                revision: 1,
            },
            artifact_governance: MusubiArtifactGovernanceStateV1::Available,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 1,
            },
            manifest,
            published_by: publisher,
            published_at_height: 42,
        };
        record.validate().expect("valid record");
    }

    #[test]
    fn alias_names_and_genesis_prices_are_exact() {
        for (alias, expected) in [
            ("a", 1_000),
            ("ab", 200),
            ("abc", 40),
            ("abcd", 8),
            ("abcde", 1),
        ] {
            let alias: MusubiAliasNameV1 = alias.parse().expect("alias");
            assert_eq!(
                MusubiAliasPricingPolicyV1::GENESIS.price_for(&alias),
                expected
            );
        }
        assert!("Upper".parse::<MusubiAliasNameV1>().is_err());
        assert!("-bad".parse::<MusubiAliasNameV1>().is_err());
        assert!("a".repeat(33).parse::<MusubiAliasNameV1>().is_err());
    }

    #[test]
    fn page_and_cursor_bounds_are_enforced() {
        let page = MusubiVersionPageV1 {
            items: vec!["1.0.0".parse().expect("version"); MUSUBI_MAX_PAGE_SIZE_V1 + 1],
            next_cursor: None,
            snapshot: snapshot(),
        };
        assert!(page.validate().is_err());

        let cursor = MusubiFinalizedCursorV1 {
            snapshot: snapshot(),
            query_hash: MusubiQueryHashV1::new([1; 32]),
            last_key: "x".repeat(MUSUBI_MAX_CURSOR_KEY_BYTES_V1 + 1),
            caller: None,
        };
        assert!(cursor.validate().is_err());

        let resolver_page = MusubiResolverIndexPageV1 {
            chain_id: ChainId::from("musubi-test-chain"),
            genesis_hash: [9; 32],
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        resolver_page
            .validate()
            .expect("resolver page has authoritative lock identity");
        assert!(
            MusubiResolverIndexPageV1 {
                genesis_hash: [0; 32],
                ..resolver_page
            }
            .validate()
            .is_err()
        );

        let directory_page = MusubiOrderedPackagePageV1 {
            chain_id: ChainId::from("musubi-test-chain"),
            genesis_hash: [9; 32],
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        directory_page
            .validate()
            .expect("directory page has authoritative lock identity");
        assert!(
            MusubiOrderedPackagePageV1 {
                genesis_hash: [0; 32],
                ..directory_page
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn sorafs_reverse_reference_keys_are_fixed_and_provider_prefix_bounded() {
        let location = MusubiArchiveLocationKeyV1::new(
            ArchiveId::new([7; 32]),
            MusubiArchiveLocationIdV1::new([8; 32]),
        );
        let pin = MusubiPinLocationReferenceV1 {
            pin_manifest: ManifestDigest::new([9; 32]),
            location,
            active: true,
        };
        let order = MusubiReplicationOrderLocationReferenceV1 {
            replication_order: ReplicationOrderId::new([10; 32]),
            location,
            active: false,
        };
        pin.validate().expect("valid pin reverse reference");
        order.validate().expect("valid order reuse tombstone");

        let provider = ProviderId::new([11; 32]);
        let range = MusubiProviderLocationKeyV1::provider_range(provider);
        assert!(range.contains(&MusubiProviderLocationKeyV1::new(provider, location)));
        assert!(!range.contains(&MusubiProviderLocationKeyV1::new(
            ProviderId::new([12; 32]),
            location,
        )));
        MusubiProviderLocationKeyV1::new(provider, location)
            .validate()
            .expect("valid provider reverse key");
    }

    #[cfg(feature = "json")]
    #[test]
    fn v1_query_request_json_rejects_unknown_secret_fields() {
        macro_rules! assert_closed_json {
            ($request_type:ty, $request:expr) => {{
                let request: $request_type = $request;
                let canonical = norito::json::to_json(&request)
                    .expect("canonical Musubi V1 query request JSON encodes");
                assert_eq!(
                    norito::json::from_json::<$request_type>(&canonical)
                        .expect("canonical Musubi V1 query request JSON decodes"),
                    request
                );
                let hostile =
                    canonical.replacen('{', "{\"private_key\":\"must-not-be-accepted\",", 1);
                assert!(
                    norito::json::from_json::<$request_type>(&hostile).is_err(),
                    "Musubi V1 query request JSON must reject unknown secret-bearing fields"
                );
            }};
        }

        let package = package("query-contract");
        let release = MusubiReleaseIdV1::new(
            package.clone(),
            "1.2.3".parse().expect("query release version"),
        );
        let page = MusubiPageRequestV1 {
            limit: MUSUBI_DEFAULT_PAGE_SIZE_V1,
            cursor: None,
        };
        assert_closed_json!(
            MusubiExactPackageQueryV1,
            MusubiExactPackageQueryV1 {
                package: package.clone()
            }
        );
        assert_closed_json!(
            MusubiExactReleaseQueryV1,
            MusubiExactReleaseQueryV1 { release }
        );
        assert_closed_json!(
            MusubiResolverIndexQueryV1,
            MusubiResolverIndexQueryV1 {
                package: package.clone(),
                requirement: Some("^1.0.0".parse().expect("query requirement")),
                page: page.clone(),
            }
        );
        assert_closed_json!(
            MusubiPackagePageQueryV1,
            MusubiPackagePageQueryV1 {
                package: package.clone(),
                page: page.clone(),
            }
        );
        assert_closed_json!(
            MusubiArchiveLocationQueryV1,
            MusubiArchiveLocationQueryV1 {
                archive_id: archive_commitment().archive_id(),
                page: page.clone(),
            }
        );
        assert_closed_json!(
            MusubiAliasQueryV1,
            MusubiAliasQueryV1 {
                alias: "query-contract".parse().expect("query alias"),
                page: page.clone(),
            }
        );
        assert_closed_json!(
            MusubiOrderedPrefixQueryV1,
            MusubiOrderedPrefixQueryV1 {
                prefix: MusubiOrderedPrefixV1::new("query/").expect("query prefix"),
                page,
            }
        );
    }
}

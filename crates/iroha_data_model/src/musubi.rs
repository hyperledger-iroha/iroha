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
/// Maximum simultaneously pending invitations for one package.
pub const MUSUBI_MAX_PENDING_INVITATIONS_V1: usize = 256;
/// Maximum package keywords.
pub const MUSUBI_MAX_KEYWORDS_V1: usize = 32;
/// Default registry page size.
pub const MUSUBI_DEFAULT_PAGE_SIZE_V1: u32 = 50;
/// Consensus maximum registry page size.
pub const MUSUBI_MAX_PAGE_SIZE_V1: usize = 100;
/// Maximum exact archive identities in one authoritative cache-retention request.
pub const MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1: usize = MUSUBI_MAX_PAGE_SIZE_V1;
/// Maximum global alias length.
pub const MUSUBI_MAX_ALIAS_BYTES_V1: usize = 32;
/// Maximum query cursor key length.
pub const MUSUBI_MAX_CURSOR_KEY_BYTES_V1: usize = 512;
/// Maximum UTF-8 byte length accepted by rich package discovery.
pub const MUSUBI_MAX_SEARCH_QUERY_BYTES_V1: usize = 256;
/// Maximum distinct normalized terms accepted by rich package discovery.
pub const MUSUBI_MAX_SEARCH_QUERY_TERMS_V1: usize = 16;
/// Maximum UTF-8 byte length of one normalized discovery term.
pub const MUSUBI_MAX_SEARCH_TERM_BYTES_V1: usize = 64;

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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
                    || matches!(
                        comparators.as_slice(),
                        [MusubiVersionComparatorV1 {
                            op: MusubiComparatorOpV1::Equal,
                            ..
                        }]
                    )
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
            Self::Caret(base) => version >= base && caret_core_is_compatible(base, version),
            Self::Tilde(base) => version >= base && tilde_core_is_compatible(base, version),
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
                .map(|item| item.trim_matches(' '))
                .map(parse_comparator)
                .collect::<Result<Vec<_>, _>>()?;
            comparators.sort();
            comparators.dedup();
            if let [
                MusubiVersionComparatorV1 {
                    op: MusubiComparatorOpV1::Equal,
                    version,
                },
            ] = comparators.as_slice()
            {
                return Ok(Self::Exact(version.clone()));
            }
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

fn caret_core_is_compatible(base: &MusubiVersionV1, candidate: &MusubiVersionV1) -> bool {
    if base.major > 0 {
        candidate.major == base.major
    } else {
        candidate.major == 0
            && if base.minor > 0 {
                candidate.minor == base.minor
            } else {
                candidate.minor == 0 && candidate.patch == base.patch
            }
    }
}

fn tilde_core_is_compatible(base: &MusubiVersionV1, candidate: &MusubiVersionV1) -> bool {
    candidate.major == base.major && candidate.minor == base.minor
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

/// Immutable archive-registration projection independent of renewable locations.
///
/// Unlike [`MusubiArchiveRecordV1`], this projection deliberately excludes the
/// mutable location revision and current location identities. A finalized
/// registration can therefore be revalidated from a later exact archive read
/// without requiring a historical copy of mutable registry state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRegistrationProjectionV1 {
    /// Derived archive identity.
    pub archive_id: ArchiveId,
    /// Complete immutable commitment.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Exact authenticated receipt consumed by archive admission.
    pub staging_receipt: MusubiSeedIngressReceiptV1,
    /// Account that registered the archive.
    pub registered_by: AccountId,
    /// Finalized block height of registration.
    pub registered_at_height: u64,
}

impl MusubiArchiveRegistrationProjectionV1 {
    /// Validate the immutable archive identity and its exact ingress binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_archive_registration_fields(
            self.archive_id,
            &self.commitment,
            &self.staging_receipt,
            &self.registered_by,
            self.registered_at_height,
        )
    }
}

fn validate_archive_registration_fields(
    archive_id: ArchiveId,
    commitment: &MusubiArchiveCommitmentV1,
    staging_receipt: &MusubiSeedIngressReceiptV1,
    registered_by: &AccountId,
    registered_at_height: u64,
) -> Result<(), ParseError> {
    commitment.validate()?;
    staging_receipt.validate()?;
    if archive_id != commitment.archive_id()
        || staging_receipt.payload.binding.archive_id != archive_id
        || staging_receipt.payload.binding.car_body_digest != commitment.car_digest
        || staging_receipt.payload.binding.car_body_length != commitment.car_size
        || &staging_receipt.payload.binding.publisher != registered_by
        || registered_at_height == 0
    {
        return Err(ParseError::new(
            "Musubi archive registration has an invalid identity or receipt",
        ));
    }
    Ok(())
}

/// Authoritative archive registration and its mutable renewable-location directory.
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
    /// Return the immutable registration fields reproducible by every later archive read.
    #[must_use]
    pub fn registration_projection(&self) -> MusubiArchiveRegistrationProjectionV1 {
        MusubiArchiveRegistrationProjectionV1 {
            archive_id: self.archive_id,
            commitment: self.commitment.clone(),
            staging_receipt: self.staging_receipt.clone(),
            registered_by: self.registered_by.clone(),
            registered_at_height: self.registered_at_height,
        }
    }

    /// Validate the commitment and its derived identity.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_archive_registration_fields(
            self.archive_id,
            &self.commitment,
            &self.staging_receipt,
            &self.registered_by,
            self.registered_at_height,
        )?;
        if self.location_revision == 0
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
            || digest_is_zero(self.pin_manifest.as_bytes())
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
        let healthy_capacity = usize::from(self.active_locations)
            .checked_mul(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
            .expect("bounded Musubi location capacity cannot overflow usize");
        if self.archive_id.is_zero()
            || usize::from(self.active_locations) > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || usize::from(self.healthy_replicas) > healthy_capacity
            || self.finalized_height == 0
            || self.index_revision == 0
            || digest_is_zero(&self.finalized_block_hash)
        {
            return Err(ParseError::new(
                "Musubi archive availability record is invalid",
            ));
        }
        let expected = if self.healthy_replicas >= MUSUBI_MIN_HEALTHY_REPLICAS_V1 {
            MusubiStorageAvailabilityV1::Selectable
        } else if self.active_locations > 0 && self.healthy_replicas > 0 {
            MusubiStorageAvailabilityV1::BelowQuorum
        } else {
            MusubiStorageAvailabilityV1::Unavailable
        };
        if self.availability != expected {
            return Err(ParseError::new(
                "Musubi archive availability classification is inconsistent with its counts",
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
        self.package.validate()?;
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
    /// Current package governance revision for compare-and-set.
    ///
    /// Every package-governance advance atomically rebases a still-pending,
    /// unexpired invitation to the successor revision.
    pub expected_governance_revision: u64,
    /// Final block height at which acceptance is valid.
    pub expires_at_height: u64,
    /// Invitation lifecycle.
    pub state: MusubiInvitationStateV1,
}

impl MusubiMaintainerInvitationV1 {
    /// Validate identity, role, and compare-and-set bounds.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
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

/// Canonical package/account/invitation key for the maintainer directory.
///
/// Accepted members use `invitation = None`; pending invitations use their
/// globally unique invitation identity. This orders an accepted member before
/// any pending invitations for the same account without requiring a scan.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MusubiMaintainerDirectoryKeyV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Accepted or invited account; absent only in a transient package-range lower bound.
    pub account: Option<AccountId>,
    /// Pending invitation identity; absent only for an accepted member.
    pub invitation: Option<MusubiInviteIdV1>,
}

impl MusubiMaintainerDirectoryKeyV1 {
    /// Construct the accepted-member directory key.
    #[must_use]
    pub const fn accepted(package: MusubiPackageIdV1, account: AccountId) -> Self {
        Self {
            package,
            account: Some(account),
            invitation: None,
        }
    }

    /// Construct a pending-invitation directory key.
    #[must_use]
    pub const fn pending(
        package: MusubiPackageIdV1,
        account: AccountId,
        invitation: MusubiInviteIdV1,
    ) -> Self {
        Self {
            package,
            account: Some(account),
            invitation: Some(invitation),
        }
    }

    /// Construct a transient lower bound for an exact package-prefix range.
    #[must_use]
    pub const fn package_start(package: MusubiPackageIdV1) -> Self {
        Self {
            package,
            account: None,
            invitation: None,
        }
    }

    /// Validate the structural package and any invitation identity.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        if self.account.is_none()
            || self
                .invitation
                .as_ref()
                .is_some_and(MusubiInviteIdV1::is_zero)
        {
            return Err(ParseError::new(
                "Musubi maintainer directory invitation identity is invalid",
            ));
        }
        Ok(())
    }
}

/// Accepted member or pending package-governance invitation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum MusubiMaintainerDirectoryEntryV1 {
    /// Accepted owner or maintainer with current authority.
    Accepted(MusubiPackageMemberV1),
    /// Invitation that has not created authority yet.
    PendingInvitation(MusubiMaintainerInvitationV1),
}

impl MusubiMaintainerDirectoryEntryV1 {
    /// Return the canonical package/account/invitation ordering key.
    #[must_use]
    pub fn key(&self) -> MusubiMaintainerDirectoryKeyV1 {
        match self {
            Self::Accepted(member) => MusubiMaintainerDirectoryKeyV1::accepted(
                member.package.clone(),
                member.account.clone(),
            ),
            Self::PendingInvitation(invitation) => MusubiMaintainerDirectoryKeyV1::pending(
                invitation.package.clone(),
                invitation.invited_account.clone(),
                invitation.invite_id,
            ),
        }
    }

    /// Return the stable text key carried by finalized pagination cursors.
    #[must_use]
    pub fn cursor_key(&self) -> String {
        let key = self.key();
        let account = key
            .account
            .as_ref()
            .expect("persisted Musubi maintainer directory entries always carry an account");
        let encoded_account = account.encode();
        let mut account_label = String::with_capacity(encoded_account.len().saturating_mul(2));
        for byte in encoded_account {
            fmt::Write::write_fmt(&mut account_label, format_args!("{byte:02x}"))
                .expect("writing into a String cannot fail");
        }
        let invitation = key.invitation.as_ref().map_or_else(
            || "accepted".to_owned(),
            |invite_id| {
                let mut label = String::with_capacity("pending-".len() + 64);
                label.push_str("pending-");
                for byte in invite_id.as_bytes() {
                    fmt::Write::write_fmt(&mut label, format_args!("{byte:02x}"))
                        .expect("writing into a String cannot fail");
                }
                label
            },
        );
        format!("{account_label}|{invitation}")
    }

    /// Validate the record and require invitations to remain pending.
    pub fn validate(&self) -> Result<(), ParseError> {
        match self {
            Self::Accepted(member) => member.validate(),
            Self::PendingInvitation(invitation) => {
                invitation.validate()?;
                if invitation.state != MusubiInvitationStateV1::Pending {
                    return Err(ParseError::new(
                        "Musubi maintainer directory contains a non-pending invitation",
                    ));
                }
                Ok(())
            }
        }
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
        self.package.validate()?;
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
        self.release.validate()?;
        self.reason.validate()?;
        if self.changed_at_height == 0 || self.revision == 0 {
            return Err(ParseError::new("Musubi release yank record is invalid"));
        }
        Ok(())
    }
}

/// Persisted outcome of an applied artifact takedown.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArtifactTakedownV1 {
    /// Enacted action digest.
    pub action_digest: MusubiGovernanceActionDigestV1,
    /// Public bounded reason.
    pub reason: MusubiReasonV1,
    /// Finalized height where the delayed action was applied.
    pub applied_at_height: u64,
}

/// Governed artifact availability, independent of yank and replication health.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiArtifactGovernanceStateV1 {
    /// No enacted takedown applies.
    Available,
    /// Parliament has enacted an action-digest-bound takedown.
    TakenDown(MusubiArtifactTakedownV1),
}

impl MusubiArtifactGovernanceStateV1 {
    /// Validate any governed takedown binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        if let Self::TakenDown(takedown) = self {
            takedown.reason.validate()?;
            if takedown.action_digest.is_zero() || takedown.applied_at_height == 0 {
                return Err(ParseError::new(
                    "Musubi artifact takedown record is invalid",
                ));
            }
        }
        Ok(())
    }
}

/// Complete resolver selection state for one exact release.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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

/// Enacted Parliament decision binding one exact Musubi action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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

/// Persisted proof that an enacted Parliament decision was consumed on-chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiGovernanceDecisionConsumptionV1 {
    /// Exact enacted decision supplied by the governance instruction.
    pub decision: MusubiGovernanceDecisionV1,
    /// Minimum enactment delay enforced by the runtime that consumed the decision.
    pub minimum_enactment_delay: u64,
    /// Block height observed by the runtime when it consumed the decision.
    pub consumed_at_height: u64,
}

impl MusubiGovernanceDecisionConsumptionV1 {
    /// Validate the nested decision and its server-observed execution boundary.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.decision.validate()?;
        let minimum_execution_height = self
            .decision
            .enacted_at_height
            .checked_add(self.minimum_enactment_delay)
            .ok_or_else(|| ParseError::new("Musubi governance enactment delay overflows"))?;
        if self.decision.execute_after_height < minimum_execution_height {
            return Err(ParseError::new(
                "Musubi governance decision has a shorter than enforced enactment delay",
            ));
        }
        if self.consumed_at_height < self.decision.execute_after_height {
            return Err(ParseError::new(
                "Musubi governance decision was consumed before its execution boundary",
            ));
        }
        Ok(())
    }
}

/// Payload for Parliament package-owner recovery.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSetRegistryPolicyActionV1 {
    /// Complete replacement policy.
    pub policy: MusubiRegistryPolicyV1,
    /// Current policy revision required by compare-and-set.
    pub expected_revision: u64,
}

/// Closed Parliament-only Musubi recovery and policy-replacement action.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
                recovery.package.validate()?;
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
            Self::RetargetAlias(recovery) => {
                recovery.alias.validate()?;
                recovery.target.validate()?;
                if recovery.expected_revision == 0 {
                    return Err(ParseError::new(
                        "Musubi Parliament alias retarget revision is invalid",
                    ));
                }
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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

    /// Validate a strict first-release transition from `current`.
    pub fn validate_successor(&self, current: &Self) -> Result<(), ParseError> {
        current.validate()?;
        self.validate()?;

        let expected_revision = current
            .revision
            .checked_add(1)
            .ok_or_else(|| ParseError::new("Musubi registry policy revision overflow"))?;
        if self.revision != expected_revision {
            return Err(ParseError::new(
                "Musubi replacement policy revision must be the exact successor",
            ));
        }

        let prices_changed = self.alias_pricing.length_1_xor != current.alias_pricing.length_1_xor
            || self.alias_pricing.length_2_xor != current.alias_pricing.length_2_xor
            || self.alias_pricing.length_3_xor != current.alias_pricing.length_3_xor
            || self.alias_pricing.length_4_xor != current.alias_pricing.length_4_xor
            || self.alias_pricing.length_5_to_32_xor != current.alias_pricing.length_5_to_32_xor;
        if prices_changed {
            let expected_pricing_revision = current
                .alias_pricing
                .revision
                .checked_add(1)
                .ok_or_else(|| ParseError::new("Musubi alias pricing revision overflow"))?;
            if self.alias_pricing.revision != expected_pricing_revision {
                return Err(ParseError::new(
                    "changed Musubi alias prices require the exact successor pricing revision",
                ));
            }
        } else if self.alias_pricing != current.alias_pricing {
            return Err(ParseError::new(
                "unchanged Musubi alias prices must retain the current pricing policy",
            ));
        }

        Ok(())
    }
}

/// Compact universal sparse-index row used by exact resolution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    ($name:ident, $item:ty, $doc:literal, $noncanonical_order:expr) => {
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
                if self.items.len() > MUSUBI_MAX_PAGE_SIZE_V1
                    || self.items.windows(2).any($noncanonical_order)
                {
                    return Err(ParseError::new(
                        "Musubi query page exceeds its item bound or is not strictly ordered",
                    ));
                }
                self.items.iter().try_for_each(<$item>::validate)?;
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
    "Ordered page of exact package records.",
    |pair: &[MusubiPackageRecordV1]| pair[0].package >= pair[1].package
);
musubi_page_type!(
    MusubiReleasePageV1,
    MusubiReleaseRecordV1,
    "Ordered page of release records with yank, takedown, and revision projections.",
    |pair: &[MusubiReleaseRecordV1]| pair[0].manifest.release >= pair[1].manifest.release
);
/// Ordered page of structured package versions bound to its exact request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiVersionPageV1 {
    /// Exact request whose results this page carries.
    pub query: MusubiPackagePageQueryV1,
    /// Ordered structured versions for `query.package`.
    pub items: Vec<MusubiVersionV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiVersionPageV1 {
    /// Validate request identity, page bounds, strict order, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self.items.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ParseError::new(
                "Musubi version page is not strictly ordered",
            ));
        }
        self.items.iter().try_for_each(MusubiVersionV1::validate)?;
        if let Some(cursor) = &self.query.page.cursor {
            let previous = cursor
                .last_key
                .parse::<MusubiVersionV1>()
                .map_err(|_| ParseError::new("Musubi version cursor key is invalid"))?;
            if self
                .items
                .first()
                .is_some_and(|version| version <= &previous)
            {
                return Err(ParseError::new(
                    "Musubi version page does not advance its structured cursor",
                ));
            }
        }
        let first_key = self.items.first().map(ToString::to_string);
        let last_key = self.items.last().map(ToString::to_string);
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            &self.next_cursor,
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    pub fn validate_for(&self, query: &MusubiPackagePageQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi version page carries a different request context",
            ));
        }
        Ok(())
    }
}

/// Ordered page of package members and invitations bound to its exact request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiMaintainerPageV1 {
    /// Exact request whose results this page carries.
    pub query: MusubiPackagePageQueryV1,
    /// Ordered accepted package members and pending invitations.
    pub items: Vec<MusubiMaintainerDirectoryEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiMaintainerPageV1 {
    /// Validate request identity, package membership, bounds, order, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self
            .items
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
        {
            return Err(ParseError::new(
                "Musubi maintainer page is not strictly ordered",
            ));
        }
        for entry in &self.items {
            entry.validate()?;
            if entry.key().package != self.query.package {
                return Err(ParseError::new(
                    "Musubi maintainer page item belongs to a different package",
                ));
            }
        }
        let first_key = self
            .items
            .first()
            .map(MusubiMaintainerDirectoryEntryV1::cursor_key);
        let last_key = self
            .items
            .last()
            .map(MusubiMaintainerDirectoryEntryV1::cursor_key);
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            &self.next_cursor,
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    pub fn validate_for(&self, query: &MusubiPackagePageQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi maintainer page carries a different request context",
            ));
        }
        Ok(())
    }
}
/// Ordered renewable locations plus their authoritative immutable archive commitment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveLocationPageV1 {
    /// Deployment-selected chain identity used by locks and archive admission.
    pub chain_id: ChainId,
    /// Hash of the first finalized block used as the genesis identity.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Current authoritative archive record and full source commitment.
    ///
    /// [`MusubiArchiveRecordV1::registration_projection`] excludes this record's mutable
    /// location directory for finality checks that outlive the named snapshot.
    pub archive: MusubiArchiveRecordV1,
    /// Ordered current non-retired locations for the archive.
    pub items: Vec<MusubiArchiveLocationV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by the archive and every location.
    pub snapshot: MusubiRegistrySnapshotV1,
}

/// Authoritative cache-retention classification for one exact archive identity.
///
/// An identity unknown to the queried registry is retained fail-closed because
/// the user cache is content-addressed but not chain-scoped. Replication health
/// never makes a published archive prunable: locked consumers may still need a
/// cached below-quorum or unavailable archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiArchiveRetentionDispositionV1 {
    /// This registry does not know the archive, so it cannot authorize deletion.
    RetainUnknown,
    /// At least one governance-available active or yanked release references the archive.
    RetainReferenced,
    /// The registered archive has no published release references.
    PruneUnreferenced,
    /// Every published release reference has an enacted Parliament takedown.
    PruneGovernedTakedown,
}

impl MusubiArchiveRetentionDispositionV1 {
    /// Whether this finalized classification requires the local cache entry to remain.
    #[must_use]
    pub const fn must_retain(self) -> bool {
        matches!(self, Self::RetainUnknown | Self::RetainReferenced)
    }
}

/// One exact finalized cache-retention decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRetentionDecisionV1 {
    /// Exact content-addressed archive identity.
    pub archive_id: ArchiveId,
    /// Fail-closed retention or explicit prune classification.
    pub disposition: MusubiArchiveRetentionDispositionV1,
    /// Governance-available, non-yanked release references.
    pub active_releases: u16,
    /// Governance-available, yanked release references.
    pub yanked_releases: u16,
    /// Parliament-taken-down release references.
    pub taken_down_releases: u16,
    /// Authoritative storage projection, absent only for an unknown archive.
    pub storage: Option<MusubiArchiveAvailabilityV1>,
}

impl MusubiArchiveRetentionDecisionV1 {
    /// Return whether this exact finalized decision requires retention.
    #[must_use]
    pub const fn must_retain(&self) -> bool {
        self.disposition.must_retain()
    }

    /// Validate identity, bounded counts, storage binding, and disposition semantics.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero() {
            return Err(ParseError::new(
                "Musubi archive retention decision uses the zero archive identity",
            ));
        }
        let referenced = usize::from(self.active_releases)
            .checked_add(usize::from(self.yanked_releases))
            .and_then(|count| count.checked_add(usize::from(self.taken_down_releases)))
            .ok_or_else(|| ParseError::new("Musubi archive retention count overflow"))?;
        if referenced > MUSUBI_MAX_RESOLUTION_NODES_V1 {
            return Err(ParseError::new(
                "Musubi archive retention decision exceeds the release-reference bound",
            ));
        }
        if let Some(storage) = &self.storage {
            storage.validate()?;
            if storage.archive_id != self.archive_id {
                return Err(ParseError::new(
                    "Musubi archive retention storage projection has a different identity",
                ));
            }
        }

        let available = usize::from(self.active_releases)
            .checked_add(usize::from(self.yanked_releases))
            .expect("two u16 Musubi release counts fit usize");
        let canonical = match self.disposition {
            MusubiArchiveRetentionDispositionV1::RetainUnknown => {
                referenced == 0 && self.storage.is_none()
            }
            MusubiArchiveRetentionDispositionV1::RetainReferenced => {
                available > 0 && self.storage.is_some()
            }
            MusubiArchiveRetentionDispositionV1::PruneUnreferenced => {
                referenced == 0 && self.storage.is_some()
            }
            MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown => {
                available == 0 && self.taken_down_releases > 0 && self.storage.is_some()
            }
        };
        if !canonical {
            return Err(ParseError::new(
                "Musubi archive retention decision is internally inconsistent",
            ));
        }
        Ok(())
    }
}

/// Bounded exact finalized cache-retention request.
///
/// `expected_snapshot` is absent on the first batch and binds every later batch
/// in the same prune operation. A node must reject a mismatching anchor instead
/// of combining decisions from different finalized registry states.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRetentionQueryV1 {
    /// Sorted, distinct, non-zero exact archive identities.
    pub archive_ids: Vec<ArchiveId>,
    /// Exact finalized snapshot established by the first batch, when present.
    pub expected_snapshot: Option<MusubiRegistrySnapshotV1>,
}

impl MusubiArchiveRetentionQueryV1 {
    /// Validate the exact batch bound, order, identities, and optional snapshot.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_ids.is_empty()
            || self.archive_ids.len() > MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1
            || self.archive_ids.iter().any(ArchiveId::is_zero)
            || self.archive_ids.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ParseError::new(
                "Musubi archive retention batch is empty, oversized, or noncanonical",
            ));
        }
        self.expected_snapshot
            .as_ref()
            .map_or(Ok(()), MusubiRegistrySnapshotV1::validate)
    }
}

/// Exact finalized cache-retention decisions for one bounded request batch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiArchiveRetentionPageV1 {
    /// Deployment-selected chain identity queried for these decisions.
    pub chain_id: ChainId,
    /// Hash of the first finalized block for the queried registry.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Decisions in the exact order of the canonical request identities.
    pub items: Vec<MusubiArchiveRetentionDecisionV1>,
    /// Finalized universal registry snapshot shared by every decision.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Consensus-committed creation time of the block named by `snapshot`.
    ///
    /// This may be zero for bootstrap fixtures. A publication expiry proof requires it to be
    /// strictly later than the exact signed transaction and receipt validity window.
    pub finalized_time_ms: u64,
}

impl MusubiArchiveRetentionPageV1 {
    /// Validate deployment identity, bounded strict order, decisions, and snapshot.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        if self.chain_id.as_str().is_empty()
            || self.genesis_hash.iter().all(|byte| *byte == 0)
            || self.items.is_empty()
            || self.items.len() > MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].archive_id >= pair[1].archive_id)
            || self.items.iter().any(|decision| {
                decision.storage.is_some_and(|storage| {
                    storage.finalized_height > self.snapshot.finalized_height
                        || storage.index_revision > self.snapshot.index_revision
                        || (storage.finalized_height == self.snapshot.finalized_height
                            && storage.finalized_block_hash != self.snapshot.finalized_block_hash)
                })
            })
        {
            return Err(ParseError::new(
                "Musubi archive retention page has an invalid deployment or item bound",
            ));
        }
        self.items
            .iter()
            .try_for_each(MusubiArchiveRetentionDecisionV1::validate)
    }
}

impl MusubiArchiveLocationPageV1 {
    /// Validate deployment identity, archive commitment, items, snapshot, and cursor.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.archive.validate()?;
        self.snapshot.validate()?;
        if self.chain_id.as_str().is_empty()
            || self.genesis_hash.iter().all(|byte| *byte == 0)
            || self.archive.staging_receipt.payload.binding.chain_id != self.chain_id
            || self
                .archive
                .staging_receipt
                .payload
                .binding
                .genesis_block_hash
                != self.genesis_hash
            || self.archive.registered_at_height > self.snapshot.finalized_height
            || self.items.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].location_id >= pair[1].location_id)
        {
            return Err(ParseError::new(
                "Musubi archive-location page has an inconsistent deployment or item bound",
            ));
        }
        for location in &self.items {
            location.validate()?;
            if location.archive_id != self.archive.archive_id
                || self
                    .archive
                    .location_ids
                    .binary_search(&location.location_id)
                    .is_err()
                || location.finalized_height > self.snapshot.finalized_height
                || location.revision > self.archive.location_revision
                || location.state == MusubiArchiveLocationStateV1::Retired
            {
                return Err(ParseError::new(
                    "Musubi archive-location page item is not a current archive location",
                ));
            }
        }
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if cursor.snapshot != self.snapshot {
                return Err(ParseError::new(
                    "Musubi archive-location page cursor uses a different finalized snapshot",
                ));
            }
        }
        Ok(())
    }
}
/// Ordered page of permanent alias history bound to its exact request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiAliasHistoryPageV1 {
    /// Exact request whose results this page carries.
    pub query: MusubiAliasQueryV1,
    /// Ordered permanent history for `query.alias`.
    pub items: Vec<MusubiAliasHistoryEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiAliasHistoryPageV1 {
    /// Validate request identity, alias membership, bounds, order, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self
            .items
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
        {
            return Err(ParseError::new(
                "Musubi alias-history page is not strictly ordered",
            ));
        }
        for entry in &self.items {
            entry.validate()?;
            if entry.alias != self.query.alias {
                return Err(ParseError::new(
                    "Musubi alias-history page item belongs to a different alias",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor {
            let (alias, revision) = cursor
                .last_key
                .rsplit_once(':')
                .ok_or_else(|| ParseError::new("Musubi alias-history cursor key is invalid"))?;
            if revision.len() != 20 {
                return Err(ParseError::new(
                    "Musubi alias-history cursor key is invalid",
                ));
            }
            let revision = revision
                .parse::<u64>()
                .map_err(|_| ParseError::new("Musubi alias-history cursor key is invalid"))?;
            if alias != self.query.alias.as_str()
                || self.items.first().is_some_and(|entry| {
                    entry.key() <= MusubiAliasHistoryKeyV1::new(self.query.alias.clone(), revision)
                })
            {
                return Err(ParseError::new(
                    "Musubi alias-history page does not advance its structured cursor",
                ));
            }
        }
        let cursor_key =
            |entry: &MusubiAliasHistoryEntryV1| format!("{}:{:020}", entry.alias, entry.revision);
        let first_key = self.items.first().map(cursor_key);
        let last_key = self.items.last().map(cursor_key);
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            &self.next_cursor,
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    pub fn validate_for(&self, query: &MusubiAliasQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi alias-history page carries a different request context",
            ));
        }
        Ok(())
    }
}
/// Ordered page of universal resolver-index rows with authoritative lock identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverIndexPageV1 {
    /// Exact request whose rows this page carries.
    pub query: MusubiResolverIndexQueryV1,
    /// Deployment-selected chain identity used by generated lockfiles.
    pub chain_id: ChainId,
    /// Hash of the first finalized block used as the genesis identity.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Ordered universal resolver-index rows.
    pub items: Vec<MusubiResolverReleaseRowV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiResolverIndexPageV1 {
    /// Validate request identity, lock identity, page bounds, rows, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        if self.chain_id.as_str().is_empty()
            || self.genesis_hash.iter().all(|byte| *byte == 0)
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].release >= pair[1].release)
        {
            return Err(ParseError::new(
                "Musubi resolver page has an invalid chain identity or item bound",
            ));
        }
        for row in &self.items {
            row.validate()?;
            if row.release.package != self.query.package
                || self
                    .query
                    .requirement
                    .as_ref()
                    .is_some_and(|requirement| !requirement.matches(&row.release.version))
            {
                return Err(ParseError::new(
                    "Musubi resolver row does not match its response request context",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor {
            let previous = cursor
                .last_key
                .parse::<MusubiVersionV1>()
                .map_err(|_| ParseError::new("Musubi resolver cursor key is invalid"))?;
            if self
                .items
                .first()
                .is_some_and(|row| row.release.version <= previous)
            {
                return Err(ParseError::new(
                    "Musubi resolver page does not advance its structured cursor",
                ));
            }
        }
        self.snapshot.validate()?;
        let first_key = self
            .items
            .first()
            .map(|row| row.release.version.to_string());
        let last_key = self.items.last().map(|row| row.release.version.to_string());
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            &self.next_cursor,
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    pub fn validate_for(&self, query: &MusubiResolverIndexQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi resolver page carries a different request context",
            ));
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
    /// Exact request whose directory rows this page carries.
    pub query: MusubiOrderedPrefixQueryV1,
    /// Deployment-selected chain identity used by generated lockfiles.
    pub chain_id: ChainId,
    /// Hash of the first finalized block used as the genesis identity.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Authoritative immutable namespace binding, present even when no package matches.
    pub namespace_binding: MusubiNamespaceBindingV1,
    /// Ordered public-directory entries.
    pub items: Vec<MusubiOrderedPackageEntryV1>,
    /// Cursor for the next page, absent at the end.
    pub next_cursor: Option<MusubiFinalizedCursorV1>,
    /// Finalized snapshot shared by every item.
    pub snapshot: MusubiRegistrySnapshotV1,
}

impl MusubiOrderedPackagePageV1 {
    /// Validate request identity, lock identity, rows, bounds, and cursor binding.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.namespace_binding.validate()?;
        let (namespace, _) = self.query.prefix.components()?;
        if self.chain_id.as_str().is_empty()
            || self.genesis_hash.iter().all(|byte| *byte == 0)
            || namespace != self.namespace_binding.namespace
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].selector >= pair[1].selector)
        {
            return Err(ParseError::new(
                "Musubi directory page has an invalid chain identity or item bound",
            ));
        }
        for item in &self.items {
            item.validate()?;
            if item.selector.namespace != self.namespace_binding.namespace
                || item.package.home_dataspace != self.namespace_binding.home_dataspace
                || item.package.scope != self.namespace_binding.scope
                || item.package.name != item.selector.name
                || !item
                    .selector
                    .to_string()
                    .starts_with(self.query.prefix.as_str())
            {
                return Err(ParseError::new(
                    "Musubi directory page item disagrees with its request or namespace binding",
                ));
            }
        }
        if let Some(cursor) = &self.query.page.cursor {
            let previous = cursor
                .last_key
                .parse::<MusubiPackageSelectorV1>()
                .map_err(|_| ParseError::new("Musubi directory cursor key is invalid"))?;
            if previous.namespace != namespace
                || !previous.to_string().starts_with(self.query.prefix.as_str())
                || self
                    .items
                    .first()
                    .is_some_and(|item| item.selector <= previous)
            {
                return Err(ParseError::new(
                    "Musubi directory page does not advance its structured cursor",
                ));
            }
        }
        self.snapshot.validate()?;
        let first_key = self.items.first().map(|item| item.selector.to_string());
        let last_key = self.items.last().map(|item| item.selector.to_string());
        validate_finalized_response_page(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            &self.next_cursor,
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    pub fn validate_for(&self, query: &MusubiOrderedPrefixQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi directory page carries a different request context",
            ));
        }
        Ok(())
    }
}

include!("musubi/query_models.rs");

#[cfg(test)]
mod tests {
    include!("musubi_tests.rs");
}

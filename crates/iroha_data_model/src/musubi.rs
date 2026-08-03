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
    account::{AccountController, AccountId, MultisigMember, MultisigPolicy},
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
/// Maximum concatenated bundle payload, including source and three mandatory metadata entries.
pub const MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1: u64 = MUSUBI_MAX_CAR_BYTES_V1;
/// Maximum regular files committed by one archive.
pub const MUSUBI_MAX_FILES_V1: u32 = 4_096;
/// Maximum chunks committed by one archive.
pub const MUSUBI_MAX_CHUNKS_V1: u32 = 16_384;
const MUSUBI_MAX_PORTABLE_PATH_COMPONENTS_V1: usize = 64;
const MUSUBI_MAX_PORTABLE_PATH_COMPONENT_BYTES_V1: usize = 255;
const MUSUBI_MAX_PORTABLE_PATH_BYTES_V1: usize = 4 * 1024;
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
/// Maximum canonical Norito bytes for any account identity carried by Musubi V1.
pub const MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1: usize = 8 * 1024;
/// Maximum signatures carried by a namespace delegation approval set.
pub const MUSUBI_MAX_NAMESPACE_DELEGATION_APPROVALS_V1: usize = 64;
/// Maximum controller approvals on a publication staging receipt or provider attestation.
pub const MUSUBI_MAX_PUBLICATION_ATTESTATION_APPROVALS_V1: usize = 64;
/// Maximum detached-signature payload bytes accepted in any Musubi V1 approval.
pub const MUSUBI_MAX_APPROVAL_SIGNATURE_PAYLOAD_BYTES_V1: usize = 3_309;
/// Maximum canonical Norito bytes for one complete provider bundle attestation.
pub const MUSUBI_MAX_PROVIDER_BUNDLE_ATTESTATION_CANONICAL_BYTES_V1: usize = 1024 * 1024;
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
/// Maximum JSON bytes accepted for one public Musubi V1 query response.
pub const MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1: usize = 32 * 1024 * 1024;
/// Conservative JSON-array payload budget for resolver-index page items.
///
/// The remaining eight MiB covers the echoed request, deployment identity,
/// finalized snapshot, continuation cursor, and JSON object framing.
pub const MUSUBI_RESOLVER_PAGE_JSON_ITEMS_BUDGET_BYTES_V1: usize = 24 * 1024 * 1024;
/// Maximum exact archive identities in one authoritative cache-retention request.
pub const MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1: usize = MUSUBI_MAX_PAGE_SIZE_V1;
/// Maximum global alias length.
pub const MUSUBI_MAX_ALIAS_BYTES_V1: usize = 32;
/// Maximum decimal digits in one unsigned 64-bit semantic-version component.
pub const MUSUBI_MAX_U64_DECIMAL_DIGITS_V1: usize = 20;
/// Maximum canonical text bytes in one structured semantic version.
pub const MUSUBI_MAX_VERSION_CURSOR_KEY_BYTES_V1: usize = 3 * MUSUBI_MAX_U64_DECIMAL_DIGITS_V1
    + 2
    + 1
    + MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1 * MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1
    + (MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1 - 1);
/// Maximum canonical text bytes in one ordered package selector or prefix.
pub const MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1: usize =
    MUSUBI_MAX_NAMESPACE_BYTES_V1 + 1 + MUSUBI_MAX_PACKAGE_NAME_BYTES_V1;
/// Maximum text bytes in one archive-location cursor key.
pub const MUSUBI_MAX_ARCHIVE_LOCATION_CURSOR_KEY_BYTES_V1: usize = 64 + 1 + 64;
/// Maximum text bytes in one alias-history cursor key.
pub const MUSUBI_MAX_ALIAS_HISTORY_CURSOR_KEY_BYTES_V1: usize =
    MUSUBI_MAX_ALIAS_BYTES_V1 + 1 + MUSUBI_MAX_U64_DECIMAL_DIGITS_V1;
/// Conservative text-byte ceiling for one maintainer-directory cursor key.
///
/// Maintainer keys hex-encode the bare canonical account payload and append
/// either `accepted` or the longer `pending-` plus a 32-byte invite identity.
/// The shared account bound includes Norito framing, so applying its full value
/// to the bare payload deliberately leaves headroom rather than claiming an
/// attainable maximum cursor length.
pub const MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1: usize =
    2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 + 1 + "pending-".len() + 2 * 32;
/// Maximum UTF-8 byte length of any finalized query cursor key.
///
/// The maintainer-directory representation is the largest V1 producer. Tests
/// keep every other structured cursor-key family below this shared ceiling.
pub const MUSUBI_MAX_CURSOR_KEY_BYTES_V1: usize = MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1;
/// Maximum UTF-8 byte length accepted by rich package discovery.
pub const MUSUBI_MAX_SEARCH_QUERY_BYTES_V1: usize = 256;
/// Maximum distinct normalized terms accepted by rich package discovery.
pub const MUSUBI_MAX_SEARCH_QUERY_TERMS_V1: usize = 16;
/// Maximum UTF-8 byte length of one normalized discovery term.
pub const MUSUBI_MAX_SEARCH_TERM_BYTES_V1: usize = 64;

/// Validate a canonical Musubi source or complete bundle path set under the portable V1 policy.
///
/// The caller supplies path components rather than platform paths. Every component must already
/// be in the exact NFC representation used by Iroha [`Name`] values. The policy also excludes
/// traversal, portable reserved names and characters, bidirectional controls, file/directory
/// prefix conflicts, and Unicode case-fold collisions which would alias on a supported
/// case-insensitive filesystem. Ordering is deliberately not required: package commitments order
/// joined path bytes, while canonical `SoraFS` plans order structural component vectors.
/// The fixed count ceiling accommodates the 4,096 committed source files plus the three mandatory
/// bundle metadata entries.
///
/// # Errors
///
/// Returns an error when the set is empty or oversized, or any path is noncanonical, unsafe,
/// duplicated, prefix-conflicting, or case-fold-colliding.
pub fn validate_musubi_portable_path_set_v1<'a, I>(paths: I) -> Result<(), ParseError>
where
    I: IntoIterator<Item = &'a [String]>,
{
    let maximum_paths = usize::try_from(MUSUBI_MAX_FILES_V1)
        .unwrap_or(usize::MAX)
        .saturating_add(3);
    let mut canonical_paths = Vec::new();
    for components in paths {
        if components.is_empty()
            || components.len() > MUSUBI_MAX_PORTABLE_PATH_COMPONENTS_V1
            || canonical_paths.len() >= maximum_paths
        {
            return Err(musubi_portable_path_error());
        }
        let mut path_bytes = components.len().saturating_sub(1);
        for component in components {
            validate_musubi_portable_component_v1(component)?;
            path_bytes = path_bytes
                .checked_add(component.len())
                .ok_or_else(musubi_portable_path_error)?;
        }
        if path_bytes > MUSUBI_MAX_PORTABLE_PATH_BYTES_V1 {
            return Err(musubi_portable_path_error());
        }
        canonical_paths.push(components.join("/"));
    }
    if canonical_paths.is_empty() {
        return Err(musubi_portable_path_error());
    }

    canonical_paths.sort();
    if canonical_paths.windows(2).any(|pair| {
        pair[0] == pair[1]
            || (pair[1].starts_with(&pair[0])
                && pair[1].as_bytes().get(pair[0].len()) == Some(&b'/'))
    }) {
        return Err(musubi_portable_path_error());
    }

    let mut folded_paths = canonical_paths
        .iter()
        .map(|path| musubi_portable_collision_key_v1(path))
        .collect::<Vec<_>>();
    folded_paths.sort();
    if folded_paths.windows(2).any(|pair| {
        pair[0] == pair[1]
            || (pair[1].starts_with(&pair[0])
                && pair[1].as_bytes().get(pair[0].len()) == Some(&b'/'))
    }) {
        return Err(musubi_portable_path_error());
    }
    Ok(())
}

fn validate_musubi_portable_component_v1(component: &str) -> Result<(), ParseError> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.len() > MUSUBI_MAX_PORTABLE_PATH_COMPONENT_BYTES_V1
        || component.contains(['/', '\\', ':'])
        || component.chars().any(|character| {
            character.is_control()
                || musubi_path_is_bidi_control_v1(character)
                || matches!(character, '<' | '>' | '"' | '|' | '?' | '*')
        })
        || component.ends_with(['.', ' '])
        || musubi_path_is_reserved_component_v1(component)
        || normalize_musubi_portable_component_v1(component)? != component
    {
        return Err(musubi_portable_path_error());
    }
    Ok(())
}

fn normalize_musubi_portable_component_v1(component: &str) -> Result<String, ParseError> {
    let mut output = String::with_capacity(component.len());
    let mut segment = String::new();
    let flush = |segment: &mut String, output: &mut String| -> Result<(), ParseError> {
        if segment.is_empty() {
            return Ok(());
        }
        let normalized = Name::from_str(segment).map_err(|_| musubi_portable_path_error())?;
        output.push_str(normalized.as_ref());
        segment.clear();
        Ok(())
    };
    for character in component.chars() {
        if matches!(character, '@' | '#' | '$') || character.is_whitespace() {
            flush(&mut segment, &mut output)?;
            output.push(character);
        } else {
            segment.push(character);
        }
    }
    flush(&mut segment, &mut output)?;
    Ok(output)
}

fn musubi_portable_collision_key_v1(path: &str) -> String {
    path.chars()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .collect()
}

fn musubi_path_is_reserved_component_v1(component: &str) -> bool {
    let basename = component.split('.').next().unwrap_or(component);
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"]
        .iter()
        .any(|reserved| basename.eq_ignore_ascii_case(reserved))
    {
        return true;
    }
    if let (Some(prefix), Some(suffix)) = (basename.get(..3), basename.get(3..)) {
        let numbered = prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT");
        let reserved_digit = suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9');
        return numbered && (reserved_digit || matches!(suffix, "¹" | "²" | "³"));
    }
    false
}

const fn musubi_path_is_bidi_control_v1(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn musubi_portable_path_error() -> ParseError {
    ParseError::new("Musubi portable path set is invalid or noncanonical")
}

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
/// Domain used to sign an authenticated `SoraFS` seed-ingress receipt.
pub const MUSUBI_SEED_INGRESS_RECEIPT_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.musubi.seed-ingress-receipt.signature.v1";
/// Domain used when a provider attests that it parsed and verified a Musubi bundle.
pub const MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.musubi.provider-bundle-attestation.signature.v1";
/// Domain used to identify one complete canonical provider bundle attestation.
pub const MUSUBI_PROVIDER_BUNDLE_ATTESTATION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.musubi.provider-bundle-attestation.digest.v1";
/// Domain used to commit one sorted provider/digest attestation set.
pub const MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SET_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.musubi.provider-bundle-attestation-set.digest.v1";

fn validate_musubi_account_id_canonical_bytes_v1(encoded: &[u8]) -> Result<(), ParseError> {
    if encoded.is_empty() || encoded.len() > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 {
        return Err(ParseError::new(
            "Musubi account identity exceeds its canonical byte bound",
        ));
    }
    Ok(())
}

/// Validate the canonical Norito byte bound shared by all Musubi V1 account identities.
///
/// # Errors
///
/// Returns an error when canonical Norito encoding fails or exceeds the fixed V1 bound.
pub fn validate_musubi_account_id_v1(account_id: &AccountId) -> Result<(), ParseError> {
    let encoded = norito::to_bytes(account_id)
        .map_err(|_| ParseError::new("Musubi account identity has no canonical Norito encoding"))?;
    validate_musubi_account_id_canonical_bytes_v1(&encoded)
}

fn validate_musubi_approval_signature_v1<T>(
    public_key: &PublicKey,
    signature: &SignatureOf<T>,
) -> Result<(), ParseError> {
    let expected_payload_len = public_key
        .try_algorithm()
        .map_err(|_| ParseError::new("Musubi approval public key algorithm is invalid"))?
        .signature_payload_len();
    let actual_payload_len = signature.payload().len();
    if expected_payload_len > MUSUBI_MAX_APPROVAL_SIGNATURE_PAYLOAD_BYTES_V1
        || actual_payload_len > MUSUBI_MAX_APPROVAL_SIGNATURE_PAYLOAD_BYTES_V1
        || actual_payload_len != expected_payload_len
    {
        return Err(ParseError::new(
            "Musubi approval signature payload length is invalid",
        ));
    }
    Ok(())
}

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
    ///
    /// # Errors
    ///
    /// Returns an error if `raw` is empty, noncanonical, overlong, or does not contain one or two
    /// valid [`Name`] segments.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return canonical namespace text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate namespace text obtained through decoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the decoded namespace is not in canonical dataspace-root or
    /// domain-qualified form.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace is invalid, the generation is zero, or the textual
    /// namespace does not agree with its structural scope.
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
    ///
    /// # Errors
    ///
    /// Returns an error if this binding is invalid, the authoritative generation is zero, or the
    /// two generations differ.
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
    ///
    /// # Errors
    ///
    /// Returns an error if `raw` is empty, overlong, or not canonical lowercase ASCII kebab text.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Return the canonical name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate package-name text obtained through decoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the decoded name is not canonical lowercase ASCII kebab text.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the package-name component is not canonical.
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
    ///
    /// # Errors
    ///
    /// Returns an error if either the namespace or package-name component is not canonical.
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

/// One canonical `SemVer` prerelease identifier.
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
    ///
    /// # Errors
    ///
    /// Returns an error if an alphanumeric identifier is empty, overlong, numeric-only, or
    /// contains a character outside the `SemVer` prerelease alphabet.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the prerelease list exceeds the V1 bound or contains a noncanonical
    /// identifier.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the prerelease list exceeds the V1 bound or contains a noncanonical
    /// identifier.
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
    ///
    /// # Errors
    ///
    /// Returns an error if `raw` is empty, noncanonical, or contains an invalid version,
    /// comparator, or wildcard requirement.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        raw.parse()
    }

    /// Validate AST bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested version is invalid or the comparator set is empty,
    /// oversized, unsorted, duplicated, or contains contradictory exact versions.
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
    MusubiProviderBundleAttestationDigestV1,
    "Domain-separated digest of one complete provider bundle attestation."
);
digest_type!(
    MusubiProviderBundleAttestationSetDigestV1,
    "Domain-separated digest of one sorted provider bundle-attestation set."
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
    /// Canonical `SoraFS` root `CID`.
    pub root_cid: ManifestRootCid,
    /// Registered `SoraFS` chunker profile.
    pub chunker: ChunkerProfileHandle,
    /// Digest of the canonical ordered chunk plan.
    pub chunk_plan_digest: MusubiContentDigestV1,
    /// Proof-of-retrievability commitment root.
    pub por_root: MusubiContentDigestV1,
    /// Uncompressed canonical bundle payload length, including mandatory metadata entries.
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
    ///
    /// # Errors
    ///
    /// Returns an error if an archive size or count is outside its V1 bound, the chunker handle
    /// is overlong, or a required commitment digest is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.content_length == 0 || self.content_length > MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1 {
            return Err(ParseError::new(
                "Musubi archive bundle payload length is out of bounds",
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

    /// Compute the domain-separated `ArchiveId` from canonical Norito bytes.
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
    ///
    /// # Errors
    ///
    /// Returns an error if a required digest is zero or the selected source size or file count is
    /// outside its V1 bound.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the descriptor version is unsupported, a required digest is zero, or
    /// the selected source size or file count is outside its V1 bound.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the commitment, receipt, or registrant is invalid, or if the archive
    /// identity, receipt fields, and nonzero registration height do not agree.
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
    validate_musubi_account_id_v1(registered_by)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if immutable registration fields are inconsistent, the location revision
    /// is zero, or location identifiers are oversized, zero, unsorted, or duplicated.
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

/// Lifecycle of one renewable `SoraFS` archive location.
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

/// Fixed-size reverse-index value from one `SoraFS` pin manifest to one Musubi location.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the pin-manifest digest, archive identity, or location identity is
    /// zero.
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

/// Fixed-size reverse-index value from one `SoraFS` replication order to one Musubi location.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the replication-order digest, archive identity, or location identity
    /// is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the provider, archive, or location identity is the all-zero sentinel.
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

/// Renewable `SoraFS` pin and replication-order binding for an archive.
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
    /// `SoraFS` replication order.
    pub replication_order: ReplicationOrderId,
    /// Distinct providers whose completions were finalized.
    pub providers: Vec<ProviderId>,
    /// Digest of the sorted immutable provider-attestation set proving this location.
    pub provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1,
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
    ///
    /// # Errors
    ///
    /// Returns an error if an identity or commitment is zero, providers are empty, oversized,
    /// unsorted, or duplicated, renewal does not precede expiry, or a revision height is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.location_id.is_zero()
            || self.archive_id.is_zero()
            || digest_is_zero(self.pin_manifest.as_bytes())
            || self.providers.is_empty()
            || self.providers.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
            || self.provider_attestation_set_digest.is_zero()
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
    ///
    /// # Errors
    ///
    /// Returns an error if the archive or finalized anchor is inert, replica counts exceed their
    /// V1 capacity, or the availability class does not agree with those counts.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the archive identity is zero, the release list is oversized,
    /// unsorted, or duplicated, or a release identifier is invalid.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the package identity or structured version is invalid.
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
    ///
    /// # Errors
    ///
    /// Returns an error if `abi_hash` is the all-zero sentinel.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the ABI version is not V1 or the ABI hash is zero.
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
    /// Published `SemVer` range.
    pub requirement: MusubiVersionReqV1,
}

impl MusubiDependencyReqV1 {
    /// Validate structural identity and the canonical version requirement.
    ///
    /// # Errors
    ///
    /// Returns an error if the dependency package or version requirement is invalid.
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
            ///
            /// # Errors
            ///
            /// Returns an error if `raw` is empty, contains surrounding whitespace or control
            /// characters, or exceeds this text type's V1 byte bound.
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
            ///
            /// # Errors
            ///
            /// Returns an error if the decoded text is empty, noncanonical, or overlong.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the keyword is empty, overlong, or not lowercase ASCII kebab text.
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
    ///
    /// # Errors
    ///
    /// Returns an error if a metadata string or keyword is invalid, or if keywords are oversized,
    /// unsorted, or duplicated.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the finalized height or index revision is zero, or if the block hash
    /// is the all-zero sentinel.
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
    ///
    /// # Errors
    ///
    /// Returns an error if a nested identity or requirement is invalid, or if the exact selection
    /// belongs to another package or does not satisfy the published requirement.
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
    /// Sorted parent-local exact edges with unique parent-local aliases.
    pub dependencies: Vec<MusubiExactDependencyEdgeV1>,
}

impl MusubiVerificationNodeV1 {
    /// Validate node commitments, dependency bounds, and edge order.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested identity or ABI binding is invalid, a required commitment is
    /// zero, or dependencies are non-normal, oversized, unsorted, duplicated, or invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        if self.release_digest.is_zero()
            || self.archive_id.is_zero()
            || self.source_digest.is_zero()
            || self.interface_digest.is_zero()
            || self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            || self
                .dependencies
                .iter()
                .any(|dependency| dependency.kind != MusubiDependencyKindV1::Normal)
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
    /// Sorted exact selections with unique parent-local aliases for every direct normal dependency
    /// of the root.
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

    /// Validate schema, graph bounds, uniqueness, reachability, cycles, and depth.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema or root is invalid, graph collections are oversized or
    /// noncanonical, a root or node edge is not normal and exact, or the graph is incomplete,
    /// unreachable, cyclic, or too deep.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.root.validate()?;
        if self.schema != Self::SCHEMA
            || self.version != MUSUBI_REGISTRY_VERSION_V1
            || self.root_dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self
                .root_dependencies
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .root_dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            || self.nodes.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self
                .nodes
                .windows(2)
                .any(|pair| pair[0].release >= pair[1].release)
            || self.nodes.iter().any(|node| node.release == self.root)
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
        validate_exact_graph(&self.root_dependencies, &self.nodes)
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

fn validate_exact_graph(
    root_dependencies: &[MusubiExactDependencyEdgeV1],
    nodes: &[MusubiVerificationNodeV1],
) -> Result<(), ParseError> {
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
            visit(
                &edge.selected,
                depth.saturating_add(1),
                by_release,
                visiting,
                complete,
            )?;
        }
        visiting.remove(release);
        complete.insert(release);
        Ok(())
    }

    let by_release = nodes
        .iter()
        .map(|node| (&node.release, node))
        .collect::<BTreeMap<_, _>>();
    let mut complete = BTreeSet::new();
    let mut visiting = BTreeSet::new();

    for dependency in root_dependencies {
        visit(
            &dependency.selected,
            1,
            &by_release,
            &mut visiting,
            &mut complete,
        )?;
    }
    if complete.len() != nodes.len() {
        return Err(ParseError::new(
            "Musubi verification graph contains unreachable exact nodes",
        ));
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
    ///
    /// # Errors
    ///
    /// Returns an error if the registry snapshot or verification lock is invalid.
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
    /// Sorted normal dependency ranges with unique parent-local aliases.
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
    ///
    /// # Errors
    ///
    /// Returns an error if a nested release field is invalid, collections exceed V1 bounds or are
    /// noncanonical, a required digest is zero, or the release depends on its own package.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.abi.validate()?;
        self.metadata.validate()?;
        if self.dependencies.len() > MUSUBI_MAX_DEPENDENCIES_V1
            || self.exports.len() > MUSUBI_MAX_EXPORTS_V1
            || self.dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
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

    /// Validate this semantic release against its complete normalized verification lock.
    ///
    /// This is the shared bundle/publication boundary: both values must be independently valid,
    /// the lock must select this exact root and digest, and every published direct dependency must
    /// correspond one-for-one with a normal exact root edge carrying the same alias, package, and
    /// requirement.
    ///
    /// # Errors
    ///
    /// Returns an error if either value is invalid or their root, digest, direct-dependency count,
    /// dependency kind, alias, package, or requirement binding differs.
    pub fn validate_verification_lock(
        &self,
        verification_lock: &MusubiVerificationLockV1,
    ) -> Result<(), ParseError> {
        self.validate()?;
        verification_lock.validate()?;
        if verification_lock.root != self.release
            || verification_lock.digest() != self.verification_lock_digest
        {
            return Err(ParseError::new(
                "Musubi semantic release and verification lock do not bind the same root",
            ));
        }
        if self.dependencies.len() != verification_lock.root_dependencies.len() {
            return Err(ParseError::new(
                "Musubi semantic release and verification lock dependency counts differ",
            ));
        }
        for (requirement, exact) in self
            .dependencies
            .iter()
            .zip(&verification_lock.root_dependencies)
        {
            if exact.kind != MusubiDependencyKindV1::Normal
                || exact.alias != requirement.alias
                || exact.package != requirement.package
                || exact.requirement != requirement.requirement
            {
                return Err(ParseError::new(
                    "Musubi semantic release does not exactly bind a verification-lock dependency",
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
    /// Sorted normal dependency ranges with unique parent-local aliases.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the semantic manifest is invalid or the archive identity is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest or proof is invalid, the proof does not bind the release
    /// and lock digest, or its exact direct dependencies differ from the manifest.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.manifest.validate()?;
        self.resolution.validate()?;
        self.manifest
            .semantic_manifest()
            .validate_verification_lock(&self.resolution.lock)
    }
}

/// Exact, replay-resistant request binding accepted by authenticated `SoraFS` seed ingress.
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
    /// `SoraFS` provider selected by the ingress broker.
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
    ///
    /// # Errors
    ///
    /// Returns an error if an account identity is invalid, a required identity, digest, or nonce
    /// is zero, or the CAR body length is outside its V1 bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.publisher)?;
        validate_musubi_account_id_v1(&self.ingress_broker)?;
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

/// Canonical expiring statement signed by an authenticated `SoraFS` seed-ingress broker.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the request binding is invalid, the schema version is unsupported, or
    /// the issue and expiry times do not define a positive lifetime within the V1 bound.
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

/// Signed, expiring `SoraFS` seed-ingress receipt used by resumable publication.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is invalid, approvals are empty, oversized, unsorted, or
    /// duplicated, or an approval signature has an invalid payload length.
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
        self.approvals.iter().try_for_each(|approval| {
            validate_musubi_approval_signature_v1(&approval.public_key, &approval.signature)
        })
    }

    /// Verify the exact request binding, receipt validity window, and broker controller quorum.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the expected binding or validity window does not
    /// match, an approval is not a broker key, a signature fails, or controller quorum is absent.
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
    ///
    /// # Errors
    ///
    /// Returns an error if an account, provider authority, assignment, finalized anchor, archive,
    /// replication order, or required bundle commitment is invalid or inert.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.completed_by)?;
        validate_musubi_account_id_v1(&self.completion_authority.provider_owner)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if the attestation version is unsupported or its exact provider binding
    /// is invalid.
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
    ///
    /// # Errors
    ///
    /// Returns an error if canonical encoding fails or is oversized, the payload is invalid,
    /// approvals are empty or noncanonical, or an approval signature length is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        let canonical = norito::to_bytes(self).map_err(|_| {
            ParseError::new("Musubi provider bundle attestation has no canonical Norito encoding")
        })?;
        if canonical.is_empty()
            || canonical.len() > MUSUBI_MAX_PROVIDER_BUNDLE_ATTESTATION_CANONICAL_BYTES_V1
        {
            return Err(ParseError::new(
                "Musubi provider bundle attestation exceeds its canonical byte bound",
            ));
        }
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
        self.approvals.iter().try_for_each(|approval| {
            validate_musubi_approval_signature_v1(&approval.public_key, &approval.signature)
        })
    }

    /// Return the deterministic immutable storage identity selected by the signed binding.
    #[must_use]
    pub const fn key(&self) -> MusubiProviderBundleAttestationKeyV1 {
        MusubiProviderBundleAttestationKeyV1 {
            archive_id: self.payload.binding.archive_id,
            replication_order: self.payload.binding.replication_order,
            provider_id: self.payload.binding.provider_id,
        }
    }

    /// Compute the domain-separated digest of the complete canonical attestation.
    #[must_use]
    pub fn digest(&self) -> MusubiProviderBundleAttestationDigestV1 {
        MusubiProviderBundleAttestationDigestV1(domain_hash(
            MUSUBI_PROVIDER_BUNDLE_ATTESTATION_DIGEST_DOMAIN_V1,
            &self.encode(),
        ))
    }

    /// Return the compact provider/digest reference used by an archive-location set commitment.
    #[must_use]
    pub fn reference(&self) -> MusubiProviderBundleAttestationRefV1 {
        MusubiProviderBundleAttestationRefV1 {
            provider_id: self.payload.binding.provider_id,
            digest: self.digest(),
        }
    }

    /// Verify the exact finalized completion binding and provider-owner controller quorum.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the expected binding differs, an approval is not a
    /// provider-owner key, a signature fails, or the provider-owner threshold is not met.
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

/// Deterministic immutable identity of one provider's proof for an archive replication order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleAttestationKeyV1 {
    /// Exact archive whose canonical bundle was verified.
    pub archive_id: ArchiveId,
    /// Exact replication order completed by the provider.
    pub replication_order: ReplicationOrderId,
    /// Provider that completed and attested to the verification.
    pub provider_id: ProviderId,
}

impl MusubiProviderBundleAttestationKeyV1 {
    /// Validate every immutable identity component.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive, replication order, or provider identity is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.archive_id.is_zero()
            || digest_is_zero(self.replication_order.as_bytes())
            || self.provider_id.as_bytes().iter().all(|byte| *byte == 0)
        {
            return Err(ParseError::new(
                "Musubi provider bundle attestation key is invalid",
            ));
        }
        Ok(())
    }
}

/// Compact immutable provider-attestation reference used by an archive-location set commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleAttestationRefV1 {
    /// Provider covered by the referenced proof.
    pub provider_id: ProviderId,
    /// Digest of the complete canonical provider attestation.
    pub digest: MusubiProviderBundleAttestationDigestV1,
}

impl MusubiProviderBundleAttestationRefV1 {
    /// Validate the compact provider and digest binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider identity or attestation digest is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.provider_id.as_bytes().iter().all(|byte| *byte == 0) || self.digest.is_zero() {
            return Err(ParseError::new(
                "Musubi provider bundle attestation reference is invalid",
            ));
        }
        Ok(())
    }
}

#[derive(Encode)]
struct MusubiProviderBundleAttestationSetPreimageV1 {
    archive_id: ArchiveId,
    replication_order: ReplicationOrderId,
    references: Vec<MusubiProviderBundleAttestationRefV1>,
}

/// Derive the aggregate digest of an archive/order-bound, provider-sorted attestation set.
///
/// # Errors
///
/// Returns an error for invalid archive/order identities or an empty, oversized, duplicate,
/// unsorted, or invalid reference set.
pub fn musubi_provider_bundle_attestation_set_digest_v1(
    archive_id: ArchiveId,
    replication_order: ReplicationOrderId,
    references: &[MusubiProviderBundleAttestationRefV1],
) -> Result<MusubiProviderBundleAttestationSetDigestV1, ParseError> {
    if archive_id.is_zero()
        || digest_is_zero(replication_order.as_bytes())
        || references.is_empty()
        || references.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
        || references
            .windows(2)
            .any(|pair| pair[0].provider_id >= pair[1].provider_id)
    {
        return Err(ParseError::new(
            "Musubi provider bundle attestation set is invalid or noncanonical",
        ));
    }
    references
        .iter()
        .try_for_each(MusubiProviderBundleAttestationRefV1::validate)?;
    let preimage = MusubiProviderBundleAttestationSetPreimageV1 {
        archive_id,
        replication_order,
        references: references.to_vec(),
    };
    Ok(MusubiProviderBundleAttestationSetDigestV1(domain_hash(
        MUSUBI_PROVIDER_BUNDLE_ATTESTATION_SET_DIGEST_DOMAIN_V1,
        &preimage.encode(),
    )))
}

/// Immutable full provider-attestation registry record addressed by its exact binding.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiProviderBundleAttestationRecordV1 {
    /// Deterministic archive/order/provider identity.
    pub key: MusubiProviderBundleAttestationKeyV1,
    /// Domain-separated digest of `attestation`.
    pub attestation_digest: MusubiProviderBundleAttestationDigestV1,
    /// Complete signed provider proof.
    pub attestation: MusubiProviderBundleVerificationAttestationV1,
    /// Archive manager that registered the immutable proof.
    pub registered_by: AccountId,
    /// Finalized height at which the immutable proof was registered.
    pub registered_at_height: u64,
}

impl MusubiProviderBundleAttestationRecordV1 {
    /// Validate the full proof and every redundant immutable identity binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the key, attestation, or registering account is invalid, or if the
    /// stored key, digest, and nonzero registration height do not bind that attestation exactly.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.key.validate()?;
        self.attestation.validate()?;
        validate_musubi_account_id_v1(&self.registered_by)?;
        if self.key != self.attestation.key()
            || self.attestation_digest.is_zero()
            || self.attestation_digest != self.attestation.digest()
            || self.registered_at_height == 0
        {
            return Err(ParseError::new(
                "Musubi provider bundle attestation record is inconsistent",
            ));
        }
        Ok(())
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
    ///
    /// # Errors
    ///
    /// Returns an error if an account identity is invalid, the version is unsupported, or the
    /// namespace binding, owner generation, or expiry height is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.owner)?;
        validate_musubi_account_id_v1(&self.delegate)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is invalid, approvals are empty, oversized, unsorted, or
    /// duplicated, or an approval signature has an invalid payload length.
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
        self.approvals.iter().try_for_each(|approval| {
            validate_musubi_approval_signature_v1(&approval.public_key, &approval.signature)
        })
    }

    /// Verify a delegation against current authoritative ownership and the claiming account.
    ///
    /// The authoritative owner and generation must come from the live SNS dataspace record or
    /// domain record selected by the immutable namespace binding. Single-key and weighted
    /// multisignature account controllers are both enforced exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, current authority or expiry does not match, an
    /// approval is not an owner key, a signature fails, or the owner threshold is not met.
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
    ///
    /// # Errors
    ///
    /// Returns an error if any package revision is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if identity, namespace, binding, revisions, claim height, owner/member
    /// bounds, ordering, membership, or account identities violate package invariants.
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
        self.owners
            .iter()
            .chain(&self.member_accounts)
            .try_for_each(validate_musubi_account_id_v1)
    }
}

/// Independent permissions granted to an accepted package maintainer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[allow(
    clippy::struct_excessive_bools,
    reason = "four independent permission bits are the canonical Musubi V1 wire shape"
)]
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

    /// Validate the structural package and bounded account identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the package or account identity is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        validate_musubi_account_id_v1(&self.account)
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
    ///
    /// # Errors
    ///
    /// Returns an error if the package or account identity is invalid, an acceptance anchor is
    /// zero, or a maintainer role grants no permissions.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        validate_musubi_account_id_v1(&self.account)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if a package or account identity is invalid, the invitation or revision
    /// anchor is zero, or a maintainer role grants no permissions.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        validate_musubi_account_id_v1(&self.invited_by)?;
        validate_musubi_account_id_v1(&self.invited_account)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if the package is invalid, the account is absent or invalid, or a present
    /// invitation identity is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        let Some(account) = &self.account else {
            return Err(ParseError::new(
                "Musubi maintainer directory invitation identity is invalid",
            ));
        };
        validate_musubi_account_id_v1(account)?;
        if self
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
        maintainer_cursor_key_label_v1(&account.encode(), key.invitation.as_ref())
    }

    /// Validate the record and require invitations to remain pending.
    ///
    /// # Errors
    ///
    /// Returns an error if the member or invitation is invalid, or if a directory invitation is
    /// no longer pending.
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

fn maintainer_cursor_key_label_v1(
    encoded_account: &[u8],
    invitation: Option<&MusubiInviteIdV1>,
) -> String {
    let suffix_len = invitation.map_or("accepted".len(), |_| "pending-".len() + 64);
    let mut label = String::with_capacity(
        encoded_account
            .len()
            .saturating_mul(2)
            .saturating_add(1 + suffix_len),
    );
    for byte in encoded_account {
        fmt::Write::write_fmt(&mut label, format_args!("{byte:02x}"))
            .expect("writing into a String cannot fail");
    }
    label.push('|');
    match invitation {
        None => label.push_str("accepted"),
        Some(invite_id) => {
            label.push_str("pending-");
            for byte in invite_id.as_bytes() {
                fmt::Write::write_fmt(&mut label, format_args!("{byte:02x}"))
                    .expect("writing into a String cannot fail");
            }
        }
    }
    label
}

fn maintainer_cursor_key_is_canonical_v1(raw: &str) -> bool {
    let Some((account, suffix)) = raw.split_once('|') else {
        return false;
    };
    let is_lower_hex = |text: &str| {
        !text.is_empty()
            && text.len().is_multiple_of(2)
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if account.len() > 2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 || !is_lower_hex(account) {
        return false;
    }
    let mut encoded_account = Vec::with_capacity(account.len() / 2);
    for pair in account.as_bytes().chunks_exact(2) {
        let nibble = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("lowercase hexadecimal was checked above"),
        };
        encoded_account.push((nibble(pair[0]) << 4) | nibble(pair[1]));
    }
    let Ok(account_id) = norito::codec::decode_exact_from_slice::<AccountId>(&encoded_account)
    else {
        return false;
    };
    if let AccountController::Multisig(policy) = account_id.controller() {
        let Ok(members) = policy
            .members()
            .iter()
            .map(|member| MultisigMember::new(member.public_key().clone(), member.weight()))
            .collect::<Result<Vec<_>, _>>()
        else {
            return false;
        };
        let Ok(normalized) =
            MultisigPolicy::from_serialized(policy.version(), policy.threshold(), members)
        else {
            return false;
        };
        if &normalized != policy {
            return false;
        }
    }
    if validate_musubi_account_id_v1(&account_id).is_err() || account_id.encode() != encoded_account
    {
        return false;
    }
    if suffix == "accepted" {
        return true;
    }
    let Some(invitation) = suffix.strip_prefix("pending-") else {
        return false;
    };
    invitation.len() == 64
        && is_lower_hex(invitation)
        && invitation.bytes().any(|byte| byte != b'0')
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
    ///
    /// # Errors
    ///
    /// Returns an error if the package, metadata, or changing account is invalid, or if the
    /// revision or finalized height is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.metadata.validate()?;
        validate_musubi_account_id_v1(&self.changed_by)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if the release, reason, or changing account is invalid, or if the
    /// transition height or revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.release.validate()?;
        self.reason.validate()?;
        validate_musubi_account_id_v1(&self.changed_by)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if a takedown reason is invalid, its action digest is zero, or its applied
    /// height is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the yank, storage-availability, or governance projection is invalid.
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
    ///
    /// # Errors
    ///
    /// Returns an error if either mutable release revision is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if any nested projection or publisher is invalid, or if manifest,
    /// digest, release, revision, and publication-height bindings are inconsistent.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.manifest.validate()?;
        self.yank.validate()?;
        self.artifact_governance.validate()?;
        self.revisions.validate()?;
        validate_musubi_account_id_v1(&self.published_by)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if the alias is empty, overlong, or not lowercase ASCII kebab text.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the policy revision or any alias price is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if an identity or policy is invalid, paid pricing does not match the
    /// policy, or the registration height or history revision is zero.
    pub fn validate(&self, policy: &MusubiAliasPricingPolicyV1) -> Result<(), ParseError> {
        self.alias.validate()?;
        self.target.validate()?;
        policy.validate()?;
        validate_musubi_account_id_v1(&self.registered_by)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if an alias or package target is invalid, action-specific history fields
    /// are inconsistent, or the finalized height is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if a decision identity or action digest is zero, enactment is zero, or
    /// execution is not delayed beyond enactment.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the decision is invalid, delay addition overflows, the enacted delay
    /// is shorter than required, or consumption precedes the execution boundary.
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

impl MusubiRecoverPackageOwnersV1 {
    /// Validate the replacement owner set and its compare-and-set revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the package or an owner is invalid, owners are empty, oversized,
    /// unsorted, or duplicated, or the expected revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        if self.owners.is_empty()
            || self.owners.len() > MUSUBI_MAX_PACKAGE_OWNERS_V1
            || self.owners.windows(2).any(|pair| pair[0] >= pair[1])
            || self.expected_revision == 0
        {
            return Err(ParseError::new(
                "Musubi Parliament owner recovery is invalid",
            ));
        }
        self.owners
            .iter()
            .try_for_each(validate_musubi_account_id_v1)
    }
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
    ///
    /// # Errors
    ///
    /// Returns an error if an action payload is invalid, a compare-and-set revision is zero or
    /// inconsistent, or a policy replacement is not the expected successor.
    pub fn validate(&self) -> Result<(), ParseError> {
        match self {
            Self::RecoverPackageOwners(recovery) => {
                recovery.validate()?;
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
    ///
    /// # Errors
    ///
    /// Returns an error if pricing is invalid, the version or revision is invalid, the allowlist
    /// is oversized or noncanonical, or a non-allowlisted mode carries entries.
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
    ///
    /// # Errors
    ///
    /// Returns an error if either policy is invalid, revision arithmetic overflows, the policy is
    /// not the exact successor, or pricing changes do not use the required pricing revision.
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
    /// Sorted normal dependency ranges with unique parent-local aliases.
    pub dependencies: Vec<MusubiDependencyReqV1>,
    /// Independent selection state.
    pub selection: MusubiReleaseSelectionStateV1,
    /// Universal index revision.
    pub index_revision: u64,
}

impl MusubiResolverReleaseRowV1 {
    /// Validate compact resolver commitments and canonical dependency order.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested projection is invalid, commitments or revision are zero,
    /// dependencies are oversized or noncanonical, selection identities do not match the row, or
    /// the availability projection is newer than the resolver row.
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
            || self
                .dependencies
                .windows(2)
                .any(|pair| pair[0].alias >= pair[1].alias)
            || self.selection.yank.release != self.release
            || self.selection.storage.archive_id != self.archive_id
            || self.selection.storage.index_revision > self.index_revision
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

/// Paired home-dataspace and universal-index view of one exact release at finality.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiExactReleaseSnapshotV1 {
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Hash of the first finalized block for the selected chain.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub genesis_hash: [u8; 32],
    /// Finalized universal registry snapshot shared by both projections.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Authoritative release record from the stable home dataspace.
    pub home_release: MusubiReleaseRecordV1,
    /// Exact resolver-grade release row from the universal dataspace.
    pub universal_release: MusubiResolverReleaseRowV1,
}

impl MusubiExactReleaseSnapshotV1 {
    /// Validate deployment identity, paired content/state, revisions, and finalized anchors.
    ///
    /// # Errors
    ///
    /// Returns an error if a nested projection or deployment identity is invalid, the home and
    /// universal views disagree, or a revision, transition, or storage anchor is not finalized.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        self.home_release.validate()?;
        self.universal_release.validate()?;

        let manifest = &self.home_release.manifest;
        let universal = &self.universal_release;
        let storage = &universal.selection.storage;
        let takedown_height = match &self.home_release.artifact_governance {
            MusubiArtifactGovernanceStateV1::Available => 0,
            MusubiArtifactGovernanceStateV1::TakenDown(takedown) => takedown.applied_at_height,
        };
        if self.chain_id.as_str().is_empty()
            || digest_is_zero(&self.genesis_hash)
            || manifest.release != universal.release
            || self.home_release.release_digest != universal.release_digest
            || manifest.archive_id != universal.archive_id
            || manifest.interface_digest != universal.interface_digest
            || manifest.abi != universal.abi
            || manifest.dependencies != universal.dependencies
            || self.home_release.yank != universal.selection.yank
            || self.home_release.artifact_governance != universal.selection.governance
            || self.home_release.revisions.yank > self.snapshot.index_revision
            || self.home_release.revisions.artifact_governance > self.snapshot.index_revision
            || universal.index_revision > self.snapshot.index_revision
            || storage.index_revision > universal.index_revision
            || storage.index_revision > self.snapshot.index_revision
            || self.home_release.published_at_height > self.snapshot.finalized_height
            || self.home_release.yank.changed_at_height < self.home_release.published_at_height
            || self.home_release.yank.changed_at_height > self.snapshot.finalized_height
            || (takedown_height != 0 && takedown_height < self.home_release.published_at_height)
            || takedown_height > self.snapshot.finalized_height
            || storage.finalized_height > self.snapshot.finalized_height
            || (self.snapshot.finalized_height == 1
                && self.genesis_hash != self.snapshot.finalized_block_hash)
            || (storage.finalized_height == self.snapshot.finalized_height
                && storage.finalized_block_hash != self.snapshot.finalized_block_hash)
        {
            return Err(ParseError::new(
                "Musubi exact release snapshot is inconsistent or not finalized",
            ));
        }
        Ok(())
    }

    /// Validate this paired result for one exact requested release.
    ///
    /// # Errors
    ///
    /// Returns an error if the query release or snapshot is invalid, or either paired projection
    /// carries a different release.
    pub fn validate_for(&self, query: &MusubiExactReleaseQueryV1) -> Result<(), ParseError> {
        query.release.validate()?;
        self.validate()?;
        if self.home_release.manifest.release != query.release
            || self.universal_release.release != query.release
        {
            return Err(ParseError::new(
                "Musubi exact release snapshot carries a different release",
            ));
        }
        Ok(())
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
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot or caller is invalid, the query hash is zero, or the last
    /// key is empty, overlong, or contains control characters.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        if let Some(caller) = &self.caller {
            validate_musubi_account_id_v1(caller)?;
        }
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
            ///
            /// # Errors
            ///
            /// Returns an error if the snapshot or an item is invalid, items exceed the page
            /// bound or are noncanonical, or the next cursor is invalid or changes snapshots.
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
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, version, or cursor data is invalid, versions are not
    /// strictly ordered, the page does not advance its cursor, or page bounds do not match.
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
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
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
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, entry, or cursor data is invalid, entries are not
    /// strictly ordered or belong to another package, or page bounds do not match.
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
        if let Some(cursor) = &self.query.page.cursor
            && (!maintainer_cursor_key_is_canonical_v1(&cursor.last_key)
                || self
                    .items
                    .iter()
                    .any(|entry| entry.cursor_key() == cursor.last_key))
        {
            return Err(ParseError::new(
                "Musubi maintainer page does not advance its exact cursor boundary",
            ));
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
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
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
    ///
    /// # Errors
    ///
    /// Returns an error if the archive is zero, release counts overflow or exceed V1 bounds, a
    /// storage projection is invalid or mismatched, or the disposition contradicts the counts.
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
    ///
    /// # Errors
    ///
    /// Returns an error if archive identities are empty, oversized, zero, unsorted, or duplicated,
    /// or if the expected snapshot is invalid.
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
    ///
    /// # Errors
    ///
    /// Returns an error if deployment or snapshot data is invalid, decisions are empty,
    /// oversized, noncanonical, or invalid, or storage anchors exceed the page snapshot.
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
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, archive, snapshot, location, or cursor data is invalid,
    /// locations are oversized or noncanonical, or an item is not current at the snapshot.
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
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, entry, or cursor data is invalid, entries are not
    /// strictly ordered or belong to another alias, or page bounds do not match.
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
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
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
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, query, snapshot, row, or cursor data is invalid, rows are
    /// noncanonical or outside the requested package/range, or page bounds do not match.
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
        validate_finalized_response_page_with_cursor_cardinality(
            &self.query.page,
            self.items.len(),
            first_key.as_deref(),
            last_key.as_deref(),
            self.next_cursor.as_ref(),
            self.snapshot,
            false,
        )?;
        #[cfg(feature = "json")]
        {
            let encoded = norito::json::to_json(self).map_err(|_| {
                ParseError::new("Musubi resolver page cannot be encoded as canonical JSON")
            })?;
            if encoded.len() > MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1 {
                return Err(ParseError::new(
                    "Musubi resolver page exceeds the public JSON response ceiling",
                ));
            }
        }
        Ok(())
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
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
    ///
    /// # Errors
    ///
    /// Returns an error if selector, package, or optional version data is invalid, or if either
    /// directory revision is zero.
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
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, query, binding, snapshot, item, or cursor data is invalid,
    /// items are noncanonical or inconsistent with the prefix/binding, or page bounds do not match.
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
            self.next_cursor.as_ref(),
            self.snapshot,
        )
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
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
    /// Parse a canonical `namespace/package-prefix` directory prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if `raw` is empty, noncanonical, overlong, lacks its separator, contains
    /// an invalid namespace, or has a nonportable package-name prefix.
    pub fn new(raw: &str) -> Result<Self, ParseError> {
        parse_clean(
            raw,
            "Musubi ordered prefix must not be empty",
            "Musubi ordered prefix is invalid",
        )?;
        if raw.len() > MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1 {
            return Err(ParseError::new("Musubi ordered prefix exceeds its bound"));
        }
        let prefix = Self(raw.to_owned());
        prefix.components()?;
        Ok(prefix)
    }

    /// Return prefix text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the structural namespace and portable package-name prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if the prefix lacks its separator, contains an invalid namespace, or has
    /// an overlong or nonportable package-name component.
    pub fn components(&self) -> Result<(MusubiNamespaceV1, &str), ParseError> {
        let (namespace, name_prefix) = self.0.split_once('/').ok_or_else(|| {
            ParseError::new("Musubi ordered prefix must use namespace/package-prefix")
        })?;
        if name_prefix.contains('/')
            || name_prefix.len() > MUSUBI_MAX_PACKAGE_NAME_BYTES_V1
            || name_prefix.starts_with('-')
            || name_prefix.contains("--")
            || !name_prefix
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ParseError::new(
                "Musubi ordered package prefix is not portable canonical text",
            ));
        }
        Ok((namespace.parse()?, name_prefix))
    }

    /// Validate prefix text obtained through decoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the decoded prefix is empty, noncanonical, overlong, or structurally
    /// invalid.
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

    /// Validate the requested bound and any supplied cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if a nonzero limit exceeds the V1 page maximum or the cursor is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.limit != 0
            && usize::try_from(self.limit).map_or(true, |limit| limit > MUSUBI_MAX_PAGE_SIZE_V1)
        {
            return Err(ParseError::new(
                "Musubi query page limit exceeds the consensus maximum",
            ));
        }
        self.cursor
            .as_ref()
            .map_or(Ok(()), MusubiFinalizedCursorV1::validate)
    }
}

fn validate_finalized_response_page(
    request: &MusubiPageRequestV1,
    item_count: usize,
    first_key: Option<&str>,
    last_key: Option<&str>,
    next_cursor: Option<&MusubiFinalizedCursorV1>,
    snapshot: MusubiRegistrySnapshotV1,
) -> Result<(), ParseError> {
    validate_finalized_response_page_with_cursor_cardinality(
        request,
        item_count,
        first_key,
        last_key,
        next_cursor,
        snapshot,
        true,
    )
}

fn validate_finalized_response_page_with_cursor_cardinality(
    request: &MusubiPageRequestV1,
    item_count: usize,
    first_key: Option<&str>,
    last_key: Option<&str>,
    next_cursor: Option<&MusubiFinalizedCursorV1>,
    snapshot: MusubiRegistrySnapshotV1,
    next_cursor_requires_full_page: bool,
) -> Result<(), ParseError> {
    request.validate()?;
    if item_count > request.effective_limit()
        || (item_count == 0 && (first_key.is_some() || last_key.is_some()))
        || (item_count > 0 && (first_key.is_none() || last_key.is_none()))
    {
        return Err(ParseError::new(
            "Musubi response page exceeds its requested bound or has invalid keys",
        ));
    }
    if let Some(cursor) = &request.cursor
        && (cursor.snapshot != snapshot || cursor.caller.is_some())
    {
        return Err(ParseError::new(
            "Musubi response page does not continue its request cursor",
        ));
    }
    if let Some(cursor) = next_cursor {
        cursor.validate()?;
        if cursor.snapshot != snapshot
            || cursor.caller.is_some()
            || (next_cursor_requires_full_page && item_count != request.effective_limit())
            || Some(cursor.last_key.as_str()) != last_key
            || request
                .cursor
                .as_ref()
                .is_some_and(|previous| previous.query_hash != cursor.query_hash)
        {
            return Err(ParseError::new(
                "Musubi response next cursor does not bind its exact response page",
            ));
        }
    }
    Ok(())
}

/// Resolver-index range request; exact resolution never uses fuzzy search.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiResolverIndexQueryV1 {
    /// Stable package identity.
    pub package: MusubiPackageIdV1,
    /// Optional `SemVer` filtering requirement.
    pub requirement: Option<MusubiVersionReqV1>,
    /// Page controls and finalized cursor.
    pub page: MusubiPageRequestV1,
}

impl MusubiResolverIndexQueryV1 {
    /// Validate structural package, optional requirement, and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the package, optional version requirement, or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.requirement
            .as_ref()
            .map_or(Ok(()), MusubiVersionReqV1::validate)?;
        self.page.validate()
    }
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

impl MusubiPackagePageQueryV1 {
    /// Validate structural package identity and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the package identity or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.page.validate()
    }
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

impl MusubiAliasQueryV1 {
    /// Validate permanent alias identity and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the alias or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.alias.validate()?;
        self.page.validate()
    }
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

impl MusubiOrderedPrefixQueryV1 {
    /// Validate canonical structural prefix and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if the ordered prefix or page controls are invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.prefix.validate()?;
        self.page.validate()
    }
}

/// Snapshot of the process-local finalized-event package-search projection.
///
/// This anchor is deliberately distinct from [`MusubiRegistrySnapshotV1`]. Search
/// projection revisions are not resolver-index revisions and must never be used to
/// select a dependency release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchSnapshotV1 {
    /// Finalized height through which the search projection has been applied.
    pub finalized_height: u64,
    /// Finalized block hash at `finalized_height`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub finalized_block_hash: [u8; 32],
    /// Process-local projection revision, changed on every visible rebuild/update.
    pub projection_revision: u64,
}

impl MusubiSearchSnapshotV1 {
    /// Validate a non-inert finalized search anchor.
    ///
    /// # Errors
    ///
    /// Returns an error if the finalized height, block hash, or projection revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.finalized_height == 0
            || digest_is_zero(&self.finalized_block_hash)
            || self.projection_revision == 0
        {
            return Err(ParseError::new("Musubi search snapshot is invalid"));
        }
        Ok(())
    }
}

/// Continuation cursor for the rebuildable package-search projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchCursorV1 {
    /// Exact finalized search projection used by the preceding page.
    pub snapshot: MusubiSearchSnapshotV1,
    /// Domain-separated hash of canonical search parameters excluding this cursor.
    pub query_hash: MusubiQueryHashV1,
    /// Last structural package returned by the preceding page.
    pub last_package: MusubiPackageIdV1,
}

impl MusubiSearchCursorV1 {
    /// Validate every cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot or last package is invalid, or the query hash is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.snapshot.validate()?;
        self.last_package.validate()?;
        if self.query_hash.is_zero() {
            return Err(ParseError::new(
                "Musubi search cursor query hash is invalid",
            ));
        }
        Ok(())
    }
}

/// Page controls for rich package discovery.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchPageRequestV1 {
    /// Requested count; zero selects [`MUSUBI_DEFAULT_PAGE_SIZE_V1`].
    pub limit: u32,
    /// Continuation cursor returned by the same normalized search.
    pub cursor: Option<MusubiSearchCursorV1>,
}

impl MusubiSearchPageRequestV1 {
    /// Effective page size capped by the public V1 maximum.
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

    /// Validate the page bound and continuation cursor.
    ///
    /// # Errors
    ///
    /// Returns an error if a nonzero limit exceeds the V1 page maximum or the cursor is invalid.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.limit != 0
            && usize::try_from(self.limit).map_or(true, |limit| limit > MUSUBI_MAX_PAGE_SIZE_V1)
        {
            return Err(ParseError::new(
                "Musubi search page limit exceeds the public V1 maximum",
            ));
        }
        self.cursor
            .as_ref()
            .map_or(Ok(()), MusubiSearchCursorV1::validate)
    }
}

/// Bounded exact-token query for the rebuildable package discovery projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchQueryV1 {
    /// Description, keyword, namespace, or package-name terms joined by whitespace.
    pub query: String,
    /// Search-specific page controls and cursor.
    pub page: MusubiSearchPageRequestV1,
}

impl MusubiSearchQueryV1 {
    /// Return sorted, distinct, Unicode-lowercased exact search terms.
    ///
    /// Hyphenated ASCII components contribute both their complete spelling and
    /// their alphanumeric words. No prefix, edit-distance, or fuzzy expansion is
    /// performed.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is empty, noncanonical, or overlong, or if normalization
    /// yields no terms, an overlong term, or more terms than the V1 bound.
    pub fn normalized_terms(&self) -> Result<Vec<String>, ParseError> {
        if self.query.is_empty()
            || self.query.len() > MUSUBI_MAX_SEARCH_QUERY_BYTES_V1
            || self.query.trim() != self.query
            || self.query.chars().any(char::is_control)
        {
            return Err(ParseError::new(
                "Musubi search query is empty, noncanonical, or exceeds its byte bound",
            ));
        }
        let mut terms = BTreeSet::new();
        for component in self.query.split_whitespace() {
            if component.len() <= MUSUBI_MAX_SEARCH_TERM_BYTES_V1
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            {
                terms.insert(component.to_ascii_lowercase());
            }
            for word in component.split(|character: char| !character.is_alphanumeric()) {
                if word.is_empty() {
                    continue;
                }
                let normalized = word
                    .chars()
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if normalized.len() > MUSUBI_MAX_SEARCH_TERM_BYTES_V1 {
                    return Err(ParseError::new(
                        "Musubi search term exceeds its UTF-8 byte bound",
                    ));
                }
                terms.insert(normalized);
                if terms.len() > MUSUBI_MAX_SEARCH_QUERY_TERMS_V1 {
                    return Err(ParseError::new(
                        "Musubi search query exceeds its normalized term bound",
                    ));
                }
            }
        }
        if terms.is_empty() || terms.len() > MUSUBI_MAX_SEARCH_QUERY_TERMS_V1 {
            return Err(ParseError::new(
                "Musubi search query has no bounded normalized terms",
            ));
        }
        Ok(terms.into_iter().collect())
    }

    /// Validate query normalization and page controls.
    ///
    /// # Errors
    ///
    /// Returns an error if term normalization or page-control validation fails.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.normalized_terms()?;
        self.page.validate()
    }
}

/// One deterministic rich package-discovery result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchHitV1 {
    /// Stable structural package identity.
    pub package: MusubiPackageIdV1,
    /// Immutable namespace used for the first package claim.
    pub claimed_namespace: MusubiNamespaceV1,
    /// Current mutable package description.
    pub description: Option<MusubiDescriptionV1>,
    /// Current sorted package keywords.
    pub keywords: Vec<MusubiKeywordV1>,
    /// Current mutable-metadata revision.
    pub metadata_revision: u64,
}

impl MusubiSearchHitV1 {
    /// Validate structural identity, namespace scope, metadata, and revision.
    ///
    /// # Errors
    ///
    /// Returns an error if package, namespace, or metadata is invalid, the namespace scope does
    /// not match the package, or the metadata revision is zero.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.package.validate()?;
        self.claimed_namespace.validate()?;
        let namespace_scope_matches =
            match (&self.package.scope, self.claimed_namespace.domain_segment()) {
                (MusubiPackageScopeV1::DataspaceRoot, None) => true,
                (MusubiPackageScopeV1::Domain(domain), Some(text)) => domain.as_ref() == text,
                _ => false,
            };
        let metadata = MusubiReleaseMetadataV1 {
            description: self.description.clone(),
            keywords: self.keywords.clone(),
            ..MusubiReleaseMetadataV1::default()
        };
        metadata.validate()?;
        if !namespace_scope_matches || self.metadata_revision == 0 {
            return Err(ParseError::new("Musubi search hit is invalid"));
        }
        Ok(())
    }
}

/// One deterministic page from the rebuildable package discovery projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSearchPageV1 {
    /// Exact bounded request whose discovery results this page carries.
    pub query: MusubiSearchQueryV1,
    /// Results ordered by structural package identity.
    pub items: Vec<MusubiSearchHitV1>,
    /// Continuation cursor, absent at the end of the result set.
    pub next_cursor: Option<MusubiSearchCursorV1>,
    /// Finalized search projection shared by every result.
    pub snapshot: MusubiSearchSnapshotV1,
}

impl MusubiSearchPageV1 {
    /// Validate request identity, page bounds, strict ordering, and cursor binding.
    ///
    /// # Errors
    ///
    /// Returns an error if query, snapshot, hit, or cursor data is invalid, hits are oversized or
    /// noncanonical, or a request/response cursor does not bind the page exactly.
    pub fn validate(&self) -> Result<(), ParseError> {
        self.query.validate()?;
        self.snapshot.validate()?;
        if self.items.len() > self.query.page.effective_limit()
            || self
                .items
                .windows(2)
                .any(|pair| pair[0].package >= pair[1].package)
        {
            return Err(ParseError::new(
                "Musubi search page exceeds its item bound or is not strictly ordered",
            ));
        }
        self.items
            .iter()
            .try_for_each(MusubiSearchHitV1::validate)?;
        if let Some(cursor) = &self.query.page.cursor
            && (cursor.snapshot != self.snapshot
                || self
                    .items
                    .first()
                    .is_some_and(|item| item.package <= cursor.last_package))
        {
            return Err(ParseError::new(
                "Musubi search page does not continue its request cursor",
            ));
        }
        if let Some(cursor) = &self.next_cursor {
            cursor.validate()?;
            if cursor.snapshot != self.snapshot
                || self.items.last().map(|hit| &hit.package) != Some(&cursor.last_package)
                || self.items.len() != self.query.page.effective_limit()
                || self
                    .query
                    .page
                    .cursor
                    .as_ref()
                    .is_some_and(|previous| previous.query_hash != cursor.query_hash)
            {
                return Err(ParseError::new(
                    "Musubi search page cursor does not bind its final result",
                ));
            }
        }
        Ok(())
    }

    /// Validate the page and require its echoed context to equal `query` exactly.
    ///
    /// # Errors
    ///
    /// Returns an error if the page is invalid or its embedded query differs from `query`.
    pub fn validate_for(&self, query: &MusubiSearchQueryV1) -> Result<(), ParseError> {
        self.validate()?;
        if &self.query != query {
            return Err(ParseError::new(
                "Musubi search page carries a different request context",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use norito::codec::DecodeAll as _;

    use super::*;
    use crate::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1;

    #[derive(Encode)]
    struct UncheckedMultisigMemberWire {
        public_key: PublicKey,
        weight: u16,
    }

    #[derive(Encode)]
    struct UncheckedMultisigPolicyWire {
        version: u8,
        threshold: u16,
        members: Vec<UncheckedMultisigMemberWire>,
    }

    #[allow(dead_code)]
    #[derive(Encode)]
    enum UncheckedAccountControllerWire {
        Single(PublicKey),
        Multisig(UncheckedMultisigPolicyWire),
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives a checked keypair");
        AccountId::new(keypair.public_key().clone())
    }

    fn unchecked_multisig_cursor_key(
        version: u8,
        threshold: u16,
        members: Vec<(PublicKey, u16)>,
    ) -> String {
        let controller = UncheckedAccountControllerWire::Multisig(UncheckedMultisigPolicyWire {
            version,
            threshold,
            members: members
                .into_iter()
                .map(|(public_key, weight)| UncheckedMultisigMemberWire { public_key, weight })
                .collect(),
        });
        maintainer_cursor_key_label_v1(&controller.encode(), None)
    }

    fn structurally_oversized_account() -> AccountId {
        let members = (0_u16..256)
            .map(|index| {
                let mut seed = [0xA5; 32];
                seed[..2].copy_from_slice(&index.to_le_bytes());
                let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                    .expect("oversized-account fixture seed derives a checked keypair");
                MultisigMember::new(keypair.public_key().clone(), 1)
                    .expect("oversized-account fixture member")
            })
            .collect();
        let policy = MultisigPolicy::new(1, members).expect("oversized-account fixture policy");
        let account = AccountId::new_multisig(policy);
        assert!(
            norito::to_bytes(&account)
                .expect("oversized account has canonical Norito bytes")
                .len()
                > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1
        );
        account
    }

    fn package(name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
            name.parse().expect("package name"),
        )
    }

    #[test]
    fn portable_path_set_accepts_canonical_bundle_paths_in_any_order() {
        let paths = [
            vec!["src".to_owned(), "caf\u{e9}.ko".to_owned()],
            vec!["Musubi.toml".to_owned()],
            vec![".musubi".to_owned(), "semantic-release.norito".to_owned()],
        ];

        validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice))
            .expect("canonical unordered bundle paths must validate");
    }

    #[test]
    fn portable_path_set_rejects_noncanonical_and_unsafe_components() {
        for component in [
            "cafe\u{301}.ko",
            "CON.ko",
            "trailing.",
            "bidirectional\u{202e}name.ko",
            "colon:name.ko",
        ] {
            let paths = [vec!["src".to_owned(), component.to_owned()]];
            assert!(
                validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err(),
                "unsafe component was accepted: {component:?}"
            );
        }

        let oversized_component = "a".repeat(MUSUBI_MAX_PORTABLE_PATH_COMPONENT_BYTES_V1 + 1);
        let paths = [vec![oversized_component]];
        assert!(validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err());

        let overdeep = vec!["a".to_owned(); MUSUBI_MAX_PORTABLE_PATH_COMPONENTS_V1 + 1];
        let paths = [overdeep];
        assert!(validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err());
    }

    #[test]
    fn portable_path_set_rejects_exact_and_casefolded_aliases_and_prefixes() {
        for paths in [
            vec![vec!["a".to_owned()], vec!["a".to_owned()]],
            vec![vec!["a".to_owned()], vec!["a".to_owned(), "z".to_owned()]],
            vec![
                vec!["src".to_owned(), "Foo.ko".to_owned()],
                vec!["src".to_owned(), "foo.ko".to_owned()],
            ],
            vec![
                vec!["src".to_owned(), "Stra\u{df}e.ko".to_owned()],
                vec!["src".to_owned(), "STRASSE.ko".to_owned()],
            ],
            vec![
                vec!["Foo".to_owned()],
                vec!["foo".to_owned(), "z".to_owned()],
            ],
        ] {
            assert!(validate_musubi_portable_path_set_v1(paths.iter().map(Vec::as_slice)).is_err());
        }
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

    fn resolver_row(version: &str) -> MusubiResolverReleaseRowV1 {
        let mut manifest = release_manifest();
        manifest.release = release("swap-core", version);
        let release_digest = manifest.release_digest();
        let release = manifest.release.clone();
        let archive_id = manifest.archive_id;
        MusubiResolverReleaseRowV1 {
            release: release.clone(),
            release_digest,
            archive_id,
            source_digest: MusubiContentDigestV1::new([0x61; 32]),
            interface_digest: manifest.interface_digest,
            abi: manifest.abi,
            dependencies: manifest.dependencies,
            selection: MusubiReleaseSelectionStateV1 {
                yank: MusubiReleaseYankV1 {
                    release,
                    yanked: false,
                    reason: "initial publication".parse().expect("yank reason"),
                    changed_by: account(17),
                    changed_at_height: 42,
                    revision: 1,
                },
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                    active_locations: 1,
                    finalized_height: 42,
                    finalized_block_hash: [0x42; 32],
                    index_revision: 3,
                },
                governance: MusubiArtifactGovernanceStateV1::Available,
            },
            index_revision: 3,
        }
    }

    #[test]
    fn resolver_row_rejects_availability_newer_than_its_row() {
        let mut row = resolver_row("1.0.0");
        row.selection.storage.index_revision = row.index_revision + 1;

        assert!(row.validate().is_err());
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
    fn account_identity_bound_is_exact_and_recursive() {
        assert!(
            validate_musubi_account_id_canonical_bytes_v1(&vec![
                0xA5;
                MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1
            ])
            .is_ok()
        );
        assert!(
            validate_musubi_account_id_canonical_bytes_v1(&vec![
                0xA5;
                MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1
                    + 1
            ])
            .is_err()
        );
        validate_musubi_account_id_v1(&account(39)).expect("ordinary account fits the bound");

        let oversized = structurally_oversized_account();
        assert!(validate_musubi_account_id_v1(&oversized).is_err());

        let package_record = MusubiPackageRecordV1 {
            package: package("bounded-accounts"),
            claimed_namespace: "dex.universal".parse().expect("namespace"),
            claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([0xA6; 32]),
            owners: vec![oversized.clone()],
            member_accounts: vec![oversized.clone()],
            claimed_at_height: 1,
            revisions: MusubiPackageRevisionsV1 {
                governance: 1,
                metadata: 1,
                archive_locations: 1,
            },
        };
        assert!(
            package_record.validate().is_err(),
            "account vectors must enforce the shared canonical bound"
        );

        let cursor = MusubiFinalizedCursorV1 {
            snapshot: snapshot(),
            query_hash: MusubiQueryHashV1::new([0xA7; 32]),
            last_key: "bounded-caller".to_owned(),
            caller: Some(oversized.clone()),
        };
        assert!(
            cursor.validate().is_err(),
            "optional caller bindings must enforce the shared canonical bound"
        );

        let provider_binding = provider_bundle_binding(oversized);
        assert!(
            provider_binding.validate().is_err(),
            "nested provider completion authorities must enforce the shared canonical bound"
        );
    }

    #[test]
    fn approval_sets_reject_wrong_signature_payload_lengths() {
        const WRONG_SIGNATURE_BYTES: [u8; 63] = [0xA8; 63];
        const LENGTH_ERROR: &str = "Musubi approval signature payload length is invalid";

        let owner_keypair = KeyPair::try_from_seed(vec![44; 32], Algorithm::Ed25519)
            .expect("namespace owner fixture keypair");
        let owner = AccountId::new(owner_keypair.public_key().clone());
        let delegation = MusubiNamespaceDelegationV1 {
            payload: MusubiNamespaceDelegationPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                namespace_binding: MusubiNamespaceBindingDigestV1::new([0xA9; 32]),
                owner_generation: 1,
                owner,
                delegate: account(45),
                expires_at_height: 10,
            },
            approvals: vec![MusubiNamespaceDelegationApprovalV1 {
                public_key: owner_keypair.public_key().clone(),
                signature: SignatureOf::from_signature(Signature::from_bytes(
                    &WRONG_SIGNATURE_BYTES,
                )),
            }],
        };
        assert_eq!(
            delegation
                .validate()
                .expect_err("short signature must fail")
                .reason(),
            LENGTH_ERROR
        );

        let broker_keypair = KeyPair::try_from_seed(vec![53; 32], Algorithm::Ed25519)
            .expect("ingress broker fixture keypair");
        let receipt = MusubiSeedIngressReceiptV1 {
            payload: MusubiSeedIngressReceiptPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                binding: seed_ingress_binding(AccountId::new(broker_keypair.public_key().clone())),
                issued_at_ms: 1_000,
                expires_at_ms: 2_000,
            },
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_keypair.public_key().clone(),
                signature: SignatureOf::from_signature(Signature::from_bytes(
                    &WRONG_SIGNATURE_BYTES,
                )),
            }],
        };
        assert_eq!(
            receipt
                .validate()
                .expect_err("short signature must fail")
                .reason(),
            LENGTH_ERROR
        );

        let provider_keypair = KeyPair::try_from_seed(vec![63; 32], Algorithm::Ed25519)
            .expect("provider owner fixture keypair");
        let attestation = MusubiProviderBundleVerificationAttestationV1 {
            payload: MusubiProviderBundleVerificationPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                binding: provider_bundle_binding(AccountId::new(
                    provider_keypair.public_key().clone(),
                )),
            },
            approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                public_key: provider_keypair.public_key().clone(),
                signature: SignatureOf::from_signature(Signature::from_bytes(
                    &WRONG_SIGNATURE_BYTES,
                )),
            }],
        };
        assert_eq!(
            attestation
                .validate()
                .expect_err("short signature must fail")
                .reason(),
            LENGTH_ERROR
        );
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
    fn archive_registration_projection_excludes_mutable_location_state() {
        let broker_keypair = KeyPair::try_from_seed(vec![52; 32], Algorithm::Ed25519)
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
        let mut archive = MusubiArchiveRecordV1 {
            archive_id: binding.archive_id,
            commitment: archive_commitment(),
            staging_receipt: receipt,
            registered_by: binding.publisher,
            registered_at_height: 7,
            location_revision: 1,
            location_ids: Vec::new(),
        };
        archive.validate().expect("canonical archive record");
        let projection = archive.registration_projection();
        projection
            .validate()
            .expect("canonical immutable registration projection");

        archive.location_revision = 9;
        archive.location_ids = vec![MusubiArchiveLocationIdV1::new([0x31; 32])];
        assert_eq!(
            archive.registration_projection(),
            projection,
            "renewable location state must not enter historical registration evidence"
        );

        let decoded =
            MusubiArchiveRegistrationProjectionV1::decode_all(&mut projection.encode().as_slice())
                .expect("registration projection Norito roundtrip");
        assert_eq!(decoded, projection);
        let mut zero_height = projection;
        zero_height.registered_at_height = 0;
        assert!(zero_height.validate().is_err());
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

        let reference = attestation.reference();
        let attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
            binding.archive_id,
            binding.replication_order,
            &[reference],
        )
        .expect("archive/order-bound provider attestation set digest");
        assert_ne!(
            attestation_set_digest,
            musubi_provider_bundle_attestation_set_digest_v1(
                ArchiveId::new([0xFA; 32]),
                binding.replication_order,
                &[reference],
            )
            .expect("different archive remains a valid commitment")
        );
        assert_ne!(
            attestation_set_digest,
            musubi_provider_bundle_attestation_set_digest_v1(
                binding.archive_id,
                ReplicationOrderId::new([0xFB; 32]),
                &[reference],
            )
            .expect("different order remains a valid commitment")
        );
        assert!(
            musubi_provider_bundle_attestation_set_digest_v1(
                ArchiveId::new([0; 32]),
                binding.replication_order,
                &[reference],
            )
            .is_err()
        );

        let record = MusubiProviderBundleAttestationRecordV1 {
            key: attestation.key(),
            attestation_digest: attestation.digest(),
            attestation: attestation.clone(),
            registered_by: binding.completed_by.clone(),
            registered_at_height: 78,
        };
        record
            .validate()
            .expect("exact provider attestation record");
        let mut mismatched_record = record.clone();
        mismatched_record.attestation_digest =
            MusubiProviderBundleAttestationDigestV1::new([0xFC; 32]);
        assert!(mismatched_record.validate().is_err());

        let mut location = MusubiArchiveLocationV1 {
            location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
            archive_id: binding.archive_id,
            pin_manifest: ManifestDigest::new([0x32; 32]),
            replication_order: binding.replication_order,
            providers: vec![binding.provider_id],
            provider_attestation_set_digest: attestation_set_digest,
            renew_after_epoch: 10,
            expires_at_epoch: 20,
            finalized_height: 30,
            revision: 1,
            state: MusubiArchiveLocationStateV1::Healthy,
        };
        location.validate().expect("valid archive location");
        location.pin_manifest = ManifestDigest::new([0; 32]);
        let decoded = MusubiArchiveLocationV1::decode_all(&mut location.encode().as_slice())
            .expect("zero pin digest remains representable on the wire");
        assert!(
            decoded.validate().is_err(),
            "decoded archive location must reject a zero pin manifest"
        );
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
    fn finalized_cursor_ceiling_covers_every_structured_v1_key_family() {
        let maximum_version = MusubiVersionV1::new(
            u64::MAX,
            u64::MAX,
            u64::MAX,
            vec![
                MusubiPrereleaseIdentifierV1::AlphaNumeric(
                    "a".repeat(MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1),
                );
                MUSUBI_MAX_PRERELEASE_IDENTIFIERS_V1
            ],
        )
        .expect("maximum bounded semantic version");
        let maximum_version_text = maximum_version.to_string();
        assert_eq!(
            maximum_version_text.len(),
            MUSUBI_MAX_VERSION_CURSOR_KEY_BYTES_V1
        );
        assert_eq!(
            maximum_version_text
                .parse::<MusubiVersionV1>()
                .expect("maximum semantic-version text reparses"),
            maximum_version
        );

        assert_eq!(
            MUSUBI_MAX_CURSOR_KEY_BYTES_V1,
            2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 + 1 + "pending-".len() + 64
        );
        assert_eq!(MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1, 16_457);
        // This synthetic bare payload exercises the deliberately conservative
        // ceiling; it is not claimed to be an attainable AccountId encoding.
        let ceiling_sized_bare_account = vec![0xA5; MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1];
        let ceiling_pending_label = maintainer_cursor_key_label_v1(
            &ceiling_sized_bare_account,
            Some(&MusubiInviteIdV1::new([0x5A; 32])),
        );
        assert_eq!(
            ceiling_pending_label.len(),
            MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1
        );
        assert!(ceiling_pending_label.ends_with(&format!("|pending-{}", "5a".repeat(32))));
        assert_eq!(
            maintainer_cursor_key_label_v1(&ceiling_sized_bare_account, None,).len(),
            2 * MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 + 1 + "accepted".len()
        );
        let cursor_account = account(44);
        let accepted = MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
            package: package("cursor-bound"),
            account: cursor_account.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        });
        let pending =
            MusubiMaintainerDirectoryEntryV1::PendingInvitation(MusubiMaintainerInvitationV1 {
                invite_id: MusubiInviteIdV1::new([0x5B; 32]),
                package: package("cursor-bound"),
                invited_by: account(45),
                invited_account: cursor_account,
                role: MusubiPackageRoleV1::Owner,
                expected_governance_revision: 1,
                expires_at_height: 2,
                state: MusubiInvitationStateV1::Pending,
            });
        accepted.validate().expect("accepted cursor fixture");
        pending.validate().expect("pending cursor fixture");
        let accepted_key = accepted.cursor_key();
        let pending_key = pending.cursor_key();
        assert!(accepted_key.ends_with("|accepted"));
        assert!(pending_key.ends_with(&format!("|pending-{}", "5b".repeat(32))));
        assert_ne!(accepted_key, pending_key);
        assert!(maintainer_cursor_key_is_canonical_v1(&accepted_key));
        assert!(maintainer_cursor_key_is_canonical_v1(&pending_key));
        assert!(!maintainer_cursor_key_is_canonical_v1(&format!(
            "aa|pending-{}",
            "00".repeat(32)
        )));
        assert!(!maintainer_cursor_key_is_canonical_v1("AA|accepted"));

        let repeated_boundary = MusubiMaintainerPageV1 {
            query: MusubiPackagePageQueryV1 {
                package: package("cursor-bound"),
                page: MusubiPageRequestV1 {
                    limit: 1,
                    cursor: Some(MusubiFinalizedCursorV1 {
                        snapshot: snapshot(),
                        query_hash: MusubiQueryHashV1::new([0x74; 32]),
                        last_key: accepted_key,
                        caller: None,
                    }),
                },
            },
            items: vec![accepted],
            next_cursor: None,
            snapshot: snapshot(),
        };
        assert!(
            repeated_boundary.validate().is_err(),
            "a maintainer response may not repeat its opaque request boundary"
        );
        for producer_bound in [
            MUSUBI_MAX_VERSION_CURSOR_KEY_BYTES_V1,
            MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1,
            MUSUBI_MAX_ARCHIVE_LOCATION_CURSOR_KEY_BYTES_V1,
            MUSUBI_MAX_ALIAS_HISTORY_CURSOR_KEY_BYTES_V1,
            MUSUBI_MAX_MAINTAINER_CURSOR_KEY_BYTES_V1,
        ] {
            assert!(producer_bound <= MUSUBI_MAX_CURSOR_KEY_BYTES_V1);
        }

        MusubiFinalizedCursorV1 {
            snapshot: snapshot(),
            query_hash: MusubiQueryHashV1::new([0x71; 32]),
            last_key: maximum_version_text,
            caller: None,
        }
        .validate()
        .expect("the longest canonical semantic version is a valid cursor tail");
        MusubiFinalizedCursorV1 {
            snapshot: snapshot(),
            query_hash: MusubiQueryHashV1::new([0x72; 32]),
            last_key: "x".repeat(MUSUBI_MAX_CURSOR_KEY_BYTES_V1),
            caller: None,
        }
        .validate()
        .expect("the exact generic cursor boundary is accepted");
        assert!(
            MusubiFinalizedCursorV1 {
                snapshot: snapshot(),
                query_hash: MusubiQueryHashV1::new([0x73; 32]),
                last_key: "x".repeat(MUSUBI_MAX_CURSOR_KEY_BYTES_V1 + 1),
                caller: None,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn maintainer_cursor_requires_an_exact_canonical_account_payload() {
        let entry = MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
            package: package("canonical-cursor"),
            account: account(46),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        });
        let canonical = entry.cursor_key();
        assert!(maintainer_cursor_key_is_canonical_v1(&canonical));
        let (encoded_account, suffix) = canonical
            .split_once('|')
            .expect("producer cursor contains its suffix separator");

        let truncated = format!(
            "{}|{suffix}",
            &encoded_account[..encoded_account.len().saturating_sub(2)]
        );
        let trailing_bytes = format!("{encoded_account}00|{suffix}");
        for invalid in ["00|accepted".to_owned(), truncated, trailing_bytes] {
            assert!(
                !maintainer_cursor_key_is_canonical_v1(&invalid),
                "malformed or noncanonical account payload survived: {invalid}"
            );
        }
    }

    #[test]
    fn maintainer_cursor_rejects_noncanonical_multisig_wire() {
        let first = account(49)
            .controller()
            .single_signatory()
            .expect("single-key fixture")
            .clone();
        let second = account(50)
            .controller()
            .single_signatory()
            .expect("single-key fixture")
            .clone();
        let policy = MultisigPolicy::new(
            1,
            vec![
                MultisigMember::new(first.clone(), 1).expect("valid member"),
                MultisigMember::new(second.clone(), 1).expect("valid member"),
            ],
        )
        .expect("valid policy");
        let canonical_members = policy
            .members()
            .iter()
            .map(|member| (member.public_key().clone(), member.weight()))
            .collect::<Vec<_>>();
        assert!(maintainer_cursor_key_is_canonical_v1(
            &unchecked_multisig_cursor_key(1, 1, canonical_members.clone())
        ));

        let mut reversed_members = canonical_members.clone();
        reversed_members.reverse();
        let invalid = [
            (
                "unsupported version",
                unchecked_multisig_cursor_key(2, 1, vec![(first.clone(), 1)]),
            ),
            (
                "zero threshold",
                unchecked_multisig_cursor_key(1, 0, vec![(first.clone(), 1)]),
            ),
            (
                "zero weight",
                unchecked_multisig_cursor_key(1, 1, vec![(first.clone(), 0)]),
            ),
            (
                "threshold overflow",
                unchecked_multisig_cursor_key(1, 2, vec![(first.clone(), 1)]),
            ),
            (
                "duplicate key",
                unchecked_multisig_cursor_key(1, 1, vec![(first.clone(), 1), (first.clone(), 1)]),
            ),
            (
                "reversed member order",
                unchecked_multisig_cursor_key(1, 1, reversed_members),
            ),
        ];
        for (case, cursor) in invalid {
            assert!(
                !maintainer_cursor_key_is_canonical_v1(&cursor),
                "semantically noncanonical multisig wire survived: {case}"
            );
        }
    }

    #[test]
    fn maintainer_page_rejects_opaque_boundary_repeated_after_first_item() {
        let package_id = package("repeated-cursor");
        let accepted = |seed| {
            MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
                package: package_id.clone(),
                account: account(seed),
                role: MusubiPackageRoleV1::Owner,
                accepted_at_height: 1,
                governance_revision: 1,
            })
        };
        let mut items = vec![accepted(47), accepted(48)];
        items.sort_by_key(MusubiMaintainerDirectoryEntryV1::key);
        let repeated_boundary = items[1].cursor_key();
        assert_ne!(items[0].cursor_key(), repeated_boundary);

        let page = MusubiMaintainerPageV1 {
            query: MusubiPackagePageQueryV1 {
                package: package_id,
                page: MusubiPageRequestV1 {
                    limit: 2,
                    cursor: Some(MusubiFinalizedCursorV1 {
                        snapshot: snapshot(),
                        query_hash: MusubiQueryHashV1::new([0x75; 32]),
                        last_key: repeated_boundary,
                        caller: None,
                    }),
                },
            },
            items,
            next_cursor: None,
            snapshot: snapshot(),
        };
        assert!(
            page.validate().is_err(),
            "an opaque request boundary may not recur later in the response page"
        );
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

        let ordered: MusubiVersionReqV1 = "<2.0.0, >=1.0.0,>=1.0.0".parse().expect("range");
        assert_eq!(ordered.to_string(), ">=1.0.0,<2.0.0");
        assert!(" ^1.2.3 ".parse::<MusubiVersionReqV1>().is_err());

        assert!(
            ">=1.0.0,=1.0.0,=1.1.0"
                .parse::<MusubiVersionReqV1>()
                .is_err()
        );

        let duplicate_exact: MusubiVersionReqV1 = "=1.2.3,=1.2.3".parse().expect("duplicate exact");
        assert!(matches!(duplicate_exact, MusubiVersionReqV1::Exact(_)));
        assert_eq!(duplicate_exact.to_string(), "=1.2.3");

        for raw in [
            "*",
            "1.2.3",
            "^0.2.3-alpha.1",
            "~1.2.3",
            "1.*",
            "1.2.*",
            "=1.2.3,=1.2.3",
            ">=1.2.3,<2.0.0,>=1.2.3",
        ] {
            let requirement: MusubiVersionReqV1 = raw.parse().expect("valid requirement");
            assert_eq!(
                requirement
                    .to_string()
                    .parse::<MusubiVersionReqV1>()
                    .expect("canonical display reparses"),
                requirement,
                "requirement display must be a canonical AST fixed point for {raw}",
            );
        }
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
    fn requirements_keep_cargo_upper_bounds_at_u64_component_limits() {
        let maximum = u64::MAX;
        let zero_major: MusubiVersionReqV1 = format!("^0.{maximum}.0")
            .parse()
            .expect("zero-major caret at the minor limit");
        assert!(
            zero_major.matches(
                &format!("0.{maximum}.1")
                    .parse()
                    .expect("same compatible minor"),
            )
        );
        assert!(!zero_major.matches(&"1.0.0".parse().expect("next major")));

        let zero_minor: MusubiVersionReqV1 = format!("^0.0.{maximum}")
            .parse()
            .expect("zero-minor caret at the patch limit");
        assert!(
            zero_minor.matches(
                &format!("0.0.{maximum}")
                    .parse()
                    .expect("exact maximum patch"),
            )
        );
        assert!(!zero_minor.matches(&"0.1.0".parse().expect("next minor")));
        assert!(!zero_minor.matches(&"1.0.0".parse().expect("next major")));

        let tilde: MusubiVersionReqV1 = format!("~0.{maximum}.0")
            .parse()
            .expect("tilde at the minor limit");
        assert!(tilde.matches(&format!("0.{maximum}.1").parse().expect("same tilde minor"),));
        assert!(!tilde.matches(&"1.0.0".parse().expect("next tilde major")));

        let maximum_major: MusubiVersionReqV1 = format!("^{maximum}.0.0")
            .parse()
            .expect("caret at the major limit");
        assert!(
            maximum_major.matches(
                &format!("{maximum}.{maximum}.{maximum}")
                    .parse()
                    .expect("same maximum major"),
            )
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

        let noncanonical_exact = MusubiVersionReqV1::Comparators(vec![MusubiVersionComparatorV1 {
            op: MusubiComparatorOpV1::Equal,
            version: "1.0.0".parse().expect("exact comparator version"),
        }]);
        assert!(
            noncanonical_exact.validate().is_err(),
            "decoded singleton equality comparators must use the Exact variant",
        );
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
        oversized.content_length = MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1 + 1;
        assert!(oversized.validate().is_err());

        let mut source_boundary_plus_metadata = archive_commitment();
        source_boundary_plus_metadata.content_length = MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1 + 1;
        source_boundary_plus_metadata
            .validate()
            .expect("bundle metadata fits above the source-only payload ceiling");
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
        let parallel_dependency = MusubiDependencyReqV1 {
            alias: "codec-next".parse().expect("parallel alias"),
            package: dependency_package.clone(),
            requirement: "^1.3.0".parse().expect("parallel requirement"),
        };
        let exact = MusubiExactDependencyEdgeV1 {
            alias: dependency.alias.clone(),
            kind: MusubiDependencyKindV1::Normal,
            package: dependency.package.clone(),
            requirement: dependency.requirement.clone(),
            selected: selected.clone(),
        };
        let parallel_exact = MusubiExactDependencyEdgeV1 {
            alias: parallel_dependency.alias.clone(),
            kind: MusubiDependencyKindV1::Normal,
            package: parallel_dependency.package.clone(),
            requirement: parallel_dependency.requirement.clone(),
            selected: parallel.clone(),
        };
        let mut lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release("swap-core", "1.2.3"),
            root_dependencies: vec![exact, parallel_exact],
            nodes: vec![node(parallel, 20), node(selected, 10)],
        };
        lock.canonicalize();
        lock.validate().expect("exact root selection validates");

        let mut manifest = release_manifest();
        manifest.dependencies = vec![dependency, parallel_dependency];
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

        publication.manifest.dependencies[0].requirement =
            "^1.1.0".parse().expect("different compatible requirement");
        publication
            .manifest
            .validate()
            .expect("the changed manifest remains independently valid");
        publication
            .resolution
            .validate()
            .expect("the exact lock remains independently valid");
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
            root_dependencies: vec![edge("first", first.clone())],
            nodes: vec![
                node(first.clone(), edge("second", second.clone())),
                node(second, edge("first", first)),
            ],
        };
        lock.canonicalize();
        assert!(lock.validate().is_err());
    }

    #[test]
    fn exact_graph_rejects_unreachable_nodes() {
        let orphan = release("orphan", "1.0.0");
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release("root", "1.0.0"),
            root_dependencies: Vec::new(),
            nodes: vec![MusubiVerificationNodeV1 {
                release: orphan,
                release_digest: MusubiReleaseDigestV1::new([1; 32]),
                archive_id: ArchiveId::new([2; 32]),
                source_digest: MusubiContentDigestV1::new([3; 32]),
                interface_digest: MusubiContentDigestV1::new([4; 32]),
                abi: MusubiAbiBindingV1::new([5; 32]).expect("ABI"),
                dependencies: Vec::new(),
            }],
        };

        let error = lock
            .validate()
            .expect_err("unreachable exact nodes must be rejected");
        assert!(error.to_string().contains("unreachable exact nodes"));
    }

    #[test]
    fn verification_lock_rejects_root_in_exact_nodes() {
        let root = release("root", "1.0.0");
        let mut lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: root.clone(),
            root_dependencies: vec![MusubiExactDependencyEdgeV1 {
                alias: "root".parse().expect("alias"),
                kind: MusubiDependencyKindV1::Normal,
                package: root.package.clone(),
                requirement: "^1.0.0".parse().expect("requirement"),
                selected: root.clone(),
            }],
            nodes: vec![MusubiVerificationNodeV1 {
                release: root,
                release_digest: MusubiReleaseDigestV1::new([1; 32]),
                archive_id: ArchiveId::new([2; 32]),
                source_digest: MusubiContentDigestV1::new([3; 32]),
                interface_digest: MusubiContentDigestV1::new([4; 32]),
                abi: MusubiAbiBindingV1::new([5; 32]).expect("ABI"),
                dependencies: Vec::new(),
            }],
        };
        lock.canonicalize();

        let error = lock
            .validate()
            .expect_err("the verification root cannot also be an exact node");
        assert!(error.to_string().contains("invalid or noncanonical"));
    }

    #[test]
    fn verification_nodes_reject_development_dependencies() {
        let parent = release("parent", "1.0.0");
        let child = release("child", "1.0.0");
        let edge = |alias: &str, kind: MusubiDependencyKindV1, selected: MusubiReleaseIdV1| {
            MusubiExactDependencyEdgeV1 {
                alias: alias.parse().expect("alias"),
                kind,
                package: selected.package.clone(),
                requirement: "^1.0.0".parse().expect("requirement"),
                selected,
            }
        };
        let node = |release: MusubiReleaseIdV1,
                    dependencies: Vec<MusubiExactDependencyEdgeV1>,
                    fill: u8| MusubiVerificationNodeV1 {
            release,
            release_digest: MusubiReleaseDigestV1::new([fill; 32]),
            archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
            source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
            interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
            abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
            dependencies,
        };
        let mut lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release("root", "1.0.0"),
            root_dependencies: vec![edge(
                "parent",
                MusubiDependencyKindV1::Normal,
                parent.clone(),
            )],
            nodes: vec![
                node(
                    parent,
                    vec![edge(
                        "child",
                        MusubiDependencyKindV1::Development,
                        child.clone(),
                    )],
                    10,
                ),
                node(child, Vec::new(), 20),
            ],
        };
        lock.canonicalize();

        let error = lock
            .validate()
            .expect_err("transitive development edges must be rejected");
        assert!(error.to_string().contains("verification node"));
    }

    #[test]
    fn parent_local_dependency_aliases_are_unique_across_wire_surfaces() {
        let first_package = package("first-dependency");
        let second_package = package("second-dependency");
        let first_release = MusubiReleaseIdV1::new(
            first_package.clone(),
            "1.1.0".parse().expect("first dependency version"),
        );
        let second_release = MusubiReleaseIdV1::new(
            second_package.clone(),
            "1.2.0".parse().expect("second dependency version"),
        );
        let requirement: MusubiVersionReqV1 = "^1.0.0".parse().expect("dependency requirement");
        let dependency = |package: MusubiPackageIdV1| MusubiDependencyReqV1 {
            alias: "shared".parse().expect("shared dependency alias"),
            package,
            requirement: requirement.clone(),
        };
        let exact = |release: MusubiReleaseIdV1| MusubiExactDependencyEdgeV1 {
            alias: "shared".parse().expect("shared dependency alias"),
            kind: MusubiDependencyKindV1::Normal,
            package: release.package.clone(),
            requirement: requirement.clone(),
            selected: release,
        };
        let node = |release: MusubiReleaseIdV1, fill: u8| MusubiVerificationNodeV1 {
            release,
            release_digest: MusubiReleaseDigestV1::new([fill; 32]),
            archive_id: ArchiveId::new([fill.saturating_add(1); 32]),
            source_digest: MusubiContentDigestV1::new([fill.saturating_add(2); 32]),
            interface_digest: MusubiContentDigestV1::new([fill.saturating_add(3); 32]),
            abi: MusubiAbiBindingV1::new([fill.saturating_add(4); 32]).expect("ABI"),
            dependencies: Vec::new(),
        };

        let mut semantic = release_manifest().semantic_manifest();
        semantic.dependencies = vec![
            dependency(first_package.clone()),
            dependency(second_package.clone()),
        ];
        semantic.dependencies.sort();
        assert!(
            semantic.validate().is_err(),
            "semantic dependencies must not reuse a parent-local alias"
        );

        let mut parent_node = node(release("parent", "1.0.0"), 10);
        parent_node.dependencies =
            vec![exact(first_release.clone()), exact(second_release.clone())];
        parent_node.dependencies.sort();
        assert!(
            parent_node.validate().is_err(),
            "transitive exact edges must not reuse a parent-local alias"
        );

        let mut lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release("root", "1.0.0"),
            root_dependencies: vec![exact(first_release.clone()), exact(second_release.clone())],
            nodes: vec![node(first_release, 20), node(second_release, 30)],
        };
        lock.canonicalize();
        assert!(
            lock.validate().is_err(),
            "verification roots must not reuse a parent-local alias"
        );

        let mut row = resolver_row("2.0.0");
        row.dependencies = vec![dependency(first_package), dependency(second_package)];
        row.dependencies.sort();
        assert!(
            row.validate().is_err(),
            "resolver rows must not retain an ambiguous dependency alias"
        );
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
    fn exact_release_snapshot_binds_home_and_universal_finalized_views() {
        let manifest = release_manifest();
        let release_id = manifest.release.clone();
        let archive_id = manifest.archive_id;
        let publisher = account(16);
        let yank = MusubiReleaseYankV1 {
            release: release_id.clone(),
            yanked: false,
            reason: "initial publication".parse().expect("reason"),
            changed_by: publisher.clone(),
            changed_at_height: 40,
            revision: 1,
        };
        let home_release = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            yank: yank.clone(),
            artifact_governance: MusubiArtifactGovernanceStateV1::Available,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 1,
            },
            manifest: manifest.clone(),
            published_by: publisher,
            published_at_height: 39,
        };
        let universal_release = MusubiResolverReleaseRowV1 {
            release: release_id.clone(),
            release_digest: manifest.release_digest(),
            archive_id,
            source_digest: MusubiContentDigestV1::new([0x71; 32]),
            interface_digest: manifest.interface_digest,
            abi: manifest.abi,
            dependencies: manifest.dependencies,
            selection: MusubiReleaseSelectionStateV1 {
                yank,
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                    active_locations: 1,
                    finalized_height: 42,
                    finalized_block_hash: [0x42; 32],
                    index_revision: 2,
                },
                governance: MusubiArtifactGovernanceStateV1::Available,
            },
            index_revision: 3,
        };
        let exact = MusubiExactReleaseSnapshotV1 {
            chain_id: ChainId::from("musubi-publish-test"),
            genesis_hash: [0x72; 32],
            snapshot: snapshot(),
            home_release,
            universal_release,
        };

        exact
            .validate()
            .expect("independent storage and row revisions are finalized by the snapshot");
        exact
            .validate_for(&MusubiExactReleaseQueryV1 {
                release: release_id,
            })
            .expect("exact response matches its query");

        let mut mismatched_state = exact.clone();
        mismatched_state.universal_release.selection.yank.yanked = true;
        assert!(mismatched_state.validate().is_err());

        let mut future_revision = exact.clone();
        future_revision.home_release.revisions.artifact_governance = 4;
        assert!(future_revision.validate().is_err());

        let mut future_storage_projection = exact.clone();
        future_storage_projection
            .universal_release
            .selection
            .storage
            .index_revision = 4;
        assert!(future_storage_projection.validate().is_err());

        let mut wrong_anchor = exact.clone();
        wrong_anchor
            .universal_release
            .selection
            .storage
            .finalized_block_hash = [0x73; 32];
        assert!(wrong_anchor.validate().is_err());

        let other_release = release("other", "1.2.3");
        assert!(
            exact
                .validate_for(&MusubiExactReleaseQueryV1 {
                    release: other_release,
                })
                .is_err()
        );
    }

    #[test]
    fn release_yank_validation_recurses_into_decoded_release_and_reason() {
        let valid = MusubiReleaseYankV1 {
            release: release("validation", "1.0.0"),
            yanked: true,
            reason: "security response".parse().expect("reason"),
            changed_by: account(8),
            changed_at_height: 42,
            revision: 2,
        };
        valid.validate().expect("valid yank record");

        for raw in [String::new(), "x".repeat(1_025)] {
            let malformed = MusubiReleaseYankV1 {
                reason: MusubiReasonV1(raw),
                ..valid.clone()
            };
            let decoded = MusubiReleaseYankV1::decode_all(&mut malformed.encode().as_slice())
                .expect("malformed reason remains representable on the wire");
            assert!(
                decoded.validate().is_err(),
                "decoded empty or oversized yank reason must fail closed"
            );
        }

        let mut malformed = valid;
        malformed.release.package.name = MusubiPackageNameV1("Upper".to_owned());
        let decoded = MusubiReleaseYankV1::decode_all(&mut malformed.encode().as_slice())
            .expect("malformed release remains representable on the wire");
        assert!(
            decoded.validate().is_err(),
            "decoded yank release identity must be recursively validated"
        );
    }

    #[test]
    fn persisted_records_recursively_validate_decoded_packages_and_takedown_reasons() {
        let mut malformed_package = package("nested");
        malformed_package.name = MusubiPackageNameV1("Upper".to_owned());
        let member = MusubiPackageMemberV1 {
            package: malformed_package.clone(),
            account: account(10),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        };
        let invitation = MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x41; 32]),
            package: malformed_package.clone(),
            invited_by: account(11),
            invited_account: account(12),
            role: MusubiPackageRoleV1::Owner,
            expected_governance_revision: 1,
            expires_at_height: 10,
            state: MusubiInvitationStateV1::Pending,
        };
        let metadata = MusubiPackageMetadataRecordV1 {
            package: malformed_package,
            metadata: MusubiReleaseMetadataV1::default(),
            revision: 1,
            changed_by: account(13),
            changed_at_height: 1,
        };
        let decoded_member =
            MusubiPackageMemberV1::decode_all(&mut member.encode().as_slice()).expect("member");
        let decoded_invitation =
            MusubiMaintainerInvitationV1::decode_all(&mut invitation.encode().as_slice())
                .expect("invitation");
        let decoded_metadata =
            MusubiPackageMetadataRecordV1::decode_all(&mut metadata.encode().as_slice())
                .expect("metadata");
        assert!(decoded_member.validate().is_err());
        assert!(decoded_invitation.validate().is_err());
        assert!(decoded_metadata.validate().is_err());

        for raw in [String::new(), "x".repeat(1_025)] {
            let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
                action_digest: MusubiGovernanceActionDigestV1::new([0x42; 32]),
                reason: MusubiReasonV1(raw),
                applied_at_height: 2,
            });
            let decoded =
                MusubiArtifactGovernanceStateV1::decode_all(&mut governance.encode().as_slice())
                    .expect("malformed takedown remains representable on the wire");
            assert!(
                decoded.validate().is_err(),
                "decoded takedown reason must be recursively validated"
            );

            let manifest = release_manifest();
            let record = MusubiReleaseRecordV1 {
                release_digest: manifest.release_digest(),
                yank: MusubiReleaseYankV1 {
                    release: manifest.release.clone(),
                    yanked: false,
                    reason: "initial publication".parse().expect("reason"),
                    changed_by: account(14),
                    changed_at_height: 1,
                    revision: 1,
                },
                artifact_governance: decoded,
                revisions: MusubiReleaseRevisionsV1 {
                    yank: 1,
                    artifact_governance: 2,
                },
                manifest,
                published_by: account(14),
                published_at_height: 1,
            };
            assert!(
                record.validate().is_err(),
                "authoritative release record must reject its malformed takedown"
            );
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn governed_takedown_json_is_closed_and_uses_applied_height() {
        let manifest = release_manifest();
        let release = manifest.release.clone();
        let archive_id = manifest.archive_id;
        let publisher = account(15);
        let yank = MusubiReleaseYankV1 {
            release: release.clone(),
            yanked: false,
            reason: "initial publication".parse().expect("reason"),
            changed_by: publisher.clone(),
            changed_at_height: 42,
            revision: 1,
        };
        let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: MusubiGovernanceActionDigestV1::new([0x51; 32]),
            reason: "governed security response".parse().expect("reason"),
            applied_at_height: 50,
        });
        let record = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            yank: yank.clone(),
            artifact_governance: governance.clone(),
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 2,
            },
            manifest: manifest.clone(),
            published_by: publisher,
            published_at_height: 42,
        };
        record.validate().expect("canonical governed release");
        let row = MusubiResolverReleaseRowV1 {
            release,
            release_digest: manifest.release_digest(),
            archive_id,
            source_digest: MusubiContentDigestV1::new([0x52; 32]),
            interface_digest: manifest.interface_digest,
            abi: manifest.abi,
            dependencies: manifest.dependencies,
            selection: MusubiReleaseSelectionStateV1 {
                yank,
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                    active_locations: 1,
                    finalized_height: 50,
                    finalized_block_hash: [0x53; 32],
                    index_revision: 2,
                },
                governance: governance.clone(),
            },
            index_revision: 2,
        };
        row.validate().expect("canonical governed resolver row");

        let governance_json = norito::json::to_json(&governance).expect("governance JSON encodes");
        assert!(governance_json.contains("\"applied_at_height\":50"));
        assert!(!governance_json.contains("enacted_at_height"));
        assert_eq!(
            norito::json::from_json::<MusubiArtifactGovernanceStateV1>(&governance_json)
                .expect("canonical governance JSON decodes"),
            governance
        );
        let legacy_height =
            governance_json.replace("\"applied_at_height\"", "\"enacted_at_height\"");
        assert!(
            norito::json::from_json::<MusubiArtifactGovernanceStateV1>(&legacy_height).is_err(),
            "the retired enactment-height spelling must not be accepted"
        );
        for (prefix, depth) in [
            ("{", "the governance envelope"),
            ("\"value\":{", "the takedown payload"),
        ] {
            let hostile = governance_json.replacen(prefix, &format!("{prefix}\"legacy\":true,"), 1);
            assert!(
                norito::json::from_json::<MusubiArtifactGovernanceStateV1>(&hostile).is_err(),
                "governance JSON must reject an unknown field at {depth}"
            );
        }

        let record_json = norito::json::to_json(&record).expect("release record JSON encodes");
        for (prefix, depth) in [
            ("{", "the release record"),
            ("\"yank\":{", "the yank projection"),
            ("\"revisions\":{", "the release revisions"),
        ] {
            let hostile = record_json.replacen(prefix, &format!("{prefix}\"legacy\":true,"), 1);
            assert!(
                norito::json::from_json::<MusubiReleaseRecordV1>(&hostile).is_err(),
                "release JSON must reject an unknown field at {depth}"
            );
        }

        let row_json = norito::json::to_json(&row).expect("resolver row JSON encodes");
        for (prefix, depth) in [
            ("{", "the resolver row"),
            ("\"selection\":{", "the selection projection"),
        ] {
            let hostile = row_json.replacen(prefix, &format!("{prefix}\"legacy\":true,"), 1);
            assert!(
                norito::json::from_json::<MusubiResolverReleaseRowV1>(&hostile).is_err(),
                "resolver JSON must reject an unknown field at {depth}"
            );
        }
    }

    #[test]
    fn archive_availability_requires_exact_runtime_classification_and_capacity() {
        let availability =
            |state, healthy_replicas, active_locations| MusubiArchiveAvailabilityV1 {
                archive_id: ArchiveId::new([0xA7; 32]),
                availability: state,
                healthy_replicas,
                active_locations,
                finalized_height: 9,
                finalized_block_hash: [0xB7; 32],
                index_revision: 3,
            };

        for record in [
            availability(MusubiStorageAvailabilityV1::Unavailable, 0, 0),
            availability(MusubiStorageAvailabilityV1::Unavailable, 0, 2),
            availability(MusubiStorageAvailabilityV1::BelowQuorum, 1, 1),
            availability(MusubiStorageAvailabilityV1::BelowQuorum, 2, 1),
            availability(MusubiStorageAvailabilityV1::Selectable, 3, 1),
        ] {
            record
                .validate()
                .expect("canonical availability projection");
        }

        let mut zero_height = availability(MusubiStorageAvailabilityV1::Unavailable, 0, 0);
        zero_height.finalized_height = 0;
        let invalid = [
            zero_height,
            availability(MusubiStorageAvailabilityV1::Selectable, 3, 0),
            availability(MusubiStorageAvailabilityV1::Selectable, 2, 1),
            availability(MusubiStorageAvailabilityV1::BelowQuorum, 0, 1),
            availability(MusubiStorageAvailabilityV1::BelowQuorum, 3, 1),
            availability(MusubiStorageAvailabilityV1::Unavailable, 1, 1),
            availability(MusubiStorageAvailabilityV1::Selectable, 65, 1),
        ];
        for record in invalid {
            assert!(
                record.validate().is_err(),
                "noncanonical availability projection must fail: {record:?}"
            );
        }
    }

    #[test]
    fn archive_retention_decisions_are_bounded_and_fail_closed() {
        let archive_id = ArchiveId::new([0xC7; 32]);
        let storage = MusubiArchiveAvailabilityV1 {
            archive_id,
            availability: MusubiStorageAvailabilityV1::Unavailable,
            healthy_replicas: 0,
            active_locations: 0,
            finalized_height: 9,
            finalized_block_hash: [0xD7; 32],
            index_revision: 3,
        };
        let referenced = MusubiArchiveRetentionDecisionV1 {
            archive_id,
            disposition: MusubiArchiveRetentionDispositionV1::RetainReferenced,
            active_releases: 1,
            yanked_releases: 2,
            taken_down_releases: 3,
            storage: Some(storage),
        };
        referenced
            .validate()
            .expect("published archives remain retained even without a healthy location");
        assert!(referenced.must_retain());
        let page = MusubiArchiveRetentionPageV1 {
            chain_id: ChainId::from("retention-model-test"),
            genesis_hash: [0xE7; 32],
            items: vec![referenced],
            snapshot: snapshot(),
            finalized_time_ms: 1_700_000_000_000,
        };
        page.validate()
            .expect("storage changes before the query anchor are valid");
        let mut future_storage = page;
        future_storage.items[0]
            .storage
            .as_mut()
            .expect("referenced storage")
            .finalized_height = 43;
        assert!(future_storage.validate().is_err());

        let unknown = MusubiArchiveRetentionDecisionV1 {
            archive_id: ArchiveId::new([0xC8; 32]),
            disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: None,
        };
        unknown
            .validate()
            .expect("unknown archives retain fail-closed");
        assert!(unknown.must_retain());

        let mut inconsistent = referenced.clone();
        inconsistent.disposition = MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown;
        assert!(inconsistent.validate().is_err());

        let request = MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![archive_id, ArchiveId::new([0xC8; 32])],
            expected_snapshot: Some(snapshot()),
        };
        request.validate().expect("canonical exact retention batch");
        let mut duplicate = request.clone();
        duplicate.archive_ids[1] = duplicate.archive_ids[0];
        assert!(duplicate.validate().is_err());
        let mut oversized = request;
        oversized.archive_ids = (1..=MUSUBI_MAX_ARCHIVE_RETENTION_BATCH_V1 + 1)
            .map(|index| {
                let mut bytes = [0_u8; 32];
                bytes[..8].copy_from_slice(
                    &u64::try_from(index)
                        .expect("bounded fixture index")
                        .to_be_bytes(),
                );
                ArchiveId::new(bytes)
            })
            .collect();
        assert!(oversized.validate().is_err());
    }

    #[test]
    fn parliament_actions_validate_decoded_nested_identifiers() {
        let owner_recovery =
            MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
                package: package("recovery"),
                owners: vec![account(9)],
                expected_revision: 1,
            });
        owner_recovery.validate().expect("valid owner recovery");
        let alias_recovery = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
            alias: "stable".parse().expect("alias"),
            target: package("replacement"),
            expected_revision: 1,
        });
        alias_recovery.validate().expect("valid alias recovery");

        let mut malformed_owner = owner_recovery;
        let MusubiParliamentActionV1::RecoverPackageOwners(recovery) = &mut malformed_owner else {
            unreachable!("owner recovery fixture")
        };
        recovery.package.name = MusubiPackageNameV1("Upper".to_owned());

        let mut malformed_alias = alias_recovery.clone();
        let MusubiParliamentActionV1::RetargetAlias(recovery) = &mut malformed_alias else {
            unreachable!("alias recovery fixture")
        };
        recovery.alias = MusubiAliasNameV1("Upper".to_owned());

        let mut malformed_target = alias_recovery;
        let MusubiParliamentActionV1::RetargetAlias(recovery) = &mut malformed_target else {
            unreachable!("alias recovery fixture")
        };
        recovery.target.name = MusubiPackageNameV1("Upper".to_owned());

        for action in [malformed_owner, malformed_alias, malformed_target] {
            let decoded = MusubiParliamentActionV1::decode_all(&mut action.encode().as_slice())
                .expect("malformed nested identity remains representable on the wire");
            assert!(
                decoded.validate().is_err(),
                "decoded Parliament action must validate every nested identity"
            );
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn parliament_action_json_rejects_unknown_fields_recursively() {
        macro_rules! assert_unknown_rejected {
            ($canonical:expr, $prefix:literal, $depth:literal) => {{
                let canonical: &str = $canonical;
                let replacement = format!("{}\"legacy\":true,", $prefix);
                let hostile = canonical.replacen($prefix, &replacement, 1);
                assert_ne!(
                    hostile, canonical,
                    "canonical Parliament action JSON must contain {}",
                    $depth
                );
                assert!(
                    norito::json::from_json::<MusubiParliamentActionV1>(&hostile).is_err(),
                    "Parliament action JSON must reject an unknown field at {}",
                    $depth
                );
            }};
        }

        let owner_recovery =
            MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
                package: package("closed-owner-recovery"),
                owners: vec![account(9)],
                expected_revision: 1,
            });
        let alias_retarget = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
            alias: "closed-alias".parse().expect("alias"),
            target: package("closed-alias-target"),
            expected_revision: 1,
        });
        let takedown = MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
            release: MusubiReleaseIdV1::new(
                package("closed-takedown"),
                "1.2.3-alpha".parse().expect("release version"),
            ),
            reason: MusubiReasonV1::new("hostile JSON regression test").expect("bounded reason"),
            expected_artifact_governance_revision: 1,
        });
        let set_policy =
            MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
                policy: MusubiRegistryPolicyV1::default(),
                expected_revision: 1,
            });

        for action in [&owner_recovery, &alias_retarget, &takedown, &set_policy] {
            let canonical =
                norito::json::to_json(action).expect("canonical Parliament action JSON encodes");
            assert_eq!(
                norito::json::from_json::<MusubiParliamentActionV1>(&canonical)
                    .expect("canonical Parliament action JSON decodes"),
                *action
            );
            assert_unknown_rejected!(canonical.as_str(), "{", "the tagged action envelope");
            assert_unknown_rejected!(canonical.as_str(), "\"value\":{", "the action payload");
        }

        let owner_json = norito::json::to_json(&owner_recovery).expect("owner action encodes");
        assert_unknown_rejected!(owner_json.as_str(), "\"package\":{", "the package identity");
        assert_unknown_rejected!(owner_json.as_str(), "\"scope\":{", "the package scope");

        let takedown_json = norito::json::to_json(&takedown).expect("takedown action encodes");
        assert_unknown_rejected!(
            takedown_json.as_str(),
            "\"release\":{",
            "the release identity"
        );
        assert_unknown_rejected!(
            takedown_json.as_str(),
            "\"version\":{",
            "the structured version"
        );
        assert_unknown_rejected!(
            takedown_json.as_str(),
            "\"prerelease\":[{",
            "the prerelease identifier envelope"
        );

        let policy_json = norito::json::to_json(&set_policy).expect("policy action encodes");
        assert_unknown_rejected!(policy_json.as_str(), "\"policy\":{", "the registry policy");
        assert_unknown_rejected!(policy_json.as_str(), "\"mode\":{", "the admission mode");
        assert_unknown_rejected!(
            policy_json.as_str(),
            "\"alias_pricing\":{",
            "the alias pricing policy"
        );
    }

    #[test]
    fn governance_decision_consumption_binds_execution_boundary_and_roundtrips() {
        let consumption = MusubiGovernanceDecisionConsumptionV1 {
            decision: MusubiGovernanceDecisionV1 {
                decision_id: [0x31; 32],
                action_digest: MusubiGovernanceActionDigestV1::new([0x42; 32]),
                enacted_at_height: 10,
                execute_after_height: 20,
            },
            minimum_enactment_delay: 10,
            consumed_at_height: 20,
        };
        consumption
            .validate()
            .expect("execution exactly at the decision boundary is valid");

        let decoded =
            MusubiGovernanceDecisionConsumptionV1::decode_all(&mut consumption.encode().as_slice())
                .expect("decision consumption Norito roundtrip");
        assert_eq!(decoded, consumption);
        decoded
            .validate()
            .expect("roundtripped consumption validates");

        let mut premature = consumption;
        premature.consumed_at_height = 19;
        assert!(premature.validate().is_err());

        let mut shortened_delay = consumption;
        shortened_delay.minimum_enactment_delay = 11;
        assert!(shortened_delay.validate().is_err());

        let mut malformed = consumption;
        malformed.decision.decision_id = [0; 32];
        assert!(malformed.validate().is_err());
    }

    #[cfg(feature = "json")]
    #[test]
    fn governance_decision_consumption_json_rejects_bare_and_unknown_forms() {
        let consumption = MusubiGovernanceDecisionConsumptionV1 {
            decision: MusubiGovernanceDecisionV1 {
                decision_id: [0x31; 32],
                action_digest: MusubiGovernanceActionDigestV1::new([0x42; 32]),
                enacted_at_height: 10,
                execute_after_height: 20,
            },
            minimum_enactment_delay: 10,
            consumed_at_height: 20,
        };
        let canonical = norito::json::to_json(&consumption)
            .expect("canonical decision consumption JSON encodes");
        assert_eq!(
            norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&canonical)
                .expect("canonical decision consumption JSON decodes"),
            consumption
        );

        let bare = norito::json::to_json(&consumption.decision)
            .expect("bare decision JSON remains representable as its own public type");
        assert!(
            norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&bare).is_err(),
            "the persisted consumption store must not accept the old bare-decision shape"
        );

        let unknown = canonical.replacen('{', "{\"legacy\":true,", 1);
        assert!(
            norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&unknown).is_err(),
            "the first-release consumption shape must reject unknown fields"
        );

        let nested_unknown =
            canonical.replacen("\"decision\":{", "\"decision\":{\"legacy\":true,", 1);
        assert_ne!(nested_unknown, canonical, "decision JSON field is present");
        assert!(
            norito::json::from_json::<MusubiGovernanceDecisionConsumptionV1>(&nested_unknown)
                .is_err(),
            "the nested first-release decision shape must reject unknown fields"
        );
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
    fn registry_policy_successors_bind_price_changes_to_pricing_revisions() {
        let current = MusubiRegistryPolicyV1::default();

        let mut mode_only = current.clone();
        mode_only.revision += 1;
        mode_only.mode = MusubiRegistryAdmissionModeV1::Closed;
        mode_only
            .validate_successor(&current)
            .expect("non-price policy changes retain the exact pricing policy");

        let mut unchanged_with_new_pricing_revision = mode_only.clone();
        unchanged_with_new_pricing_revision.alias_pricing.revision += 1;
        assert!(
            unchanged_with_new_pricing_revision
                .validate_successor(&current)
                .is_err(),
            "pricing revision must not advance when prices are unchanged"
        );

        let mut changed_without_new_pricing_revision = mode_only.clone();
        changed_without_new_pricing_revision
            .alias_pricing
            .length_5_to_32_xor += 1;
        assert!(
            changed_without_new_pricing_revision
                .validate_successor(&current)
                .is_err(),
            "changed prices must advance the pricing revision"
        );

        let mut changed = changed_without_new_pricing_revision;
        changed.alias_pricing.revision += 1;
        changed
            .validate_successor(&current)
            .expect("changed prices with the exact successor revision are canonical");

        let mut skipped_pricing_revision = changed.clone();
        skipped_pricing_revision.alias_pricing.revision += 1;
        assert!(
            skipped_pricing_revision
                .validate_successor(&current)
                .is_err(),
            "changed prices must not skip a pricing revision"
        );

        let mut skipped = mode_only;
        skipped.revision += 1;
        assert!(
            skipped.validate_successor(&current).is_err(),
            "registry policy revisions must not skip"
        );

        let mut exhausted_policy = current.clone();
        exhausted_policy.revision = u64::MAX;
        assert!(
            current.validate_successor(&exhausted_policy).is_err(),
            "an exhausted registry revision cannot have a successor"
        );

        let mut exhausted_pricing = current.clone();
        exhausted_pricing.alias_pricing.revision = u64::MAX;
        let mut changed_after_exhausted_pricing = exhausted_pricing.clone();
        changed_after_exhausted_pricing.revision += 1;
        changed_after_exhausted_pricing
            .alias_pricing
            .length_5_to_32_xor += 1;
        assert!(
            changed_after_exhausted_pricing
                .validate_successor(&exhausted_pricing)
                .is_err(),
            "an exhausted pricing revision cannot describe changed prices"
        );
    }

    #[test]
    fn page_and_cursor_bounds_are_enforced() {
        for limit in [
            0,
            u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1).expect("page maximum fits u32"),
        ] {
            MusubiPageRequestV1 {
                limit,
                cursor: None,
            }
            .validate()
            .expect("default and exact-maximum page limits are canonical");
        }
        for limit in [
            u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1 + 1).expect("page overflow fixture fits u32"),
            u32::MAX,
        ] {
            assert!(
                MusubiPageRequestV1 {
                    limit,
                    cursor: None,
                }
                .validate()
                .is_err(),
                "oversized page limit {limit} must be rejected instead of clamped"
            );
        }

        let ordered = MusubiVersionPageV1 {
            query: MusubiPackagePageQueryV1 {
                package: package("page-bounds"),
                page: MusubiPageRequestV1 {
                    limit: 2,
                    cursor: None,
                },
            },
            items: vec![
                "1.0.0".parse().expect("version"),
                "2.0.0".parse().expect("version"),
            ],
            next_cursor: None,
            snapshot: snapshot(),
        };
        ordered.validate().expect("strictly ordered version page");
        let mut reversed = ordered.clone();
        reversed.items.reverse();
        assert!(reversed.validate().is_err());
        let mut duplicate = ordered.clone();
        duplicate.items[1] = duplicate.items[0].clone();
        assert!(duplicate.validate().is_err());
        let malformed = MusubiVersionV1 {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: vec![MusubiPrereleaseIdentifierV1::AlphaNumeric(String::new())],
        };
        let decoded = MusubiVersionPageV1::decode_all(
            &mut MusubiVersionPageV1 {
                query: ordered.query.clone(),
                items: vec![malformed],
                next_cursor: None,
                snapshot: snapshot(),
            }
            .encode()
            .as_slice(),
        )
        .expect("malformed page item remains representable on the wire");
        assert!(
            decoded.validate().is_err(),
            "page validation must recurse into decoded items"
        );

        let page = MusubiVersionPageV1 {
            query: MusubiPackagePageQueryV1 {
                package: package("page-overflow"),
                page: MusubiPageRequestV1 {
                    limit: u32::try_from(MUSUBI_MAX_PAGE_SIZE_V1).expect("page maximum fits u32"),
                    cursor: None,
                },
            },
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
            query: MusubiResolverIndexQueryV1 {
                package: package("resolver-page"),
                requirement: None,
                page: MusubiPageRequestV1 {
                    limit: 50,
                    cursor: None,
                },
            },
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
            query: MusubiOrderedPrefixQueryV1 {
                prefix: MusubiOrderedPrefixV1::new("sora/").expect("directory prefix"),
                page: MusubiPageRequestV1 {
                    limit: 50,
                    cursor: None,
                },
            },
            chain_id: ChainId::from("musubi-test-chain"),
            genesis_hash: [9; 32],
            namespace_binding: MusubiNamespaceBindingV1 {
                namespace: "sora".parse().expect("namespace"),
                home_dataspace: DataSpaceId::new(7),
                scope: MusubiPackageScopeV1::DataspaceRoot,
                generation: 1,
            },
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
    fn empty_response_pages_retain_their_exact_query_identity() {
        let package_id = package("empty-context");
        let package_query = MusubiPackagePageQueryV1 {
            package: package_id.clone(),
            page: MusubiPageRequestV1 {
                limit: 7,
                cursor: None,
            },
        };
        let versions = MusubiVersionPageV1 {
            query: package_query.clone(),
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        versions
            .validate_for(&package_query)
            .expect("empty version page retains its package and page controls");
        let mut other_package_query = package_query.clone();
        other_package_query.package = package("other-context");
        assert!(versions.validate_for(&other_package_query).is_err());

        let resolver_query = MusubiResolverIndexQueryV1 {
            package: package_id,
            requirement: Some("^1.2.3".parse().expect("requirement")),
            page: MusubiPageRequestV1 {
                limit: 9,
                cursor: None,
            },
        };
        let resolver = MusubiResolverIndexPageV1 {
            query: resolver_query.clone(),
            chain_id: ChainId::from("musubi-test-chain"),
            genesis_hash: [9; 32],
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        resolver
            .validate_for(&resolver_query)
            .expect("empty resolver page retains package, requirement, and page controls");
        let mut other_resolver_query = resolver_query.clone();
        other_resolver_query.requirement = Some("~1.2.3".parse().expect("requirement"));
        assert!(resolver.validate_for(&other_resolver_query).is_err());

        let maintainers = MusubiMaintainerPageV1 {
            query: package_query.clone(),
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        maintainers
            .validate_for(&package_query)
            .expect("empty maintainer page retains its package context");

        let alias_query = MusubiAliasQueryV1 {
            alias: "math".parse().expect("alias"),
            page: MusubiPageRequestV1 {
                limit: 11,
                cursor: None,
            },
        };
        let history = MusubiAliasHistoryPageV1 {
            query: alias_query.clone(),
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        history
            .validate_for(&alias_query)
            .expect("empty alias-history page retains its alias context");

        let prefix_query = MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("sora/math-").expect("prefix"),
            page: MusubiPageRequestV1 {
                limit: 13,
                cursor: None,
            },
        };
        let directory = MusubiOrderedPackagePageV1 {
            query: prefix_query.clone(),
            chain_id: ChainId::from("musubi-test-chain"),
            genesis_hash: [9; 32],
            namespace_binding: MusubiNamespaceBindingV1 {
                namespace: "sora".parse().expect("namespace"),
                home_dataspace: DataSpaceId::new(7),
                scope: MusubiPackageScopeV1::DataspaceRoot,
                generation: 1,
            },
            items: Vec::new(),
            next_cursor: None,
            snapshot: snapshot(),
        };
        directory
            .validate_for(&prefix_query)
            .expect("empty directory page retains its complete prefix context");

        let search_query = MusubiSearchQueryV1 {
            query: "arithmetic math".to_owned(),
            page: MusubiSearchPageRequestV1 {
                limit: 15,
                cursor: None,
            },
        };
        let search = MusubiSearchPageV1 {
            query: search_query.clone(),
            items: Vec::new(),
            next_cursor: None,
            snapshot: MusubiSearchSnapshotV1 {
                finalized_height: 5,
                finalized_block_hash: [7; 32],
                projection_revision: 9,
            },
        };
        search
            .validate_for(&search_query)
            .expect("empty first search page retains its exact terms and page controls");
        let mut other_search_query = search_query.clone();
        other_search_query.query = "math arithmetic".to_owned();
        assert!(search.validate_for(&other_search_query).is_err());
    }

    #[test]
    fn version_page_cursor_advances_by_structured_semver() {
        let snapshot = snapshot();
        let cursor = MusubiFinalizedCursorV1 {
            snapshot,
            query_hash: MusubiQueryHashV1::new([0x31; 32]),
            last_key: "1.2.0".to_owned(),
            caller: None,
        };
        let page = MusubiVersionPageV1 {
            query: MusubiPackagePageQueryV1 {
                package: package("semver-cursor"),
                page: MusubiPageRequestV1 {
                    limit: 2,
                    cursor: Some(cursor),
                },
            },
            items: vec!["1.10.0".parse().expect("version")],
            next_cursor: None,
            snapshot,
        };
        page.validate()
            .expect("1.10.0 follows 1.2.0 by structured SemVer, not lexical text order");

        let mut prerelease = page;
        prerelease
            .query
            .page
            .cursor
            .as_mut()
            .expect("cursor")
            .last_key = "2.0.0-alpha.10".to_owned();
        prerelease.items = vec!["2.0.0-beta.1".parse().expect("prerelease")];
        prerelease
            .validate()
            .expect("prerelease cursor advancement uses structured SemVer ordering");
    }

    #[test]
    fn finalized_next_cursor_binds_the_exact_full_page_tail() {
        let snapshot = snapshot();
        let query = MusubiPackagePageQueryV1 {
            package: package("next-cursor"),
            page: MusubiPageRequestV1 {
                limit: 1,
                cursor: None,
            },
        };
        let mut page = MusubiVersionPageV1 {
            query,
            items: vec!["1.0.0".parse().expect("version")],
            next_cursor: Some(MusubiFinalizedCursorV1 {
                snapshot,
                query_hash: MusubiQueryHashV1::new([0x41; 32]),
                last_key: "1.0.0".to_owned(),
                caller: None,
            }),
            snapshot,
        };
        page.validate().expect("exact full-page cursor tail");
        page.next_cursor.as_mut().expect("cursor").last_key = "1.0.1".to_owned();
        assert!(page.validate().is_err());
    }

    #[test]
    fn resolver_next_cursor_may_bind_a_nonempty_byte_budgeted_short_page() {
        assert!(
            MUSUBI_RESOLVER_PAGE_JSON_ITEMS_BUDGET_BYTES_V1
                < MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1
        );
        let snapshot = snapshot();
        let row = resolver_row("1.0.0");
        row.validate().expect("resolver row is canonical");
        let cursor = MusubiFinalizedCursorV1 {
            snapshot,
            query_hash: MusubiQueryHashV1::new([0x51; 32]),
            last_key: row.release.version.to_string(),
            caller: None,
        };
        let query = MusubiResolverIndexQueryV1 {
            package: row.release.package.clone(),
            requirement: None,
            page: MusubiPageRequestV1 {
                limit: 2,
                cursor: None,
            },
        };
        let page = MusubiResolverIndexPageV1 {
            query: query.clone(),
            chain_id: ChainId::from("musubi-resolver-page-test"),
            genesis_hash: [0x52; 32],
            items: vec![row],
            next_cursor: Some(cursor.clone()),
            snapshot,
        };
        page.validate_for(&query)
            .expect("resolver byte budgeting may truncate before the requested item limit");
        #[cfg(feature = "json")]
        assert!(
            norito::json::to_json(&page)
                .expect("resolver page JSON")
                .len()
                <= MUSUBI_PUBLIC_QUERY_MAX_RESPONSE_BYTES_V1
        );

        let version_page = MusubiVersionPageV1 {
            query: MusubiPackagePageQueryV1 {
                package: query.package,
                page: query.page,
            },
            items: vec!["1.0.0".parse().expect("version")],
            next_cursor: Some(cursor),
            snapshot,
        };
        assert!(
            version_page.validate().is_err(),
            "non-resolver page types must retain the exact-full-page continuation invariant"
        );
    }

    #[test]
    fn ordered_prefix_requires_canonical_namespace_and_package_prefix() {
        for invalid in ["sora", "sora/-math", "sora/math--", "sora/math/extra"] {
            assert!(
                MusubiOrderedPrefixV1::new(invalid).is_err(),
                "invalid ordered prefix `{invalid}` must be rejected"
            );
        }
        assert!(
            MusubiOrderedPrefixV1::new(&format!(
                "sora/{}",
                "a".repeat(MUSUBI_MAX_PACKAGE_NAME_BYTES_V1 + 1)
            ))
            .is_err(),
            "an ordered package prefix may not exceed the package-name bound"
        );
        let prefix = MusubiOrderedPrefixV1::new("apps.sora/math-").expect("canonical prefix");
        let (namespace, package_prefix) = prefix.components().expect("prefix components");
        assert_eq!(namespace.as_str(), "apps.sora");
        assert_eq!(package_prefix, "math-");

        let maximum = format!(
            "{}/{}",
            "a".repeat(MUSUBI_MAX_NAMESPACE_BYTES_V1),
            "b".repeat(MUSUBI_MAX_PACKAGE_NAME_BYTES_V1)
        );
        assert_eq!(maximum.len(), MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1);
        MusubiOrderedPrefixV1::new(&maximum)
            .expect("the exact structural ordered-prefix boundary is accepted");
        assert!(MusubiOrderedPrefixV1::new(&(maximum + "c")).is_err());
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
            MusubiArchiveRetentionQueryV1,
            MusubiArchiveRetentionQueryV1 {
                archive_ids: vec![archive_commitment().archive_id()],
                expected_snapshot: Some(snapshot()),
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
            MusubiSearchQueryV1,
            MusubiSearchQueryV1 {
                query: "zero-knowledge verifier".to_owned(),
                page: MusubiSearchPageRequestV1 {
                    limit: MUSUBI_DEFAULT_PAGE_SIZE_V1,
                    cursor: None,
                },
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

    #[cfg(feature = "json")]
    #[test]
    fn archive_retention_json_rejects_unknown_fields_recursively() {
        macro_rules! assert_unknown_rejected {
            ($type:ty, $canonical:expr, $prefix:literal, $depth:literal) => {{
                let canonical: &str = $canonical;
                let replacement = format!("{}\"legacy\":true,", $prefix);
                let hostile = canonical.replacen($prefix, &replacement, 1);
                assert_ne!(
                    hostile, canonical,
                    "canonical archive-retention JSON must contain {}",
                    $depth
                );
                assert!(
                    norito::json::from_json::<$type>(&hostile).is_err(),
                    "archive-retention JSON must reject an unknown field at {}",
                    $depth
                );
            }};
        }

        let snapshot = snapshot();
        let snapshot_json =
            norito::json::to_json(&snapshot).expect("registry snapshot JSON encodes");
        assert_unknown_rejected!(
            MusubiRegistrySnapshotV1,
            snapshot_json.as_str(),
            "{",
            "the registry snapshot"
        );

        let storage = MusubiArchiveAvailabilityV1 {
            archive_id: archive_commitment().archive_id(),
            availability: MusubiStorageAvailabilityV1::Selectable,
            healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
            active_locations: 1,
            finalized_height: snapshot.finalized_height,
            finalized_block_hash: snapshot.finalized_block_hash,
            index_revision: snapshot.index_revision,
        };
        storage.validate().expect("canonical availability fixture");
        let storage_json =
            norito::json::to_json(&storage).expect("archive availability JSON encodes");
        assert_unknown_rejected!(
            MusubiArchiveAvailabilityV1,
            storage_json.as_str(),
            "{",
            "the availability projection"
        );
        assert_unknown_rejected!(
            MusubiArchiveAvailabilityV1,
            storage_json.as_str(),
            "\"availability\":{",
            "the storage-availability envelope"
        );

        let request = MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![storage.archive_id],
            expected_snapshot: Some(snapshot),
        };
        let request_json =
            norito::json::to_json(&request).expect("archive-retention request JSON encodes");
        assert_unknown_rejected!(
            MusubiArchiveRetentionQueryV1,
            request_json.as_str(),
            "\"expected_snapshot\":{",
            "the request snapshot"
        );

        let decision = MusubiArchiveRetentionDecisionV1 {
            archive_id: storage.archive_id,
            disposition: MusubiArchiveRetentionDispositionV1::RetainReferenced,
            active_releases: 1,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: Some(storage),
        };
        decision.validate().expect("canonical retention decision");
        let decision_json =
            norito::json::to_json(&decision).expect("archive-retention decision JSON encodes");
        assert_unknown_rejected!(
            MusubiArchiveRetentionDecisionV1,
            decision_json.as_str(),
            "\"disposition\":{",
            "the retention-disposition envelope"
        );
        assert_unknown_rejected!(
            MusubiArchiveRetentionDecisionV1,
            decision_json.as_str(),
            "\"storage\":{",
            "the nested availability projection"
        );
    }

    #[test]
    fn search_terms_are_bounded_exact_and_canonical() {
        let request = MusubiSearchQueryV1 {
            query: "Zero-Knowledge verifier verifier".to_owned(),
            page: MusubiSearchPageRequestV1 {
                limit: 0,
                cursor: None,
            },
        };
        assert_eq!(
            request.normalized_terms().expect("normalized terms"),
            vec![
                "knowledge".to_owned(),
                "verifier".to_owned(),
                "zero".to_owned(),
                "zero-knowledge".to_owned(),
            ]
        );
        assert_eq!(request.page.effective_limit(), 50);

        let mut too_many = request;
        too_many.query = (0..=MUSUBI_MAX_SEARCH_QUERY_TERMS_V1)
            .map(|index| format!("term{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(too_many.validate().is_err());
    }
}

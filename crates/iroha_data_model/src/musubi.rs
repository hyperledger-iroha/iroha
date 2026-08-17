//! First-release Musubi registry wire model.
//!
//! Musubi package identity is structural and stable: a package is keyed by its
//! home [`DataSpaceId`], package scope, and package name.  Human-facing
//! `namespace/package` selectors are resolved through immutable namespace
//! bindings before they enter releases, resolver rows, or lock graphs.
use core::cmp::Ordering;
use iroha_crypto::{Hash, HashOf, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, io,
    str::FromStr,
    string::String,
    vec::Vec,
};
mod streaming;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::{AccountController, AccountId, MultisigMember, MultisigPolicy},
    error::ParseError,
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
use streaming::canonical_frame_len;
#[cfg(feature = "json")]
use streaming::musubi_json_len_bounded;
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
/// Maximum canonical bytes in the mandatory Musubi artifact-descriptor bundle file.
pub const MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1: u64 = 64 * 1024;
/// Maximum canonical bytes in one mandatory Musubi bundle metadata file.
///
/// The semantic release and exact verification lock share this provider-memory corridor; the
/// artifact descriptor uses [`MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1`].
pub const MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1: u64 = 2 * 1024 * 1024;
const MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_USIZE_V1: usize = 64 * 1024;
const MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_USIZE_V1: usize = 2 * 1024 * 1024;
// Norito charges nested length-delimited bodies, container reservations, and any required
// whole-input realignment copy cumulatively. The 48 MiB corridor is regression-tested against the
// producer-reachable 2,056,570-byte dense-lock fixture with at least 5 MiB of reviewed headroom.
const MUSUBI_BUNDLE_METADATA_DECODE_MAX_ALLOCATED_BYTES_V1: usize =
    24 * MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_USIZE_V1;
const MUSUBI_ARTIFACT_DESCRIPTOR_DECODE_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    32,
    MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_USIZE_V1,
    256,
    128 * 1024,
    32,
);
const MUSUBI_SEMANTIC_RELEASE_DECODE_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    1_024,
    MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_USIZE_V1,
    100_000,
    MUSUBI_BUNDLE_METADATA_DECODE_MAX_ALLOCATED_BYTES_V1,
    64,
);
// A dense valid graph can carry 1,024 nodes with 256 edges each and up to 16 requirement
// comparators per edge. The element limit covers those counts; the independent file and
// allocation ceilings bound which complete graph shapes are admissible bundle metadata.
const MUSUBI_VERIFICATION_LOCK_DECODE_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    1_024,
    MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_USIZE_V1,
    8_000_000,
    MUSUBI_BUNDLE_METADATA_DECODE_MAX_ALLOCATED_BYTES_V1,
    64,
);
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
/// Maximum canonical Norito bytes for one replication-order/archive trust binding.
///
/// The commitment itself has fixed-size digests and counters plus a chunker handle bounded by
/// [`MusubiArchiveCommitmentV1::validate`]. This deliberately conservative ceiling keeps decoded
/// snapshot values bounded if that commitment grows within the first-release schema.
pub const MUSUBI_MAX_REPLICATION_ORDER_ARCHIVE_BINDING_CANONICAL_BYTES_V1: usize = 4 * 1024;
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
/// Maintainer keys hex-encode the bare canonical account payload and append either `accepted` or
/// the longer `pending-` plus a 32-byte invite identity. The shared account bound includes Norito
/// framing, so applying its full value to the bare payload deliberately leaves headroom rather than
/// claiming an attainable maximum cursor length.
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
/// The caller supplies path components rather than platform paths. Every component must already be
/// in the exact NFC representation used by Iroha [`Name`] values. The policy also excludes
/// traversal, portable reserved names and characters, bidirectional controls, file/directory prefix
/// conflicts, and Unicode case-fold collisions which would alias on a supported case-insensitive
/// filesystem. Ordering is deliberately not required: package commitments order joined path bytes,
/// while canonical `SoraFS` plans order structural component vectors. The fixed count ceiling
/// accommodates the 4,096 committed source files plus the three mandatory bundle metadata entries.
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
#[cfg(test)]
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
    let encoded_len = canonical_frame_len(account_id)
        .map_err(|_| ParseError::new("Musubi account identity has no canonical Norito encoding"))?;
    if encoded_len == 0 || encoded_len > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 {
        return Err(ParseError::new(
            "Musubi account identity exceeds its canonical byte bound",
        ));
    }
    Ok(())
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
struct Blake3Writer<'a>(&'a mut blake3::Hasher);
impl io::Write for Blake3Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn domain_hash_value<T: norito::core::NoritoSerialize>(domain: &[u8], value: &T) -> [u8; 32] {
    let encoded_len = norito::codec::encode_adaptive_into(value, &mut io::sink())
        .expect("Musubi canonical hash preflight must serialize");
    let mut hasher = blake3::Hasher::new();
    hasher.update(
        &u64::try_from(domain.len())
            .expect("Musubi hash domain length fits u64")
            .to_le_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        &u64::try_from(encoded_len)
            .expect("Musubi encoded payload length fits u64")
            .to_le_bytes(),
    );
    let written = norito::codec::encode_adaptive_into(value, &mut Blake3Writer(&mut hasher))
        .expect("Musubi canonical hash payload must serialize");
    assert_eq!(
        written, encoded_len,
        "Musubi canonical hash length changed between passes"
    );
    *hasher.finalize().as_bytes()
}
fn domain_signing_hash<T: Encode>(domain: &[u8], payload: &T) -> HashOf<T> {
    let encoded_len = norito::codec::encode_adaptive_into(payload, &mut io::sink())
        .expect("Musubi signing-hash preflight must serialize");
    let domain_len = u64::try_from(domain.len())
        .expect("Musubi signature domain length fits u64")
        .to_le_bytes();
    let encoded_len_bytes = u64::try_from(encoded_len)
        .expect("Musubi signed payload length fits u64")
        .to_le_bytes();
    let hash = Hash::new_from_writer(|writer| {
        io::Write::write_all(writer, &domain_len)?;
        io::Write::write_all(writer, domain)?;
        io::Write::write_all(writer, &encoded_len_bytes)?;
        let mut writer = writer;
        let written = norito::codec::encode_adaptive_into(payload, &mut writer)
            .map_err(|error| io::Error::other(error.to_string()))?;
        if written != encoded_len {
            return Err(io::Error::other(
                "Musubi signing-hash length changed between passes",
            ));
        }
        Ok(())
    })
    .expect("Musubi signing hash writer is infallible");
    HashOf::from_untyped_unchecked(hash)
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
        MusubiNamespaceBindingDigestV1(domain_hash_value(
            MUSUBI_NAMESPACE_BINDING_DIGEST_DOMAIN_V1,
            self,
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiVersionComparatorV1 {
    /// Comparator operator.
    pub op: MusubiComparatorOpV1,
    /// Complete structured version.
    pub version: MusubiVersionV1,
}
/// Payload of a `MAJOR.MINOR.*` wildcard requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiMinorWildcardV1 {
    /// Required major component.
    pub major: u64,
    /// Required minor component.
    pub minor: u64,
}
/// Canonical Cargo-style version requirement AST.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
            #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
        ArchiveId(domain_hash_value(MUSUBI_ARCHIVE_ID_DOMAIN_V1, self))
    }
}
include!("musubi/bundle_file_decode.rs");
/// Typed descriptor parsed and verified by every provider before serving a bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(decode_from_slice)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    /// Decode one exact canonical artifact-descriptor bundle file under the shared V1 limits.
    ///
    /// # Errors
    ///
    /// Returns one stable payload-free error when the file is empty, oversized, malformed,
    /// trailing, noncanonical, or fails descriptor validation.
    pub fn decode_canonical_bundle_file(bytes: &[u8]) -> Result<Self, ParseError> {
        decode_canonical_bundle_file_v1(
            bytes,
            MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1,
            MUSUBI_ARTIFACT_DESCRIPTOR_DECODE_LIMITS_V1,
            Self::validate,
            "Musubi artifact descriptor bundle file is invalid or out of bounds",
        )
    }
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
/// Unlike [`MusubiArchiveRecordV1`], this projection deliberately excludes the mutable location
/// revision and current location identities. A finalized registration can therefore be revalidated
/// from a later exact archive read without requiring a historical copy of mutable registry state.
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    /// Returns an error if the pin-manifest digest, archive identity, or location identity is zero.
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
include!("musubi/replication_order_lifecycle.rs");
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
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiKotodamaEditionV1 {
    /// First-release Kotodama edition.
    V1,
}
/// Exact IVM ABI binding embedded in every release and lock node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiAbiBindingV1 {
    /// Must equal [`MUSUBI_IVM_ABI_VERSION_V1`].
    pub abi_version: u16,
    /// Canonical IVM ABI hash.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[norito(decode_from_slice)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    /// Decode one exact canonical verification-lock bundle file under the shared V1 limits.
    ///
    /// # Errors
    ///
    /// Returns one stable payload-free error when the file is empty, oversized, malformed,
    /// trailing, noncanonical, or fails verification-lock validation.
    pub fn decode_canonical_bundle_file(bytes: &[u8]) -> Result<Self, ParseError> {
        decode_canonical_bundle_file_v1(
            bytes,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            MUSUBI_VERIFICATION_LOCK_DECODE_LIMITS_V1,
            Self::validate,
            "Musubi verification lock bundle file is invalid or out of bounds",
        )
    }
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
        MusubiVerificationLockDigestV1(domain_hash_value(
            MUSUBI_VERIFICATION_LOCK_DIGEST_DOMAIN_V1,
            self,
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[norito(decode_from_slice)]
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
    /// Decode one exact canonical semantic-release bundle file under the shared V1 limits.
    ///
    /// # Errors
    ///
    /// Returns one stable payload-free error when the file is empty, oversized, malformed,
    /// trailing, noncanonical, or fails semantic-release validation.
    pub fn decode_canonical_bundle_file(bytes: &[u8]) -> Result<Self, ParseError> {
        decode_canonical_bundle_file_v1(
            bytes,
            MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
            MUSUBI_SEMANTIC_RELEASE_DECODE_LIMITS_V1,
            Self::validate,
            "Musubi semantic release bundle file is invalid or out of bounds",
        )
    }
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
        streaming::validate_semantic_release_fields(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            self.interface_digest,
            &self.metadata,
            self.verification_lock_digest,
        )
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
        streaming::validate_semantic_release_lock(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            &self.metadata,
            (self.interface_digest, self.verification_lock_digest),
            verification_lock,
        )
    }
    /// Domain-separated digest used inside bundles, staging receipts, and provider attestations.
    #[must_use]
    pub fn semantic_digest(&self) -> MusubiSemanticReleaseDigestV1 {
        MusubiSemanticReleaseDigestV1(domain_hash_value(
            MUSUBI_SEMANTIC_RELEASE_DIGEST_DOMAIN_V1,
            self,
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
        streaming::semantic_release_digest(self)
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
        streaming::validate_semantic_release_fields(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            self.interface_digest,
            &self.metadata,
            self.verification_lock_digest,
        )?;
        if self.archive_id.is_zero() {
            return Err(ParseError::new(
                "Musubi registry release manifest has an invalid archive identity",
            ));
        }
        Ok(())
    }
    fn validate_verification_lock(
        &self,
        verification_lock: &MusubiVerificationLockV1,
    ) -> Result<(), ParseError> {
        streaming::validate_semantic_release_lock(
            &self.release,
            &self.abi,
            &self.dependencies,
            &self.exports,
            &self.metadata,
            (self.interface_digest, self.verification_lock_digest),
            verification_lock,
        )
    }
    /// Domain-separated immutable release digest.
    #[must_use]
    pub fn release_digest(&self) -> MusubiReleaseDigestV1 {
        MusubiReleaseDigestV1(domain_hash_value(MUSUBI_RELEASE_DIGEST_DOMAIN_V1, self))
    }
}
/// Publication payload that binds a release to its independently validated exact proof.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
            .validate_verification_lock(&self.resolution.lock)
    }
}
/// Exact, replay-resistant request binding accepted by authenticated `SoraFS` seed ingress.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiSeedIngressReceiptBindingV1 {
    /// Exact deployment identity derived from the committed genesis header.
    pub network_id: NetworkId,
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
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; 32],
}
impl MusubiSeedIngressReceiptBindingV1 {
    /// Validate every exact deployment, actor, commitment, and anti-replay binding.
    ///
    /// # Errors
    ///
    /// Returns an error if an account identity is invalid, the exact network identity is
    /// malformed, a required identity, digest, or nonce is zero, or the CAR body length is outside
    /// its V1 bound.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.publisher)?;
        validate_musubi_account_id_v1(&self.ingress_broker)?;
        if self.network_id.as_bytes()[31] & 1 != 1
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
    /// Exact deployment identity derived from the committed genesis header.
    pub network_id: NetworkId,
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
    /// replication order, exact network identity, or required bundle commitment is invalid or
    /// inert.
    pub fn validate(&self) -> Result<(), ParseError> {
        validate_musubi_account_id_v1(&self.completed_by)?;
        validate_musubi_account_id_v1(&self.completion_authority.provider_owner)?;
        if self.network_id.as_bytes()[31] & 1 != 1
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
        let canonical_len = canonical_frame_len(self).map_err(|_| {
            ParseError::new("Musubi provider bundle attestation has no canonical Norito encoding")
        })?;
        if canonical_len == 0
            || canonical_len > MUSUBI_MAX_PROVIDER_BUNDLE_ATTESTATION_CANONICAL_BYTES_V1
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
        MusubiProviderBundleAttestationDigestV1(domain_hash_value(
            MUSUBI_PROVIDER_BUNDLE_ATTESTATION_DIGEST_DOMAIN_V1,
            self,
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MusubiNamespaceDelegationApprovalV1 {
    /// Controller key that produced the signature.
    pub public_key: PublicKey,
    /// Signature of [`MusubiNamespaceDelegationPayloadV1::signing_hash`].
    pub signature: SignatureOf<MusubiNamespaceDelegationPayloadV1>,
}
/// Generation-bound authority to claim an absent package in one namespace.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiPackageRoleV1 {
    /// Package owner with governance authority.
    Owner,
    /// Maintainer with explicitly independent permissions.
    Maintainer(MusubiMaintainerPermissionsV1),
}
/// Canonical package-local ordered key for an accepted member.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    #[cfg_attr(feature = "json", norito(json = "streaming::account_i105_json"))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", deny_unknown_fields)
)]
pub enum MusubiAliasHistoryActionV1 {
    /// Initial paid registration.
    Registered,
    /// Parliament recovery retarget; normal owners cannot retarget.
    ParliamentRetarget,
}
/// Canonical permanent-alias history key ordered by alias and revision.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
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
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
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
        MusubiGovernanceActionDigestV1(domain_hash_value(
            b"iroha.musubi.parliament-action.v1",
            self,
        ))
    }
}
include!("musubi/registry_policy_types.rs");
include!("musubi/registry_policy_impl.rs");
include!("musubi/query_models.rs");
#[cfg(test)]
mod tests {
    include!("musubi_tests.rs");
    include!("musubi/registry_query_tests.rs");
}

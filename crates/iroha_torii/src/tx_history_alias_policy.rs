//! Bounded startup loader for transaction-history mandatory-alias policy files.
use crate::secure_file_metadata::{self, SecureMetadata};
use iroha_data_model::{
    alias_setup::AccountAliasName, name::MAX_NAME_BYTES, nexus::DataSpaceCatalog,
};
use norito::{
    DecodeLimits,
    json::{JsonPreflightLimits, JsonPreflightProfile, Parser, preflight_slice},
};
#[cfg(unix)]
use std::fs::OpenOptions;
use std::{
    alloc::{Layout, alloc},
    fmt,
    fs::{self, File},
    io::{self, Read as _},
    path::Path,
    str::FromStr as _,
    sync::Arc,
};
#[cfg(test)]
use std::{path::PathBuf, sync::Mutex};
const MAX_ACCOUNT_ALIAS_LITERAL_BYTES: usize = 3 * MAX_NAME_BYTES + 2;
const JSON_VALUE_DEPTH: usize = 3;
/// Canonical mandatory aliases retained in one exact, sorted allocation.
///
/// A canonical alias embeds its dataspace, so retaining a second map/set graph
/// would duplicate both identity text and allocator overhead. Cloning the
/// startup policy shares this immutable allocation.
#[derive(Clone, Default)]
pub(crate) struct MandatoryAliasPolicy(Arc<Box<[Box<str>]>>);
impl MandatoryAliasPolicy {
    pub(crate) fn contains(&self, dataspace: &str, alias: &str) -> bool {
        let Some((_, scope)) = alias.rsplit_once('@') else {
            return false;
        };
        let alias_dataspace = scope.rsplit_once('.').map_or(scope, |(_, value)| value);
        alias_dataspace == dataspace
            && self
                .0
                .binary_search_by(|candidate| candidate.as_ref().cmp(alias))
                .is_ok()
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        self.0.len()
    }
}
#[derive(Debug)]
pub(crate) enum PolicyLoadError {
    Io(io::Error),
    Invalid(&'static str),
}
impl fmt::Display for PolicyLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}
impl From<io::Error> for PolicyLoadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl std::error::Error for PolicyLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}
/// Load the configured mandatory-alias policy under its complete startup envelope.
///
/// The file is opened without following its final path component and must keep the same identity,
/// type, length, and metadata through the read. JSON is lexically admitted before a streaming typed
/// parser reserves exact flat arrays, so no recursive `Value` graph or collect-before-cap
/// representation is constructed.
pub(crate) fn load_mandatory_alias_policy(
    path: &Path,
    catalog: &DataSpaceCatalog,
    maximum_file_bytes: usize,
) -> Result<MandatoryAliasPolicy, PolicyLoadError> {
    try_load_mandatory_alias_policy(path, catalog, maximum_file_bytes)
}
fn try_load_mandatory_alias_policy(
    path: &Path,
    catalog: &DataSpaceCatalog,
    maximum_file_bytes: usize,
) -> Result<MandatoryAliasPolicy, PolicyLoadError> {
    let bytes = read_exact_stable_policy_file(path, maximum_file_bytes)?;
    parse_mandatory_alias_policy(&bytes, catalog, maximum_file_bytes)
}
fn policy_decode_limits(maximum_file_bytes: usize) -> Result<DecodeLimits, PolicyLoadError> {
    if !(1..=iroha_config::parameters::defaults::torii::tx_history::
        MANDATORY_ALIASES_MAX_FILE_BYTES_V1)
        .contains(&maximum_file_bytes)
    {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file limit is outside the first-release corridor",
        ));
    }
    let phase_units =
        iroha_config::parameters::defaults::torii::tx_history::MANDATORY_ALIASES_MEMORY_PHASE_UNITS;
    let decode_units = phase_units.checked_sub(1).ok_or(PolicyLoadError::Invalid(
        "invalid alias-policy memory geometry",
    ))?;
    let maximum_allocated_bytes =
        maximum_file_bytes
            .checked_mul(decode_units)
            .ok_or(PolicyLoadError::Invalid(
                "alias-policy memory geometry exceeds the host address space",
            ))?;
    Ok(DecodeLimits::new(
        maximum_file_bytes,
        maximum_file_bytes,
        maximum_file_bytes,
        maximum_allocated_bytes,
        JSON_VALUE_DEPTH,
    ))
}
fn parse_mandatory_alias_policy(
    bytes: &[u8],
    catalog: &DataSpaceCatalog,
    maximum_file_bytes: usize,
) -> Result<MandatoryAliasPolicy, PolicyLoadError> {
    if bytes.len() > maximum_file_bytes {
        return Err(PolicyLoadError::Invalid(
            "alias-policy document exceeds its configured byte limit",
        ));
    }
    let source = std::str::from_utf8(bytes)
        .map_err(|_| PolicyLoadError::Invalid("alias-policy document is not valid UTF-8"))?;
    let limits = policy_decode_limits(maximum_file_bytes)?;
    let profile = preflight_slice(
        bytes,
        JsonPreflightLimits::from_decode_limits(maximum_file_bytes, limits),
    )
    .map_err(|_| PolicyLoadError::Invalid("alias-policy JSON failed lexical admission"))?;
    let (result, _usage) = norito::core::with_decode_limits_measured(limits, || {
        parse_mandatory_alias_policy_inner(source, catalog, profile)
    });
    result
}
fn parse_mandatory_alias_policy_inner(
    source: &str,
    catalog: &DataSpaceCatalog,
    profile: JsonPreflightProfile,
) -> Result<MandatoryAliasPolicy, PolicyLoadError> {
    let mut parser = Parser::new(source);
    let entry_count = parser
        .preflight_object_entries()
        .map_err(|_| PolicyLoadError::Invalid("alias-policy root must be a JSON object"))?;
    if entry_count != profile.root_container_entries() {
        return Err(PolicyLoadError::Invalid(
            "alias-policy root entry count changed after admission",
        ));
    }
    let mut dataspaces = ExactBoxBuilder::<Box<str>>::new(entry_count)?;
    let mut aliases = ExactBoxBuilder::<Box<str>>::new(profile.array_entries())?;
    parser
        .expect(b'{')
        .map_err(|_| PolicyLoadError::Invalid("alias-policy root must be a JSON object"))?;
    for index in 0..entry_count {
        if index != 0 {
            parser
                .expect(b',')
                .map_err(|_| PolicyLoadError::Invalid("alias-policy object is malformed"))?;
        }
        let dataspace = parse_exact_json_string(&mut parser, MAX_NAME_BYTES)
            .map_err(|_| PolicyLoadError::Invalid("alias-policy object key is invalid"))?;
        parser
            .expect(b':')
            .map_err(|_| PolicyLoadError::Invalid("alias-policy object is malformed"))?;
        validate_dataspace_key(&dataspace, catalog)?;
        parse_alias_array(&mut parser, &dataspace, &mut aliases)
            .map_err(|_| PolicyLoadError::Invalid("alias-policy array is invalid"))?;
        dataspaces.push(dataspace)?;
    }
    parser
        .expect(b'}')
        .map_err(|_| PolicyLoadError::Invalid("alias-policy object is malformed"))?;
    parser.skip_ws();
    if !parser.eof() {
        return Err(PolicyLoadError::Invalid(
            "alias-policy document contains trailing data",
        ));
    }
    let mut dataspaces = dataspaces.finish()?;
    dataspaces.sort_unstable();
    if dataspaces
        .windows(2)
        .any(|pair| pair[0].as_ref() == pair[1].as_ref())
    {
        return Err(PolicyLoadError::Invalid(
            "alias-policy contains a duplicate dataspace key",
        ));
    }
    let mut aliases = aliases.finish()?;
    aliases.sort_unstable();
    if aliases
        .windows(2)
        .any(|pair| pair[0].as_ref() == pair[1].as_ref())
    {
        return Err(PolicyLoadError::Invalid(
            "alias-policy contains a duplicate alias",
        ));
    }
    Ok(MandatoryAliasPolicy(Arc::new(aliases)))
}
fn validate_dataspace_key(
    dataspace: &str,
    catalog: &DataSpaceCatalog,
) -> Result<(), PolicyLoadError> {
    if dataspace.is_empty()
        || dataspace.trim() != dataspace
        || dataspace.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(PolicyLoadError::Invalid(
            "alias-policy dataspace keys must be canonical",
        ));
    }
    if catalog.by_alias(dataspace).is_none() {
        return Err(PolicyLoadError::Invalid(
            "alias-policy references an unknown dataspace",
        ));
    }
    Ok(())
}
fn parse_alias_array(
    parser: &mut Parser<'_>,
    dataspace: &str,
    aliases: &mut ExactBoxBuilder<Box<str>>,
) -> Result<(), norito::json::Error> {
    let entry_count = parser.preflight_array_entries()?;
    parser.expect(b'[')?;
    for index in 0..entry_count {
        if index != 0 {
            parser.expect(b',')?;
        }
        let alias = parse_exact_json_string(parser, MAX_ACCOUNT_ALIAS_LITERAL_BYTES)?;
        validate_alias_literal(&alias, dataspace)
            .map_err(|message| norito::json::Error::Message(message.to_owned()))?;
        aliases
            .push(alias)
            .map_err(|_| norito::json::Error::AllocationFailed)?;
    }
    parser.expect(b']')?;
    Ok(())
}
struct ExactBoxBuilder<T> {
    storage: Box<[std::mem::MaybeUninit<T>]>,
    initialized: usize,
}
impl<T> ExactBoxBuilder<T> {
    fn new(length: usize) -> Result<Self, PolicyLoadError> {
        let allocation_bytes =
            length
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(PolicyLoadError::Invalid(
                    "alias-policy exact array size overflow",
                ))?;
        norito::core::reserve_decode_allocation(allocation_bytes)
            .map_err(|_| PolicyLoadError::Invalid("alias-policy allocation budget exceeded"))?;
        Ok(Self {
            storage: allocate_exact_uninit(length)?,
            initialized: 0,
        })
    }
    fn push(&mut self, value: T) -> Result<(), PolicyLoadError> {
        let slot = self
            .storage
            .get_mut(self.initialized)
            .ok_or(PolicyLoadError::Invalid(
                "alias-policy array entry count changed after admission",
            ))?;
        slot.write(value);
        self.initialized += 1;
        Ok(())
    }
    #[allow(unsafe_code)]
    fn finish(mut self) -> Result<Box<[T]>, PolicyLoadError> {
        if self.initialized != self.storage.len() {
            return Err(PolicyLoadError::Invalid(
                "alias-policy array entry count changed after admission",
            ));
        }
        let raw = Box::into_raw(std::mem::take(&mut self.storage)) as *mut [T];
        self.initialized = 0;
        // SAFETY: every element was initialized by `push`, the allocation is
        // exact for `[T]`, and ownership was removed from `self` above.
        Ok(unsafe { Box::from_raw(raw) })
    }
}
impl<T> Drop for ExactBoxBuilder<T> {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        for value in &mut self.storage[..self.initialized] {
            // SAFETY: exactly this prefix was initialized by successful calls
            // to `push`; all remaining elements stay `MaybeUninit`.
            unsafe { value.assume_init_drop() };
        }
    }
}
#[allow(unsafe_code)]
fn parse_exact_json_string(
    parser: &mut Parser<'_>,
    maximum_decoded_bytes: usize,
) -> Result<Box<str>, norito::json::Error> {
    let raw = parser.raw_value_slice()?;
    let mut counter = Parser::new(raw);
    let decoded_bytes = counter.skip_string_bounded(maximum_decoded_bytes)?;
    counter.skip_ws();
    if !counter.eof() {
        return Err(norito::json::Error::Message(
            "expected one JSON string".to_owned(),
        ));
    }
    norito::core::reserve_decode_allocation(decoded_bytes)
        .map_err(norito::json::Error::from_decode_resource)?;
    let mut storage =
        allocate_exact_uninit(decoded_bytes).map_err(|_| norito::json::Error::AllocationFailed)?;
    decode_json_string_into(raw, &mut storage)?;
    // SAFETY: `decode_json_string_into` succeeds only after initializing the
    // complete exact destination.
    let bytes = unsafe { storage.assume_init() };
    std::str::from_utf8(&bytes).map_err(|_| norito::json::Error::InvalidUtf8)?;
    // SAFETY: `[u8]` and `str` have identical pointer metadata and allocation
    // layout. UTF-8 was validated immediately above, so ownership can transfer
    // without a spare-capacity allocation.
    Ok(unsafe { Box::from_raw(Box::into_raw(bytes) as *mut str) })
}
fn decode_json_string_into(
    raw: &str,
    output: &mut [std::mem::MaybeUninit<u8>],
) -> Result<(), norito::json::Error> {
    let source = raw.as_bytes();
    if source.first() != Some(&b'"') || source.last() != Some(&b'"') {
        return Err(norito::json::Error::Message(
            "expected one JSON string".to_owned(),
        ));
    }
    let mut input = 1usize;
    let end = source.len() - 1;
    let mut written = 0usize;
    while input < end {
        if source[input] != b'\\' {
            let start = input;
            while input < end && source[input] != b'\\' {
                input += 1;
            }
            write_exact_bytes(output, &mut written, &source[start..input])?;
            continue;
        }
        input += 1;
        let escape = *source
            .get(input)
            .ok_or_else(|| norito::json::Error::Message("unterminated JSON escape".to_owned()))?;
        input += 1;
        match escape {
            b'"' | b'\\' | b'/' => write_exact_bytes(output, &mut written, &[escape])?,
            b'b' => write_exact_bytes(output, &mut written, &[0x08])?,
            b'f' => write_exact_bytes(output, &mut written, &[0x0c])?,
            b'n' => write_exact_bytes(output, &mut written, b"\n")?,
            b'r' => write_exact_bytes(output, &mut written, b"\r")?,
            b't' => write_exact_bytes(output, &mut written, b"\t")?,
            b'u' => {
                let high = decode_json_hex_quad(source, &mut input)?;
                let scalar = if (0xd800..=0xdbff).contains(&high) {
                    let pair_end = input.checked_add(2).ok_or_else(|| {
                        norito::json::Error::Message("invalid JSON surrogate pair".to_owned())
                    })?;
                    if source.get(input..pair_end) != Some(br"\u") {
                        return Err(norito::json::Error::Message(
                            "expected JSON low surrogate".to_owned(),
                        ));
                    }
                    input = pair_end;
                    let low = decode_json_hex_quad(source, &mut input)?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err(norito::json::Error::Message(
                            "invalid JSON low surrogate".to_owned(),
                        ));
                    }
                    0x1_0000 + (((high - 0xd800) << 10) | (low - 0xdc00))
                } else if (0xdc00..=0xdfff).contains(&high) {
                    return Err(norito::json::Error::Message(
                        "unexpected JSON low surrogate".to_owned(),
                    ));
                } else {
                    high
                };
                let character = char::from_u32(scalar).ok_or_else(|| {
                    norito::json::Error::Message("invalid JSON Unicode scalar".to_owned())
                })?;
                let mut encoded = [0_u8; 4];
                write_exact_bytes(
                    output,
                    &mut written,
                    character.encode_utf8(&mut encoded).as_bytes(),
                )?;
            }
            _ => {
                return Err(norito::json::Error::Message(
                    "invalid JSON escape".to_owned(),
                ));
            }
        }
    }
    if written != output.len() {
        return Err(norito::json::Error::Message(
            "JSON string length changed between passes".to_owned(),
        ));
    }
    Ok(())
}
fn write_exact_bytes(
    output: &mut [std::mem::MaybeUninit<u8>],
    written: &mut usize,
    bytes: &[u8],
) -> Result<(), norito::json::Error> {
    let end = written
        .checked_add(bytes.len())
        .ok_or(norito::json::Error::AllocationFailed)?;
    let destination = output.get_mut(*written..end).ok_or_else(|| {
        norito::json::Error::Message("JSON string length changed between passes".to_owned())
    })?;
    for (slot, byte) in destination.iter_mut().zip(bytes) {
        slot.write(*byte);
    }
    *written = end;
    Ok(())
}
fn decode_json_hex_quad(source: &[u8], input: &mut usize) -> Result<u32, norito::json::Error> {
    let mut value = 0_u32;
    for _ in 0..4 {
        let byte = *source.get(*input).ok_or_else(|| {
            norito::json::Error::Message("unterminated JSON Unicode escape".to_owned())
        })?;
        *input += 1;
        value = (value << 4)
            | match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' => u32::from(byte - b'a' + 10),
                b'A'..=b'F' => u32::from(byte - b'A' + 10),
                _ => {
                    return Err(norito::json::Error::Message(
                        "invalid JSON Unicode escape".to_owned(),
                    ));
                }
            };
    }
    Ok(value)
}
fn validate_alias_literal(alias: &str, dataspace: &str) -> Result<(), &'static str> {
    if alias.len() > MAX_ACCOUNT_ALIAS_LITERAL_BYTES {
        return Err("alias-policy alias exceeds the canonical literal limit");
    }
    let parsed = AccountAliasName::from_str(alias).map_err(|_| "alias-policy alias is invalid")?;
    let (label, scope) = alias
        .split_once('@')
        .ok_or("alias-policy alias is invalid")?;
    let (domain, literal_dataspace) = scope
        .split_once('.')
        .map_or((None, scope), |(domain, dataspace)| {
            (Some(domain), dataspace)
        });
    if parsed.label.as_ref() != label
        || parsed.domain.as_ref().map(|value| value.as_ref()) != domain
        || parsed.dataspace.as_ref() != literal_dataspace
        || literal_dataspace != dataspace
    {
        return Err("alias-policy aliases must be canonical and match their dataspace key");
    }
    Ok(())
}
#[allow(unsafe_code)]
fn read_exact_stable_policy_file(
    path: &Path,
    maximum: usize,
) -> Result<Box<[u8]>, PolicyLoadError> {
    if maximum == 0 {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file limit must be non-zero",
        ));
    }
    let named_before = secure_file_metadata::from_path(path)?;
    validate_direct_regular_file(&named_before)?;
    if named_before.len() > u64::try_from(maximum).unwrap_or(u64::MAX) {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file exceeds its configured byte limit",
        ));
    }
    #[cfg(test)]
    replace_policy_file_for_test(path)?;
    let mut file = open_direct_regular_file(path)?;
    let opened_before = secure_file_metadata::from_file(&file)?;
    validate_direct_regular_file(&opened_before)?;
    if !secure_file_metadata::unchanged(&named_before, &opened_before) {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file changed identity while opening",
        ));
    }
    let length = usize::try_from(opened_before.len())
        .map_err(|_| PolicyLoadError::Invalid("alias-policy file length does not fit this host"))?;
    if length > maximum {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file exceeds its configured byte limit",
        ));
    }
    let mut storage = allocate_exact_uninit(length)?;
    for byte in &mut storage {
        byte.write(0);
    }
    // SAFETY: every byte in the exact boxed allocation was initialized above.
    let mut bytes = unsafe { storage.assume_init() };
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            PolicyLoadError::Invalid("alias-policy file changed length while reading")
        } else {
            PolicyLoadError::Io(error)
        }
    })?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file grew while reading or exceeds its configured byte limit",
        ));
    }
    let opened_after = secure_file_metadata::from_file(&file)?;
    let named_after = secure_file_metadata::from_path(path)?;
    validate_direct_regular_file(&opened_after)?;
    validate_direct_regular_file(&named_after)?;
    if !secure_file_metadata::unchanged(&opened_before, &opened_after)
        || !secure_file_metadata::unchanged(&opened_after, &named_after)
    {
        return Err(PolicyLoadError::Invalid(
            "alias-policy file changed while reading",
        ));
    }
    Ok(bytes)
}
#[allow(unsafe_code)]
fn allocate_exact_uninit<T>(
    length: usize,
) -> Result<Box<[std::mem::MaybeUninit<T>]>, PolicyLoadError> {
    if length == 0 || std::mem::size_of::<T>() == 0 {
        let mut empty_or_zst = Vec::with_capacity(length);
        // SAFETY: `MaybeUninit<T>` may hold an uninitialized `T`; a zero-sized
        // element requires no backing allocation.
        unsafe { empty_or_zst.set_len(length) };
        return Ok(empty_or_zst.into_boxed_slice());
    }
    let layout = Layout::array::<std::mem::MaybeUninit<T>>(length)
        .map_err(|_| PolicyLoadError::Invalid("alias-policy allocation layout overflow"))?;
    // SAFETY: `layout` is a valid non-zero byte layout. A null allocation is
    // reported before ownership is constructed.
    let allocation = unsafe { alloc(layout) }.cast::<std::mem::MaybeUninit<T>>();
    if allocation.is_null() {
        return Err(PolicyLoadError::Invalid("alias-policy allocation failed"));
    }
    let slice = std::ptr::slice_from_raw_parts_mut(allocation, length);
    // SAFETY: the pointer owns exactly `layout`, whose element count and
    // alignment match this boxed `MaybeUninit<T>` slice.
    Ok(unsafe { Box::from_raw(slice) })
}
fn validate_direct_regular_file(metadata: &SecureMetadata) -> Result<(), PolicyLoadError> {
    if !secure_file_metadata::is_direct_file(metadata)
        || secure_file_metadata::number_of_links(metadata) != Some(1)
    {
        return Err(PolicyLoadError::Invalid(
            "alias-policy input must be a direct single-link regular file",
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn open_direct_regular_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(
        (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOCTTY)
            .bits() as i32,
    );
    options.open(path)
}
#[cfg(windows)]
fn open_direct_regular_file(path: &Path) -> io::Result<File> {
    secure_file_metadata::open_direct_file(path)
}
#[cfg(not(any(unix, windows)))]
fn open_direct_regular_file(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "stable direct-file opens are unavailable on this platform",
    ))
}
#[cfg(test)]
static POLICY_FILE_REPLACEMENT: Mutex<Option<(PathBuf, PathBuf)>> = Mutex::new(None);
#[cfg(test)]
fn replace_policy_file_for_test(path: &Path) -> io::Result<()> {
    let replacement = {
        let mut hook = POLICY_FILE_REPLACEMENT
            .lock()
            .expect("alias-policy replacement hook lock");
        if hook.as_ref().is_some_and(|(expected, _)| expected == path) {
            hook.take().map(|(_, replacement)| replacement)
        } else {
            None
        }
    };
    if let Some(replacement) = replacement {
        fs::rename(replacement, path)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::nexus::{DataSpaceId, DataSpaceMetadata};
    fn catalog() -> DataSpaceCatalog {
        DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "retail".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("test dataspace catalog")
    }
    #[test]
    fn streaming_policy_parser_preserves_canonical_aliases() {
        let document =
            br#"{"retail":["merchant@retail","treasury@banking.retail"],"universal":[]}"#;
        let parsed = parse_mandatory_alias_policy(document, &catalog(), document.len())
            .expect("canonical policy");
        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains("retail", "merchant@retail"));
        assert!(parsed.contains("retail", "treasury@banking.retail"));
        assert!(!parsed.contains("universal", "merchant@retail"));
    }
    #[test]
    fn streaming_policy_parser_decodes_json_escapes_into_exact_strings() {
        let document = br#"{"retail":["merch\u0061nt@retail","treas\u0075ry@retail"]}"#;
        let parsed = parse_mandatory_alias_policy(document, &catalog(), document.len())
            .expect("escaped canonical policy");
        assert!(parsed.contains("retail", "merchant@retail"));
        assert!(parsed.contains("retail", "treasury@retail"));
        let mut parser = Parser::new(r#""\ud83d\ude00""#);
        assert_eq!(
            &*parse_exact_json_string(&mut parser, 4).expect("surrogate pair"),
            "😀"
        );
    }
    #[test]
    fn streaming_policy_parser_rejects_noncanonical_duplicate_and_cross_dataspace_aliases() {
        for document in [
            br#"{"Retail":[]}"#.as_slice(),
            br#"{"retail":["Merchant@retail"]}"#.as_slice(),
            br#"{"retail":["merchant@universal"]}"#.as_slice(),
            br#"{"retail":["merchant@retail","merchant@retail"]}"#.as_slice(),
            br#"{"retail":[],"retail":[]}"#.as_slice(),
        ] {
            assert!(
                parse_mandatory_alias_policy(document, &catalog(), document.len()).is_err(),
                "document must fail: {}",
                String::from_utf8_lossy(document)
            );
        }
    }
    #[test]
    fn policy_file_reader_accepts_exact_limit_and_rejects_plus_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("aliases.json");
        fs::write(&path, b"{}").expect("write exact policy");
        assert_eq!(
            &*read_exact_stable_policy_file(&path, 2).expect("exact file"),
            b"{}"
        );
        assert!(read_exact_stable_policy_file(&path, 1).is_err());
    }
    #[test]
    fn policy_loader_returns_missing_and_malformed_file_errors_without_unwinding() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let missing = directory.path().join("missing.json");
        let missing_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            load_mandatory_alias_policy(&missing, &catalog(), 1024)
        }))
        .expect("missing policy must return an error instead of unwinding");
        assert!(missing_result.is_err());

        let malformed = directory.path().join("malformed.json");
        fs::write(&malformed, b"[").expect("write malformed policy");
        let malformed_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            load_mandatory_alias_policy(&malformed, &catalog(), 1024)
        }))
        .expect("malformed policy must return an error instead of unwinding");
        assert!(malformed_result.is_err());
    }
    #[test]
    fn policy_file_reader_rejects_path_replacement() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("aliases.json");
        let replacement = directory.path().join("replacement.json");
        fs::write(&path, b"{}").expect("write original policy");
        fs::write(&replacement, b"[]").expect("write replacement policy");
        *POLICY_FILE_REPLACEMENT
            .lock()
            .expect("alias-policy replacement hook lock") = Some((path.clone(), replacement));
        assert!(read_exact_stable_policy_file(&path, 2).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn policy_file_reader_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("target.json");
        let link = directory.path().join("aliases.json");
        fs::write(&target, b"{}").expect("write target policy");
        symlink(&target, &link).expect("create policy symlink");
        assert!(read_exact_stable_policy_file(&link, 2).is_err());
    }
    #[test]
    fn policy_memory_geometry_has_exact_boundaries() {
        let maximum = 1024;
        let limits = policy_decode_limits(maximum).expect("valid geometry");
        assert_eq!(
            limits.max_total_allocated_bytes(),
            maximum
                * (iroha_config::parameters::defaults::torii::tx_history::
                    MANDATORY_ALIASES_MEMORY_PHASE_UNITS
                    - 1)
        );
        let hard_max = iroha_config::parameters::defaults::torii::tx_history::
            MANDATORY_ALIASES_MAX_FILE_BYTES_V1;
        assert!(policy_decode_limits(hard_max).is_ok());
        assert!(policy_decode_limits(hard_max + 1).is_err());
    }
    #[test]
    fn exact_array_geometry_fits_the_retained_decode_units() {
        let raw_bytes = 1024usize;
        let root_slots = raw_bytes / 5;
        let alias_slots = raw_bytes / 3;
        let exact_arrays = root_slots
            .checked_mul(std::mem::size_of::<Box<str>>())
            .and_then(|bytes| {
                alias_slots
                    .checked_mul(std::mem::size_of::<Box<str>>())
                    .and_then(|aliases| bytes.checked_add(aliases))
            })
            .and_then(|bytes| bytes.checked_add(raw_bytes))
            .expect("geometry fits");
        let decode_units =
            iroha_config::parameters::defaults::torii::tx_history::
                MANDATORY_ALIASES_MEMORY_PHASE_UNITS
                - 1;
        assert!(exact_arrays <= raw_bytes * decode_units);
    }
}

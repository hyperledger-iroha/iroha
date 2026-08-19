//! Bounded durable state storage for development and test hosts.
use crate::VMError;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64_STANDARD};
use iroha_data_model::state_path::StatePath;
use norito::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read as _},
    ops::Bound,
    path::{Path, PathBuf},
};
/// Maximum number of entries retained by one first-release development overlay.
pub(crate) const STATE_OVERLAY_MAX_ENTRIES_V1: usize = 4_096;
/// Maximum aggregate UTF-8 bytes retained in overlay paths.
pub(crate) const STATE_OVERLAY_MAX_PATH_BYTES_V1: usize = 4 * 1024 * 1024;
/// Maximum aggregate raw value bytes retained by one overlay.
pub(crate) const STATE_OVERLAY_MAX_VALUE_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum encoded size of one persisted overlay file.
pub(crate) const STATE_OVERLAY_MAX_FILE_BYTES_V1: usize = 30 * 1024 * 1024;
const STATE_OVERLAY_MAX_BASE64_VALUE_BYTES_V1: usize =
    crate::syscalls::STATE_MAX_VALUE_BYTES.div_ceil(3) * 4;
const STATE_OVERLAY_MAX_RAW_JSON_PATH_BYTES_V1: usize = crate::syscalls::STATE_MAX_PATH_BYTES * 6;
const STATE_OVERLAY_MAX_RAW_JSON_VALUE_BYTES_V1: usize =
    STATE_OVERLAY_MAX_BASE64_VALUE_BYTES_V1 * 6;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const STATE_OVERLAY_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const STATE_OVERLAY_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const STATE_OVERLAY_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("durable state overlay loading requires a defined no-follow flag on this target");
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OverlayResourceUsage {
    path_bytes: usize,
    value_bytes: usize,
}
/// Durable host-backed state overlay used by dev/test hosts.
///
/// Values are stored as raw `NoritoBytes` payloads keyed by a canonical state
/// path. Pointer-ABI envelopes exist only at the guest/host boundary.
/// When a persistence path is provided, the overlay writes a Norito JSON map
/// `{ path: base64(raw_payload_bytes) }` so test runs can survive VM restarts.
#[derive(Clone, Debug, Default)]
pub struct DurableStateOverlay {
    persist_path: Option<PathBuf>,
    data: BTreeMap<StatePath, Vec<u8>>,
    path_bytes: usize,
    value_bytes: usize,
}
/// Snapshot of a durable state overlay.
#[derive(Clone, Debug, Default)]
pub struct DurableStateSnapshot {
    data: BTreeMap<StatePath, Vec<u8>>,
}
impl DurableStateOverlay {
    fn validate_entry(path: &StatePath, value: &[u8]) -> Result<(), VMError> {
        crate::host::validate_state_path(path)?;
        crate::host::validate_state_value_payload_len(value.len())
    }
    fn ensure_resource_limits(
        entry_count: usize,
        path_bytes: usize,
        value_bytes: usize,
    ) -> Result<(), VMError> {
        if entry_count > STATE_OVERLAY_MAX_ENTRIES_V1
            || path_bytes > STATE_OVERLAY_MAX_PATH_BYTES_V1
            || value_bytes > STATE_OVERLAY_MAX_VALUE_BYTES_V1
        {
            return Err(VMError::NoritoInvalid);
        }
        Ok(())
    }
    fn validate_collection(
        data: &BTreeMap<StatePath, Vec<u8>>,
    ) -> Result<OverlayResourceUsage, VMError> {
        Self::ensure_resource_limits(data.len(), 0, 0)?;
        let mut usage = OverlayResourceUsage::default();
        for (path, value) in data {
            Self::validate_entry(path, value)?;
            usage.path_bytes = usage
                .path_bytes
                .checked_add(path.as_ref().len())
                .ok_or(VMError::NoritoInvalid)?;
            usage.value_bytes = usage
                .value_bytes
                .checked_add(value.len())
                .ok_or(VMError::NoritoInvalid)?;
            Self::ensure_resource_limits(data.len(), usage.path_bytes, usage.value_bytes)?;
        }
        Ok(usage)
    }
    /// Create an in-memory overlay with no persistence.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            persist_path: None,
            data: BTreeMap::new(),
            path_bytes: 0,
            value_bytes: 0,
        }
    }
    /// Create an overlay that persists to the given path.
    pub fn with_persist_path(path: PathBuf) -> Result<Self, VMError> {
        let mut overlay = Self {
            persist_path: Some(path),
            data: BTreeMap::new(),
            path_bytes: 0,
            value_bytes: 0,
        };
        overlay.reload_from_disk()?;
        Ok(overlay)
    }
    /// Return a copy of the raw `NoritoBytes` payload for a path, if present.
    pub fn get(&self, path: &StatePath) -> Option<Vec<u8>> {
        self.data.get(path).cloned()
    }
    /// Borrow a stored raw payload without materializing a copy.
    ///
    /// Stateful syscall handlers use this to inspect the response length and
    /// prove affordability before allocating or copying guest-visible output.
    pub fn get_ref(&self, path: &StatePath) -> Option<&[u8]> {
        self.data.get(path).map(Vec::as_slice)
    }
    /// Borrow a validated state-value payload.
    ///
    /// `set`, `restore`, and persisted-state loading enforce the payload-size
    /// bound before the value enters `data`. `STATE_LEN` can therefore read the
    /// stored vector length in constant time, independent of value size.
    pub(crate) fn value_payload_ref(&self, path: &StatePath) -> Result<Option<&[u8]>, VMError> {
        Ok(self.get_ref(path))
    }
    /// Insert or replace the raw payload for the provided path.
    pub fn set(&mut self, path: &StatePath, value: Vec<u8>) -> Result<(), VMError> {
        Self::validate_entry(path, &value)?;
        let previous_value_len = self.data.get(path).map_or(0, Vec::len);
        let is_new = !self.data.contains_key(path);
        let next_entry_count = self.data.len().saturating_add(if is_new { 1 } else { 0 });
        let next_path_bytes = if is_new {
            self.path_bytes
                .checked_add(path.as_ref().len())
                .ok_or(VMError::NoritoInvalid)?
        } else {
            self.path_bytes
        };
        let next_value_bytes = self
            .value_bytes
            .checked_sub(previous_value_len)
            .and_then(|bytes| bytes.checked_add(value.len()))
            .ok_or(VMError::NoritoInvalid)?;
        Self::ensure_resource_limits(next_entry_count, next_path_bytes, next_value_bytes)?;
        let key = path.clone();
        let prev = self.data.insert(key.clone(), value);
        let previous_usage = OverlayResourceUsage {
            path_bytes: self.path_bytes,
            value_bytes: self.value_bytes,
        };
        self.path_bytes = next_path_bytes;
        self.value_bytes = next_value_bytes;
        if let Err(err) = self.flush() {
            match prev {
                Some(old) => {
                    self.data.insert(key, old);
                }
                None => {
                    self.data.remove(&key);
                }
            }
            self.path_bytes = previous_usage.path_bytes;
            self.value_bytes = previous_usage.value_bytes;
            return Err(err);
        }
        Ok(())
    }
    /// Iterate over the stored state paths.
    pub fn keys(&self) -> impl Iterator<Item = &StatePath> {
        self.data.keys()
    }
    /// Visit only keys in the ordered range sharing `prefix` as text.
    ///
    /// Callers apply any stricter path-segment rule. Starting at the ordered
    /// lower bound prevents an attacker-selected state prefix from charging or
    /// examining unrelated keys that sort before it.
    pub fn keys_with_text_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = &'a StatePath> + 'a {
        self.data
            .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
            .map(|(key, _)| key)
            .take_while(move |key| (*key).as_ref().starts_with(prefix))
    }
    /// Delete the raw payload for the provided path.
    pub fn del(&mut self, path: &StatePath) -> Result<(), VMError> {
        let previous_usage = OverlayResourceUsage {
            path_bytes: self.path_bytes,
            value_bytes: self.value_bytes,
        };
        let next_usage = if let Some(value) = self.data.get(path) {
            OverlayResourceUsage {
                path_bytes: self
                    .path_bytes
                    .checked_sub(path.as_ref().len())
                    .ok_or(VMError::NoritoInvalid)?,
                value_bytes: self
                    .value_bytes
                    .checked_sub(value.len())
                    .ok_or(VMError::NoritoInvalid)?,
            }
        } else {
            previous_usage
        };
        let prev = self.data.remove(path);
        self.path_bytes = next_usage.path_bytes;
        self.value_bytes = next_usage.value_bytes;
        if let Err(err) = self.flush() {
            if let Some(old) = prev {
                self.data.insert(path.clone(), old);
            }
            self.path_bytes = previous_usage.path_bytes;
            self.value_bytes = previous_usage.value_bytes;
            return Err(err);
        }
        Ok(())
    }
    /// Take a snapshot of the current overlay contents.
    #[must_use]
    pub fn checkpoint(&self) -> DurableStateSnapshot {
        DurableStateSnapshot {
            data: self.data.clone(),
        }
    }
    /// Restore from a previously taken snapshot and persist to disk if needed.
    pub fn restore(&mut self, snapshot: &DurableStateSnapshot) -> Result<(), VMError> {
        let usage = Self::validate_collection(&snapshot.data)?;
        let replacement = snapshot.data.clone();
        let previous_data = std::mem::replace(&mut self.data, replacement);
        let previous_usage = OverlayResourceUsage {
            path_bytes: std::mem::replace(&mut self.path_bytes, usage.path_bytes),
            value_bytes: std::mem::replace(&mut self.value_bytes, usage.value_bytes),
        };
        if let Err(err) = self.flush() {
            self.data = previous_data;
            self.path_bytes = previous_usage.path_bytes;
            self.value_bytes = previous_usage.value_bytes;
            return Err(err);
        }
        Ok(())
    }
    /// Force a flush of the current in-memory overlay to disk (no-op if no path).
    pub fn flush(&self) -> Result<(), VMError> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        let usage = Self::validate_collection(&self.data)?;
        if usage.path_bytes != self.path_bytes || usage.value_bytes != self.value_bytes {
            return Err(VMError::NoritoInvalid);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| VMError::NoritoInvalid)?;
        }
        let encoded_len = persisted_json_encoded_len(&self.data)?;
        let mut serialized = String::with_capacity(encoded_len);
        serialized.push('{');
        for (index, (key, value)) in self.data.iter().enumerate() {
            if index != 0 {
                serialized.push(',');
            }
            json::write_json_string(key.as_ref(), &mut serialized);
            serialized.push(':');
            serialized.push('"');
            B64_STANDARD.encode_string(value, &mut serialized);
            serialized.push('"');
        }
        serialized.push('}');
        if serialized.len() != encoded_len || serialized.len() > STATE_OVERLAY_MAX_FILE_BYTES_V1 {
            return Err(VMError::NoritoInvalid);
        }
        fs::write(path, serialized.as_bytes()).map_err(|_| VMError::NoritoInvalid)?;
        Ok(())
    }
    fn reload_from_disk(&mut self) -> Result<(), VMError> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        let Some(bytes) = read_persisted_overlay_file(path).map_err(|_| VMError::NoritoInvalid)?
        else {
            return Ok(());
        };
        let (data, usage) = Self::decode_persisted(bytes)?;
        self.data = data;
        self.path_bytes = usage.path_bytes;
        self.value_bytes = usage.value_bytes;
        Ok(())
    }
    fn decode_persisted(
        bytes: Vec<u8>,
    ) -> Result<(BTreeMap<StatePath, Vec<u8>>, OverlayResourceUsage), VMError> {
        preflight_persisted_json(&bytes)?;
        let persisted: BTreeMap<String, String> =
            json::from_slice(&bytes).map_err(|_| VMError::NoritoInvalid)?;
        drop(bytes);
        let mut map = BTreeMap::new();
        let mut usage = OverlayResourceUsage::default();
        for (k, v) in persisted {
            if k.len() > crate::syscalls::STATE_MAX_PATH_BYTES {
                return Err(VMError::NoritoInvalid);
            }
            let path: StatePath = k.parse().map_err(|_| VMError::NoritoInvalid)?;
            let encoded = v.trim();
            if encoded.len() > STATE_OVERLAY_MAX_BASE64_VALUE_BYTES_V1 {
                return Err(VMError::NoritoInvalid);
            }
            let decoded = B64_STANDARD
                .decode(encoded.as_bytes())
                .map_err(|_| VMError::NoritoInvalid)?;
            Self::validate_entry(&path, &decoded)?;
            usage.path_bytes = usage
                .path_bytes
                .checked_add(path.as_ref().len())
                .ok_or(VMError::NoritoInvalid)?;
            usage.value_bytes = usage
                .value_bytes
                .checked_add(decoded.len())
                .ok_or(VMError::NoritoInvalid)?;
            Self::ensure_resource_limits(
                map.len().saturating_add(1),
                usage.path_bytes,
                usage.value_bytes,
            )?;
            if map.insert(path, decoded).is_some() {
                return Err(VMError::NoritoInvalid);
            }
        }
        Ok((map, usage))
    }
}
fn persisted_json_encoded_len(data: &BTreeMap<StatePath, Vec<u8>>) -> Result<usize, VMError> {
    let mut bytes = 2usize;
    for (index, (path, value)) in data.iter().enumerate() {
        if index != 0 {
            bytes = bytes.checked_add(1).ok_or(VMError::NoritoInvalid)?;
        }
        bytes = bytes
            .checked_add(json_string_encoded_len(path.as_ref())?)
            .and_then(|total| total.checked_add(1))
            .and_then(|total| total.checked_add(2))
            .ok_or(VMError::NoritoInvalid)?;
        let encoded_value = base64::encoded_len(value.len(), true).ok_or(VMError::NoritoInvalid)?;
        bytes = bytes
            .checked_add(encoded_value)
            .ok_or(VMError::NoritoInvalid)?;
        if bytes > STATE_OVERLAY_MAX_FILE_BYTES_V1 {
            return Err(VMError::NoritoInvalid);
        }
    }
    Ok(bytes)
}
fn json_string_encoded_len(value: &str) -> Result<usize, VMError> {
    value.chars().try_fold(2usize, |bytes, character| {
        let added = match character {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{08}' | '\u{0c}' => 2,
            character if (character as u32) < 0x20 => 6,
            character => character.len_utf8(),
        };
        bytes.checked_add(added).ok_or(VMError::NoritoInvalid)
    })
}
fn preflight_persisted_json(bytes: &[u8]) -> Result<(), VMError> {
    if bytes.len() > STATE_OVERLAY_MAX_FILE_BYTES_V1 {
        return Err(VMError::NoritoInvalid);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| VMError::NoritoInvalid)?;
    let mut cursor = FlatOverlayJsonCursor::new(text.as_bytes());
    cursor.skip_whitespace();
    cursor.expect(b'{')?;
    cursor.skip_whitespace();
    if cursor.consume(b'}') {
        cursor.skip_whitespace();
        return cursor.finish();
    }
    let mut entries = 0usize;
    loop {
        if entries == STATE_OVERLAY_MAX_ENTRIES_V1 {
            return Err(VMError::NoritoInvalid);
        }
        cursor.scan_string(STATE_OVERLAY_MAX_RAW_JSON_PATH_BYTES_V1)?;
        cursor.skip_whitespace();
        cursor.expect(b':')?;
        cursor.skip_whitespace();
        cursor.scan_string(STATE_OVERLAY_MAX_RAW_JSON_VALUE_BYTES_V1)?;
        entries = entries.saturating_add(1);
        cursor.skip_whitespace();
        if cursor.consume(b'}') {
            cursor.skip_whitespace();
            return cursor.finish();
        }
        cursor.expect(b',')?;
        cursor.skip_whitespace();
    }
}
struct FlatOverlayJsonCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> FlatOverlayJsonCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn skip_whitespace(&mut self) {
        while matches!(
            self.bytes.get(self.offset),
            Some(b' ' | b'\n' | b'\r' | b'\t')
        ) {
            self.offset = self.offset.saturating_add(1);
        }
    }
    fn consume(&mut self, expected: u8) -> bool {
        if self.bytes.get(self.offset).copied() == Some(expected) {
            self.offset = self.offset.saturating_add(1);
            true
        } else {
            false
        }
    }
    fn expect(&mut self, expected: u8) -> Result<(), VMError> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(VMError::NoritoInvalid)
        }
    }
    fn scan_string(&mut self, maximum_raw_bytes: usize) -> Result<(), VMError> {
        self.expect(b'"')?;
        let start = self.offset;
        let mut escaped = false;
        while let Some(&byte) = self.bytes.get(self.offset) {
            if self.offset.saturating_sub(start) > maximum_raw_bytes {
                return Err(VMError::NoritoInvalid);
            }
            self.offset = self.offset.saturating_add(1);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                return Ok(());
            }
        }
        Err(VMError::NoritoInvalid)
    }
    fn finish(&self) -> Result<(), VMError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(VMError::NoritoInvalid)
        }
    }
}
fn read_persisted_overlay_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let named_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if state_overlay_metadata_is_link(&named_before) || !named_before.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay must be a direct regular file",
        ));
    }
    let maximum =
        u64::try_from(STATE_OVERLAY_MAX_FILE_BYTES_V1).expect("fixed state overlay limit fits u64");
    if named_before.len() > maximum {
        return Err(state_overlay_file_too_large());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(STATE_OVERLAY_O_NOFOLLOW_FLAG);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if state_overlay_metadata_is_link(&opened_before)
        || !opened_before.is_file()
        || !state_overlay_metadata_identifies_same_file(&named_before, &opened_before)
        || opened_before.len() > maximum
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay changed identity or type while opening",
        ));
    }
    let capacity = usize::try_from(opened_before.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay length cannot be addressed",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > STATE_OVERLAY_MAX_FILE_BYTES_V1 {
        return Err(state_overlay_file_too_large());
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay byte count cannot be represented",
        )
    })?;
    if state_overlay_metadata_is_link(&named_after)
        || !named_after.is_file()
        || !state_overlay_metadata_identifies_same_file(&opened_before, &opened_after)
        || !state_overlay_metadata_identifies_same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != named_after.len()
        || opened_after.len() != observed
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay changed while it was being read",
        ));
    }
    Ok(Some(bytes))
}
fn state_overlay_file_too_large() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "durable state overlay exceeds the first-release file limit",
    )
}
fn state_overlay_metadata_is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}
#[cfg(unix)]
fn state_overlay_metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(windows)]
fn state_overlay_metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}
#[cfg(not(any(unix, windows)))]
fn state_overlay_metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}
impl DurableStateSnapshot {
    #[must_use]
    pub fn new(data: BTreeMap<StatePath, Vec<u8>>) -> Self {
        Self { data }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn path(value: &str) -> StatePath {
        value.parse().expect("canonical state path")
    }
    #[test]
    fn raw_payload_ingress_enforces_value_bound() {
        let mut overlay = DurableStateOverlay::in_memory();
        let maximum = vec![0xabu8; crate::syscalls::STATE_MAX_VALUE_BYTES];
        let bounded = path("bounded");
        overlay
            .set(&bounded, maximum.clone())
            .expect("maximum raw payload must fit");
        assert_eq!(
            overlay
                .value_payload_ref(&bounded)
                .expect("valid overlay")
                .expect("stored payload"),
            maximum
        );
        let oversized = vec![0xcdu8; crate::syscalls::STATE_MAX_VALUE_BYTES + 1];
        let oversized_path = path("oversized");
        assert_eq!(
            overlay.set(&oversized_path, oversized),
            Err(VMError::NoritoInvalid)
        );
        assert!(overlay.get_ref(&oversized_path).is_none());
    }
    #[test]
    fn restore_validates_all_entries_before_replacing_state() {
        let mut overlay = DurableStateOverlay::in_memory();
        let existing = path("existing");
        overlay
            .set(&existing, b"kept".to_vec())
            .expect("seed overlay");
        let snapshot = DurableStateSnapshot::new(BTreeMap::from([
            (path("valid"), b"value".to_vec()),
            (
                path("oversized"),
                vec![0u8; crate::syscalls::STATE_MAX_VALUE_BYTES + 1],
            ),
        ]));
        assert_eq!(overlay.restore(&snapshot), Err(VMError::NoritoInvalid));
        assert_eq!(overlay.get_ref(&existing), Some(b"kept".as_slice()));
        assert!(overlay.get_ref(&path("valid")).is_none());
    }
    #[test]
    fn text_prefix_iterator_starts_at_the_ordered_lower_bound() {
        let mut overlay = DurableStateOverlay::in_memory();
        for path in ["accounts/1", "orders/1", "orders/2", "payments/1"] {
            overlay
                .set(&path.parse().expect("canonical state path"), Vec::new())
                .expect("insert bounded entry");
        }
        assert_eq!(
            overlay
                .keys_with_text_prefix("orders")
                .map(AsRef::<str>::as_ref)
                .collect::<Vec<_>>(),
            vec!["orders/1", "orders/2"]
        );
    }
    #[test]
    fn persisted_overlay_rejects_canonically_duplicate_state_paths() {
        let duplicate = r#"{"root/e\u0301":"AQ==","root/é":"Ag=="}"#;
        assert_eq!(
            DurableStateOverlay::decode_persisted(duplicate.as_bytes().to_vec()),
            Err(VMError::NoritoInvalid)
        );
    }
    #[test]
    fn aggregate_limits_accept_boundaries_and_reject_first_overflow() {
        assert_eq!(
            DurableStateOverlay::ensure_resource_limits(
                STATE_OVERLAY_MAX_ENTRIES_V1,
                STATE_OVERLAY_MAX_PATH_BYTES_V1,
                STATE_OVERLAY_MAX_VALUE_BYTES_V1,
            ),
            Ok(())
        );
        for overflow in [
            (
                STATE_OVERLAY_MAX_ENTRIES_V1 + 1,
                STATE_OVERLAY_MAX_PATH_BYTES_V1,
                STATE_OVERLAY_MAX_VALUE_BYTES_V1,
            ),
            (
                STATE_OVERLAY_MAX_ENTRIES_V1,
                STATE_OVERLAY_MAX_PATH_BYTES_V1 + 1,
                STATE_OVERLAY_MAX_VALUE_BYTES_V1,
            ),
            (
                STATE_OVERLAY_MAX_ENTRIES_V1,
                STATE_OVERLAY_MAX_PATH_BYTES_V1,
                STATE_OVERLAY_MAX_VALUE_BYTES_V1 + 1,
            ),
        ] {
            assert_eq!(
                DurableStateOverlay::ensure_resource_limits(overflow.0, overflow.1, overflow.2),
                Err(VMError::NoritoInvalid)
            );
        }
    }
    #[test]
    fn aggregate_value_overflow_is_rejected_before_mutation() {
        let mut overlay = DurableStateOverlay::in_memory();
        let full_values = STATE_OVERLAY_MAX_VALUE_BYTES_V1 / crate::syscalls::STATE_MAX_VALUE_BYTES;
        assert_eq!(
            full_values * crate::syscalls::STATE_MAX_VALUE_BYTES,
            STATE_OVERLAY_MAX_VALUE_BYTES_V1
        );
        for index in 0..full_values {
            overlay
                .set(
                    &path(&format!("bounded/{index}")),
                    vec![0xa5; crate::syscalls::STATE_MAX_VALUE_BYTES],
                )
                .expect("aggregate boundary must fit");
        }
        let overflow = path("bounded/overflow");
        assert_eq!(
            overlay.set(&overflow, vec![0x5a]),
            Err(VMError::NoritoInvalid)
        );
        assert!(overlay.get_ref(&overflow).is_none());
        assert_eq!(overlay.value_bytes, STATE_OVERLAY_MAX_VALUE_BYTES_V1);
    }
    #[test]
    fn persisted_json_rejects_entry_count_before_typed_decode() {
        let mut encoded = String::from("{");
        for index in 0..=STATE_OVERLAY_MAX_ENTRIES_V1 {
            if index != 0 {
                encoded.push(',');
            }
            encoded.push_str(&format!("\"key/{index}\":\"\""));
        }
        encoded.push('}');
        assert_eq!(
            DurableStateOverlay::decode_persisted(encoded.into_bytes()),
            Err(VMError::NoritoInvalid)
        );
    }
    #[test]
    fn persisted_reader_rejects_sparse_file_above_limit() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let directory = std::env::temp_dir().join(format!(
            "ivm_state_overlay_bound_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create temporary directory");
        let file_path = directory.join("state.json");
        let file = fs::File::create(&file_path).expect("create sparse state file");
        file.set_len(
            u64::try_from(STATE_OVERLAY_MAX_FILE_BYTES_V1 + 1).expect("test length fits u64"),
        )
        .expect("extend sparse state file");
        drop(file);
        let error = read_persisted_overlay_file(&file_path).expect_err("oversize must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
}

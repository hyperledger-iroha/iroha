//! Bounded durable state storage for development and test hosts.
use crate::VMError;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64_STANDARD};
use iroha_data_model::state_path::StatePath;
use norito::json;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Read as _, Write as _},
    ops::Bound,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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
static STATE_OVERLAY_TEMP_NONCE: AtomicU64 = AtomicU64::new(0);
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
/// The configured path's ancestor directories are an operator-owned trust
/// boundary; leaf publication itself is exclusive, single-link checked, and
/// atomic so a replaced link or special file is never opened for writing.
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
        write_persisted_overlay_file(path, serialized.as_bytes())
            .map_err(|_| VMError::NoritoInvalid)?;
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
fn write_persisted_overlay_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "durable state overlay path must name a file",
        )
    })?;
    let (temporary_path, mut temporary_file) = (0..128)
        .find_map(|_| {
            let nonce = STATE_OVERLAY_TEMP_NONCE.fetch_add(1, Ordering::Relaxed);
            let mut name = OsString::from(".");
            name.push(file_name);
            name.push(format!(".ivm-state-tmp-{}-{nonce}", std::process::id()));
            let candidate = parent.join(name);
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options
                    .mode(0o600)
                    .custom_flags(STATE_OVERLAY_O_NOFOLLOW_FLAG);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                options
                    .share_mode(STATE_OVERLAY_WINDOWS_FILE_SHARE_READ_WRITE_DELETE)
                    .custom_flags(STATE_OVERLAY_WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT);
            }
            match options.open(&candidate) {
                Ok(file) => Some(Ok((candidate, file))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not reserve a durable state overlay temporary file",
            )
        })?;
    let result = (|| {
        validate_persisted_overlay_open_file(&temporary_path, &temporary_file)?;
        temporary_file.write_all(bytes)?;
        temporary_file.sync_all()?;
        let observed_len = temporary_file.metadata()?.len();
        if observed_len
            != u64::try_from(bytes.len()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "durable state overlay length cannot be represented",
                )
            })?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "durable state overlay temporary write was incomplete",
            ));
        }
        validate_persisted_overlay_open_file(&temporary_path, &temporary_file)?;
        validate_persisted_overlay_destination(path)?;
        replace_persisted_overlay_file(&temporary_path, path)?;
        sync_overlay_parent_best_effort(parent);
        Ok(())
    })();
    if result.is_err()
        && validate_persisted_overlay_open_file(&temporary_path, &temporary_file).is_ok()
    {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}
#[cfg(not(windows))]
fn validate_persisted_overlay_open_file(path: &Path, file: &fs::File) -> io::Result<()> {
    let named = fs::symlink_metadata(path)?;
    let opened = file.metadata()?;
    if state_overlay_metadata_is_link(&named)
        || !named.is_file()
        || !opened.is_file()
        || !state_overlay_metadata_is_single_link(&named)
        || !state_overlay_metadata_is_single_link(&opened)
        || !state_overlay_metadata_identifies_same_file(&named, &opened)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay temporary changed identity or type",
        ));
    }
    Ok(())
}
#[cfg(windows)]
fn validate_persisted_overlay_open_file(path: &Path, file: &fs::File) -> io::Result<()> {
    let opened = state_overlay_windows_file_snapshot(file)?;
    let named_file = open_state_overlay_windows_file(path)?;
    let named = state_overlay_windows_file_snapshot(&named_file)?;
    if !opened.is_direct_single_link_regular_file()
        || !named.is_direct_single_link_regular_file()
        || !opened.same_identity(named)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay temporary changed identity or type",
        ));
    }
    Ok(())
}
#[cfg(not(windows))]
fn validate_persisted_overlay_destination(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if state_overlay_metadata_is_link(&metadata)
        || !metadata.is_file()
        || !state_overlay_metadata_is_single_link(&metadata)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay destination must be a direct single-link regular file",
        ));
    }
    Ok(())
}
#[cfg(windows)]
fn validate_persisted_overlay_destination(path: &Path) -> io::Result<()> {
    let file = match open_state_overlay_windows_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !state_overlay_windows_file_snapshot(&file)?.is_direct_single_link_regular_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay destination must be a direct single-link regular file",
        ));
    }
    Ok(())
}
#[cfg(not(windows))]
fn replace_persisted_overlay_file(temporary_path: &Path, path: &Path) -> io::Result<()> {
    fs::rename(temporary_path, path)
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn replace_persisted_overlay_file(temporary_path: &Path, path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    let wide_path = |value: &Path| -> io::Result<Vec<u16>> {
        let mut wide = value.as_os_str().encode_wide().collect::<Vec<_>>();
        if wide.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "durable state overlay path contains a null code unit",
            ));
        }
        wide.push(0);
        Ok(wide)
    };
    let temporary_path = wide_path(temporary_path)?;
    let path = wide_path(path)?;
    // SAFETY: both slices are null-terminated and remain alive for the call.
    let succeeded = unsafe {
        state_overlay_move_file_ex(
            temporary_path.as_ptr(),
            path.as_ptr(),
            STATE_OVERLAY_WINDOWS_MOVEFILE_REPLACE_EXISTING
                | STATE_OVERLAY_WINDOWS_MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(unix)]
fn sync_overlay_parent_best_effort(parent: &Path) {
    if let Ok(directory) = fs::File::open(parent) {
        let _ = directory.sync_all();
    }
}
#[cfg(not(unix))]
fn sync_overlay_parent_best_effort(_parent: &Path) {}
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
#[cfg(not(windows))]
fn read_persisted_overlay_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let named_before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if state_overlay_metadata_is_link(&named_before)
        || !named_before.is_file()
        || !state_overlay_metadata_is_single_link(&named_before)
    {
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
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if state_overlay_metadata_is_link(&opened_before)
        || !opened_before.is_file()
        || !state_overlay_metadata_is_single_link(&opened_before)
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
    std::io::Read::by_ref(&mut file)
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
        || !state_overlay_metadata_is_single_link(&opened_after)
        || !state_overlay_metadata_is_single_link(&named_after)
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
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_FILE_SHARE_READ_WRITE_DELETE: u32 =
    0x0000_0001 | 0x0000_0002 | 0x0000_0004;
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
#[cfg(windows)]
const STATE_OVERLAY_WINDOWS_MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;
#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct StateOverlayWindowsFileTime {
    low: u32,
    high: u32,
}
#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct StateOverlayWindowsByHandleFileInformation {
    file_attributes: u32,
    creation_time: StateOverlayWindowsFileTime,
    _last_access_time: StateOverlayWindowsFileTime,
    last_write_time: StateOverlayWindowsFileTime,
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}
#[cfg(windows)]
const _: () = assert!(std::mem::size_of::<StateOverlayWindowsByHandleFileInformation>() == 52);
#[cfg(windows)]
const _: () = assert!(std::mem::align_of::<StateOverlayWindowsByHandleFileInformation>() == 4);
#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateOverlayWindowsFileSnapshot {
    file_attributes: u32,
    creation_time: u64,
    last_write_time: u64,
    volume_serial_number: u32,
    file_size: u64,
    file_index: u64,
    number_of_links: u32,
}
#[cfg(windows)]
impl StateOverlayWindowsFileSnapshot {
    const fn is_direct_regular_file(self) -> bool {
        self.file_attributes
            & (STATE_OVERLAY_WINDOWS_FILE_ATTRIBUTE_DIRECTORY
                | STATE_OVERLAY_WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT)
            == 0
    }
    const fn is_direct_single_link_regular_file(self) -> bool {
        self.is_direct_regular_file() && self.number_of_links == 1
    }
    const fn same_identity(self, other: Self) -> bool {
        self.volume_serial_number == other.volume_serial_number
            && self.file_index == other.file_index
    }
    const fn same_revision(self, other: Self) -> bool {
        self.file_attributes == other.file_attributes
            && self.creation_time == other.creation_time
            && self.last_write_time == other.last_write_time
            && self.file_size == other.file_size
    }
}
#[cfg(windows)]
#[link(name = "kernel32")]
#[allow(unsafe_code)]
unsafe extern "system" {
    #[link_name = "GetFileInformationByHandle"]
    fn state_overlay_get_file_information_by_handle(
        file: *mut std::ffi::c_void,
        information: *mut StateOverlayWindowsByHandleFileInformation,
    ) -> i32;
    #[link_name = "MoveFileExW"]
    fn state_overlay_move_file_ex(
        existing_file_name: *const u16,
        new_file_name: *const u16,
        flags: u32,
    ) -> i32;
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn state_overlay_windows_file_snapshot(
    file: &fs::File,
) -> io::Result<StateOverlayWindowsFileSnapshot> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle as _};
    let mut information = MaybeUninit::<StateOverlayWindowsByHandleFileInformation>::uninit();
    // SAFETY: `file` owns a valid kernel handle for the duration of the call,
    // and `information` has the exact writable Win32 ABI layout expected by
    // `GetFileInformationByHandle`.
    let succeeded = unsafe {
        state_overlay_get_file_information_by_handle(
            file.as_raw_handle().cast(),
            information.as_mut_ptr(),
        )
    };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Win32 initializes every field when the call succeeds.
    let information = unsafe { information.assume_init() };
    let combine = |high: u32, low: u32| u64::from(high) << 32 | u64::from(low);
    Ok(StateOverlayWindowsFileSnapshot {
        file_attributes: information.file_attributes,
        creation_time: combine(
            information.creation_time.high,
            information.creation_time.low,
        ),
        last_write_time: combine(
            information.last_write_time.high,
            information.last_write_time.low,
        ),
        volume_serial_number: information.volume_serial_number,
        file_size: combine(information.file_size_high, information.file_size_low),
        file_index: combine(information.file_index_high, information.file_index_low),
        number_of_links: information.number_of_links,
    })
}
#[cfg(windows)]
fn open_state_overlay_windows_file(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .share_mode(STATE_OVERLAY_WINDOWS_FILE_SHARE_READ_WRITE_DELETE)
        .custom_flags(
            STATE_OVERLAY_WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT
                | STATE_OVERLAY_WINDOWS_FILE_FLAG_BACKUP_SEMANTICS,
        );
    options.open(path)
}
#[cfg(windows)]
fn read_persisted_overlay_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let mut file = match open_state_overlay_windows_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let opened_before = state_overlay_windows_file_snapshot(&file)?;
    if !opened_before.is_direct_single_link_regular_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay must be a direct regular file",
        ));
    }
    let maximum =
        u64::try_from(STATE_OVERLAY_MAX_FILE_BYTES_V1).expect("fixed state overlay limit fits u64");
    if opened_before.file_size > maximum {
        return Err(state_overlay_file_too_large());
    }
    let capacity = usize::try_from(opened_before.file_size).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay length cannot be addressed",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > STATE_OVERLAY_MAX_FILE_BYTES_V1 {
        return Err(state_overlay_file_too_large());
    }
    let opened_after = state_overlay_windows_file_snapshot(&file)?;
    let named_after_file = open_state_overlay_windows_file(path)?;
    let named_after = state_overlay_windows_file_snapshot(&named_after_file)?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "durable state overlay byte count cannot be represented",
        )
    })?;
    if !opened_after.is_direct_single_link_regular_file()
        || !named_after.is_direct_single_link_regular_file()
        || !opened_before.same_identity(opened_after)
        || !opened_after.same_identity(named_after)
        || !opened_before.same_revision(opened_after)
        || !opened_after.same_revision(named_after)
        || opened_after.file_size != observed
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
#[cfg(not(windows))]
fn state_overlay_metadata_is_link(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
#[cfg(unix)]
fn state_overlay_metadata_is_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() == 1
}
#[cfg(not(any(unix, windows)))]
fn state_overlay_metadata_is_single_link(_metadata: &fs::Metadata) -> bool {
    true
}
#[cfg(unix)]
fn state_overlay_metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
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
    fn temporary_directory(label: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        std::env::temp_dir().join(format!(
            "ivm_state_overlay_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ))
    }
    #[cfg(unix)]
    #[test]
    fn atomic_flush_rejects_replaced_symlink_without_writing_its_target() {
        use std::os::unix::fs::symlink;
        let directory = temporary_directory("symlink_swap");
        fs::create_dir_all(&directory).expect("create temporary directory");
        let overlay_path = directory.join("state.json");
        let victim_path = directory.join("victim.json");
        let mut overlay = DurableStateOverlay::with_persist_path(overlay_path.clone())
            .expect("create persisted overlay");
        overlay
            .set(&path("seed"), b"one".to_vec())
            .expect("seed persisted overlay");
        fs::write(&victim_path, b"do not replace").expect("write victim fixture");
        fs::remove_file(&overlay_path).expect("remove overlay leaf");
        symlink(&victim_path, &overlay_path).expect("replace leaf with symlink");

        assert_eq!(
            overlay.set(&path("second"), b"two".to_vec()),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(fs::read(&victim_path).unwrap(), b"do not replace");
        assert!(overlay.get_ref(&path("second")).is_none());
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
    #[test]
    fn atomic_flush_rejects_replaced_hard_link_without_writing_its_alias() {
        let directory = temporary_directory("hard_link_swap");
        fs::create_dir_all(&directory).expect("create temporary directory");
        let overlay_path = directory.join("state.json");
        let victim_path = directory.join("victim.json");
        let mut overlay = DurableStateOverlay::with_persist_path(overlay_path.clone())
            .expect("create persisted overlay");
        overlay
            .set(&path("seed"), b"one".to_vec())
            .expect("seed persisted overlay");
        fs::write(&victim_path, b"do not replace").expect("write victim fixture");
        fs::remove_file(&overlay_path).expect("remove overlay leaf");
        fs::hard_link(&victim_path, &overlay_path).expect("replace leaf with hard link");

        assert_eq!(
            overlay.set(&path("second"), b"two".to_vec()),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(fs::read(&victim_path).unwrap(), b"do not replace");
        assert!(overlay.get_ref(&path("second")).is_none());
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
    #[test]
    fn atomic_flush_rejects_replaced_directory_without_blocking() {
        let directory = temporary_directory("directory_swap");
        fs::create_dir_all(&directory).expect("create temporary directory");
        let overlay_path = directory.join("state.json");
        let mut overlay = DurableStateOverlay::with_persist_path(overlay_path.clone())
            .expect("create persisted overlay");
        overlay
            .set(&path("seed"), b"one".to_vec())
            .expect("seed persisted overlay");
        fs::remove_file(&overlay_path).expect("remove overlay leaf");
        fs::create_dir(&overlay_path).expect("replace leaf with directory");

        assert_eq!(
            overlay.set(&path("second"), b"two".to_vec()),
            Err(VMError::NoritoInvalid)
        );
        assert!(overlay_path.is_dir());
        assert!(overlay.get_ref(&path("second")).is_none());
        fs::remove_dir_all(directory).expect("remove temporary directory");
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
    #[cfg(windows)]
    #[test]
    fn windows_handle_snapshot_distinguishes_equal_files() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let directory = std::env::temp_dir().join(format!(
            "ivm_state_overlay_windows_identity_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create temporary directory");
        let first_path = directory.join("first.json");
        let second_path = directory.join("second.json");
        fs::write(&first_path, b"same bytes").expect("write first fixture");
        fs::write(&second_path, b"same bytes").expect("write second fixture");
        let first = open_state_overlay_windows_file(&first_path).expect("open first fixture");
        let first_again =
            open_state_overlay_windows_file(&first_path).expect("reopen first fixture");
        let second = open_state_overlay_windows_file(&second_path).expect("open second fixture");
        let first_snapshot =
            state_overlay_windows_file_snapshot(&first).expect("snapshot first fixture");
        let first_again_snapshot = state_overlay_windows_file_snapshot(&first_again)
            .expect("snapshot reopened first fixture");
        let second_snapshot =
            state_overlay_windows_file_snapshot(&second).expect("snapshot second fixture");
        assert!(first_snapshot.is_direct_regular_file());
        assert!(first_snapshot.same_identity(first_again_snapshot));
        assert!(first_snapshot.same_revision(first_again_snapshot));
        assert!(!first_snapshot.same_identity(second_snapshot));
        assert_eq!(first_snapshot.file_size, b"same bytes".len() as u64);
        drop((first, first_again, second));
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
    #[cfg(windows)]
    #[test]
    fn windows_persisted_reader_rejects_directory_as_invalid_data() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let directory = std::env::temp_dir().join(format!(
            "ivm_state_overlay_windows_directory_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("create temporary directory");
        let error = read_persisted_overlay_file(&directory)
            .expect_err("state overlay directory must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
}

//! Exit-routing helpers for bridging SoraNet circuits to external services.
//!
//! This module contains lightweight routing metadata and stream framing helpers
//! used by the relay runtime when a circuit requests an exit-bound stream
//! (e.g., Norito RPC/streaming). The actual protocol adapters live in the
//! runtime; this module focuses on configuration and deterministic decoding.
//!
//! First-release on-disk catalogs are producer-canonical binary Norito files:
//! traversal, file bytes, decoded fields, and retained routes each have an
//! independent hard corridor declared below. Catalog files must be direct,
//! stable regular files and their filename route/stream IDs must match the
//! decoded update before the route can enter the cache.
use crate::config::{
    ConfigError, ExitRoutingConfig, KaigiStreamRoutingConfig, NoritoStreamRoutingConfig,
};
use blake3::Hasher as Blake3Hasher;
use iroha_data_model::soranet::RelayId;
use norito::{
    DecodeLimits, Error as NoritoError, decode_from_bytes_with_limits,
    streaming::{PrivacyRouteUpdate, SoranetAccessKind, SoranetRoute, SoranetStreamTag},
};
use std::{
    collections::HashMap,
    fs::{self, File, Metadata as FsMetadata, OpenOptions},
    io::{self, Read as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use thiserror::Error;
const DEFAULT_GAR_CATEGORY_READ_ONLY: &str = "stream.norito.read_only";
const DEFAULT_GAR_CATEGORY_AUTH: &str = "stream.norito.authenticated";
const ROUTE_OPEN_FRAME_LEN: usize = 34;
const NORITO_STREAM_SUBDIR: &str = "norito-stream";
const NORITO_STREAM_LABEL: &str = "norito-stream";
const KAIGI_STREAM_SUBDIR: &str = "kaigi-stream";
const KAIGI_STREAM_LABEL: &str = "kaigi-stream";
const NORITO_FILE_EXTENSION: &str = "norito";
const DEFAULT_KAIGI_CATEGORY_PUBLIC: &str = "stream.kaigi.public";
const DEFAULT_KAIGI_CATEGORY_AUTH: &str = "stream.kaigi.authenticated";
const KAIGI_ROOM_DOMAIN_TAG: &[u8] = b"soranet.kaigi.room_id.v1";
// First-release route-catalog corridors. The producer's default provision
// queue is 256 entries; 4,096 route files retain sixteen complete queue
// generations while bounding stale-spool traversal. A route update is a small
// fixed control record plus one HPKE-wrapped exit token and one multiaddr, so
// 64 KiB/file, 32 KiB/token, and 8 KiB/multiaddr preserve substantial protocol
// headroom. Encoded scan bytes and retained decoded state are bounded
// independently because duplicate/stale files do not all survive in the map.
const ROUTE_CATALOG_MAX_DIRECTORY_ENTRIES_V1: usize = 8_192;
const ROUTE_CATALOG_MAX_FILES_V1: usize = 4_096;
const ROUTE_CATALOG_MAX_ROUTES_V1: usize = 4_096;
const ROUTE_CATALOG_FILE_MAX_BYTES_V1: usize = 64 * 1024;
const ROUTE_CATALOG_MAX_ENCODED_BYTES_V1: usize = 64 * 1024 * 1024;
const ROUTE_CATALOG_MAX_RETAINED_BYTES_V1: usize = 32 * 1024 * 1024;
const ROUTE_EXIT_TOKEN_MAX_BYTES_V1: usize = 32 * 1024;
const ROUTE_MULTIADDR_MAX_BYTES_V1: usize = 8 * 1024;
const ROUTE_CATALOG_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    ROUTE_EXIT_TOKEN_MAX_BYTES_V1,
    ROUTE_CATALOG_FILE_MAX_BYTES_V1,
    ROUTE_EXIT_TOKEN_MAX_BYTES_V1 + 64,
    128 * 1024,
    16,
);
#[cfg(any(target_os = "macos", target_os = "ios"))]
const ROUTE_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const ROUTE_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const ROUTE_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
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
compile_error!("soranet-relay requires a defined no-follow open flag on this Unix target");
#[cfg(unix)]
type RouteFileIdentity = (u64, u64);
#[cfg(windows)]
type RouteFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type RouteFileIdentity = ();
#[cfg(unix)]
fn route_file_identity(metadata: &FsMetadata) -> RouteFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}
#[cfg(windows)]
fn route_file_identity(metadata: &FsMetadata) -> RouteFileIdentity {
    use std::os::windows::fs::MetadataExt as _;
    (metadata.volume_serial_number(), metadata.file_index())
}
#[cfg(not(any(unix, windows)))]
fn route_file_identity(_metadata: &FsMetadata) -> RouteFileIdentity {}
#[cfg(unix)]
const fn route_file_identity_available(_identity: RouteFileIdentity) -> bool {
    true
}
#[cfg(windows)]
const fn route_file_identity_available(identity: RouteFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}
#[cfg(not(any(unix, windows)))]
const fn route_file_identity_available(_identity: RouteFileIdentity) -> bool {
    false
}
#[cfg(windows)]
fn route_file_is_reparse_point(metadata: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn route_file_is_reparse_point(_metadata: &FsMetadata) -> bool {
    false
}
fn validate_direct_route_file(metadata: &FsMetadata) -> io::Result<()> {
    if metadata.file_type().is_symlink()
        || route_file_is_reparse_point(metadata)
        || !metadata.file_type().is_file()
        || !route_file_identity_available(route_file_identity(metadata))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "route update must be a direct regular file with a stable identity",
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn open_direct_route_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(ROUTE_O_NOFOLLOW_FLAG);
    options.open(path)
}
#[cfg(windows)]
fn open_direct_route_file(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}
#[cfg(not(any(unix, windows)))]
fn open_direct_route_file(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "stable direct-file opens are unavailable on this platform",
    ))
}
#[cfg(unix)]
fn route_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    route_file_identity(left) == route_file_identity(right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn route_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    route_file_identity(left) == route_file_identity(right)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}
#[cfg(not(any(unix, windows)))]
fn route_file_metadata_unchanged(_left: &FsMetadata, _right: &FsMetadata) -> bool {
    false
}
fn read_bounded_direct_route_file(path: &Path, maximum: usize) -> io::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    validate_direct_route_file(&before)?;
    let maximum_u64 = u64::try_from(maximum).unwrap_or(u64::MAX);
    if before.len() > maximum_u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "route update is {} bytes; remaining first-release limit is {maximum} bytes",
                before.len()
            ),
        ));
    }
    let mut file = open_direct_route_file(path)?;
    let opened = file.metadata()?;
    validate_direct_route_file(&opened)?;
    if !route_file_metadata_unchanged(&before, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "route update changed between inspection and open",
        ));
    }
    let expected_len = usize::try_from(opened.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "route update length is not representable on this host",
        )
    })?;
    if expected_len > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("route update exceeds the remaining first-release {maximum}-byte limit"),
        ));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(expected_len).map_err(|error| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            format!("failed to reserve {expected_len} route-update bytes: {error}"),
        )
    })?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "route update changed length while being read",
            )
        } else {
            error
        }
    })?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("route update grew or exceeds its {maximum}-byte limit"),
        ));
    }
    let after_file = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    validate_direct_route_file(&after_file)?;
    validate_direct_route_file(&after_path)?;
    if !route_file_metadata_unchanged(&opened, &after_file)
        || !route_file_metadata_unchanged(&opened, &after_path)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "route update changed while being read",
        ));
    }
    Ok(bytes)
}
#[derive(Clone, Copy)]
struct CanonicalRouteFileName {
    route_id: [u8; 32],
    stream_id: [u8; 32],
}
fn parse_canonical_route_file_name(path: &Path) -> Result<CanonicalRouteFileName, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "route filename must be valid UTF-8".to_owned())?;
    let stem = name
        .strip_suffix(".norito")
        .ok_or_else(|| "route filename must end in `.norito`".to_owned())?;
    let (counter, route_and_stream) = stem
        .split_once("-route-")
        .ok_or_else(|| "route filename is missing the `-route-` separator".to_owned())?;
    let (route_hex, stream_hex) = route_and_stream
        .split_once("-stream-")
        .ok_or_else(|| "route filename is missing the `-stream-` separator".to_owned())?;
    if counter.len() != 16 || !counter.bytes().all(is_lower_hex_digit) {
        return Err("route filename counter must be 16 lowercase hexadecimal digits".to_owned());
    }
    Ok(CanonicalRouteFileName {
        route_id: decode_lower_hash(route_hex, "route id")?,
        stream_id: decode_lower_hash(stream_hex, "stream id")?,
    })
}
fn decode_lower_hash(value: &str, label: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(is_lower_hex_digit) {
        return Err(format!(
            "route filename {label} must be 64 lowercase hexadecimal digits"
        ));
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded)
        .map_err(|error| format!("route filename {label} is invalid: {error}"))?;
    Ok(decoded)
}
fn is_lower_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}
fn checked_add_with_limit(current: usize, additional: usize, maximum: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= maximum)
}
fn checked_replace_with_limit(
    current: usize,
    previous: usize,
    replacement: usize,
    maximum: usize,
) -> Option<usize> {
    current
        .checked_sub(previous)
        .and_then(|total| total.checked_add(replacement))
        .filter(|total| *total <= maximum)
}
/// Static exit routing configuration derived from user config.
#[derive(Clone, Debug)]
pub struct ExitRouting {
    norito_stream: Option<NoritoStreamRoute>,
    kaigi_stream: Option<KaigiStreamRoute>,
}
impl ExitRouting {
    pub fn from_config(cfg: &ExitRoutingConfig) -> Result<Self, ConfigError> {
        let norito_stream = match &cfg.norito_stream {
            Some(route_cfg) => Some(NoritoStreamRoute::from_config(route_cfg)?),
            None => None,
        };
        let kaigi_stream = match &cfg.kaigi_stream {
            Some(route_cfg) => Some(KaigiStreamRoute::from_config(route_cfg)?),
            None => None,
        };
        Ok(Self {
            norito_stream,
            kaigi_stream,
        })
    }
    pub fn prepare(&self, relay_id: RelayId) -> ExitRoutingState {
        let norito_stream = self
            .norito_stream
            .as_ref()
            .map(|route| Arc::new(NoritoStreamState::new(route.clone(), relay_id)));
        let kaigi_stream = self
            .kaigi_stream
            .as_ref()
            .map(|route| Arc::new(KaigiStreamState::new(route.clone(), relay_id)));
        ExitRoutingState {
            norito_stream,
            kaigi_stream,
        }
    }
}
/// Prepared exit routing state bound to a relay identifier.
#[derive(Clone)]
pub struct ExitRoutingState {
    norito_stream: Option<Arc<NoritoStreamState>>,
    kaigi_stream: Option<Arc<KaigiStreamState>>,
}
impl ExitRoutingState {
    pub fn norito_stream(&self) -> Option<Arc<NoritoStreamState>> {
        self.norito_stream.as_ref().map(Arc::clone)
    }
    pub fn kaigi_stream(&self) -> Option<Arc<KaigiStreamState>> {
        self.kaigi_stream.as_ref().map(Arc::clone)
    }
}
/// Norito streaming route configuration resolved from user config.
#[derive(Clone, Debug)]
pub struct NoritoStreamRoute {
    torii_ws_url: String,
    connect_timeout: Duration,
    padding_target: Duration,
    gar_read_only: String,
    gar_authenticated: String,
    spool_dir: Option<PathBuf>,
    route_refresh_interval: Duration,
}
impl NoritoStreamRoute {
    fn from_config(cfg: &NoritoStreamRoutingConfig) -> Result<Self, ConfigError> {
        let url = cfg.torii_ws_url.trim();
        if url.is_empty() {
            return Err(ConfigError::Routing(
                "norito_stream.torii_ws_url must not be empty".to_string(),
            ));
        }
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err(ConfigError::Routing(format!(
                "norito_stream.torii_ws_url must start with ws:// or wss:// (got `{url}`)"
            )));
        }
        let gar_read_only = cfg
            .gar_category_read_only
            .as_deref()
            .unwrap_or(DEFAULT_GAR_CATEGORY_READ_ONLY)
            .to_owned();
        let gar_authenticated = cfg
            .gar_category_authenticated
            .as_deref()
            .unwrap_or(DEFAULT_GAR_CATEGORY_AUTH)
            .to_owned();
        let connect_timeout = Duration::from_millis(cfg.connect_timeout_millis.max(1));
        let padding_target = Duration::from_millis(cfg.padding_target_millis.max(1));
        let route_refresh_interval = Duration::from_secs(cfg.route_refresh_secs.max(1));
        Ok(Self {
            torii_ws_url: url.to_owned(),
            connect_timeout,
            padding_target,
            gar_read_only,
            gar_authenticated,
            spool_dir: cfg.spool_dir.clone(),
            route_refresh_interval,
        })
    }
    fn torii_ws_url(&self) -> &str {
        &self.torii_ws_url
    }
    fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }
    fn padding_target(&self) -> Duration {
        self.padding_target
    }
    fn gar_category(&self, authenticated: bool) -> &str {
        if authenticated {
            &self.gar_authenticated
        } else {
            &self.gar_read_only
        }
    }
    fn spool_dir(&self, relay_id: RelayId) -> Option<PathBuf> {
        let base = self.spool_dir.clone()?;
        let relay_hex = hex::encode(relay_id);
        let mut path = base.join(format!("exit-{relay_hex}"));
        path.push(NORITO_STREAM_SUBDIR);
        Some(path)
    }
    fn route_refresh_interval(&self) -> Duration {
        self.route_refresh_interval
    }
}
/// Kaigi streaming route configuration resolved from user config.
#[derive(Clone, Debug)]
pub struct KaigiStreamRoute {
    hub_ws_url: String,
    connect_timeout: Duration,
    gar_public: String,
    gar_authenticated: String,
    spool_dir: Option<PathBuf>,
    route_refresh_interval: Duration,
}
impl KaigiStreamRoute {
    fn from_config(cfg: &KaigiStreamRoutingConfig) -> Result<Self, ConfigError> {
        let url = cfg.hub_ws_url.trim();
        if url.is_empty() {
            return Err(ConfigError::Routing(
                "kaigi_stream.hub_ws_url must not be empty".to_string(),
            ));
        }
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err(ConfigError::Routing(format!(
                "kaigi_stream.hub_ws_url must start with ws:// or wss:// (got `{url}`)"
            )));
        }
        let gar_public = cfg
            .gar_category_public
            .as_deref()
            .unwrap_or(DEFAULT_KAIGI_CATEGORY_PUBLIC)
            .to_owned();
        let gar_authenticated = cfg
            .gar_category_authenticated
            .as_deref()
            .unwrap_or(DEFAULT_KAIGI_CATEGORY_AUTH)
            .to_owned();
        let connect_timeout = Duration::from_millis(cfg.connect_timeout_millis.max(1));
        let route_refresh_interval = Duration::from_secs(cfg.route_refresh_secs.max(1));
        Ok(Self {
            hub_ws_url: url.to_owned(),
            connect_timeout,
            gar_public,
            gar_authenticated,
            spool_dir: cfg.spool_dir.clone(),
            route_refresh_interval,
        })
    }
    fn hub_ws_url(&self) -> &str {
        &self.hub_ws_url
    }
    fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }
    fn gar_category(&self, authenticated: bool) -> &str {
        if authenticated {
            &self.gar_authenticated
        } else {
            &self.gar_public
        }
    }
    fn spool_dir(&self, relay_id: RelayId) -> Option<PathBuf> {
        let base = self.spool_dir.clone()?;
        let relay_hex = hex::encode(relay_id);
        let mut path = base.join(format!("exit-{relay_hex}"));
        path.push(KAIGI_STREAM_SUBDIR);
        Some(path)
    }
    fn route_refresh_interval(&self) -> Duration {
        self.route_refresh_interval
    }
}
/// Cached route record extracted from a Norito/Kaigi route update.
#[derive(Clone, Debug)]
pub struct RouteRecord {
    pub channel_id: [u8; 32],
    pub route_id: [u8; 32],
    pub stream_id: [u8; 32],
    pub exit_token: Vec<u8>,
    pub exit_multiaddr: String,
    pub padding_budget_ms: Option<u16>,
    pub access_kind: SoranetAccessKind,
    pub stream_tag: SoranetStreamTag,
    pub valid_from_segment: u64,
    pub valid_until_segment: u64,
}
struct RouteCatalog {
    root: PathBuf,
    refresh_interval: Duration,
    expected_tag: SoranetStreamTag,
    stream_label: &'static str,
    cache: Mutex<RouteCache>,
}
struct RouteCache {
    last_refresh: Option<Instant>,
    routes: HashMap<[u8; 32], RouteRecord>,
}
impl RouteCatalog {
    fn new(
        root: PathBuf,
        refresh_interval: Duration,
        expected_tag: SoranetStreamTag,
        stream_label: &'static str,
    ) -> Self {
        Self {
            root,
            refresh_interval,
            expected_tag,
            stream_label,
            cache: Mutex::new(RouteCache {
                last_refresh: None,
                routes: HashMap::new(),
            }),
        }
    }
    fn lookup(&self, channel_id: &[u8; 32]) -> Result<Option<RouteRecord>, RouteCatalogError> {
        let mut cache = self.cache.lock().expect("route catalog mutex poisoned");
        let refresh_due = match cache.last_refresh {
            None => true,
            Some(last) => last.elapsed() >= self.refresh_interval,
        };
        if refresh_due {
            cache.routes = self.scan()?;
            cache.last_refresh = Some(Instant::now());
        }
        Ok(cache.routes.get(channel_id).cloned())
    }
    fn scan(&self) -> Result<HashMap<[u8; 32], RouteRecord>, RouteCatalogError> {
        let mut records: HashMap<[u8; 32], RouteRecord> = HashMap::new();
        let mut directory_entries = 0usize;
        let mut route_files = 0usize;
        let mut encoded_bytes = 0usize;
        let mut route_heap_bytes = 0usize;
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(records),
            Err(error) => {
                return Err(RouteCatalogError::Io {
                    stream_label: self.stream_label,
                    path: self.root.clone(),
                    error,
                });
            }
        };
        for entry in entries {
            directory_entries = checked_add_with_limit(
                directory_entries,
                1,
                ROUTE_CATALOG_MAX_DIRECTORY_ENTRIES_V1,
            )
            .ok_or_else(|| RouteCatalogError::ResourceLimit {
                stream_label: self.stream_label,
                path: self.root.clone(),
                resource: "directory entries",
                found: directory_entries.saturating_add(1),
                maximum: ROUTE_CATALOG_MAX_DIRECTORY_ENTRIES_V1,
            })?;
            let entry = entry.map_err(|error| RouteCatalogError::Io {
                stream_label: self.stream_label,
                path: self.root.clone(),
                error,
            })?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(NORITO_FILE_EXTENSION) {
                continue;
            }
            route_files = checked_add_with_limit(route_files, 1, ROUTE_CATALOG_MAX_FILES_V1)
                .ok_or_else(|| RouteCatalogError::ResourceLimit {
                    stream_label: self.stream_label,
                    path: self.root.clone(),
                    resource: "route files",
                    found: route_files.saturating_add(1),
                    maximum: ROUTE_CATALOG_MAX_FILES_V1,
                })?;
            let canonical_name = parse_canonical_route_file_name(&path).map_err(|reason| {
                RouteCatalogError::Validation {
                    stream_label: self.stream_label,
                    path: path.clone(),
                    reason,
                }
            })?;
            let advertised =
                fs::symlink_metadata(&path).map_err(|error| RouteCatalogError::Io {
                    stream_label: self.stream_label,
                    path: path.clone(),
                    error,
                })?;
            validate_direct_route_file(&advertised).map_err(|error| RouteCatalogError::Io {
                stream_label: self.stream_label,
                path: path.clone(),
                error,
            })?;
            let advertised_len = usize::try_from(advertised.len()).map_err(|_| {
                RouteCatalogError::ResourceLimit {
                    stream_label: self.stream_label,
                    path: path.clone(),
                    resource: "route file bytes",
                    found: usize::MAX,
                    maximum: ROUTE_CATALOG_FILE_MAX_BYTES_V1,
                }
            })?;
            if advertised_len > ROUTE_CATALOG_FILE_MAX_BYTES_V1 {
                return Err(RouteCatalogError::ResourceLimit {
                    stream_label: self.stream_label,
                    path: path.clone(),
                    resource: "route file bytes",
                    found: advertised_len,
                    maximum: ROUTE_CATALOG_FILE_MAX_BYTES_V1,
                });
            }
            checked_add_with_limit(
                encoded_bytes,
                advertised_len,
                ROUTE_CATALOG_MAX_ENCODED_BYTES_V1,
            )
            .ok_or_else(|| RouteCatalogError::ResourceLimit {
                stream_label: self.stream_label,
                path: self.root.clone(),
                resource: "encoded bytes",
                found: encoded_bytes.saturating_add(advertised_len),
                maximum: ROUTE_CATALOG_MAX_ENCODED_BYTES_V1,
            })?;
            let bytes = read_bounded_direct_route_file(&path, ROUTE_CATALOG_FILE_MAX_BYTES_V1)
                .map_err(|error| RouteCatalogError::Io {
                    stream_label: self.stream_label,
                    path: path.clone(),
                    error,
                })?;
            encoded_bytes = checked_add_with_limit(
                encoded_bytes,
                bytes.len(),
                ROUTE_CATALOG_MAX_ENCODED_BYTES_V1,
            )
            .ok_or_else(|| RouteCatalogError::ResourceLimit {
                stream_label: self.stream_label,
                path: self.root.clone(),
                resource: "encoded bytes",
                found: encoded_bytes.saturating_add(bytes.len()),
                maximum: ROUTE_CATALOG_MAX_ENCODED_BYTES_V1,
            })?;
            let mut update: PrivacyRouteUpdate =
                decode_from_bytes_with_limits(&bytes, ROUTE_CATALOG_DECODE_LIMITS_V1).map_err(
                    |error| RouteCatalogError::Decode {
                        stream_label: self.stream_label,
                        path: path.clone(),
                        error,
                    },
                )?;
            validate_route_update(&update).map_err(|reason| RouteCatalogError::Validation {
                stream_label: self.stream_label,
                path: path.clone(),
                reason,
            })?;
            let route_id = update.route_id;
            let stream_id = update.stream_id;
            if canonical_name.route_id != route_id || canonical_name.stream_id != stream_id {
                return Err(RouteCatalogError::Validation {
                    stream_label: self.stream_label,
                    path: path.clone(),
                    reason: "route filename IDs do not match the decoded update".to_owned(),
                });
            }
            let Some(route) = update.soranet.take() else {
                continue;
            };
            if route.stream_tag != self.expected_tag {
                continue;
            }
            let record = RouteRecord::from_update(update, route);
            // Preserve the original read_dir traversal tie-break: an equal
            // valid-from segment replaces the route encountered earlier.
            let should_retain = records
                .get(&record.channel_id)
                .is_none_or(|existing| record.valid_from_segment >= existing.valid_from_segment);
            if !should_retain {
                continue;
            }
            let is_new_route = !records.contains_key(&record.channel_id);
            let previous_heap_bytes = records
                .get(&record.channel_id)
                .map_or(0, RouteRecord::heap_bytes);
            if is_new_route {
                if records.len() >= ROUTE_CATALOG_MAX_ROUTES_V1 {
                    return Err(RouteCatalogError::ResourceLimit {
                        stream_label: self.stream_label,
                        path: self.root.clone(),
                        resource: "retained routes",
                        found: records.len().saturating_add(1),
                        maximum: ROUTE_CATALOG_MAX_ROUTES_V1,
                    });
                }
                records
                    .try_reserve(1)
                    .map_err(|error| RouteCatalogError::Allocation {
                        stream_label: self.stream_label,
                        error,
                    })?;
            }
            let replacement_heap_bytes = record.heap_bytes();
            let next_route_heap_bytes = checked_replace_with_limit(
                route_heap_bytes,
                previous_heap_bytes,
                replacement_heap_bytes,
                ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
            )
            .ok_or_else(|| RouteCatalogError::ResourceLimit {
                stream_label: self.stream_label,
                path: self.root.clone(),
                resource: "retained route heap bytes",
                found: route_heap_bytes
                    .saturating_sub(previous_heap_bytes)
                    .saturating_add(replacement_heap_bytes),
                maximum: ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
            })?;
            let retained_bytes = route_map_bucket_bytes(records.capacity())
                .and_then(|bytes| bytes.checked_add(next_route_heap_bytes))
                .unwrap_or(usize::MAX);
            if retained_bytes > ROUTE_CATALOG_MAX_RETAINED_BYTES_V1 {
                return Err(RouteCatalogError::ResourceLimit {
                    stream_label: self.stream_label,
                    path: self.root.clone(),
                    resource: "aggregate retained bytes",
                    found: retained_bytes,
                    maximum: ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
                });
            }
            records.insert(record.channel_id, record);
            route_heap_bytes = next_route_heap_bytes;
        }
        Ok(records)
    }
}
/// Errors surfaced while reading or decoding route catalogs.
#[derive(Debug, Error)]
pub enum RouteCatalogError {
    #[error("failed to read {stream_label} catalog at `{path}`: {error}")]
    Io {
        stream_label: &'static str,
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("failed to decode {stream_label} route `{path}`: {error}")]
    Decode {
        stream_label: &'static str,
        path: PathBuf,
        error: NoritoError,
    },
    /// A catalog count or byte budget was exceeded.
    #[error(
        "{stream_label} catalog `{path}` has {found} {resource}; first-release limit is {maximum}"
    )]
    ResourceLimit {
        /// Stable stream label used in diagnostics.
        stream_label: &'static str,
        /// Catalog root or offending route file.
        path: PathBuf,
        /// Counted resource name.
        resource: &'static str,
        /// Observed count or byte size.
        found: usize,
        /// Maximum admitted count or byte size.
        maximum: usize,
    },
    /// A decoded update or its producer filename was not canonical.
    #[error("invalid {stream_label} route `{path}`: {reason}")]
    Validation {
        /// Stable stream label used in diagnostics.
        stream_label: &'static str,
        /// Offending route file.
        path: PathBuf,
        /// Validation failure without payload contents.
        reason: String,
    },
    /// A bounded route-map reservation failed.
    #[error("failed to reserve {stream_label} route catalog: {error}")]
    Allocation {
        /// Stable stream label used in diagnostics.
        stream_label: &'static str,
        /// Allocation failure detail.
        error: std::collections::TryReserveError,
    },
}
impl RouteRecord {
    fn from_update(update: PrivacyRouteUpdate, route: SoranetRoute) -> Self {
        let channel_id: [u8; 32] = route.channel_id.into();
        let route_id = update.route_id;
        let stream_id = update.stream_id;
        Self {
            channel_id,
            route_id,
            stream_id,
            exit_token: update.exit_token,
            exit_multiaddr: route.exit_multiaddr,
            padding_budget_ms: route.padding_budget_ms,
            access_kind: route.access_kind,
            stream_tag: route.stream_tag,
            valid_from_segment: update.valid_from_segment,
            valid_until_segment: update.valid_until_segment,
        }
    }
    fn heap_bytes(&self) -> usize {
        self.exit_token
            .capacity()
            .saturating_add(self.exit_multiaddr.capacity())
    }
}
fn validate_route_update(update: &PrivacyRouteUpdate) -> Result<(), String> {
    let route_id = update.route_id;
    if route_id.iter().all(|byte| *byte == 0) {
        return Err("route id must not be all zeroes".to_owned());
    }
    let stream_id = update.stream_id;
    if stream_id.iter().all(|byte| *byte == 0) {
        return Err("stream id must not be all zeroes".to_owned());
    }
    if update.valid_until_segment < update.valid_from_segment {
        return Err("route validity window ends before it starts".to_owned());
    }
    if update.exit_token.len() > ROUTE_EXIT_TOKEN_MAX_BYTES_V1 {
        return Err(format!(
            "exit token is {} bytes; first-release limit is {ROUTE_EXIT_TOKEN_MAX_BYTES_V1}",
            update.exit_token.len()
        ));
    }
    if let Some(route) = &update.soranet {
        if route.channel_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err("SoraNet channel id must not be all zeroes".to_owned());
        }
        if route.exit_multiaddr.len() > ROUTE_MULTIADDR_MAX_BYTES_V1 {
            return Err(format!(
                "exit multiaddr is {} bytes; first-release limit is {ROUTE_MULTIADDR_MAX_BYTES_V1}",
                route.exit_multiaddr.len()
            ));
        }
    }
    Ok(())
}
fn route_map_bucket_bytes(capacity: usize) -> Option<usize> {
    capacity
        .checked_mul(std::mem::size_of::<([u8; 32], RouteRecord)>())
        // Include hash-table control bytes and allocator rounding.
        .and_then(|bytes| bytes.checked_mul(2))
}
/// Prepared Norito streaming state with optional on-disk route catalog.
#[derive(Clone)]
pub struct NoritoStreamState {
    config: NoritoStreamRoute,
    catalog: Option<Arc<RouteCatalog>>,
}
impl NoritoStreamState {
    fn new(config: NoritoStreamRoute, relay_id: RelayId) -> Self {
        let catalog = config.spool_dir(relay_id).map(|root| {
            Arc::new(RouteCatalog::new(
                root,
                config.route_refresh_interval(),
                SoranetStreamTag::NoritoStream,
                NORITO_STREAM_LABEL,
            ))
        });
        Self { config, catalog }
    }
    pub fn connect_timeout(&self) -> Duration {
        self.config.connect_timeout()
    }
    pub fn padding_target(&self) -> Duration {
        self.config.padding_target()
    }
    pub fn torii_ws_url(&self) -> &str {
        self.config.torii_ws_url()
    }
    pub fn gar_category(&self, authenticated: bool) -> &str {
        self.config.gar_category(authenticated)
    }
    pub fn lookup_channel(
        &self,
        channel_id: &[u8; 32],
    ) -> Result<Option<RouteRecord>, RouteCatalogError> {
        match &self.catalog {
            Some(catalog) => catalog.lookup(channel_id),
            None => Ok(None),
        }
    }
}
/// Prepared Kaigi streaming state with optional on-disk route catalog.
#[derive(Clone)]
pub struct KaigiStreamState {
    config: KaigiStreamRoute,
    catalog: Option<Arc<RouteCatalog>>,
}
impl KaigiStreamState {
    fn new(config: KaigiStreamRoute, relay_id: RelayId) -> Self {
        let catalog = config.spool_dir(relay_id).map(|root| {
            Arc::new(RouteCatalog::new(
                root,
                config.route_refresh_interval(),
                SoranetStreamTag::Kaigi,
                KAIGI_STREAM_LABEL,
            ))
        });
        Self { config, catalog }
    }
    pub fn connect_timeout(&self) -> Duration {
        self.config.connect_timeout()
    }
    pub fn hub_ws_url(&self) -> &str {
        self.config.hub_ws_url()
    }
    pub fn gar_category(&self, authenticated: bool) -> &str {
        self.config.gar_category(authenticated)
    }
    pub fn lookup_channel(
        &self,
        channel_id: &[u8; 32],
    ) -> Result<Option<RouteRecord>, RouteCatalogError> {
        match &self.catalog {
            Some(catalog) => catalog.lookup(channel_id),
            None => Ok(None),
        }
    }
}
/// Derive a blinded Kaigi room identifier from route metadata.
///
/// The derivation uses BLAKE3 with a fixed domain separator so room identifiers
/// remain stable across relays while hiding the underlying channel and route
/// identifiers from observers.
#[must_use]
pub fn derive_kaigi_room_id(
    channel_id: &[u8; 32],
    route_id: &[u8; 32],
    stream_id: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(KAIGI_ROOM_DOMAIN_TAG);
    hasher.update(channel_id);
    hasher.update(route_id);
    hasher.update(stream_id);
    let digest = hasher.finalize();
    let mut room_id = [0u8; 32];
    room_id.copy_from_slice(digest.as_bytes());
    room_id
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitStreamTag {
    /// Norito streaming route.
    NoritoStream,
    /// Kaigi streaming route.
    KaigiStream,
}
impl ExitStreamTag {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::NoritoStream),
            0x02 => Some(Self::KaigiStream),
            _ => None,
        }
    }
    pub fn as_u8(self) -> u8 {
        match self {
            Self::NoritoStream => 0x01,
            Self::KaigiStream => 0x02,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteFlags(u8);
impl RouteFlags {
    /// Flag indicating an authenticated exit route.
    pub const AUTHENTICATED: u8 = 0x01;
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u8 {
        self.0
    }
    pub const fn is_authenticated(self) -> bool {
        (self.0 & Self::AUTHENTICATED) != 0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteOpenFrame {
    tag: ExitStreamTag,
    flags: RouteFlags,
    channel_id: [u8; 32],
}
impl RouteOpenFrame {
    /// Fixed length of the on-wire route-open frame.
    pub const fn length() -> usize {
        ROUTE_OPEN_FRAME_LEN
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, RouteOpenFrameError> {
        if bytes.len() != ROUTE_OPEN_FRAME_LEN {
            return Err(RouteOpenFrameError::InvalidLength(bytes.len()));
        }
        let tag_byte = bytes[0];
        let Some(tag) = ExitStreamTag::from_u8(tag_byte) else {
            return Err(RouteOpenFrameError::UnknownTag(tag_byte));
        };
        let flags = RouteFlags::new(bytes[1]);
        let mut channel_id = [0u8; 32];
        channel_id.copy_from_slice(&bytes[2..34]);
        if channel_id.iter().all(|&b| b == 0) {
            return Err(RouteOpenFrameError::ZeroChannelId);
        }
        Ok(Self {
            tag,
            flags,
            channel_id,
        })
    }
    pub const fn tag(&self) -> ExitStreamTag {
        self.tag
    }
    pub const fn flags(&self) -> RouteFlags {
        self.flags
    }
    pub const fn channel_id(&self) -> &[u8; 32] {
        &self.channel_id
    }
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteOpenFrameError {
    /// Frame length does not match the expected 34-byte payload.
    #[error("route open frame length must be {ROUTE_OPEN_FRAME_LEN} bytes (got {0})")]
    InvalidLength(usize),
    /// Unknown stream tag detected in the first byte.
    #[error("unknown stream tag: {0:#04x}")]
    UnknownTag(u8),
    /// Channel ID was all zeroes, which is invalid.
    #[error("channel identifier must not be all zeros")]
    ZeroChannelId,
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::{
        streaming::{PrivacyRouteUpdate, SoranetChannelId, SoranetRoute, SoranetStreamTag},
        to_bytes,
    };
    use tempfile::TempDir;
    fn sample_route() -> NoritoStreamRoutingConfig {
        NoritoStreamRoutingConfig {
            torii_ws_url: "wss://torii.test/norito/stream".into(),
            connect_timeout_millis: 750,
            padding_target_millis: 45,
            gar_category_read_only: None,
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 5,
        }
    }
    fn sample_kaigi_route() -> KaigiStreamRoutingConfig {
        KaigiStreamRoutingConfig {
            hub_ws_url: "wss://kaigi.test/hub".into(),
            connect_timeout_millis: 900,
            gar_category_public: None,
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 6,
        }
    }
    #[test]
    fn norito_route_defaults_apply() {
        let cfg = sample_route();
        let route = NoritoStreamRoute::from_config(&cfg).expect("route config");
        assert_eq!(route.torii_ws_url(), cfg.torii_ws_url);
        assert_eq!(
            route.connect_timeout(),
            Duration::from_millis(cfg.connect_timeout_millis)
        );
        assert_eq!(
            route.padding_target(),
            Duration::from_millis(cfg.padding_target_millis)
        );
        assert_eq!(route.gar_category(false), DEFAULT_GAR_CATEGORY_READ_ONLY);
        assert_eq!(route.gar_category(true), DEFAULT_GAR_CATEGORY_AUTH);
    }
    #[test]
    fn kaigi_route_defaults_apply() {
        let cfg = sample_kaigi_route();
        let route = KaigiStreamRoute::from_config(&cfg).expect("route config");
        assert_eq!(route.hub_ws_url(), cfg.hub_ws_url);
        assert_eq!(
            route.connect_timeout(),
            Duration::from_millis(cfg.connect_timeout_millis)
        );
        assert_eq!(route.gar_category(false), DEFAULT_KAIGI_CATEGORY_PUBLIC);
        assert_eq!(route.gar_category(true), DEFAULT_KAIGI_CATEGORY_AUTH);
    }
    #[test]
    fn exit_routing_constructs_optional_route() {
        let cfg = ExitRoutingConfig {
            norito_stream: Some(sample_route()),
            kaigi_stream: Some(sample_kaigi_route()),
        };
        let routing = ExitRouting::from_config(&cfg).expect("routing config");
        let relay_id = [0xAA; 32];
        let state = routing.prepare(relay_id);
        assert!(state.norito_stream().is_some());
        assert!(state.kaigi_stream().is_some());
    }
    #[test]
    fn exit_routing_uses_relay_scoped_spool_dirs() {
        let temp = TempDir::new().expect("temp dir");
        let relay_id = [0x42; 32];
        let channel_id = [0x99; 32];
        let mut norito_cfg = sample_route();
        norito_cfg.spool_dir = Some(temp.path().to_path_buf());
        norito_cfg.route_refresh_secs = 0;
        let mut kaigi_cfg = sample_kaigi_route();
        kaigi_cfg.spool_dir = Some(temp.path().to_path_buf());
        kaigi_cfg.route_refresh_secs = 0;
        let routing = ExitRouting::from_config(&ExitRoutingConfig {
            norito_stream: Some(norito_cfg),
            kaigi_stream: Some(kaigi_cfg),
        })
        .expect("routing config");
        let state = routing.prepare(relay_id);
        let spool_root = temp.path().join(format!("exit-{}", hex::encode(relay_id)));
        let write_update = |dir: &std::path::Path,
                            route_id: [u8; 32],
                            stream_tag: SoranetStreamTag,
                            exit_multiaddr: &str| {
            let update = PrivacyRouteUpdate {
                route_id,
                stream_id: [0x21; 32],
                content_key_id: 5,
                valid_from_segment: 10,
                valid_until_segment: 20,
                exit_token: vec![0xEE, 0xFF],
                soranet: Some(SoranetRoute {
                    channel_id: SoranetChannelId::new(channel_id),
                    exit_multiaddr: exit_multiaddr.into(),
                    padding_budget_ms: Some(17),
                    access_kind: SoranetAccessKind::Authenticated,
                    stream_tag,
                }),
            };
            let file_name = format!(
                "0000000000000001-route-{}-stream-{}.{}",
                hex::encode(route_id),
                hex::encode(update.stream_id),
                NORITO_FILE_EXTENSION
            );
            fs::create_dir_all(dir).expect("create spool dir");
            let bytes = to_bytes(&update).expect("encode route");
            fs::write(dir.join(file_name), bytes).expect("write route");
        };
        write_update(
            &spool_root.join(NORITO_STREAM_SUBDIR),
            [0x11; 32],
            SoranetStreamTag::NoritoStream,
            "/dns/norito.example/tcp/443/quic",
        );
        write_update(
            &spool_root.join(KAIGI_STREAM_SUBDIR),
            [0x22; 32],
            SoranetStreamTag::Kaigi,
            "/dns/kaigi.example/tcp/443/quic",
        );
        let norito_record = state
            .norito_stream()
            .expect("norito route configured")
            .lookup_channel(&channel_id)
            .expect("catalog lookup")
            .expect("route present");
        assert_eq!(norito_record.stream_tag, SoranetStreamTag::NoritoStream);
        assert_eq!(
            norito_record.exit_multiaddr,
            "/dns/norito.example/tcp/443/quic"
        );
        let kaigi_record = state
            .kaigi_stream()
            .expect("kaigi route configured")
            .lookup_channel(&channel_id)
            .expect("catalog lookup")
            .expect("route present");
        assert_eq!(kaigi_record.stream_tag, SoranetStreamTag::Kaigi);
        assert_eq!(
            kaigi_record.exit_multiaddr,
            "/dns/kaigi.example/tcp/443/quic"
        );
    }
    #[test]
    fn route_open_decodes_norito_stream() {
        let mut bytes = [0u8; ROUTE_OPEN_FRAME_LEN];
        bytes[0] = ExitStreamTag::NoritoStream.as_u8();
        bytes[1] = RouteFlags::AUTHENTICATED;
        bytes[2..34].copy_from_slice(&[0xAA; 32]);
        let frame = RouteOpenFrame::decode(&bytes).expect("decode frame");
        assert_eq!(frame.tag(), ExitStreamTag::NoritoStream);
        assert!(frame.flags().is_authenticated());
        assert_eq!(frame.channel_id(), &[0xAA; 32]);
    }
    #[test]
    fn route_open_decodes_kaigi_stream() {
        let mut bytes = [0u8; ROUTE_OPEN_FRAME_LEN];
        bytes[0] = ExitStreamTag::KaigiStream.as_u8();
        bytes[2..34].copy_from_slice(&[0xBB; 32]);
        let frame = RouteOpenFrame::decode(&bytes).expect("decode frame");
        assert_eq!(frame.tag(), ExitStreamTag::KaigiStream);
        assert!(!frame.flags().is_authenticated());
        assert_eq!(frame.channel_id(), &[0xBB; 32]);
    }
    #[test]
    fn route_open_rejects_unknown_tag() {
        let mut bytes = [0u8; ROUTE_OPEN_FRAME_LEN];
        bytes[0] = 0xFF;
        bytes[2..34].copy_from_slice(&[0x01; 32]);
        let err = RouteOpenFrame::decode(&bytes).expect_err("decode fails");
        assert_eq!(err, RouteOpenFrameError::UnknownTag(0xFF));
    }
    #[test]
    fn route_open_rejects_zero_channel() {
        let mut bytes = [0u8; ROUTE_OPEN_FRAME_LEN];
        bytes[0] = ExitStreamTag::NoritoStream.as_u8();
        let err = RouteOpenFrame::decode(&bytes).expect_err("decode fails");
        assert_eq!(err, RouteOpenFrameError::ZeroChannelId);
    }
    #[test]
    fn derive_kaigi_room_id_blinds_channel_and_route_ids() {
        let channel_id = [0x01; 32];
        let route_id = [0x02; 32];
        let stream_id = [0x03; 32];
        let room_id = derive_kaigi_room_id(&channel_id, &route_id, &stream_id);
        assert_eq!(
            room_id,
            derive_kaigi_room_id(&channel_id, &route_id, &stream_id),
            "derivation must be stable"
        );
        assert_ne!(room_id, channel_id, "room id must not leak the channel id");
        assert_ne!(room_id, route_id, "room id must not reuse the route id");
        assert_ne!(room_id, stream_id, "room id must not reuse the stream id");
        let changed_route = derive_kaigi_room_id(&channel_id, &[0x04; 32], &stream_id);
        let changed_stream = derive_kaigi_room_id(&channel_id, &route_id, &[0x05; 32]);
        assert_ne!(
            room_id, changed_route,
            "route id changes must produce distinct room ids"
        );
        assert_ne!(
            room_id, changed_stream,
            "stream id changes must produce distinct room ids"
        );
    }
    #[test]
    fn route_catalog_returns_latest_route() {
        let temp = TempDir::new().expect("temp dir");
        let catalog_root = temp.path();
        let channel_id = [0x11; 32];
        let route_id_old = [0x22; 32];
        let route_id_new = [0x33; 32];
        let stream_id = [0x44; 32];
        let make_update = |route_id: [u8; 32], valid_from: u64| PrivacyRouteUpdate {
            route_id,
            stream_id,
            content_key_id: 7,
            valid_from_segment: valid_from,
            valid_until_segment: valid_from + 10,
            exit_token: vec![0xAA, 0xBB],
            soranet: Some(SoranetRoute {
                channel_id: SoranetChannelId::new(channel_id),
                exit_multiaddr: "/dns/torii/tcp/443/quic".into(),
                padding_budget_ms: Some(23),
                access_kind: SoranetAccessKind::Authenticated,
                stream_tag: SoranetStreamTag::NoritoStream,
            }),
        };
        let write_update = |root: &std::path::Path, update: PrivacyRouteUpdate, seq: u32| {
            let file_name = format!(
                "{seq:016x}-route-{}-stream-{}.{}",
                hex::encode(update.route_id),
                hex::encode(update.stream_id),
                NORITO_FILE_EXTENSION
            );
            let path = root.join(file_name);
            fs::create_dir_all(root).expect("create spool dir");
            let bytes = to_bytes(&update).expect("encode norito");
            fs::write(path, bytes).expect("write norito file");
        };
        let older = make_update(route_id_old, 10);
        let newer = make_update(route_id_new, 42);
        write_update(catalog_root, older, 0);
        write_update(catalog_root, newer, 1);
        let catalog = RouteCatalog::new(
            catalog_root.to_path_buf(),
            Duration::from_millis(0),
            SoranetStreamTag::NoritoStream,
            NORITO_STREAM_LABEL,
        );
        let record = catalog
            .lookup(&channel_id)
            .expect("catalog lookup")
            .expect("route present");
        assert_eq!(record.route_id, route_id_new);
        assert_eq!(record.valid_from_segment, 42);
        assert_eq!(record.valid_until_segment, 52);
        assert_eq!(record.exit_multiaddr, "/dns/torii/tcp/443/quic");
        assert_eq!(record.padding_budget_ms, Some(23));
        assert_eq!(record.access_kind, SoranetAccessKind::Authenticated);
        assert_eq!(record.stream_tag, SoranetStreamTag::NoritoStream);
    }
    #[test]
    fn route_catalog_filters_by_stream_tag() {
        let temp = TempDir::new().expect("temp dir");
        let catalog_root = temp.path();
        let channel_id = [0x55; 32];
        let update = PrivacyRouteUpdate {
            route_id: [0x11; 32],
            stream_id: [0x22; 32],
            content_key_id: 3,
            valid_from_segment: 4,
            valid_until_segment: 8,
            exit_token: vec![0xBB, 0xCC],
            soranet: Some(SoranetRoute {
                channel_id: SoranetChannelId::new(channel_id),
                exit_multiaddr: "/dns/kaigi.example/tcp/9944/ws".into(),
                padding_budget_ms: None,
                access_kind: SoranetAccessKind::ReadOnly,
                stream_tag: SoranetStreamTag::Kaigi,
            }),
        };
        let file_name = format!(
            "0000000000000001-route-{}-stream-{}.{}",
            hex::encode(update.route_id),
            hex::encode(update.stream_id),
            NORITO_FILE_EXTENSION
        );
        let path = catalog_root.join(file_name);
        fs::create_dir_all(catalog_root).expect("create catalog dir");
        let bytes = to_bytes(&update).expect("encode kaigi route");
        fs::write(path, bytes).expect("write kaigi route");
        let norito_catalog = RouteCatalog::new(
            catalog_root.to_path_buf(),
            Duration::from_secs(0),
            SoranetStreamTag::NoritoStream,
            NORITO_STREAM_LABEL,
        );
        assert!(
            norito_catalog
                .lookup(&channel_id)
                .expect("norito lookup succeeds")
                .is_none(),
            "catalog configured for norito must skip kaigi routes"
        );
        let kaigi_catalog = RouteCatalog::new(
            catalog_root.to_path_buf(),
            Duration::from_secs(0),
            SoranetStreamTag::Kaigi,
            KAIGI_STREAM_LABEL,
        );
        let record = kaigi_catalog
            .lookup(&channel_id)
            .expect("kaigi lookup succeeds")
            .expect("kaigi route expected");
        assert_eq!(record.stream_tag, SoranetStreamTag::Kaigi);
    }
    #[test]
    fn route_catalog_respects_refresh_interval() {
        let temp = TempDir::new().expect("temp dir");
        let catalog_root = temp.path();
        let channel_id = [0x51; 32];
        let route_id_initial = [0x99; 32];
        let route_id_updated = [0xA5; 32];
        let stream_id = [0x77; 32];
        let make_update = |route_id: [u8; 32], valid_from: u64| PrivacyRouteUpdate {
            route_id,
            stream_id,
            content_key_id: 19,
            valid_from_segment: valid_from,
            valid_until_segment: valid_from + 5,
            exit_token: vec![0xDE, 0xAD, 0xBE, 0xEF],
            soranet: Some(SoranetRoute {
                channel_id: SoranetChannelId::new(channel_id),
                exit_multiaddr: "/dns/torii.refresh.test/tcp/7443/quic".into(),
                padding_budget_ms: Some(11),
                access_kind: SoranetAccessKind::ReadOnly,
                stream_tag: SoranetStreamTag::NoritoStream,
            }),
        };
        let write_update = |root: &std::path::Path, update: PrivacyRouteUpdate, seq: u32| {
            let file_name = format!(
                "{seq:016x}-route-{}-stream-{}.{}",
                hex::encode(update.route_id),
                hex::encode(update.stream_id),
                NORITO_FILE_EXTENSION
            );
            let path = root.join(file_name);
            fs::create_dir_all(root).expect("create spool dir");
            let bytes = to_bytes(&update).expect("encode norito");
            fs::write(path, bytes).expect("write norito file");
        };
        write_update(catalog_root, make_update(route_id_initial, 2), 0);
        let catalog = RouteCatalog::new(
            catalog_root.to_path_buf(),
            Duration::from_secs(1),
            SoranetStreamTag::NoritoStream,
            NORITO_STREAM_LABEL,
        );
        let initial = catalog
            .lookup(&channel_id)
            .expect("catalog lookup")
            .expect("route present");
        assert_eq!(initial.route_id, route_id_initial);
        assert_eq!(initial.valid_from_segment, 2);
        assert_eq!(initial.stream_tag, SoranetStreamTag::NoritoStream);
        write_update(catalog_root, make_update(route_id_updated, 7), 1);
        let cached = catalog
            .lookup(&channel_id)
            .expect("catalog lookup")
            .expect("route present");
        assert_eq!(cached.route_id, route_id_initial);
        assert_eq!(cached.stream_tag, SoranetStreamTag::NoritoStream);
        std::thread::sleep(Duration::from_millis(1100));
        let refreshed = catalog
            .lookup(&channel_id)
            .expect("catalog lookup")
            .expect("route present");
        assert_eq!(refreshed.route_id, route_id_updated);
        assert_eq!(refreshed.valid_from_segment, 7);
        assert_eq!(refreshed.stream_tag, SoranetStreamTag::NoritoStream);
    }
    #[test]
    fn derive_room_id_depends_on_route_metadata() {
        let channel = [0xA1; 32];
        let route_one = [0xB2; 32];
        let route_two = [0xC3; 32];
        let stream = [0xD4; 32];
        let room_one = derive_kaigi_room_id(&channel, &route_one, &stream);
        let room_two = derive_kaigi_room_id(&channel, &route_two, &stream);
        assert_ne!(room_one, channel, "room id must not leak the channel id");
        assert_ne!(
            room_one, room_two,
            "changing the route id must yield a different room id"
        );
    }
    #[test]
    fn route_catalog_file_limit_accepts_exact_and_rejects_plus_one() {
        let temp = TempDir::new().expect("temp dir");
        let exact = temp.path().join("exact.norito");
        fs::write(&exact, vec![0xA5; ROUTE_CATALOG_FILE_MAX_BYTES_V1])
            .expect("write exact route file");
        assert_eq!(
            read_bounded_direct_route_file(&exact, ROUTE_CATALOG_FILE_MAX_BYTES_V1)
                .expect("exact file boundary")
                .len(),
            ROUTE_CATALOG_FILE_MAX_BYTES_V1
        );
        let oversized = temp.path().join("oversized.norito");
        fs::write(&oversized, vec![0xA5; ROUTE_CATALOG_FILE_MAX_BYTES_V1 + 1])
            .expect("write oversized route file");
        assert!(
            read_bounded_direct_route_file(&oversized, ROUTE_CATALOG_FILE_MAX_BYTES_V1).is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn route_catalog_file_read_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = TempDir::new().expect("temp dir");
        let target = temp.path().join("target.norito");
        let link = temp.path().join("link.norito");
        fs::write(&target, b"route").expect("write route target");
        symlink(&target, &link).expect("create route symlink");
        assert!(read_bounded_direct_route_file(&link, ROUTE_CATALOG_FILE_MAX_BYTES_V1).is_err());
    }
    #[test]
    fn route_catalog_count_and_byte_limits_accept_exact_and_reject_plus_one() {
        for maximum in [
            ROUTE_CATALOG_MAX_DIRECTORY_ENTRIES_V1,
            ROUTE_CATALOG_MAX_FILES_V1,
            ROUTE_CATALOG_MAX_ROUTES_V1,
            ROUTE_CATALOG_MAX_ENCODED_BYTES_V1,
        ] {
            assert_eq!(
                checked_add_with_limit(maximum - 1, 1, maximum),
                Some(maximum)
            );
            assert_eq!(checked_add_with_limit(maximum, 1, maximum), None);
        }
        assert_eq!(
            checked_replace_with_limit(
                ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
                1,
                1,
                ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
            ),
            Some(ROUTE_CATALOG_MAX_RETAINED_BYTES_V1)
        );
        assert_eq!(
            checked_replace_with_limit(
                ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
                1,
                2,
                ROUTE_CATALOG_MAX_RETAINED_BYTES_V1,
            ),
            None
        );
    }
    #[test]
    fn route_update_field_limits_accept_exact_and_reject_plus_one() {
        let mut update = PrivacyRouteUpdate {
            route_id: [0x11; 32],
            stream_id: [0x22; 32],
            content_key_id: 3,
            valid_from_segment: 4,
            valid_until_segment: 8,
            exit_token: vec![0xA5; ROUTE_EXIT_TOKEN_MAX_BYTES_V1],
            soranet: Some(SoranetRoute {
                channel_id: SoranetChannelId::new([0x33; 32]),
                exit_multiaddr: "x".repeat(ROUTE_MULTIADDR_MAX_BYTES_V1),
                padding_budget_ms: None,
                access_kind: SoranetAccessKind::ReadOnly,
                stream_tag: SoranetStreamTag::NoritoStream,
            }),
        };
        validate_route_update(&update).expect("exact field boundaries");
        update.valid_until_segment = update.valid_from_segment;
        validate_route_update(&update).expect("one inclusive segment remains valid");
        update.exit_token.push(0xA5);
        assert!(validate_route_update(&update).is_err());
        update.exit_token.pop();
        update
            .soranet
            .as_mut()
            .expect("route")
            .exit_multiaddr
            .push('x');
        assert!(validate_route_update(&update).is_err());
    }
    #[test]
    fn route_catalog_rejects_filename_payload_mismatch() {
        let temp = TempDir::new().expect("temp dir");
        let channel_id = [0x44; 32];
        let update = PrivacyRouteUpdate {
            route_id: [0x11; 32],
            stream_id: [0x22; 32],
            content_key_id: 3,
            valid_from_segment: 4,
            valid_until_segment: 8,
            exit_token: vec![0xA5],
            soranet: Some(SoranetRoute {
                channel_id: SoranetChannelId::new(channel_id),
                exit_multiaddr: "/dns/torii/tcp/443/quic".into(),
                padding_budget_ms: None,
                access_kind: SoranetAccessKind::ReadOnly,
                stream_tag: SoranetStreamTag::NoritoStream,
            }),
        };
        let path = temp.path().join(format!(
            "0000000000000001-route-{}-stream-{}.norito",
            hex::encode([0x99; 32]),
            hex::encode(update.stream_id),
        ));
        fs::write(&path, to_bytes(&update).expect("encode update")).expect("write update");
        let catalog = RouteCatalog::new(
            temp.path().to_path_buf(),
            Duration::ZERO,
            SoranetStreamTag::NoritoStream,
            NORITO_STREAM_LABEL,
        );
        assert!(matches!(
            catalog.lookup(&channel_id),
            Err(RouteCatalogError::Validation { .. })
        ));
    }
    #[test]
    fn route_catalog_returns_none_when_empty() {
        let temp = TempDir::new().expect("temp dir");
        let catalog = RouteCatalog::new(
            temp.path().to_path_buf(),
            Duration::from_secs(1),
            SoranetStreamTag::NoritoStream,
            NORITO_STREAM_LABEL,
        );
        let channel_id = [0xAA; 32];
        let result = catalog.lookup(&channel_id).expect("catalog lookup");
        assert!(result.is_none());
    }
}

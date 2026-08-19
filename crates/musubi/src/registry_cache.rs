//! Authenticated local cache of finalized Musubi V1 resolver inputs.
//!
//! The cache stores only canonical public query requests and the finalized,
//! validated namespace-directory and sparse-index pages returned for those
//! requests.  It deliberately excludes endpoint URLs, credentials, bearer
//! material, provider locations, timestamps, and filesystem paths.  A cache
//! snapshot is published only after one successful graph collection observed a
//! single exact network, finalized block, and index revision.
//!
//! Torii's current finalized query pages do not carry a portable consensus
//! inclusion proof.  Cache authenticity is therefore rooted in the online
//! reader's validation plus the private, identity-checked user cache directory; the
//! domain-separated snapshot commitment detects subsequent corruption. Linux and Android bind
//! catalog reads to the retained cache-root descriptor through `/proc/self/fd`; other platforms
//! reject offline catalog reads until an equivalent safe descriptor-rooted primitive is available.
//! TODO: Verify and retain a portable finalized-state inclusion proof here once the public query
//! contract exposes one.
use crate::{
    atomic_io::{AtomicWriteError, AtomicWriteErrorCode, AtomicWriteRoot},
    cache::{CacheError, MusubiCache},
    graph::{GraphErrorV1, ResolverRegistrySourceV1},
    lockfile::LockfileV1,
    registry::{RegistryErrorV1, RegistryReadClientV1},
};
use iroha_data_model::{
    NetworkId,
    musubi::{
        MUSUBI_MAX_RESOLUTION_NODES_V1, MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1,
        MusubiPackageIdV1, MusubiPackageSelectorV1, MusubiRegistrySnapshotV1,
        MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1,
    },
};
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};
const CACHE_SCHEMA: &str = "musubi-resolver-cache";
const CACHE_VERSION: u8 = 1;
const CACHE_FILE: &str = "resolver-index-v1.norito";
const SNAPSHOT_DIGEST_DOMAIN: &[u8] = b"musubi-resolver-cache-snapshot-v1\0";
const MAX_CACHED_SNAPSHOTS_V1: usize = 16;
const MAX_CACHED_PAGES_V1: usize = MUSUBI_MAX_RESOLUTION_NODES_V1 * 17;
const MAX_CACHED_ROW_OCCURRENCES_V1: usize = MUSUBI_MAX_RESOLUTION_NODES_V1 * 64;
const MAX_CACHE_FILE_BYTES_V1: u64 = 64 * 1024 * 1024;
const MAX_CACHE_FILE_BYTES_USIZE_V1: usize = 64 * 1024 * 1024;
const MAX_CACHE_DECODE_ELEMENTS_V1: usize = 1_000_000;
const MAX_CACHE_DECODE_ALLOCATION_V1: usize = 128 * 1024 * 1024;
const CACHE_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MAX_CACHED_ROW_OCCURRENCES_V1,
    MAX_CACHE_FILE_BYTES_USIZE_V1,
    MAX_CACHE_DECODE_ELEMENTS_V1,
    MAX_CACHE_DECODE_ALLOCATION_V1,
    64,
);
const COMMIT_RETRIES: usize = 4;
/// One exact ordered-prefix request and validated finalized response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct CachedOrderedPageV1 {
    request: MusubiOrderedPrefixQueryV1,
    response: MusubiOrderedPackagePageV1,
}
/// One exact resolver-index request and validated finalized response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct CachedResolverPageV1 {
    request: MusubiResolverIndexQueryV1,
    response: MusubiResolverIndexPageV1,
}
/// Complete coherent set of pages consumed by one successful graph collection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ResolverIndexCacheSnapshotV1 {
    network_id: NetworkId,
    account_chain_discriminant: u16,
    snapshot: MusubiRegistrySnapshotV1,
    ordered_pages: Vec<CachedOrderedPageV1>,
    resolver_pages: Vec<CachedResolverPageV1>,
}
impl ResolverIndexCacheSnapshotV1 {
    #[expect(
        clippy::too_many_lines,
        reason = "cache snapshot admission validates every deployment, page-order, cursor, and coherence invariant in one fail-closed pass"
    )]
    fn validate(&self) -> Result<(), ResolverIndexCacheErrorV1> {
        if self.network_id.as_bytes()[31] & 1 != 1 || self.account_chain_discriminant == 0 {
            return Err(invalid("cache snapshot has an invalid deployment identity"));
        }
        self.snapshot
            .validate()
            .map_err(|error| invalid(error.reason()))?;
        if self.ordered_pages.is_empty()
            || self.ordered_pages.len() > MAX_CACHED_PAGES_V1
            || self.resolver_pages.len() > MAX_CACHED_PAGES_V1
        {
            return Err(invalid("cache snapshot page count is invalid"));
        }
        validate_canonical_order(&self.ordered_pages, |page| Ok(page.request.encode()))?;
        validate_canonical_order(&self.resolver_pages, |page| Ok(page.request.encode()))?;
        let mut bindings = BTreeMap::new();
        let mut directory = BTreeMap::new();
        for page in &self.ordered_pages {
            page.request
                .prefix
                .validate()
                .map_err(|error| invalid(error.reason()))?;
            page.request
                .page
                .validate()
                .map_err(|error| invalid(error.reason()))?;
            page.response
                .validate_for(&page.request)
                .map_err(|error| invalid(error.reason()))?;
            self.validate_anchor(page.response.network_id, page.response.snapshot)?;
            validate_request_cursor(page.request.page.cursor.as_ref(), self.snapshot)?;
            for item in &page.response.items {
                if !item
                    .selector
                    .to_string()
                    .starts_with(page.request.prefix.as_str())
                {
                    return Err(invalid(
                        "cached directory item does not match its ordered-prefix request",
                    ));
                }
                insert_identical(
                    &mut directory,
                    item.selector.clone(),
                    item.clone(),
                    "directory",
                )?;
            }
            insert_identical(
                &mut bindings,
                page.response.namespace_binding.namespace.clone(),
                page.response.namespace_binding.clone(),
                "namespace binding",
            )?;
        }
        let mut row_occurrences = 0usize;
        let mut releases = BTreeMap::new();
        for page in &self.resolver_pages {
            page.request
                .package
                .validate()
                .map_err(|error| invalid(error.reason()))?;
            page.request
                .page
                .validate()
                .map_err(|error| invalid(error.reason()))?;
            if let Some(requirement) = &page.request.requirement {
                requirement
                    .validate()
                    .map_err(|error| invalid(error.reason()))?;
            }
            page.response
                .validate_for(&page.request)
                .map_err(|error| invalid(error.reason()))?;
            self.validate_anchor(page.response.network_id, page.response.snapshot)?;
            validate_request_cursor(page.request.page.cursor.as_ref(), self.snapshot)?;
            row_occurrences = row_occurrences
                .checked_add(page.response.items.len())
                .ok_or_else(|| invalid("cached resolver row count overflow"))?;
            if row_occurrences > MAX_CACHED_ROW_OCCURRENCES_V1 {
                return Err(invalid("cached resolver row count exceeds its V1 bound"));
            }
            for row in &page.response.items {
                if row.release.package != page.request.package
                    || page
                        .request
                        .requirement
                        .as_ref()
                        .is_some_and(|requirement| !requirement.matches(&row.release.version))
                    || row.index_revision != self.snapshot.index_revision
                {
                    return Err(invalid(
                        "cached resolver row does not match its exact request and snapshot",
                    ));
                }
                insert_identical(
                    &mut releases,
                    row.release.clone(),
                    row.clone(),
                    "resolver row",
                )?;
            }
        }
        validate_page_chains(
            &self.ordered_pages,
            ordered_base_key,
            |page| &page.request.page,
            |page| &page.response.next_cursor,
        )?;
        validate_page_chains(
            &self.resolver_pages,
            resolver_base_key,
            |page| &page.request.page,
            |page| &page.response.next_cursor,
        )?;
        Ok(())
    }
    fn validate_anchor(
        &self,
        network_id: NetworkId,
        snapshot: MusubiRegistrySnapshotV1,
    ) -> Result<(), ResolverIndexCacheErrorV1> {
        if network_id != self.network_id || snapshot != self.snapshot {
            return Err(invalid(
                "cached query pages do not share one exact finalized anchor",
            ));
        }
        Ok(())
    }
    fn digest(&self) -> Result<[u8; 32], ResolverIndexCacheErrorV1> {
        let encoded = canonical(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(SNAPSHOT_DIGEST_DOMAIN);
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
    fn merge(&mut self, other: Self) -> Result<(), ResolverIndexCacheErrorV1> {
        if !same_anchor(self, &other) {
            return Err(invalid("cannot merge different finalized cache snapshots"));
        }
        merge_pages(&mut self.ordered_pages, other.ordered_pages, |page| {
            Ok(page.request.encode())
        })?;
        merge_pages(&mut self.resolver_pages, other.resolver_pages, |page| {
            Ok(page.request.encode())
        })?;
        self.validate()
    }
    fn is_not_older_than(&self, lock: &LockfileV1) -> bool {
        self.network_id == lock.network_id
            && (self.snapshot == lock.snapshot
                || (self.snapshot.finalized_height > lock.snapshot.finalized_height
                    && self.snapshot.index_revision >= lock.snapshot.index_revision))
    }
}
/// Snapshot plus its domain-separated local integrity commitment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct CommittedResolverSnapshotV1 {
    value: ResolverIndexCacheSnapshotV1,
    digest: [u8; 32],
}
impl CommittedResolverSnapshotV1 {
    fn new(value: ResolverIndexCacheSnapshotV1) -> Result<Self, ResolverIndexCacheErrorV1> {
        value.validate()?;
        let digest = value.digest()?;
        Ok(Self { value, digest })
    }
    fn validate(&self) -> Result<(), ResolverIndexCacheErrorV1> {
        self.value.validate()?;
        if self.digest != self.value.digest()? {
            return Err(invalid(
                "cached resolver snapshot commitment does not match",
            ));
        }
        Ok(())
    }
}
/// Strict first-release cache catalog.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct ResolverIndexCacheCatalogV1 {
    schema: String,
    version: u8,
    snapshots: Vec<CommittedResolverSnapshotV1>,
}
impl Default for ResolverIndexCacheCatalogV1 {
    fn default() -> Self {
        Self {
            schema: CACHE_SCHEMA.to_owned(),
            version: CACHE_VERSION,
            snapshots: Vec::new(),
        }
    }
}
impl ResolverIndexCacheCatalogV1 {
    fn validate(&self) -> Result<(), ResolverIndexCacheErrorV1> {
        if self.schema != CACHE_SCHEMA
            || self.version != CACHE_VERSION
            || self.snapshots.len() > MAX_CACHED_SNAPSHOTS_V1
        {
            return Err(invalid(
                "resolver cache schema, version, or snapshot bound is invalid",
            ));
        }
        self.validate_snapshots()
    }
    fn validate_snapshots(&self) -> Result<(), ResolverIndexCacheErrorV1> {
        validate_canonical_order(&self.snapshots, |entry| Ok(snapshot_key(&entry.value)))?;
        let mut finalized = BTreeMap::new();
        let mut deployment_revisions = BTreeMap::new();
        for entry in &self.snapshots {
            entry.validate()?;
            let deployment = (
                entry.value.network_id,
                entry.value.account_chain_discriminant,
            );
            let key = (deployment, entry.value.snapshot.finalized_height);
            let identity = (
                entry.value.snapshot.finalized_block_hash,
                entry.value.snapshot.index_revision,
            );
            if finalized
                .insert(key, identity)
                .is_some_and(|old| old != identity)
            {
                return Err(invalid(
                    "cache contains conflicting finalized identities at one height",
                ));
            }
            let revision = entry.value.snapshot.index_revision;
            if deployment_revisions
                .insert(
                    deployment,
                    (entry.value.snapshot.finalized_height, revision),
                )
                .is_some_and(|(previous_height, previous_revision)| {
                    entry.value.snapshot.finalized_height > previous_height
                        && revision < previous_revision
                })
            {
                return Err(invalid(
                    "cache contains a resolver-index revision rollback at a higher finalized height",
                ));
            }
        }
        Ok(())
    }
    fn insert(
        &mut self,
        snapshot: ResolverIndexCacheSnapshotV1,
    ) -> Result<(), ResolverIndexCacheErrorV1> {
        snapshot.validate()?;
        if let Some(existing) = self
            .snapshots
            .iter_mut()
            .find(|entry| same_anchor(&entry.value, &snapshot))
        {
            existing.value.merge(snapshot)?;
            existing.digest = existing.value.digest()?;
        } else {
            self.snapshots
                .push(CommittedResolverSnapshotV1::new(snapshot)?);
        }
        self.snapshots
            .sort_by(|left, right| snapshot_key(&left.value).cmp(&snapshot_key(&right.value)));
        // Validate the complete candidate history before retention. Otherwise a
        // conflicting or rolled-back oldest snapshot could be truncated first
        // and silently evade the cross-snapshot checks below.
        self.validate_snapshots()?;
        if self.snapshots.len() > MAX_CACHED_SNAPSHOTS_V1 {
            self.snapshots.sort_by(|left, right| {
                snapshot_recency(&right.value)
                    .cmp(&snapshot_recency(&left.value))
                    .then_with(|| snapshot_key(&left.value).cmp(&snapshot_key(&right.value)))
            });
            self.snapshots.truncate(MAX_CACHED_SNAPSHOTS_V1);
        }
        self.snapshots
            .sort_by(|left, right| snapshot_key(&left.value).cmp(&snapshot_key(&right.value)));
        self.validate()
    }
}
/// Durable cache handle rooted in the platform-owned Musubi cache directory.
#[derive(Debug)]
pub struct ResolverIndexCacheV1 {
    write_root: AtomicWriteRoot,
    root_identity: DirectoryIdentityV1,
}
impl ResolverIndexCacheV1 {
    /// Open the resolver cache below an explicit trusted user cache root.
    pub(super) fn open(user_cache_root: &Path) -> Result<Self, ResolverIndexCacheErrorV1> {
        let archive_cache =
            MusubiCache::open(user_cache_root).map_err(ResolverIndexCacheErrorV1::Cache)?;
        let registry_root = archive_cache.root().join("registry-v1");
        let metadata = fs::symlink_metadata(&registry_root)
            .map_err(|source| io_error("inspect resolver cache root", &registry_root, source))?;
        validate_private_directory(&registry_root, &metadata)?;
        let write_root =
            AtomicWriteRoot::new(&registry_root).map_err(ResolverIndexCacheErrorV1::AtomicWrite)?;
        Ok(Self {
            write_root,
            root_identity: DirectoryIdentityV1::capture(&metadata),
        })
    }
    /// Atomically merge one successfully collected coherent snapshot.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "publishing takes ownership of one completed snapshot while cloning it only across bounded optimistic commit retries"
    )]
    pub(super) fn publish(
        &self,
        snapshot: ResolverIndexCacheSnapshotV1,
    ) -> Result<(), ResolverIndexCacheErrorV1> {
        snapshot.validate()?;
        for attempt in 0..COMMIT_RETRIES {
            let mut catalog = self.load_catalog()?.unwrap_or_default();
            catalog.insert(snapshot.clone())?;
            let encoded = norito::encode_canonical(&catalog)
                .map_err(|error| ResolverIndexCacheErrorV1::Codec(error.to_string()))?;
            if encoded.len() as u64 > MAX_CACHE_FILE_BYTES_V1 {
                return Err(invalid("encoded resolver cache exceeds its V1 byte bound"));
            }
            match self.write_root.replace(Path::new(CACHE_FILE), &encoded) {
                Ok(()) => return Ok(()),
                Err(error)
                    if error.code() == AtomicWriteErrorCode::ConcurrentModification
                        && attempt + 1 < COMMIT_RETRIES => {}
                Err(error) => return Err(ResolverIndexCacheErrorV1::AtomicWrite(error)),
            }
        }
        unreachable!("bounded commit retry loop returns on its final iteration")
    }
    /// Load newest-first coherent sources suitable for one offline resolution.
    pub(super) fn sources(
        &self,
        previous: Option<&LockfileV1>,
    ) -> Result<Vec<CachedResolverSourceV1>, ResolverIndexCacheErrorV1> {
        let catalog = self.load_catalog()?.ok_or_else(|| {
            ResolverIndexCacheErrorV1::OfflineMiss("resolver cache is empty".to_owned())
        })?;
        let mut deployments = BTreeSet::new();
        let mut snapshots = catalog
            .snapshots
            .into_iter()
            .map(|entry| entry.value)
            .filter(|snapshot| {
                let compatible = previous.is_none_or(|lock| snapshot.is_not_older_than(lock));
                if compatible {
                    deployments.insert((snapshot.network_id, snapshot.account_chain_discriminant));
                }
                compatible
            })
            .collect::<Vec<_>>();
        if deployments.len() > 1 {
            return Err(ResolverIndexCacheErrorV1::OfflineMiss(
                "resolver cache contains more than one network/discriminant deployment".to_owned(),
            ));
        }
        if snapshots.is_empty() {
            return Err(ResolverIndexCacheErrorV1::OfflineMiss(
                "resolver cache has no snapshot compatible with the existing lock".to_owned(),
            ));
        }
        snapshots.sort_by(|left, right| {
            snapshot_recency(right)
                .cmp(&snapshot_recency(left))
                .then_with(|| snapshot_key(left).cmp(&snapshot_key(right)))
        });
        Ok(snapshots
            .into_iter()
            .map(CachedResolverSourceV1::new)
            .collect())
    }
    fn load_catalog(
        &self,
    ) -> Result<Option<ResolverIndexCacheCatalogV1>, ResolverIndexCacheErrorV1> {
        self.validate_root()?;
        let bytes = self
            .write_root
            .load_private_descriptor_rooted(Path::new(CACHE_FILE), MAX_CACHE_FILE_BYTES_USIZE_V1)
            .map_err(ResolverIndexCacheErrorV1::AtomicWrite)?;
        let Some(bytes) = bytes else {
            return Ok(None);
        };
        let catalog: ResolverIndexCacheCatalogV1 =
            norito::decode_canonical_with_limits(&bytes, CACHE_DECODE_LIMITS_V1)
                .map_err(|error| ResolverIndexCacheErrorV1::Codec(error.to_string()))?;
        catalog.validate()?;
        Ok(Some(catalog))
    }
    fn validate_root(&self) -> Result<(), ResolverIndexCacheErrorV1> {
        let path = self.write_root.path();
        let metadata = fs::symlink_metadata(path)
            .map_err(|source| io_error("revalidate resolver cache root", path, source))?;
        validate_private_directory(path, &metadata)?;
        if !self.root_identity.matches(&metadata) {
            return Err(invalid("resolver cache root identity changed"));
        }
        Ok(())
    }
}
/// Online source wrapper that records only successfully returned validated pages.
pub struct RecordingResolverSourceV1<'a> {
    inner: &'a RegistryReadClientV1,
    ordered: RefCell<Vec<CachedOrderedPageV1>>,
    resolver: RefCell<Vec<CachedResolverPageV1>>,
}
impl<'a> RecordingResolverSourceV1<'a> {
    /// Wrap one authenticated online registry reader.
    pub(super) fn new(inner: &'a RegistryReadClientV1) -> Self {
        Self {
            inner,
            ordered: RefCell::new(Vec::new()),
            resolver: RefCell::new(Vec::new()),
        }
    }
    /// Finish a coherent capture after graph collection succeeds.
    pub(super) fn finish(self) -> Result<ResolverIndexCacheSnapshotV1, ResolverIndexCacheErrorV1> {
        let mut ordered_pages = self.ordered.into_inner();
        let mut resolver_pages = self.resolver.into_inner();
        ordered_pages.sort_by_cached_key(|page| page.request.encode());
        resolver_pages.sort_by_cached_key(|page| page.request.encode());
        let anchor = ordered_pages
            .first()
            .map(|page| (page.response.network_id, page.response.snapshot))
            .or_else(|| {
                resolver_pages
                    .first()
                    .map(|page| (page.response.network_id, page.response.snapshot))
            })
            .ok_or_else(|| invalid("successful graph collection captured no registry page"))?;
        let snapshot = ResolverIndexCacheSnapshotV1 {
            network_id: anchor.0,
            account_chain_discriminant: self.inner.account_chain_discriminant(),
            snapshot: anchor.1,
            ordered_pages,
            resolver_pages,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}
impl ResolverRegistrySourceV1 for RecordingResolverSourceV1<'_> {
    type Error = RegistryErrorV1;
    fn map_error(error: Self::Error) -> GraphErrorV1 {
        GraphErrorV1::Registry(error.to_string())
    }
    fn ordered_prefix(
        &self,
        request: &MusubiOrderedPrefixQueryV1,
    ) -> Result<MusubiOrderedPackagePageV1, Self::Error> {
        let response = self.inner.ordered_prefix(request)?;
        self.ordered.borrow_mut().push(CachedOrderedPageV1 {
            request: request.clone(),
            response: response.clone(),
        });
        Ok(response)
    }
    fn resolver_index(
        &self,
        request: &MusubiResolverIndexQueryV1,
    ) -> Result<MusubiResolverIndexPageV1, Self::Error> {
        let response = self.inner.resolver_index(request)?;
        self.resolver.borrow_mut().push(CachedResolverPageV1 {
            request: request.clone(),
            response: response.clone(),
        });
        Ok(response)
    }
}
/// One validated immutable snapshot replayed as the ordinary graph source.
#[derive(Clone, Debug)]
pub struct CachedResolverSourceV1 {
    snapshot: ResolverIndexCacheSnapshotV1,
}
impl CachedResolverSourceV1 {
    fn new(snapshot: ResolverIndexCacheSnapshotV1) -> Self {
        Self { snapshot }
    }
    /// Bind local package text structurally using the cached immutable namespace binding.
    pub(super) fn bind_selector_namespace(
        &self,
        selector: &MusubiPackageSelectorV1,
    ) -> Result<MusubiPackageIdV1, ResolverIndexCacheSourceErrorV1> {
        let mut binding = None;
        for page in &self.snapshot.ordered_pages {
            if page.response.namespace_binding.namespace == selector.namespace {
                match &binding {
                    Some(previous) if previous != &page.response.namespace_binding => {
                        return Err(ResolverIndexCacheSourceErrorV1::Ambiguous);
                    }
                    Some(_) => {}
                    None => binding = Some(page.response.namespace_binding.clone()),
                }
            }
        }
        let binding = binding.ok_or(ResolverIndexCacheSourceErrorV1::Miss)?;
        Ok(MusubiPackageIdV1::new(
            binding.home_dataspace,
            binding.scope,
            selector.name.clone(),
        ))
    }
    /// Return the exact finalized anchor represented by this source.
    #[cfg(all(test, any(target_os = "linux", target_os = "android")))]
    pub(super) const fn snapshot(&self) -> MusubiRegistrySnapshotV1 {
        self.snapshot.snapshot
    }
    /// Return the non-secret account-network discriminant captured with this snapshot.
    pub(super) const fn account_chain_discriminant(&self) -> u16 {
        self.snapshot.account_chain_discriminant
    }
}
impl ResolverRegistrySourceV1 for CachedResolverSourceV1 {
    type Error = ResolverIndexCacheSourceErrorV1;
    fn map_error(error: Self::Error) -> GraphErrorV1 {
        GraphErrorV1::OfflineMiss(error.to_string())
    }
    fn ordered_prefix(
        &self,
        request: &MusubiOrderedPrefixQueryV1,
    ) -> Result<MusubiOrderedPackagePageV1, Self::Error> {
        exact_page(
            &self.snapshot.ordered_pages,
            request,
            |entry| &entry.request,
            |entry| &entry.response,
        )
    }
    fn resolver_index(
        &self,
        request: &MusubiResolverIndexQueryV1,
    ) -> Result<MusubiResolverIndexPageV1, Self::Error> {
        exact_page(
            &self.snapshot.resolver_pages,
            request,
            |entry| &entry.request,
            |entry| &entry.response,
        )
    }
}
/// Stable cache-source lookup failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolverIndexCacheSourceErrorV1 {
    /// No complete exact request coverage exists in this snapshot.
    Miss,
    /// Multiple disagreeing cached responses matched one exact request.
    Ambiguous,
}
impl fmt::Display for ResolverIndexCacheSourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Miss => formatter
                .write_str("MUSUBI_E_OFFLINE_MISS: exact resolver cache coverage is missing"),
            Self::Ambiguous => {
                formatter.write_str("MUSUBI_E_OFFLINE_MISS: resolver cache coverage is ambiguous")
            }
        }
    }
}
impl Error for ResolverIndexCacheSourceErrorV1 {}
/// Stable resolver-index cache failure.
#[derive(Debug)]
pub enum ResolverIndexCacheErrorV1 {
    /// The shared archive/cache root failed its safety checks.
    Cache(CacheError),
    /// Root-confined atomic replacement failed.
    AtomicWrite(AtomicWriteError),
    /// A bounded, fixed-path filesystem operation failed.
    Io {
        /// Stable non-secret operation label.
        operation: &'static str,
        /// Cache-owned path involved in the failure.
        path: PathBuf,
        /// Underlying I/O failure.
        source: io::Error,
    },
    /// Canonical Norito encoding or decoding failed.
    Codec(String),
    /// Decoded cache content violated the V1 schema.
    Invalid(String),
    /// No unambiguous complete cached snapshot covered a request.
    OfflineMiss(String),
}
impl fmt::Display for ResolverIndexCacheErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::AtomicWrite(error) => write!(formatter, "{error}"),
            Self::Io {
                operation,
                path,
                source,
            } => {
                write!(
                    formatter,
                    "failed to {operation} `{}`: {source}",
                    path.display()
                )
            }
            Self::Codec(reason) => write!(
                formatter,
                "resolver cache has invalid canonical Norito: {reason}"
            ),
            Self::Invalid(reason) => write!(formatter, "invalid resolver cache: {reason}"),
            Self::OfflineMiss(reason) => write!(formatter, "MUSUBI_E_OFFLINE_MISS: {reason}"),
        }
    }
}
impl Error for ResolverIndexCacheErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::AtomicWrite(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Codec(_) | Self::Invalid(_) | Self::OfflineMiss(_) => None,
        }
    }
}
fn exact_page<T, Q, P, FQ, FP>(
    pages: &[T],
    request: &Q,
    query: FQ,
    response: FP,
) -> Result<P, ResolverIndexCacheSourceErrorV1>
where
    Q: PartialEq,
    P: Clone + PartialEq,
    FQ: for<'a> Fn(&'a T) -> &'a Q,
    FP: for<'a> Fn(&'a T) -> &'a P,
{
    let mut found = None;
    for page in pages.iter().filter(|page| query(page) == request) {
        match &found {
            Some(previous) if previous != response(page) => {
                return Err(ResolverIndexCacheSourceErrorV1::Ambiguous);
            }
            Some(_) => {}
            None => found = Some(response(page).clone()),
        }
    }
    found.ok_or(ResolverIndexCacheSourceErrorV1::Miss)
}
fn validate_request_cursor(
    cursor: Option<&iroha_data_model::musubi::MusubiFinalizedCursorV1>,
    snapshot: MusubiRegistrySnapshotV1,
) -> Result<(), ResolverIndexCacheErrorV1> {
    if cursor.is_some_and(|cursor| cursor.snapshot != snapshot || cursor.caller.is_some()) {
        return Err(invalid(
            "cached public query cursor has a different snapshot or a caller binding",
        ));
    }
    Ok(())
}
fn validate_canonical_order<T, K, F>(values: &[T], key: F) -> Result<(), ResolverIndexCacheErrorV1>
where
    K: Ord,
    F: Fn(&T) -> Result<K, ResolverIndexCacheErrorV1>,
{
    let mut previous = None;
    for value in values {
        let current = key(value)?;
        if previous
            .as_ref()
            .is_some_and(|previous| previous >= &current)
        {
            return Err(invalid(
                "resolver cache records are not uniquely canonicalized",
            ));
        }
        previous = Some(current);
    }
    Ok(())
}
fn validate_page_chains<T, B, FBase, FRequest, FNext>(
    pages: &[T],
    base_key: FBase,
    request: FRequest,
    next: FNext,
) -> Result<(), ResolverIndexCacheErrorV1>
where
    B: Ord,
    FBase: Fn(&T) -> B,
    FRequest: for<'a> Fn(&'a T) -> &'a iroha_data_model::musubi::MusubiPageRequestV1,
    FNext: for<'a> Fn(&'a T) -> &'a Option<iroha_data_model::musubi::MusubiFinalizedCursorV1>,
{
    let mut groups: BTreeMap<B, Vec<&T>> = BTreeMap::new();
    for page in pages {
        groups.entry(base_key(page)).or_default().push(page);
    }
    for group in groups.into_values() {
        let starts = group
            .iter()
            .copied()
            .filter(|page| request(page).cursor.is_none())
            .collect::<Vec<_>>();
        if starts.len() != 1 {
            return Err(invalid("cached pagination chain has no unique first page"));
        }
        let mut visited = BTreeSet::new();
        let mut current = starts[0];
        loop {
            let key = canonical(request(current))?;
            if !visited.insert(key) {
                return Err(invalid("cached pagination chain contains a cycle"));
            }
            let Some(cursor) = next(current) else { break };
            let matches = group
                .iter()
                .copied()
                .filter(|candidate| request(candidate).cursor.as_ref() == Some(cursor))
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(invalid(
                    "cached pagination chain is incomplete or ambiguous",
                ));
            }
            current = matches[0];
        }
        if visited.len() != group.len() {
            return Err(invalid(
                "cached pagination chain contains unreachable pages",
            ));
        }
    }
    Ok(())
}
fn ordered_base_key(page: &CachedOrderedPageV1) -> (String, u32) {
    (
        page.request.prefix.as_str().to_owned(),
        page.request.page.limit,
    )
}
fn resolver_base_key(page: &CachedResolverPageV1) -> (Vec<u8>, Vec<u8>, u32) {
    (
        page.request.package.encode(),
        page.request.requirement.encode(),
        page.request.page.limit,
    )
}
fn insert_identical<K: Ord, V: PartialEq>(
    values: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    label: &str,
) -> Result<(), ResolverIndexCacheErrorV1> {
    use std::collections::btree_map::Entry;
    match values.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(value);
        }
        Entry::Occupied(entry) if entry.get() != &value => {
            return Err(invalid(format!(
                "cached {label} has conflicting values at one finalized snapshot"
            )));
        }
        Entry::Occupied(_) => {}
    }
    Ok(())
}
fn merge_pages<T, F>(
    destination: &mut Vec<T>,
    incoming: Vec<T>,
    key: F,
) -> Result<(), ResolverIndexCacheErrorV1>
where
    T: PartialEq,
    F: Fn(&T) -> Result<Vec<u8>, ResolverIndexCacheErrorV1>,
{
    for value in incoming {
        let value_key = key(&value)?;
        if let Some(existing) = destination
            .iter()
            .find(|existing| key(existing).is_ok_and(|existing_key| existing_key == value_key))
        {
            if existing != &value {
                return Err(invalid("same cached request has conflicting responses"));
            }
        } else {
            destination.push(value);
        }
    }
    destination.sort_by_cached_key(|value| key(value).unwrap_or_default());
    Ok(())
}
fn same_anchor(left: &ResolverIndexCacheSnapshotV1, right: &ResolverIndexCacheSnapshotV1) -> bool {
    left.network_id == right.network_id
        && left.account_chain_discriminant == right.account_chain_discriminant
        && left.snapshot == right.snapshot
}
fn snapshot_key(snapshot: &ResolverIndexCacheSnapshotV1) -> (NetworkId, u16, u64, [u8; 32], u64) {
    (
        snapshot.network_id,
        snapshot.account_chain_discriminant,
        snapshot.snapshot.finalized_height,
        snapshot.snapshot.finalized_block_hash,
        snapshot.snapshot.index_revision,
    )
}
fn snapshot_recency(snapshot: &ResolverIndexCacheSnapshotV1) -> (u64, u64, [u8; 32]) {
    (
        snapshot.snapshot.finalized_height,
        snapshot.snapshot.index_revision,
        snapshot.snapshot.finalized_block_hash,
    )
}
fn canonical<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, ResolverIndexCacheErrorV1> {
    norito::encode_canonical(value)
        .map_err(|error| ResolverIndexCacheErrorV1::Codec(error.to_string()))
}
fn invalid(reason: impl Into<String>) -> ResolverIndexCacheErrorV1 {
    ResolverIndexCacheErrorV1::Invalid(reason.into())
}
fn io_error(operation: &'static str, path: &Path, source: io::Error) -> ResolverIndexCacheErrorV1 {
    ResolverIndexCacheErrorV1::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
#[derive(Clone, Debug)]
struct DirectoryIdentityV1 {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}
impl DirectoryIdentityV1 {
    fn capture(metadata: &fs::Metadata) -> Self {
        Self {
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        }
    }
    fn matches(&self, metadata: &fs::Metadata) -> bool {
        #[cfg(unix)]
        {
            self.device == metadata.dev() && self.inode == metadata.ino()
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            false
        }
    }
}
fn validate_private_directory(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ResolverIndexCacheErrorV1> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid(format!(
            "`{}` is not a real cache directory",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(invalid(format!("`{}` is not private", path.display())));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use crate::{
        graph::resolve_workspace_offline_cached, resolver::ResolveModeV1, workspace::load_workspace,
    };
    use iroha_data_model::{
        musubi::{
            MusubiNamespaceBindingV1, MusubiNamespaceV1, MusubiOrderedPackageEntryV1,
            MusubiOrderedPrefixV1, MusubiPackageScopeV1, MusubiPageRequestV1,
        },
        nexus::DataSpaceId,
    };
    use tempfile::TempDir;
    fn network_id() -> NetworkId {
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
            .parse()
            .expect("network id")
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn other_network_id() -> NetworkId {
        "hash:214A4C8F95074B216BE2F72EB93166506DAE0B1026ED01EF5A760632CD93ABAB#50FA"
            .parse()
            .expect("other network id")
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const APP: &str = r#"manifest-version = 1
[package]
namespace = "apps.sora"
name = "app"
version = "1.0.0"
edition = "1"
abi-version = 1
[lib]
exports = []
"#;
    fn snapshot(height: u64, byte: u8) -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: height,
            finalized_block_hash: [byte; 32],
            index_revision: height,
        }
    }
    fn binding(namespace: &str) -> MusubiNamespaceBindingV1 {
        let namespace: MusubiNamespaceV1 = namespace.parse().expect("namespace");
        MusubiNamespaceBindingV1 {
            home_dataspace: DataSpaceId::new(7),
            scope: namespace
                .domain_segment()
                .map_or(MusubiPackageScopeV1::DataspaceRoot, |domain| {
                    MusubiPackageScopeV1::Domain(domain.parse().expect("domain"))
                }),
            namespace,
            generation: 1,
        }
    }
    fn ordered_page(namespace: &str, height: u64, byte: u8) -> CachedOrderedPageV1 {
        let prefix = format!("{namespace}/");
        let request = MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new(&prefix).expect("prefix"),
            page: MusubiPageRequestV1 {
                limit: 1,
                cursor: None,
            },
        };
        CachedOrderedPageV1 {
            request: request.clone(),
            response: MusubiOrderedPackagePageV1 {
                query: request,
                network_id: network_id(),
                namespace_binding: binding(namespace),
                items: Vec::<MusubiOrderedPackageEntryV1>::new(),
                next_cursor: None,
                snapshot: snapshot(height, byte),
            },
        }
    }
    fn image(namespace: &str, height: u64, byte: u8) -> ResolverIndexCacheSnapshotV1 {
        ResolverIndexCacheSnapshotV1 {
            network_id: network_id(),
            account_chain_discriminant: 369,
            snapshot: snapshot(height, byte),
            ordered_pages: vec![ordered_page(namespace, height, byte)],
            resolver_pages: Vec::new(),
        }
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn install_cache_ancestor_aba_hooks(
        trusted_root: PathBuf,
        alternate_root: PathBuf,
        held_trusted_root: PathBuf,
    ) {
        let swap_trusted_root = trusted_root.clone();
        let swap_alternate_root = alternate_root.clone();
        let swap_held_root = held_trusted_root.clone();
        crate::atomic_io::install_descriptor_root_read_test_hooks(
            move || {
                fs::rename(&swap_trusted_root, &swap_held_root)
                    .expect("move genuine ancestor out of the pathname");
                fs::rename(&swap_alternate_root, &swap_trusted_root)
                    .expect("move alternate ancestor into the pathname");
            },
            move || {
                fs::rename(&trusted_root, &alternate_root)
                    .expect("move alternate ancestor back out of the pathname");
                fs::rename(&held_trusted_root, &trusted_root)
                    .expect("restore genuine ancestor before final validation");
            },
        );
    }
    #[test]
    fn canonical_snapshot_rejects_page_order_and_mixed_anchors() {
        let mut value = image("apps.sora", 10, 10);
        value.ordered_pages.push(ordered_page("libs.sora", 10, 10));
        value
            .ordered_pages
            .sort_by_cached_key(|page| page.request.encode());
        value.validate().expect("canonical pages");
        value.ordered_pages.reverse();
        assert!(matches!(
            value.validate(),
            Err(ResolverIndexCacheErrorV1::Invalid(_))
        ));
        let mut mixed = image("apps.sora", 10, 10);
        mixed.ordered_pages[0].response.snapshot = snapshot(11, 11);
        assert!(matches!(
            mixed.validate(),
            Err(ResolverIndexCacheErrorV1::Invalid(_))
        ));
        let mut invalid_network = image("apps.sora", 10, 10);
        invalid_network.account_chain_discriminant = 0;
        assert!(matches!(
            invalid_network.validate(),
            Err(ResolverIndexCacheErrorV1::Invalid(_))
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn durable_cache_detects_tampering_and_offline_misses() {
        let temp = TempDir::new().expect("temp root");
        let root = temp.path().join("cache");
        let cache = ResolverIndexCacheV1::open(&root).expect("open cache");
        cache
            .publish(image("apps.sora", 10, 10))
            .expect("publish cache");
        let sources = cache.sources(None).expect("load source");
        let missing = MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("other.sora/").expect("prefix"),
            page: MusubiPageRequestV1 {
                limit: 1,
                cursor: None,
            },
        };
        assert!(matches!(
            sources[0].ordered_prefix(&missing),
            Err(ResolverIndexCacheSourceErrorV1::Miss)
        ));
        let path = root.join("registry-v1").join(CACHE_FILE);
        let mut bytes = fs::read(&path).expect("cache bytes");
        let last = bytes.last_mut().expect("non-empty cache");
        *last ^= 0x80;
        fs::write(&path, bytes).expect("tamper cache");
        assert!(matches!(
            cache.sources(None),
            Err(ResolverIndexCacheErrorV1::Codec(_) | ResolverIndexCacheErrorV1::Invalid(_))
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn descriptor_rooted_read_cannot_load_forged_bytes_from_an_aba_root() {
        let temp = TempDir::new().expect("temp root");
        let trusted_user_root = temp.path().join("trusted-cache");
        let forged_user_root = temp.path().join("forged-cache");
        let cache = ResolverIndexCacheV1::open(&trusted_user_root).expect("trusted empty cache");
        let forged_cache =
            ResolverIndexCacheV1::open(&forged_user_root).expect("forged fixture cache");
        forged_cache
            .publish(image("forged.sora", 99, 99))
            .expect("publish structurally valid forged catalog");
        assert_eq!(
            forged_cache.sources(None).expect("forged source")[0].snapshot(),
            snapshot(99, 99),
            "the alternate bytes must be independently admissible"
        );
        drop(forged_cache);
        install_cache_ancestor_aba_hooks(
            trusted_user_root.clone(),
            forged_user_root,
            temp.path().join("trusted-cache-held-for-test"),
        );
        assert!(matches!(
            cache.sources(None),
            Err(ResolverIndexCacheErrorV1::OfflineMiss(reason))
                if reason == "resolver cache is empty"
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn descriptor_rooted_read_cannot_observe_forged_absence_from_an_aba_root() {
        let temp = TempDir::new().expect("temp root");
        let trusted_user_root = temp.path().join("trusted-cache");
        let empty_user_root = temp.path().join("empty-cache");
        let cache = ResolverIndexCacheV1::open(&trusted_user_root).expect("trusted cache");
        cache
            .publish(image("apps.sora", 10, 10))
            .expect("publish trusted catalog");
        drop(ResolverIndexCacheV1::open(&empty_user_root).expect("empty alternate cache"));
        install_cache_ancestor_aba_hooks(
            trusted_user_root.clone(),
            empty_user_root,
            temp.path().join("trusted-cache-held-for-test"),
        );
        let sources = cache
            .sources(None)
            .expect("descriptor-rooted read sees retained genuine catalog");
        assert_eq!(sources[0].snapshot(), snapshot(10, 10));
        drop(cache);
        let reopened = ResolverIndexCacheV1::open(&trusted_user_root)
            .expect("restart binds the restored genuine root");
        assert_eq!(
            reopened.sources(None).expect("restart source")[0].snapshot(),
            snapshot(10, 10)
        );
    }
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    #[test]
    fn offline_cache_read_fails_closed_without_descriptor_rooted_open() {
        let temp = TempDir::new().expect("temp root");
        let cache = ResolverIndexCacheV1::open(&temp.path().join("cache")).expect("cache");
        assert!(matches!(
            cache.sources(None),
            Err(ResolverIndexCacheErrorV1::AtomicWrite(error))
                if error.code() == AtomicWriteErrorCode::UnsupportedPlatform
        ));
    }
    #[cfg(not(unix))]
    #[test]
    fn resolver_cache_open_fails_closed_without_a_safe_root_handle() {
        let temp = TempDir::new().expect("temp root");
        let error = ResolverIndexCacheV1::open(&temp.path().join("cache"))
            .expect_err("non-Unix resolver cache must fail closed");
        match &error {
            ResolverIndexCacheErrorV1::Cache(CacheError::UnsupportedPlatform) => {}
            ResolverIndexCacheErrorV1::AtomicWrite(error)
                if error.code() == AtomicWriteErrorCode::UnsupportedPlatform => {}
            _ => panic!("unexpected non-Unix cache-open error: {error}"),
        }
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn newest_snapshot_is_selected_without_mixing() {
        let temp = TempDir::new().expect("temp root");
        let cache = ResolverIndexCacheV1::open(&temp.path().join("cache")).expect("cache");
        cache.publish(image("apps.sora", 10, 10)).expect("older");
        cache.publish(image("apps.sora", 12, 12)).expect("newer");
        let sources = cache.sources(None).expect("sources");
        assert_eq!(sources[0].snapshot(), snapshot(12, 12));
        assert_eq!(sources[0].account_chain_discriminant(), 369);
        assert_eq!(sources[1].snapshot(), snapshot(10, 10));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn same_finalized_block_requires_the_exact_locked_index_revision() {
        let stable = image("apps.sora", 10, 10);
        let lock = LockfileV1::new(
            stable.network_id,
            stable.snapshot,
            vec![crate::lockfile::LockedRootV1 {
                package: "apps.sora/app".parse().expect("selector"),
                dependencies: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("stable lock");
        let mut equivocated = stable.clone();
        equivocated.snapshot.index_revision += 1;
        equivocated.ordered_pages[0].response.snapshot = equivocated.snapshot;
        assert!(stable.is_not_older_than(&lock));
        assert!(
            !equivocated.is_not_older_than(&lock),
            "one finalized block hash cannot authenticate two index revisions"
        );
        let temp = TempDir::new().expect("temp root");
        let cache = ResolverIndexCacheV1::open(&temp.path().join("cache")).expect("cache");
        cache
            .publish(equivocated)
            .expect("internally coherent equivocated fixture");
        assert!(matches!(
            cache.sources(Some(&lock)),
            Err(ResolverIndexCacheErrorV1::OfflineMiss(_))
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn higher_finalized_height_cannot_roll_back_the_resolver_index_revision() {
        let temp = TempDir::new().expect("temp root");
        let cache = ResolverIndexCacheV1::open(&temp.path().join("cache")).expect("cache");
        let stable = image("apps.sora", 10, 10);
        cache.publish(stable.clone()).expect("stable snapshot");
        let lock = LockfileV1::new(
            stable.network_id,
            stable.snapshot,
            vec![crate::lockfile::LockedRootV1 {
                package: "apps.sora/app".parse().expect("selector"),
                dependencies: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("stable lock");
        for height in 12_u64..=26 {
            cache
                .publish(image(
                    "apps.sora",
                    height,
                    u8::try_from(height).expect("fixture height fits u8"),
                ))
                .expect("newer monotonic snapshot");
        }
        let cache_file = temp
            .path()
            .join("cache")
            .join("registry-v1")
            .join(CACHE_FILE);
        let stable_bytes = fs::read(&cache_file).expect("stable cache bytes");
        let mut rollback = image("apps.sora", 11, 11);
        rollback.snapshot.index_revision = 9;
        rollback.ordered_pages[0].response.snapshot = rollback.snapshot;
        assert!(
            !rollback.is_not_older_than(&lock),
            "a higher block cannot make a lower index revision fresh"
        );
        assert!(matches!(
            cache.publish(rollback),
            Err(ResolverIndexCacheErrorV1::Invalid(_))
        ));
        assert_eq!(
            fs::read(&cache_file).expect("cache bytes after rejected rollback"),
            stable_bytes,
            "failed rollback publication must not replace the durable catalog"
        );
        let sources = cache
            .sources(Some(&lock))
            .expect("rejected rollback leaves durable cache unchanged");
        assert_eq!(sources.len(), MAX_CACHED_SNAPSHOTS_V1);
        assert!(
            sources
                .iter()
                .any(|source| source.snapshot() == stable.snapshot),
            "pre-retention validation must preserve the oldest stable anchor"
        );
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn offline_fresh_and_frozen_resolution_use_only_complete_cached_pages() {
        let temp = TempDir::new().expect("temp root");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("project root");
        fs::write(project.join("Musubi.toml"), APP).expect("manifest");
        let workspace = load_workspace(&project).expect("workspace");
        let selected = vec!["apps.sora/app".parse().expect("selector")];
        let cache = ResolverIndexCacheV1::open(&temp.path().join("cache")).expect("cache");
        cache
            .publish(image("apps.sora", 10, 10))
            .expect("publish snapshot");
        let fresh = resolve_workspace_offline_cached(
            &cache,
            &workspace,
            &selected,
            None,
            None,
            ResolveModeV1::UpdateLock,
        )
        .expect("fresh offline graph");
        assert!(fresh.outcome.changed);
        let frozen = resolve_workspace_offline_cached(
            &cache,
            &workspace,
            &selected,
            Some(fresh.outcome.lockfile.clone()),
            None,
            ResolveModeV1::Locked,
        )
        .expect("frozen offline graph");
        assert!(!frozen.outcome.changed);
        assert_eq!(frozen.outcome.lockfile, fresh.outcome.lockfile);
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn offline_source_rejects_ambiguous_deployments_and_snapshots_older_than_lock() {
        let temp = TempDir::new().expect("temp root");
        let cache = ResolverIndexCacheV1::open(&temp.path().join("cache")).expect("cache");
        cache
            .publish(image("apps.sora", 10, 10))
            .expect("first deployment");
        let stale_lock = LockfileV1::new(
            network_id(),
            snapshot(20, 20),
            vec![crate::lockfile::LockedRootV1 {
                package: "apps.sora/app".parse().expect("selector"),
                dependencies: Vec::new(),
            }],
            Vec::new(),
        )
        .expect("future lock anchor");
        stale_lock.validate().expect("valid lock");
        assert!(matches!(
            cache.sources(Some(&stale_lock)),
            Err(ResolverIndexCacheErrorV1::OfflineMiss(_))
        ));
        let mut other = image("apps.sora", 11, 11);
        other.network_id = other_network_id();
        other.ordered_pages[0].response.network_id = other.network_id;
        cache.publish(other).expect("second deployment");
        assert!(matches!(
            cache.sources(None),
            Err(ResolverIndexCacheErrorV1::OfflineMiss(_))
        ));
    }
}

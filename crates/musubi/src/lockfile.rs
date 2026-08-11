//! Consumer-owned Musubi V1 exact lock graphs.
//!
//! The lock records only stable registry identities and immutable commitments.
//! It deliberately contains no cache paths, provider URLs, source plans,
//! timestamps, credentials, or bearer material.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fmt::Write as _,
    io,
    path::Path,
};

use iroha_data_model::{
    NetworkId,
    musubi::{
        ArchiveId, MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1, MUSUBI_MAX_DEPENDENCIES_V1,
        MUSUBI_MAX_RESOLUTION_DEPTH_V1, MUSUBI_MAX_RESOLUTION_NODES_V1, MUSUBI_REGISTRY_VERSION_V1,
        MusubiAbiBindingV1, MusubiContentDigestV1, MusubiDependencyKindV1,
        MusubiExactDependencyEdgeV1, MusubiPackageIdV1, MusubiPackageScopeV1,
        MusubiPackageSelectorV1, MusubiRegistrySnapshotV1, MusubiReleaseDigestV1,
        MusubiReleaseIdV1, MusubiVerificationLockV1, MusubiVerificationNodeV1, MusubiVersionReqV1,
        MusubiVersionV1,
    },
    nexus::DataSpaceId,
};

use crate::{
    atomic_io::{AtomicWriteError, AtomicWriteRoot},
    local_file::read_bounded_single_link_regular_file_v1,
};

/// Canonical schema label required in every first-release lock.
pub const LOCK_SCHEMA: &str = "musubi-lock";
/// The only supported consumer lock version.
pub const LOCK_VERSION: u8 = 1;
/// Maximum encoded size of one consumer-owned `Musubi.lock`.
pub const MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1: u64 = 128 * 1024 * 1024;
/// Maximum encoded size of one publication verification lock.
///
/// This is the shared provider-verification and immutable-cache metadata ceiling, so a locally
/// produced package remains admissible through both downstream boundaries. Neither the
/// human-readable representation nor its typed Norito companion may consume an unbounded
/// allocation before package admission.
pub const MUSUBI_MAX_VERIFICATION_LOCK_BYTES_V1: u64 = MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1;
/// Maximum total local roots, including selected members and reachable path packages.
///
/// The 256 declared-member roster and a package-bearing workspace root share
/// this corridor with every recursively reachable local path package.
pub const MUSUBI_MAX_CONSUMER_LOCK_ROOTS_V1: usize = 257;
/// Maximum total dependency edges across every root and registry node.
///
/// The aggregate corridor admits one fully populated 256-edge package plus its
/// incoming edge, while preventing every one of the 1,024 nodes from consuming
/// that allowance independently. It also keeps deterministic resolver
/// backtracking within 512 edge-bearing frames plus one terminal frame and
/// bounds its structurally shared metadata.
pub const MUSUBI_MAX_CONSUMER_LOCK_EDGES_V1: usize = 512;

const ROOT_KEYS: &[&str] = &[
    "schema",
    "version",
    "network-id",
    "finalized-height",
    "finalized-block-hash",
    "index-revision",
    "root",
    "node",
];
const ROOT_ENTRY_KEYS: &[&str] = &["package", "dependency"];
const NODE_KEYS: &[&str] = &[
    "home-dataspace",
    "scope",
    "domain",
    "name",
    "version",
    "release-digest",
    "archive-id",
    "source-digest",
    "interface-digest",
    "abi-version",
    "abi-hash",
    "dependency",
];
const EDGE_KEYS: &[&str] = &[
    "alias",
    "kind",
    "home-dataspace",
    "scope",
    "domain",
    "name",
    "requirement",
    "selected-version",
];

/// One selected or recursively reachable local root and its parent-local exact edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockedRootV1 {
    /// Canonical namespaced local package selector.
    pub package: MusubiPackageSelectorV1,
    /// Sorted normal and root-local development dependency edges with unique parent-local aliases.
    pub dependencies: Vec<MusubiExactDependencyEdgeV1>,
}

impl LockedRootV1 {
    /// Validate the full root identity, edge bounds, order, and requirements.
    pub fn validate(&self) -> Result<(), LockfileError> {
        self.package
            .validate()
            .map_err(|error| LockfileError::invalid(error.reason()))?;
        validate_edges(&self.dependencies, true, "workspace root")
    }
}

/// A complete consumer-owned exact dependency graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockfileV1 {
    /// Fixed schema discriminator.
    pub schema: String,
    /// Fixed schema version.
    pub version: u8,
    /// Exact genesis-derived deployment identity.
    pub network_id: NetworkId,
    /// Finalized universal resolver snapshot used when this graph last changed.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Sorted selected workspace and recursively reachable local path roots.
    pub roots: Vec<LockedRootV1>,
    /// Sorted exact immutable registry nodes. Parallel package versions are allowed.
    pub nodes: Vec<MusubiVerificationNodeV1>,
}

impl LockfileV1 {
    /// Construct, canonicalize, and validate a first-release lock.
    pub fn new(
        network_id: NetworkId,
        snapshot: MusubiRegistrySnapshotV1,
        roots: Vec<LockedRootV1>,
        nodes: Vec<MusubiVerificationNodeV1>,
    ) -> Result<Self, LockfileError> {
        validate_consumer_lock_collection_counts(&roots, &nodes)?;
        let mut lock = Self {
            schema: LOCK_SCHEMA.to_owned(),
            version: LOCK_VERSION,
            network_id,
            snapshot,
            roots,
            nodes,
        };
        lock.canonicalize();
        lock.validate()?;
        Ok(lock)
    }

    /// Sort every set-like collection without erasing duplicates.
    pub fn canonicalize(&mut self) {
        for root in &mut self.roots {
            root.dependencies.sort();
        }
        self.roots
            .sort_by(|left, right| left.package.cmp(&right.package));
        for node in &mut self.nodes {
            node.dependencies.sort();
        }
        self.nodes
            .sort_by(|left, right| left.release.cmp(&right.release));
    }

    /// Validate the schema, identities, immutable nodes, exact edges, cycles, and bounds.
    pub fn validate(&self) -> Result<(), LockfileError> {
        if self.schema != LOCK_SCHEMA || self.version != LOCK_VERSION {
            return Err(LockfileError::Legacy);
        }
        if self.network_id.as_bytes()[31] & 1 != 1 {
            return Err(LockfileError::invalid(
                "network id must be an exact marked genesis identity",
            ));
        }
        self.snapshot
            .validate()
            .map_err(|error| LockfileError::invalid(error.reason()))?;
        if self.roots.is_empty() {
            return Err(LockfileError::invalid(
                "lock graph must contain at least one selected workspace root",
            ));
        }
        validate_consumer_lock_collection_counts(&self.roots, &self.nodes)?;
        if self
            .roots
            .windows(2)
            .any(|pair| pair[0].package >= pair[1].package)
        {
            return Err(LockfileError::invalid(
                "workspace roots must be uniquely sorted",
            ));
        }
        if self.nodes.len() > MUSUBI_MAX_RESOLUTION_NODES_V1
            || self
                .nodes
                .windows(2)
                .any(|pair| pair[0].release >= pair[1].release)
        {
            return Err(LockfileError::invalid(
                "registry nodes exceed the graph bound or are not uniquely sorted",
            ));
        }
        for root in &self.roots {
            root.validate()?;
        }
        for node in &self.nodes {
            node.validate()
                .map_err(|error| LockfileError::invalid(error.reason()))?;
            validate_edges(&node.dependencies, false, "registry node")?;
        }
        validate_graph(&self.roots, &self.nodes)
    }

    /// Parse a strict first-release lock document.
    ///
    /// The UTF-8 document must be no larger than
    /// [`MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1`]. Duplicate TOML keys are rejected
    /// by the parser and every unknown field is rejected here. A missing or
    /// retired schema returns [`LockfileError::Legacy`].
    pub fn parse(document: &str) -> Result<Self, LockfileError> {
        validate_consumer_lock_document_bytes(document.len())?;
        let table = document
            .parse::<toml::Table>()
            .map_err(LockfileError::Toml)?;

        if optional_string(&table, "schema")? != Some(LOCK_SCHEMA)
            || optional_integer(&table, "version")? != Some(i64::from(LOCK_VERSION))
        {
            return Err(LockfileError::Legacy);
        }
        reject_unknown(&table, ROOT_KEYS, "lock document")?;

        let network_id = required_string(&table, "network-id")?
            .parse::<NetworkId>()
            .map_err(|error| LockfileError::invalid(error.to_string()))?;
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: parse_u64_string(&table, "finalized-height")?,
            finalized_block_hash: parse_digest(required_string(&table, "finalized-block-hash")?)?,
            index_revision: parse_u64_string(&table, "index-revision")?,
        };
        let root_values = parse_table_array(&table, "root")?;
        let node_values = parse_table_array(&table, "node")?;
        validate_serialized_consumer_lock_collection_counts(root_values, node_values)?;
        let roots = root_values
            .iter()
            .map(parse_root)
            .collect::<Result<Vec<_>, _>>()?;
        let nodes = node_values
            .iter()
            .map(parse_node)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(network_id, snapshot, roots, nodes)
    }

    /// Load and strictly parse a lock document.
    ///
    /// The final component must be one bounded, single-link regular file and
    /// is opened without following a symlink. Invalid UTF-8 is returned as
    /// [`io::ErrorKind::InvalidData`]. Ancestor replacement remains covered by
    /// the repository's descriptor-relative local-input release gate.
    pub fn read(path: &Path) -> Result<Self, LockfileError> {
        let bytes =
            read_bounded_single_link_regular_file_v1(path, MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1)
                .map_err(LockfileError::Io)?;
        let document = String::from_utf8(bytes).map_err(|error| {
            LockfileError::Io(io::Error::new(io::ErrorKind::InvalidData, error))
        })?;
        Self::parse(&document)
    }

    /// Render the unique canonical TOML representation.
    ///
    /// Root, node, and aggregate-edge counts are rejected before full semantic
    /// validation. Rendering never clones or silently repairs a caller-mutated
    /// invalid graph; callers may invoke [`Self::canonicalize`] explicitly.
    /// The formatter stops accepting bytes at the exact consumer-lock ceiling
    /// and returns [`LockfileError::Invalid`] instead of growing beyond it.
    pub fn render(&self) -> Result<String, LockfileError> {
        validate_consumer_lock_collection_counts(&self.roots, &self.nodes)?;
        self.validate()?;

        let mut output = BoundedLockDocumentV1::new();
        writeln!(output, "schema = {}", quote(LOCK_SCHEMA)).expect("write to string");
        writeln!(output, "version = {LOCK_VERSION}").expect("write to string");
        writeln!(
            output,
            "network-id = {}",
            quote(&self.network_id.to_string())
        )
        .expect("write to string");
        writeln!(
            output,
            "finalized-height = {}",
            quote(&self.snapshot.finalized_height.to_string())
        )
        .expect("write to string");
        writeln!(
            output,
            "finalized-block-hash = {}",
            quote(&hex_digest(self.snapshot.finalized_block_hash))
        )
        .expect("write to string");
        writeln!(
            output,
            "index-revision = {}",
            quote(&self.snapshot.index_revision.to_string())
        )
        .expect("write to string");

        for root in &self.roots {
            if output.is_exhausted() {
                break;
            }
            writeln!(output, "\n[[root]]").expect("write to string");
            writeln!(output, "package = {}", quote(&root.package.to_string()))
                .expect("write to string");
            for edge in &root.dependencies {
                if output.is_exhausted() {
                    break;
                }
                writeln!(output, "\n[[root.dependency]]").expect("write to string");
                render_edge(&mut output, edge);
            }
        }
        for node in &self.nodes {
            if output.is_exhausted() {
                break;
            }
            writeln!(output, "\n[[node]]").expect("write to string");
            render_node(&mut output, node, "node.dependency");
        }
        output.finish()
    }

    /// Durably and atomically replace a root-relative lockfile.
    pub fn write_atomic(
        &self,
        write_root: &AtomicWriteRoot,
        relative_path: &Path,
    ) -> Result<(), LockfileError> {
        let document = self.render()?;
        write_root
            .replace(relative_path, document.as_bytes())
            .map_err(LockfileError::Atomic)
    }

    /// Produce the normalized publication verification lock for `workspace_root`.
    ///
    /// Root-local development edges and nodes reachable only through them are
    /// deliberately omitted from published verification material.
    pub fn verification_lock(
        &self,
        workspace_root: &MusubiPackageSelectorV1,
        published_root: MusubiReleaseIdV1,
    ) -> Result<MusubiVerificationLockV1, LockfileError> {
        self.validate()?;
        let root = self
            .roots
            .binary_search_by(|candidate| candidate.package.cmp(workspace_root))
            .ok()
            .and_then(|index| self.roots.get(index))
            .ok_or_else(|| LockfileError::invalid("selected publication root is not locked"))?;
        let by_release = self
            .nodes
            .iter()
            .map(|node| (&node.release, node))
            .collect::<BTreeMap<_, _>>();
        let mut reachable = BTreeSet::new();
        let mut pending = root
            .dependencies
            .iter()
            .filter(|edge| edge.kind == MusubiDependencyKindV1::Normal)
            .map(|edge| &edge.selected)
            .collect::<Vec<_>>();
        while let Some(release) = pending.pop() {
            if !reachable.insert(release) {
                continue;
            }
            let node = by_release.get(release).ok_or_else(|| {
                LockfileError::invalid("publication graph references a missing node")
            })?;
            pending.extend(node.dependencies.iter().map(|edge| &edge.selected));
        }
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: published_root,
            root_dependencies: root
                .dependencies
                .iter()
                .filter(|edge| edge.kind == MusubiDependencyKindV1::Normal)
                .cloned()
                .collect(),
            nodes: self
                .nodes
                .iter()
                .filter(|node| reachable.contains(&node.release))
                .cloned()
                .collect(),
        };
        lock.validate()
            .map_err(|error| LockfileError::invalid(error.reason()))?;
        Ok(lock)
    }
}

/// Render the normalized, publication-only exact graph packaged with a release.
///
/// The document intentionally uses the same first-release schema/version marker
/// as a consumer lock, but omits network identity, snapshot, workspace selectors, and
/// development edges. Its root and every dependency are structural identities.
/// The typed Norito lock remains authoritative; this deterministic TOML is the
/// source-tree representation intended for human inspection and clean rebuilds.
pub fn render_verification_lock(lock: &MusubiVerificationLockV1) -> Result<String, LockfileError> {
    render_verification_lock_with_limit(lock, MUSUBI_MAX_VERIFICATION_LOCK_BYTES_V1)
}

fn render_verification_lock_with_limit(
    lock: &MusubiVerificationLockV1,
    limit: u64,
) -> Result<String, LockfileError> {
    lock.validate()
        .map_err(|error| LockfileError::invalid(error.reason()))?;

    let mut output = BoundedLockDocumentV1::with_limit(limit);
    writeln!(output, "schema = {}", quote(LOCK_SCHEMA)).expect("write to string");
    writeln!(output, "version = {LOCK_VERSION}").expect("write to string");
    writeln!(output, "kind = {}", quote("verification")).expect("write to string");
    writeln!(output, "\n[root]").expect("write to string");
    render_package(&mut output, &lock.root.package);
    writeln!(
        output,
        "version = {}",
        quote(&lock.root.version.to_string())
    )
    .expect("write to string");
    for edge in &lock.root_dependencies {
        if output.is_exhausted() {
            break;
        }
        writeln!(output, "\n[[root.dependency]]").expect("write to string");
        render_edge(&mut output, edge);
    }
    for node in &lock.nodes {
        if output.is_exhausted() {
            break;
        }
        writeln!(output, "\n[[node]]").expect("write to string");
        render_node(&mut output, node, "node.dependency");
    }
    output.finish_with_bound("verification lock", limit)
}

/// Stable lockfile failure categories.
#[derive(Debug)]
pub enum LockfileError {
    /// A retired or absent schema must be regenerated; it is never upgraded in place.
    Legacy,
    /// TOML syntax was invalid (including duplicate keys).
    Toml(toml::de::Error),
    /// The strict V1 schema or graph invariant was violated.
    Invalid(String),
    /// Reading the lock failed.
    Io(io::Error),
    /// Durable atomic replacement failed.
    Atomic(AtomicWriteError),
}

impl LockfileError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

impl fmt::Display for LockfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Legacy => formatter
                .write_str("unsupported pre-release Musubi.lock; regenerate it with Musubi V1"),
            Self::Toml(error) => write!(formatter, "invalid Musubi.lock TOML: {error}"),
            Self::Invalid(message) => write!(formatter, "invalid Musubi V1 lock: {message}"),
            Self::Io(error) => write!(formatter, "failed to read Musubi.lock: {error}"),
            Self::Atomic(error) => write!(formatter, "failed to replace Musubi.lock: {error}"),
        }
    }
}

impl std::error::Error for LockfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toml(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Atomic(error) => Some(error),
            Self::Legacy | Self::Invalid(_) => None,
        }
    }
}

fn validate_edges(
    edges: &[MusubiExactDependencyEdgeV1],
    allow_development: bool,
    context: &str,
) -> Result<(), LockfileError> {
    if edges.len() > MUSUBI_MAX_DEPENDENCIES_V1
        || edges.windows(2).any(|pair| pair[0] >= pair[1])
        || edges.windows(2).any(|pair| pair[0].alias >= pair[1].alias)
    {
        return Err(LockfileError::invalid(format!(
            "{context} dependency edges exceed the bound, are not canonically sorted, or reuse an alias"
        )));
    }
    for edge in edges {
        edge.validate()
            .map_err(|error| LockfileError::invalid(error.reason()))?;
        if !allow_development && edge.kind == MusubiDependencyKindV1::Development {
            return Err(LockfileError::invalid(
                "development dependencies may appear only on selected workspace roots",
            ));
        }
    }
    Ok(())
}

fn validate_graph(
    roots: &[LockedRootV1],
    nodes: &[MusubiVerificationNodeV1],
) -> Result<(), LockfileError> {
    fn visit<'a>(
        release: &'a MusubiReleaseIdV1,
        depth: u16,
        by_release: &BTreeMap<&'a MusubiReleaseIdV1, &'a MusubiVerificationNodeV1>,
        visiting: &mut BTreeSet<&'a MusubiReleaseIdV1>,
        complete: &mut BTreeSet<&'a MusubiReleaseIdV1>,
    ) -> Result<(), LockfileError> {
        if depth > MUSUBI_MAX_RESOLUTION_DEPTH_V1 {
            return Err(LockfileError::invalid(
                "dependency graph exceeds the V1 depth bound",
            ));
        }
        if complete.contains(release) {
            return Ok(());
        }
        if !visiting.insert(release) {
            return Err(LockfileError::invalid(format!(
                "dependency graph contains a cycle at `{release}`"
            )));
        }
        let node = by_release
            .get(release)
            .ok_or_else(|| LockfileError::invalid("dependency graph node is missing"))?;
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
    for edge in roots
        .iter()
        .flat_map(|root| root.dependencies.iter())
        .chain(nodes.iter().flat_map(|node| node.dependencies.iter()))
    {
        if !by_release.contains_key(&edge.selected) {
            return Err(LockfileError::invalid(format!(
                "dependency edge references missing node `{}`",
                edge.selected
            )));
        }
    }

    let mut visiting = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for root in roots {
        for edge in &root.dependencies {
            visit(&edge.selected, 1, &by_release, &mut visiting, &mut complete)?;
        }
    }
    if complete.len() != nodes.len() {
        return Err(LockfileError::invalid(
            "lock graph contains a node unreachable from every selected workspace root",
        ));
    }
    Ok(())
}

fn parse_root(value: &toml::Value) -> Result<LockedRootV1, LockfileError> {
    let table = value
        .as_table()
        .ok_or_else(|| LockfileError::invalid("root entries must be TOML tables"))?;
    reject_unknown(table, ROOT_ENTRY_KEYS, "root entry")?;
    let package = required_string(table, "package")?
        .parse()
        .map_err(|error: iroha_data_model::ParseError| LockfileError::invalid(error.reason()))?;
    let dependencies = parse_table_array(table, "dependency")?
        .iter()
        .map(parse_edge)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LockedRootV1 {
        package,
        dependencies,
    })
}

fn parse_node(value: &toml::Value) -> Result<MusubiVerificationNodeV1, LockfileError> {
    let table = value
        .as_table()
        .ok_or_else(|| LockfileError::invalid("node entries must be TOML tables"))?;
    reject_unknown(table, NODE_KEYS, "node entry")?;
    let release = MusubiReleaseIdV1::new(
        parse_package(table)?,
        parse_version(required_string(table, "version")?)?,
    );
    let abi_version = required_integer(table, "abi-version")?;
    let abi_version = u16::try_from(abi_version)
        .map_err(|_| LockfileError::invalid("abi-version is outside the u16 range"))?;
    let abi_hash = parse_digest(required_string(table, "abi-hash")?)?;
    let abi = MusubiAbiBindingV1 {
        abi_version,
        abi_hash,
    };
    let dependencies = parse_table_array(table, "dependency")?
        .iter()
        .map(parse_edge)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MusubiVerificationNodeV1 {
        release,
        release_digest: MusubiReleaseDigestV1::new(parse_digest(required_string(
            table,
            "release-digest",
        )?)?),
        archive_id: ArchiveId::new(parse_digest(required_string(table, "archive-id")?)?),
        source_digest: MusubiContentDigestV1::new(parse_digest(required_string(
            table,
            "source-digest",
        )?)?),
        interface_digest: MusubiContentDigestV1::new(parse_digest(required_string(
            table,
            "interface-digest",
        )?)?),
        abi,
        dependencies,
    })
}

fn parse_edge(value: &toml::Value) -> Result<MusubiExactDependencyEdgeV1, LockfileError> {
    let table = value
        .as_table()
        .ok_or_else(|| LockfileError::invalid("dependency entries must be TOML tables"))?;
    reject_unknown(table, EDGE_KEYS, "dependency entry")?;
    let package = parse_package(table)?;
    let kind = match required_string(table, "kind")? {
        "normal" => MusubiDependencyKindV1::Normal,
        "development" => MusubiDependencyKindV1::Development,
        other => {
            return Err(LockfileError::invalid(format!(
                "unknown dependency kind `{other}`"
            )));
        }
    };
    Ok(MusubiExactDependencyEdgeV1 {
        alias: required_string(table, "alias")?.parse().map_err(
            |error: iroha_data_model::ParseError| LockfileError::invalid(error.reason()),
        )?,
        kind,
        package: package.clone(),
        requirement: MusubiVersionReqV1::new(required_string(table, "requirement")?)
            .map_err(|error| LockfileError::invalid(error.reason()))?,
        selected: MusubiReleaseIdV1::new(
            package,
            parse_version(required_string(table, "selected-version")?)?,
        ),
    })
}

fn parse_package(table: &toml::Table) -> Result<MusubiPackageIdV1, LockfileError> {
    let home_dataspace = parse_u64_string(table, "home-dataspace")?;
    let scope = match required_string(table, "scope")? {
        "dataspace-root" => {
            if table.contains_key("domain") {
                return Err(LockfileError::invalid(
                    "dataspace-root package must not declare domain",
                ));
            }
            MusubiPackageScopeV1::DataspaceRoot
        }
        "domain" => {
            MusubiPackageScopeV1::Domain(required_string(table, "domain")?.parse().map_err(
                |error: iroha_data_model::ParseError| LockfileError::invalid(error.reason()),
            )?)
        }
        other => {
            return Err(LockfileError::invalid(format!(
                "unknown package scope `{other}`"
            )));
        }
    };
    let name = required_string(table, "name")?
        .parse()
        .map_err(|error: iroha_data_model::ParseError| LockfileError::invalid(error.reason()))?;
    Ok(MusubiPackageIdV1::new(
        DataSpaceId::new(home_dataspace),
        scope,
        name,
    ))
}

fn render_edge(output: &mut impl fmt::Write, edge: &MusubiExactDependencyEdgeV1) {
    writeln!(output, "alias = {}", quote(edge.alias.as_ref())).expect("write to string");
    let kind = match edge.kind {
        MusubiDependencyKindV1::Normal => "normal",
        MusubiDependencyKindV1::Development => "development",
    };
    writeln!(output, "kind = {}", quote(kind)).expect("write to string");
    render_package(output, &edge.package);
    writeln!(
        output,
        "requirement = {}",
        quote(&edge.requirement.to_string())
    )
    .expect("write to string");
    writeln!(
        output,
        "selected-version = {}",
        quote(&edge.selected.version.to_string())
    )
    .expect("write to string");
}

fn render_node(output: &mut impl fmt::Write, node: &MusubiVerificationNodeV1, edge_table: &str) {
    render_package(output, &node.release.package);
    writeln!(
        output,
        "version = {}",
        quote(&node.release.version.to_string())
    )
    .expect("write to string");
    writeln!(
        output,
        "release-digest = {}",
        quote(&hex_digest(*node.release_digest.as_bytes()))
    )
    .expect("write to string");
    writeln!(
        output,
        "archive-id = {}",
        quote(&hex_digest(*node.archive_id.as_bytes()))
    )
    .expect("write to string");
    writeln!(
        output,
        "source-digest = {}",
        quote(&hex_digest(*node.source_digest.as_bytes()))
    )
    .expect("write to string");
    writeln!(
        output,
        "interface-digest = {}",
        quote(&hex_digest(*node.interface_digest.as_bytes()))
    )
    .expect("write to string");
    writeln!(output, "abi-version = {}", node.abi.abi_version).expect("write to string");
    writeln!(
        output,
        "abi-hash = {}",
        quote(&hex_digest(node.abi.abi_hash))
    )
    .expect("write to string");
    for edge in &node.dependencies {
        writeln!(output, "\n[[{edge_table}]]").expect("write to string");
        render_edge(output, edge);
    }
}

fn render_package(output: &mut impl fmt::Write, package: &MusubiPackageIdV1) {
    writeln!(
        output,
        "home-dataspace = {}",
        quote(&package.home_dataspace.as_u64().to_string())
    )
    .expect("write to string");
    match &package.scope {
        MusubiPackageScopeV1::DataspaceRoot => {
            writeln!(output, "scope = {}", quote("dataspace-root")).expect("write to string");
        }
        MusubiPackageScopeV1::Domain(domain) => {
            writeln!(output, "scope = {}", quote("domain")).expect("write to string");
            writeln!(output, "domain = {}", quote(domain.as_ref())).expect("write to string");
        }
    }
    writeln!(output, "name = {}", quote(package.name.as_str())).expect("write to string");
}

fn reject_unknown(
    table: &toml::Table,
    allowed: &[&str],
    context: &str,
) -> Result<(), LockfileError> {
    if let Some(key) = table.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(LockfileError::invalid(format!(
            "unknown field `{key}` in {context}"
        )));
    }
    Ok(())
}

fn required_string<'a>(table: &'a toml::Table, key: &str) -> Result<&'a str, LockfileError> {
    optional_string(table, key)?
        .ok_or_else(|| LockfileError::invalid(format!("missing string field `{key}`")))
}

fn optional_string<'a>(
    table: &'a toml::Table,
    key: &str,
) -> Result<Option<&'a str>, LockfileError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| LockfileError::invalid(format!("field `{key}` must be a string")))
        })
        .transpose()
}

fn required_integer(table: &toml::Table, key: &str) -> Result<i64, LockfileError> {
    optional_integer(table, key)?
        .ok_or_else(|| LockfileError::invalid(format!("missing integer field `{key}`")))
}

fn optional_integer(table: &toml::Table, key: &str) -> Result<Option<i64>, LockfileError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_integer()
                .ok_or_else(|| LockfileError::invalid(format!("field `{key}` must be an integer")))
        })
        .transpose()
}

fn parse_u64_string(table: &toml::Table, key: &str) -> Result<u64, LockfileError> {
    let raw = required_string(table, key)?;
    let value = raw.parse::<u64>().map_err(|_| {
        LockfileError::invalid(format!("field `{key}` must be a canonical u64 string"))
    })?;
    if value.to_string() != raw {
        return Err(LockfileError::invalid(format!(
            "field `{key}` must be a canonical u64 string"
        )));
    }
    Ok(value)
}

fn parse_table_array<'a>(
    table: &'a toml::Table,
    key: &str,
) -> Result<&'a [toml::Value], LockfileError> {
    table.get(key).map_or(Ok(&[]), |value| {
        value.as_array().map(Vec::as_slice).ok_or_else(|| {
            LockfileError::invalid(format!("field `{key}` must be an array of tables"))
        })
    })
}

fn parse_version(raw: &str) -> Result<MusubiVersionV1, LockfileError> {
    raw.parse()
        .map_err(|error: iroha_data_model::ParseError| LockfileError::invalid(error.reason()))
}

fn parse_digest(raw: &str) -> Result<[u8; 32], LockfileError> {
    if raw.len() != 64 || raw.bytes().any(|byte| !byte.is_ascii_hexdigit()) {
        return Err(LockfileError::invalid(
            "digest must contain exactly 64 hexadecimal digits",
        ));
    }
    if raw.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(LockfileError::invalid(
            "digest hexadecimal text must be lowercase",
        ));
    }
    let bytes = hex::decode(raw)
        .map_err(|error| LockfileError::invalid(format!("invalid digest: {error}")))?;
    bytes
        .try_into()
        .map_err(|_| LockfileError::invalid("digest must decode to 32 bytes"))
}

fn hex_digest(bytes: [u8; 32]) -> String {
    hex::encode(bytes)
}

fn quote(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

fn validate_consumer_lock_document_bytes(byte_len: usize) -> Result<(), LockfileError> {
    let byte_len = u64::try_from(byte_len).unwrap_or(u64::MAX);
    if byte_len > MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1 {
        return Err(LockfileError::invalid(format!(
            "consumer lock exceeds the {MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1}-byte bound"
        )));
    }
    Ok(())
}

fn validate_consumer_lock_collection_counts(
    roots: &[LockedRootV1],
    nodes: &[MusubiVerificationNodeV1],
) -> Result<(), LockfileError> {
    if roots.len() > MUSUBI_MAX_CONSUMER_LOCK_ROOTS_V1 {
        return Err(LockfileError::invalid(
            "workspace roots exceed the consumer-lock bound",
        ));
    }
    if nodes.len() > MUSUBI_MAX_RESOLUTION_NODES_V1 {
        return Err(LockfileError::invalid(
            "registry nodes exceed the consumer-lock bound",
        ));
    }
    let total_edges = roots
        .iter()
        .map(|root| root.dependencies.len())
        .chain(nodes.iter().map(|node| node.dependencies.len()))
        .try_fold(0_usize, |total, count| total.checked_add(count))
        .ok_or_else(|| LockfileError::invalid("consumer-lock edge count overflowed"))?;
    if total_edges > MUSUBI_MAX_CONSUMER_LOCK_EDGES_V1 {
        return Err(LockfileError::invalid(
            "dependency edges exceed the total consumer-lock bound",
        ));
    }
    Ok(())
}

fn validate_serialized_consumer_lock_collection_counts(
    roots: &[toml::Value],
    nodes: &[toml::Value],
) -> Result<(), LockfileError> {
    if roots.len() > MUSUBI_MAX_CONSUMER_LOCK_ROOTS_V1 {
        return Err(LockfileError::invalid(
            "workspace roots exceed the consumer-lock bound",
        ));
    }
    if nodes.len() > MUSUBI_MAX_RESOLUTION_NODES_V1 {
        return Err(LockfileError::invalid(
            "registry nodes exceed the consumer-lock bound",
        ));
    }
    let total_edges = roots
        .iter()
        .chain(nodes)
        .filter_map(toml::Value::as_table)
        .filter_map(|table| table.get("dependency"))
        .filter_map(toml::Value::as_array)
        .try_fold(0_usize, |total, edges| total.checked_add(edges.len()))
        .ok_or_else(|| LockfileError::invalid("consumer-lock edge count overflowed"))?;
    if total_edges > MUSUBI_MAX_CONSUMER_LOCK_EDGES_V1 {
        return Err(LockfileError::invalid(
            "dependency edges exceed the total consumer-lock bound",
        ));
    }
    Ok(())
}

struct BoundedLockDocumentV1 {
    output: String,
    limit: usize,
    exceeded: bool,
    allocation_failed: bool,
}

impl BoundedLockDocumentV1 {
    const RESERVE_CHUNK_BYTES: usize = 64 * 1024;

    fn new() -> Self {
        Self::with_limit(MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1)
    }

    fn with_limit(limit: u64) -> Self {
        Self {
            output: String::new(),
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
            exceeded: false,
            allocation_failed: false,
        }
    }

    fn is_exhausted(&self) -> bool {
        self.exceeded || self.allocation_failed
    }

    fn finish(self) -> Result<String, LockfileError> {
        let output = self.finish_with_bound("consumer lock", MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1)?;
        validate_consumer_lock_document_bytes(output.len())?;
        Ok(output)
    }

    fn finish_with_bound(
        self,
        document: &'static str,
        limit: u64,
    ) -> Result<String, LockfileError> {
        if self.allocation_failed {
            return Err(LockfileError::invalid(format!(
                "{document} rendering allocation failed"
            )));
        }
        if self.exceeded {
            return Err(LockfileError::invalid(format!(
                "{document} exceeds the {limit}-byte bound"
            )));
        }
        Ok(self.output)
    }
}

impl fmt::Write for BoundedLockDocumentV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.exceeded || self.allocation_failed {
            return Ok(());
        }
        let Some(length) = self.output.len().checked_add(value.len()) else {
            self.exceeded = true;
            return Ok(());
        };
        if length > self.limit {
            self.exceeded = true;
            return Ok(());
        }
        if length > self.output.capacity() {
            let target = length
                .div_ceil(Self::RESERVE_CHUNK_BYTES)
                .saturating_mul(Self::RESERVE_CHUNK_BYTES)
                .min(self.limit);
            if self
                .output
                .try_reserve_exact(target.saturating_sub(self.output.len()))
                .is_err()
            {
                self.allocation_failed = true;
                return Ok(());
            }
        }
        self.output.push_str(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};

    use tempfile::tempdir;

    use super::*;

    fn network_id() -> NetworkId {
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
            .parse()
            .expect("network id")
    }

    fn package(dataspace: u64, name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(dataspace),
            MusubiPackageScopeV1::DataspaceRoot,
            name.parse().expect("package name"),
        )
    }

    fn root_selector(index: usize) -> MusubiPackageSelectorV1 {
        format!("test/app{index}")
            .parse()
            .expect("root package selector")
    }

    fn edge(
        alias: &str,
        package: MusubiPackageIdV1,
        requirement: &str,
        version: &str,
        kind: MusubiDependencyKindV1,
    ) -> MusubiExactDependencyEdgeV1 {
        MusubiExactDependencyEdgeV1 {
            alias: alias.parse().expect("alias"),
            package: package.clone(),
            requirement: requirement.parse().expect("requirement"),
            selected: MusubiReleaseIdV1::new(package, version.parse().expect("version")),
            kind,
        }
    }

    fn node(
        package: MusubiPackageIdV1,
        version: &str,
        dependencies: Vec<MusubiExactDependencyEdgeV1>,
        seed: u8,
    ) -> MusubiVerificationNodeV1 {
        MusubiVerificationNodeV1 {
            release: MusubiReleaseIdV1::new(package, version.parse().expect("version")),
            release_digest: MusubiReleaseDigestV1::new([seed; 32]),
            archive_id: ArchiveId::new([seed.wrapping_add(1); 32]),
            source_digest: MusubiContentDigestV1::new([seed.wrapping_add(2); 32]),
            interface_digest: MusubiContentDigestV1::new([seed.wrapping_add(3); 32]),
            abi: MusubiAbiBindingV1::new([seed.wrapping_add(4); 32]).expect("ABI"),
            dependencies,
        }
    }

    fn lock() -> LockfileV1 {
        let leaf = package(2, "leaf");
        let parent = package(1, "parent");
        let parent_edge = edge(
            "leaf",
            leaf.clone(),
            "^1.0.0",
            "1.4.0",
            MusubiDependencyKindV1::Normal,
        );
        let root_edge = edge(
            "parent",
            parent.clone(),
            "^2.0.0",
            "2.1.0",
            MusubiDependencyKindV1::Development,
        );
        LockfileV1::new(
            network_id(),
            MusubiRegistrySnapshotV1 {
                finalized_height: 17,
                finalized_block_hash: [8; 32],
                index_revision: 3,
            },
            vec![LockedRootV1 {
                package: "test/app".parse().expect("root package"),
                dependencies: vec![root_edge],
            }],
            vec![
                node(leaf, "1.4.0", vec![], 20),
                node(parent, "2.1.0", vec![parent_edge], 10),
            ],
        )
        .expect("lock")
    }

    #[test]
    fn canonical_roundtrip_is_stable_and_secret_free() {
        let lock = lock();
        let first = lock.render().expect("render");
        let decoded = LockfileV1::parse(&first).expect("parse");
        let second = decoded.render().expect("render again");
        assert_eq!(decoded, lock);
        assert_eq!(first, second);
        for forbidden in [
            "cache_path",
            "provider-url",
            "bearer",
            "credential",
            "source-plan",
            "timestamp",
        ] {
            assert!(!first.contains(forbidden));
        }
    }

    #[test]
    fn render_rejects_mutated_noncanonical_or_oversized_fields_without_repair() {
        let mut noncanonical = lock();
        noncanonical.nodes.reverse();
        assert!(matches!(
            noncanonical.render(),
            Err(LockfileError::Invalid(reason)) if reason.contains("uniquely sorted")
        ));
        noncanonical.canonicalize();
        noncanonical.render().expect("explicit canonicalization");

        let mut oversized_nested = lock();
        oversized_nested.nodes[0].release.version.prerelease = vec![
            iroha_data_model::musubi::MusubiPrereleaseIdentifierV1::AlphaNumeric(
                "x".repeat(iroha_data_model::musubi::MUSUBI_MAX_PRERELEASE_IDENTIFIER_BYTES_V1 + 1),
            ),
        ];
        assert!(matches!(
            oversized_nested.render(),
            Err(LockfileError::Invalid(reason)) if reason.contains("prerelease")
        ));

        let mut oversized_schema = lock();
        oversized_schema.schema = "x".repeat(1024 * 1024);
        assert!(matches!(
            oversized_schema.render(),
            Err(LockfileError::Legacy)
        ));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one boundary test keeps byte, root, aggregate-edge, bounded-writer, and serialized preflight checks adjacent"
    )]
    fn consumer_lock_resource_bounds_are_enforced() {
        let exact_max = usize::try_from(MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1)
            .expect("consumer-lock byte bound fits usize");
        validate_consumer_lock_document_bytes(exact_max).expect("exact byte bound");
        assert!(matches!(
            validate_consumer_lock_document_bytes(exact_max + 1),
            Err(LockfileError::Invalid(reason)) if reason.contains("byte bound")
        ));

        let mut exact_roots = lock();
        let root_dependencies = exact_roots.roots[0].dependencies.clone();
        exact_roots.roots = (0..MUSUBI_MAX_CONSUMER_LOCK_ROOTS_V1)
            .map(|index| LockedRootV1 {
                package: root_selector(index),
                dependencies: root_dependencies.clone(),
            })
            .collect();
        exact_roots.canonicalize();
        exact_roots.validate().expect("exact root-count bound");

        let mut too_many_roots = exact_roots;
        too_many_roots.roots.push(LockedRootV1 {
            package: root_selector(MUSUBI_MAX_CONSUMER_LOCK_ROOTS_V1),
            dependencies: root_dependencies,
        });
        too_many_roots.canonicalize();
        assert!(matches!(
            too_many_roots.validate(),
            Err(LockfileError::Invalid(reason)) if reason.contains("workspace roots")
        ));
        assert!(matches!(
            too_many_roots.render(),
            Err(LockfileError::Invalid(reason)) if reason.contains("workspace roots")
        ));

        let mut exact_edges = lock();
        let parent_package = exact_edges
            .nodes
            .iter()
            .find(|node| node.release.package.name.as_str() == "parent")
            .expect("parent node")
            .release
            .package
            .clone();
        exact_edges.roots = (0..2)
            .map(|root_index| {
                let edge_count = if root_index == 1 { 255 } else { 256 };
                let dependencies = (0..edge_count)
                    .map(|edge_index| {
                        edge(
                            &format!("d{edge_index:03}"),
                            parent_package.clone(),
                            "^2.0.0",
                            "2.1.0",
                            MusubiDependencyKindV1::Normal,
                        )
                    })
                    .collect();
                LockedRootV1 {
                    package: root_selector(root_index),
                    dependencies,
                }
            })
            .collect();
        exact_edges.canonicalize();
        assert_eq!(
            exact_edges
                .roots
                .iter()
                .map(|root| root.dependencies.len())
                .chain(exact_edges.nodes.iter().map(|node| node.dependencies.len()))
                .sum::<usize>(),
            MUSUBI_MAX_CONSUMER_LOCK_EDGES_V1
        );
        exact_edges.validate().expect("exact total-edge bound");

        let mut too_many_edges = exact_edges;
        too_many_edges.roots[1].dependencies.push(edge(
            "d255",
            parent_package,
            "^2.0.0",
            "2.1.0",
            MusubiDependencyKindV1::Normal,
        ));
        too_many_edges.canonicalize();
        assert!(matches!(
            too_many_edges.validate(),
            Err(LockfileError::Invalid(reason)) if reason.contains("total consumer-lock bound")
        ));

        let mut bounded = BoundedLockDocumentV1 {
            output: String::new(),
            limit: 4,
            exceeded: false,
            allocation_failed: false,
        };
        fmt::Write::write_str(&mut bounded, "1234").expect("exact bounded write");
        fmt::Write::write_str(&mut bounded, "5").expect("overflow is retained as state");
        assert_eq!(bounded.output, "1234");
        assert!(matches!(
            bounded.finish_with_bound("test lock", 4),
            Err(LockfileError::Invalid(reason)) if reason.contains("byte bound")
        ));

        let mut too_many_serialized_roots = lock().render().expect("render fixture");
        for _ in 0..MUSUBI_MAX_CONSUMER_LOCK_ROOTS_V1 {
            too_many_serialized_roots.push_str("\n[[root]]\n");
        }
        assert!(matches!(
            LockfileV1::parse(&too_many_serialized_roots),
            Err(LockfileError::Invalid(reason)) if reason.contains("workspace roots")
        ));
    }

    #[test]
    fn read_rejects_an_oversized_sparse_consumer_lock_before_parsing() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("Musubi.lock");
        let expected = lock();
        fs::write(&path, expected.render().expect("render lock")).expect("write valid lock");
        assert_eq!(LockfileV1::read(&path).expect("read valid lock"), expected);

        let file = File::create(&path).expect("create sparse lock");
        file.set_len(MUSUBI_MAX_CONSUMER_LOCK_BYTES_V1 + 1)
            .expect("size sparse lock");

        assert!(matches!(
            LockfileV1::read(&path),
            Err(LockfileError::Io(error)) if error.kind() == io::ErrorKind::InvalidData
        ));
    }

    #[test]
    fn read_rejects_invalid_utf8_as_invalid_data() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("Musubi.lock");
        fs::write(&path, [0xFF]).expect("write invalid UTF-8 lock");
        assert!(matches!(
            LockfileV1::read(&path),
            Err(LockfileError::Io(error)) if error.kind() == io::ErrorKind::InvalidData
        ));
    }

    #[test]
    fn old_missing_and_future_schemas_require_regeneration() {
        for document in [
            "version = 3\n",
            "schema = \"musubi-lock\"\nversion = 2\n",
            "schema = \"legacy\"\nversion = 1\n",
            "package = \"pre-release-name\"\nsource = \"legacy-cache-plan\"\n",
        ] {
            assert!(matches!(
                LockfileV1::parse(document),
                Err(LockfileError::Legacy)
            ));
        }
    }

    #[test]
    fn unknown_and_duplicate_fields_are_rejected() {
        let document = lock().render().expect("render");
        let unknown = format!("unknown = true\n{document}");
        assert!(matches!(
            LockfileV1::parse(&unknown),
            Err(LockfileError::Invalid(_))
        ));
        let duplicate = document.replacen("version = 1", "version = 1\nversion = 1", 1);
        assert!(matches!(
            LockfileV1::parse(&duplicate),
            Err(LockfileError::Toml(_))
        ));
    }

    #[test]
    fn noncanonical_decimal_identifiers_are_rejected() {
        let document = lock().render().expect("render");
        for (canonical, noncanonical) in [
            ("finalized-height = \"17\"", "finalized-height = \"017\""),
            ("index-revision = \"3\"", "index-revision = \"+3\""),
            ("home-dataspace = \"1\"", "home-dataspace = \"01\""),
        ] {
            assert!(document.contains(canonical));
            let malformed = document.replacen(canonical, noncanonical, 1);
            assert!(matches!(
                LockfileV1::parse(&malformed),
                Err(LockfileError::Invalid(reason))
                    if reason.contains("must be a canonical u64 string")
            ));
        }
    }

    #[test]
    fn development_edges_never_propagate() {
        let mut lock = lock();
        let dependency_package = lock.nodes[1].release.package.clone();
        lock.nodes[0].dependencies.push(edge(
            "bad-dev",
            dependency_package,
            "^2.0.0",
            "2.1.0",
            MusubiDependencyKindV1::Development,
        ));
        lock.nodes[0].dependencies.sort();
        assert!(matches!(lock.validate(), Err(LockfileError::Invalid(_))));
    }

    #[test]
    fn consumer_roots_reject_duplicate_aliases_across_dependency_kinds() {
        let mut locked = lock();
        let root = locked.roots.first_mut().expect("fixture workspace root");
        root.dependencies.push(edge(
            "parent",
            package(3, "parallel"),
            "^1.0.0",
            "1.1.0",
            MusubiDependencyKindV1::Normal,
        ));
        root.dependencies.sort();

        let error = root
            .validate()
            .expect_err("one parent-local alias cannot name normal and development edges");
        assert!(error.to_string().contains("reuse an alias"));
    }

    #[test]
    fn cycles_and_missing_nodes_are_rejected() {
        let mut missing = lock();
        missing.nodes.pop();
        assert!(matches!(missing.validate(), Err(LockfileError::Invalid(_))));

        let mut cyclic = lock();
        let leaf_index = cyclic
            .nodes
            .iter()
            .position(|node| node.release.package.name.as_str() == "leaf")
            .expect("leaf node");
        let parent_index = cyclic
            .nodes
            .iter()
            .position(|node| node.release.package.name.as_str() == "parent")
            .expect("parent node");
        let leaf_release = cyclic.nodes[leaf_index].release.clone();
        let parent_release = cyclic.nodes[parent_index].release.clone();
        cyclic.nodes[leaf_index].dependencies.push(edge(
            "parent",
            parent_release.package.clone(),
            "^2.0.0",
            &parent_release.version.to_string(),
            MusubiDependencyKindV1::Normal,
        ));
        cyclic.nodes[leaf_index].dependencies.sort();
        assert_eq!(
            cyclic.nodes[parent_index].dependencies[0].selected,
            leaf_release
        );
        assert!(matches!(cyclic.validate(), Err(LockfileError::Invalid(_))));
    }

    #[test]
    fn multiple_versions_of_one_package_are_preserved() {
        let mut lock = lock();
        let shared = package(7, "shared");
        lock.nodes.push(node(shared.clone(), "1.9.0", vec![], 30));
        lock.nodes.push(node(shared.clone(), "2.2.0", vec![], 40));
        lock.roots[0].dependencies.extend([
            edge(
                "shared-v1",
                shared.clone(),
                "^1.0.0",
                "1.9.0",
                MusubiDependencyKindV1::Normal,
            ),
            edge(
                "shared-v2",
                shared,
                "^2.0.0",
                "2.2.0",
                MusubiDependencyKindV1::Normal,
            ),
        ]);
        lock.canonicalize();
        lock.validate().expect("parallel versions are valid");
        assert_eq!(
            lock.nodes
                .iter()
                .filter(|node| node.release.package.name.as_str() == "shared")
                .count(),
            2
        );
    }

    #[test]
    fn publication_verification_lock_omits_root_development_graph() {
        let lock = lock();
        let published = MusubiReleaseIdV1::new(
            package(9, "app"),
            "1.0.0".parse().expect("published version"),
        );
        let verification = lock
            .verification_lock(&"test/app".parse().expect("root package"), published)
            .expect("verification lock");
        assert!(verification.nodes.is_empty());
    }

    #[test]
    fn publication_verification_toml_is_structural_and_snapshot_free() {
        let mut lock = lock();
        lock.roots[0].dependencies[0].kind = MusubiDependencyKindV1::Normal;
        lock.canonicalize();
        lock.validate().expect("normal publication graph");
        let published = MusubiReleaseIdV1::new(
            package(9, "app"),
            "1.0.0".parse().expect("published version"),
        );
        let verification = lock
            .verification_lock(&"test/app".parse().expect("root package"), published)
            .expect("verification lock");
        let rendered = render_verification_lock(&verification).expect("render verification lock");

        assert!(rendered.starts_with(
            "schema = \"musubi-lock\"\nversion = 1\nkind = \"verification\"\n\n[root]\n"
        ));
        assert!(rendered.contains("home-dataspace = \"9\""));
        assert!(rendered.contains("[[root.dependency]]"));
        assert!(rendered.contains("[[node]]"));
        assert!(!rendered.contains("network-id ="));
        assert!(!rendered.contains("finalized-height"));
        assert!(!rendered.contains("development"));
        assert_eq!(
            rendered,
            render_verification_lock(&verification).expect("repeat render")
        );
    }

    #[test]
    fn publication_verification_toml_honors_its_aggregate_byte_bound() {
        let mut lock = lock();
        lock.roots[0].dependencies[0].kind = MusubiDependencyKindV1::Normal;
        lock.canonicalize();
        let verification = lock
            .verification_lock(
                &"test/app".parse().expect("root package"),
                MusubiReleaseIdV1::new(
                    package(9, "app"),
                    "1.0.0".parse().expect("published version"),
                ),
            )
            .expect("verification lock");
        let rendered = render_verification_lock(&verification).expect("bounded render");
        let exact = u64::try_from(rendered.len()).expect("rendered length fits u64");

        assert_eq!(
            render_verification_lock_with_limit(&verification, exact)
                .expect("exact verification-lock byte bound"),
            rendered
        );
        assert!(matches!(
            render_verification_lock_with_limit(&verification, exact - 1),
            Err(LockfileError::Invalid(reason))
                if reason.contains("verification lock") && reason.contains("byte bound")
        ));
    }

    #[test]
    fn roots_with_the_same_name_in_distinct_namespaces_remain_distinct() {
        let mut lock = lock();
        let second = LockedRootV1 {
            package: "other/app".parse().expect("second root package"),
            dependencies: lock.roots[0].dependencies.clone(),
        };
        lock.roots.push(second);
        lock.canonicalize();
        lock.validate().expect("namespaced roots do not collide");
        let rendered = lock.render().expect("render namespaced roots");
        assert!(rendered.contains("package = \"test/app\""));
        assert!(rendered.contains("package = \"other/app\""));
        assert_eq!(LockfileV1::parse(&rendered).expect("parse roots"), lock);
    }
}

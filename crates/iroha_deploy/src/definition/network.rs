//! The network definition, `networks/<name>.toml` (spec §3.2).

use std::{
    net::IpAddr,
    path::{Path, PathBuf},
};

use iroha_config_base::{
    ReadConfig,
    read::{ConfigReader, FinalWrap, ReadConfig as ReadConfigTrait},
};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT,
};
use iroha_primitives::numeric::Quantity;

use super::{
    ByteSize, CredentialScope, DefinitionError, Entries, HostIdentity, HostKey, HostPins, HttpsUrl,
    Issues, Monitor, NodePorts, Origin, Permission, Placement, ProfileId, ReleaseSource, Role,
    Slug, Ssh, Upstream, ValueError, check_host, check_unique,
    hosts::{
        DEFAULT_P2P_PORT, DEFAULT_TORII_PORT, FailureDomain, Member, check_edge_common,
        check_members, check_origins, check_ssh, check_ssh_presence, failure_domain,
        fault_tolerance, placement,
    },
    read_optional, read_source, source_from_file, source_from_str,
};

const DEFAULT_JOURNAL_MAX: ByteSize = ByteSize::new(2 << 30);

/// `[network]`: identity and authority.
#[derive(Debug, Clone, ReadConfig)]
pub struct NetworkSection {
    /// `[a-z0-9-]{1,32}`; names the state directory, units and host paths.
    pub name: Slug,
    /// The compiled profile.
    pub profile: ProfileId,
    /// The chain id; absent means a fresh `UUIDv4` per generation (devnets).
    pub chain_id: Option<String>,
    /// The chain discriminant; absent means the profile default
    /// (see [`NetworkDefinition::chain_discriminant`]).
    pub chain_discriminant: Option<u16>,
    /// The public `https` root; required iff `[edge]` is present.
    pub public_root: Option<HttpsUrl>,
    /// A 0600 Ed25519 admin key; absent means one is generated into
    /// `<state>/keys/admin.key`.
    pub admin_key: Option<PathBuf>,
    /// Extra Torii operator keys; `<state>/keys/operator.key` is always included.
    #[config(default)]
    pub operators: Vec<PublicKey>,
}

/// `[release]`: where signed release bundles come from.
#[derive(Debug, Clone, Default, ReadConfig)]
pub struct Release {
    /// A URL or directory of bundles; absent means the official channel
    /// compiled into the CLI.
    pub source: Option<ReleaseSource>,
    /// Ed25519 bundle signers; absent means the CI and maintainer keys
    /// compiled into the CLI.
    pub signers: Option<Vec<PublicKey>>,
}

/// `[inrou]`: the Inrou toggle for validator-role nodes.
///
/// Local definitions may enable it too: `up` refuses it unless the machine
/// runs Linux with KVM API 12 as root (spec §5 S3), a host check, not a
/// definition rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ReadConfig)]
pub struct Inrou {
    /// Whether validators run Inrou.
    #[config(default)]
    pub enabled: bool,
}

/// `[faucet]`: absent values take the profile default.
#[derive(Debug, Clone, Default, PartialEq, Eq, ReadConfig)]
pub struct Faucet {
    /// Whether the faucet runs.
    pub enabled: Option<bool>,
    /// The amount per request.
    pub amount: Option<Quantity>,
}

/// One `[[onboarding.credential]]`; renaming or removing an id rotates or revokes it.
#[derive(Debug, Clone, PartialEq, Eq, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct OnboardingCredential {
    /// The credential id; it names `<state>/credentials/<id>.token`.
    pub id: String,
    /// The onboarding scope.
    pub scope: CredentialScope,
}

/// `[edge]`: the public TLS edge and its routing to nodes.
#[derive(Debug, Clone, PartialEq, Eq, ReadConfig)]
pub struct Edge {
    /// The edge SSH host.
    pub host: String,
    /// The edge host's pinned key.
    pub host_key: HostKey,
    /// The public domain, e.g. `taira.sora.org`.
    pub domain: String,
    /// The certificate path on the edge host (certbot owns renewal).
    pub tls_certificate: PathBuf,
    /// The private key path on the edge host.
    pub tls_private_key: PathBuf,
    /// Allowed CORS origins.
    #[config(default)]
    pub cors_origins: Vec<String>,
    /// Serve each node at its own `node.domain`.
    #[config(default)]
    pub per_node_domains: bool,
    /// An optional explorer virtual host.
    pub explorer_domain: Option<String>,
    /// The explorer document root on the edge host.
    pub explorer_root: Option<PathBuf>,
    /// How the edge reaches node Torii endpoints.
    #[config(default)]
    pub upstream: Upstream,
}

/// `[retention]`: what hosts keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ReadConfig)]
pub struct Retention {
    /// Releases kept per host, including the running one.
    #[config(default = "3")]
    pub releases: u32,
    /// Retired ledgers kept per node.
    #[config(default = "1")]
    pub previous_ledgers: u32,
    /// Failed operations' state kept per node.
    #[config(default = "1")]
    pub failed_ops: u32,
    /// The journald size cap.
    #[config(default = "DEFAULT_JOURNAL_MAX")]
    pub journal_max: ByteSize,
}

/// `[local]`: local and container driver settings.
#[derive(Debug, Clone, PartialEq, Eq, ReadConfig)]
pub struct Local {
    /// The bind address.
    #[config(default = "Local::DEFAULT_BIND_HOST.to_owned()")]
    pub bind_host: String,
    /// The advertised host; defaults to `bind_host`.
    pub public_host: Option<String>,
    /// Torii port of node 0; node `i` uses `base_torii_port + i`.
    #[config(default = "Local::DEFAULT_BASE_TORII_PORT")]
    pub base_torii_port: u16,
    /// P2P port of node 0; node `i` uses `base_p2p_port + i`.
    #[config(default = "Local::DEFAULT_BASE_P2P_PORT")]
    pub base_p2p_port: u16,
}

impl Local {
    /// Default bind address.
    pub const DEFAULT_BIND_HOST: &'static str = "127.0.0.1";
    /// Default Torii port of node 0.
    pub const DEFAULT_BASE_TORII_PORT: u16 = 29080;
    /// Default P2P port of node 0.
    pub const DEFAULT_BASE_P2P_PORT: u16 = 29337;

    /// The advertised host.
    pub fn public_host(&self) -> &str {
        self.public_host.as_deref().unwrap_or(&self.bind_host)
    }
}

impl Default for Local {
    fn default() -> Self {
        Self {
            bind_host: Self::DEFAULT_BIND_HOST.to_owned(),
            public_host: None,
            base_torii_port: Self::DEFAULT_BASE_TORII_PORT,
            base_p2p_port: Self::DEFAULT_BASE_P2P_PORT,
        }
    }
}

/// `[scaling]`: perf-network shape for the local and container drivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ReadConfig)]
pub struct Scaling {
    /// Lane count.
    pub lanes: u32,
    /// Pre-created account count.
    pub accounts: u64,
}

/// One `[[grant]]`, converged on-chain by the admin (exact set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The grantee. It was parsed under the definition's chain discriminant;
    /// display it inside `ChainDiscriminantGuard::enter(definition.chain_discriminant())`
    /// to show the literal the operator wrote.
    pub account: AccountId,
    /// The permission.
    pub permission: Permission,
}

#[derive(Debug, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GrantEntry {
    account: String,
    permission: Permission,
}

/// One `[[node]]`.
#[derive(Debug, Clone, PartialEq, Eq, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct Node {
    /// Unique slug.
    pub name: Slug,
    /// The role, `validator` by default.
    #[norito(default)]
    pub role: Role,
    /// SSH host; absent means the local driver runs the node.
    pub host: Option<String>,
    /// The host's pinned key.
    pub host_key: Option<HostKey>,
    /// Advertised P2P host; defaults to `host`, or `[local] public_host`.
    pub address: Option<String>,
    /// Private Torii bind address; required iff `edge.upstream = "private"`.
    pub private_address: Option<String>,
    /// Per-node public domain; required iff `edge.per_node_domains`.
    pub domain: Option<String>,
    /// P2P port; 1337 remotely, `base_p2p_port + index` locally.
    pub p2p_port: Option<u16>,
    /// Torii port; 8080 remotely, `base_torii_port + index` locally.
    pub torii_port: Option<u16>,
    /// Failure domain label; defaults to the host identity.
    pub failure_domain: Option<String>,
}

/// A parsed and validated network definition.
#[derive(Debug, Clone)]
pub struct NetworkDefinition {
    /// The absolute path of the definition file.
    pub file: PathBuf,
    /// `[network]`.
    pub network: NetworkSection,
    /// `[release]`.
    pub release: Release,
    /// `[ssh]`; present iff nodes are remote.
    pub ssh: Option<Ssh>,
    /// `[inrou]`.
    pub inrou: Inrou,
    /// `[faucet]`.
    pub faucet: Faucet,
    /// `[[onboarding.credential]]`.
    pub onboarding_credentials: Vec<OnboardingCredential>,
    /// `[edge]`.
    pub edge: Option<Edge>,
    /// `[monitor]`.
    pub monitor: Monitor,
    /// `[retention]`.
    pub retention: Retention,
    /// `[local]`; only in local definitions.
    pub local: Option<Local>,
    /// `[scaling]`; only in local definitions.
    pub scaling: Option<Scaling>,
    /// `[[grant]]`.
    pub grants: Vec<Grant>,
    /// `[[node]]`, in file order.
    pub nodes: Vec<Node>,
}

/// The file as read, before paths are resolved and grants are parsed.
struct RawNetwork {
    network: NetworkSection,
    release: Release,
    ssh: Option<Ssh>,
    inrou: Inrou,
    faucet: Faucet,
    credentials: Entries<OnboardingCredential>,
    edge: Option<Edge>,
    monitor: Monitor,
    retention: Retention,
    local: Option<Local>,
    scaling: Option<Scaling>,
    grants: Entries<GrantEntry>,
    nodes: Entries<Node>,
}

impl ReadConfigTrait for RawNetwork {
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self> {
        let network = reader.read_nested("network");
        let release = reader.read_nested("release");
        let ssh = read_optional(reader, "ssh");
        let inrou = reader.read_nested("inrou");
        let faucet = reader.read_nested("faucet");
        let credentials = reader
            .read_parameter(["onboarding", "credential"])
            .value_or_default()
            .finish();
        let edge = read_optional(reader, "edge");
        let monitor = reader.read_nested("monitor");
        let retention = reader.read_nested("retention");
        let local = read_optional(reader, "local");
        let scaling = read_optional(reader, "scaling");
        let grants = reader.read_parameter(["grant"]).value_or_default().finish();
        let nodes = reader.read_parameter(["node"]).value_or_default().finish();
        FinalWrap::value_fn(move || Self {
            network: network.unwrap(),
            release: release.unwrap(),
            ssh: ssh.unwrap(),
            inrou: inrou.unwrap(),
            faucet: faucet.unwrap(),
            credentials: credentials.unwrap(),
            edge: edge.unwrap(),
            monitor: monitor.unwrap(),
            retention: retention.unwrap(),
            local: local.unwrap(),
            scaling: scaling.unwrap(),
            grants: grants.unwrap(),
            nodes: nodes.unwrap(),
        })
    }
}

impl NetworkDefinition {
    /// Load a definition file, expanding `~` to the current user's home directory.
    ///
    /// # Errors
    ///
    /// See [`DefinitionError`].
    pub fn load(path: impl AsRef<Path>) -> Result<Self, DefinitionError> {
        Self::load_with_home(path, std::env::home_dir().as_deref())
    }

    /// Load a definition file, expanding `~` to `home`.
    ///
    /// # Errors
    ///
    /// See [`DefinitionError`].
    pub fn load_with_home(
        path: impl AsRef<Path>,
        home: Option<&Path>,
    ) -> Result<Self, DefinitionError> {
        let origin = Origin::new(path.as_ref(), home);
        let source = source_from_file(&origin.file)?;
        Self::build(read_source(source, &origin.file)?, &origin)
    }

    /// Parse definition text as if it were read from `path`.
    ///
    /// # Errors
    ///
    /// See [`DefinitionError`].
    pub fn parse(
        text: &str,
        path: impl AsRef<Path>,
        home: Option<&Path>,
    ) -> Result<Self, DefinitionError> {
        let origin = Origin::new(path.as_ref(), home);
        let source = source_from_str(text, &origin.file)?;
        Self::build(read_source(source, &origin.file)?, &origin)
    }

    fn build(raw: RawNetwork, origin: &Origin) -> Result<Self, DefinitionError> {
        let RawNetwork {
            mut network,
            mut release,
            mut ssh,
            inrou,
            faucet,
            credentials,
            edge,
            mut monitor,
            retention,
            local,
            scaling,
            grants,
            nodes,
        } = raw;
        let mut issues = Issues::default();
        if let Some(path) = network.admin_key.as_mut() {
            origin.resolve("network.admin_key", path, &mut issues);
        }
        if let Some(ReleaseSource::Directory(path)) = release.source.as_mut() {
            origin.resolve("release.source", path, &mut issues);
        }
        if let Some(ssh) = ssh.as_mut() {
            origin.resolve("ssh.identity", &mut ssh.identity, &mut issues);
        }
        if let Some(path) = monitor.webhook_file.as_mut() {
            origin.resolve("monitor.webhook_file", path, &mut issues);
        }
        let discriminant = network
            .chain_discriminant
            .unwrap_or_else(|| network.profile.default_chain_discriminant());
        let grants = parse_grants(grants.0, discriminant, &mut issues);
        let definition = Self {
            file: origin.file.clone(),
            network,
            release,
            ssh,
            inrou,
            faucet,
            onboarding_credentials: credentials.0,
            edge,
            monitor,
            retention,
            local,
            scaling,
            grants,
            nodes: nodes.0,
        };
        definition.validate(&mut issues);
        issues.finish(&origin.file, definition)
    }

    fn validate(&self, issues: &mut Issues) {
        self.check_network_section(issues);
        let placement = self.check_nodes(issues);
        check_ssh_presence(placement, self.ssh.as_ref(), issues);
        if let Some(ssh) = &self.ssh {
            check_ssh(ssh, self.edge.is_some(), issues);
        }
        self.check_edge(placement, issues);
        self.check_local_only(placement, issues);
        // TODO(P5): `[inrou] enabled` is a host precondition, not a definition rule.
        // G0 checks at plan and `up` time that validator hosts (the local machine for
        // local definitions) run Linux with KVM API 12 as root, and that the release
        // carries a guest image for every validator ISA (spec §3.2, §5 S3, §9).
        self.check_credentials(issues);
        if self.retention.releases == 0 {
            issues.push(
                "retention.releases",
                "must keep at least the running release",
            );
        }
    }

    fn check_network_section(&self, issues: &mut Issues) {
        if let Some(chain_id) = &self.network.chain_id
            && (chain_id.is_empty() || chain_id.trim() != chain_id)
        {
            issues.push(
                "network.chain_id",
                "must be non-empty without surrounding whitespace",
            );
        }
        check_unique("network.operators", "key", &self.network.operators, issues);
        if let Some(signers) = &self.release.signers {
            if signers.is_empty() {
                issues.push(
                    "release.signers",
                    "must list at least one key; omit it to use the compiled-in signers",
                );
            }
            for (index, key) in signers.iter().enumerate() {
                if key.try_algorithm().ok() != Some(Algorithm::Ed25519) {
                    issues.push(
                        format!("release.signers[{index}]"),
                        "must be an Ed25519 key",
                    );
                }
            }
            check_unique("release.signers", "key", signers, issues);
        }
    }

    fn check_nodes(&self, issues: &mut Issues) -> Option<Placement> {
        let members: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| Member {
                name: &node.name,
                validator: node.role == Role::Validator,
                host: node.host.as_deref(),
                host_key: node.host_key.as_ref(),
                address: node.address.as_deref(),
                failure_domain: node.failure_domain.as_deref(),
                ports: self.node_ports(index),
            })
            .collect();
        let edge_pin = self
            .edge
            .as_ref()
            .map(|edge| ("edge", edge.host.as_str(), &edge.host_key));
        let placement = check_members(
            "node",
            &members,
            MAX_VALIDATORS_PER_HEIGHT,
            edge_pin.as_slice(),
            issues,
        );
        for (index, member) in members.iter().enumerate() {
            if member.ports.is_none() {
                issues.push(
                    format!("node[{index}]"),
                    "local port base + node index exceeds 65535",
                );
            }
        }
        placement
    }

    fn check_edge(&self, placement: Option<Placement>, issues: &mut Issues) {
        match (&self.edge, &self.network.public_root) {
            (Some(_), None) => {
                issues.push("network.public_root", "required when [edge] is present")
            }
            (None, Some(_)) => issues.push("network.public_root", "only used with [edge]"),
            _ => {}
        }
        if self.edge.is_some() && placement == Some(Placement::Local) {
            // Pinning local nodes would only lead to "host_key requires host".
            issues.push(
                "edge",
                "[edge] needs remote nodes; set `host` on every node or remove [edge]",
            );
        }
        for (index, node) in self.nodes.iter().enumerate() {
            self.check_node_edge_fields(index, node, issues);
        }
        let Some(edge) = &self.edge else {
            return;
        };
        let mut remote_paths = vec![
            ("edge.tls_certificate", edge.tls_certificate.as_path()),
            ("edge.tls_private_key", edge.tls_private_key.as_path()),
        ];
        remote_paths.extend(
            edge.explorer_root
                .as_deref()
                .map(|root| ("edge.explorer_root", root)),
        );
        check_edge_common(&edge.host, &edge.domain, &remote_paths, issues);
        check_origins("edge.cors_origins", &edge.cors_origins, issues);
        if let Some(domain) = &edge.explorer_domain {
            issues.take("edge.explorer_domain", check_host(domain));
        }
        if edge.explorer_domain.is_some() != edge.explorer_root.is_some() {
            issues.push(
                "edge.explorer_root",
                "explorer_domain and explorer_root must be set together",
            );
        }
    }

    fn check_node_edge_fields(&self, index: usize, node: &Node, issues: &mut Issues) {
        let key = format!("node[{index}]");
        let edge = self.edge.as_ref();
        // Only a node with a `host` can pin one; a local node gets the `edge` issue.
        if edge.is_some() && node.host.is_some() && node.host_key.is_none() {
            issues.push(
                format!("{key}.host_key"),
                "required when [edge] is present (every host is pinned)",
            );
        }
        let per_node_domains = edge.is_some_and(|edge| edge.per_node_domains);
        match &node.domain {
            None if per_node_domains => {
                issues.push(
                    format!("{key}.domain"),
                    "required when edge.per_node_domains = true",
                );
            }
            Some(_) if !per_node_domains => {
                issues.push(
                    format!("{key}.domain"),
                    "only used with edge.per_node_domains = true",
                );
            }
            Some(domain) => {
                issues.take(format!("{key}.domain"), check_host(domain));
            }
            None => {}
        }
        let private = edge.is_some_and(|edge| edge.upstream == Upstream::Private);
        match &node.private_address {
            None if private => issues.push(
                format!("{key}.private_address"),
                "required when edge.upstream = \"private\" (Torii binds to it)",
            ),
            Some(_) if !private => issues.push(
                format!("{key}.private_address"),
                "only used with edge.upstream = \"private\"",
            ),
            Some(address) if address.parse::<IpAddr>().is_err() => {
                issues.push(format!("{key}.private_address"), "must be an IP address");
            }
            _ => {}
        }
    }

    fn check_local_only(&self, placement: Option<Placement>, issues: &mut Issues) {
        if placement == Some(Placement::Remote) {
            if self.local.is_some() {
                issues.push(
                    "local",
                    "[local] is only for local definitions (no node has `host`)",
                );
            }
            if self.scaling.is_some() {
                issues.push(
                    "scaling",
                    "[scaling] is only for local definitions (no node has `host`)",
                );
            }
        }
        if let Some(local) = &self.local {
            match local.bind_host.parse::<IpAddr>() {
                Err(_) => issues.push("local.bind_host", "must be an IP address"),
                Ok(ip) if ip.is_unspecified() && local.public_host.is_none() => issues.push(
                    "local.public_host",
                    "required when bind_host is an unspecified address",
                ),
                Ok(_) => {}
            }
            if let Some(host) = &local.public_host {
                issues.take("local.public_host", check_host(host));
            }
        }
        if self.scaling.is_some_and(|scaling| scaling.lanes == 0) {
            issues.push("scaling.lanes", "must be at least 1");
        }
    }

    fn check_credentials(&self, issues: &mut Issues) {
        for (index, credential) in self.onboarding_credentials.iter().enumerate() {
            issues.take(
                format!("onboarding.credential[{index}].id"),
                check_credential_id(&credential.id),
            );
        }
        check_unique(
            "onboarding.credential",
            "credential id",
            self.onboarding_credentials
                .iter()
                .map(|credential| &credential.id),
            issues,
        );
    }

    /// The effective chain discriminant.
    pub fn chain_discriminant(&self) -> u16 {
        self.network
            .chain_discriminant
            .unwrap_or_else(|| self.network.profile.default_chain_discriminant())
    }

    /// Whether the local driver runs this network (no node has a `host`).
    pub fn is_local(&self) -> bool {
        placement(self.nodes.iter().map(|node| node.host.is_some())) == Some(Placement::Local)
    }

    /// Validator-role nodes, in file order.
    pub fn validators(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .iter()
            .filter(|node| node.role == Role::Validator)
    }

    /// Observer-role nodes, in file order.
    pub fn observers(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|node| node.role == Role::Observer)
    }

    /// `f` for the `3f+1` validators.
    pub fn f(&self) -> usize {
        fault_tolerance(self.validators().count())
    }

    /// The effective `[local]` settings (defaults when the table is absent).
    pub fn local_settings(&self) -> Local {
        self.local.clone().unwrap_or_default()
    }

    /// Effective ports of `nodes[index]`: explicit ports, else 1337/8080 on
    /// remote hosts or `[local]` base + index locally. `None` when out of range
    /// or when a local port would exceed 65535.
    pub fn node_ports(&self, index: usize) -> Option<NodePorts> {
        let node = self.nodes.get(index)?;
        let local = self.is_local().then(|| self.local_settings());
        let default_port = |remote: u16, base: fn(&Local) -> u16| match &local {
            None => Some(remote),
            Some(local) => base(local).checked_add(u16::try_from(index).ok()?),
        };
        Some(NodePorts {
            p2p: match node.p2p_port {
                Some(port) => port,
                None => default_port(DEFAULT_P2P_PORT, |local| local.base_p2p_port)?,
            },
            torii: match node.torii_port {
                Some(port) => port,
                None => default_port(DEFAULT_TORII_PORT, |local| local.base_torii_port)?,
            },
        })
    }

    /// The advertised P2P host of `nodes[index]`: `address`, else `host`, else
    /// `[local] public_host`.
    pub fn advertised_address(&self, index: usize) -> Option<String> {
        let node = self.nodes.get(index)?;
        Some(
            node.address
                .clone()
                .or_else(|| node.host.clone())
                .unwrap_or_else(|| self.local_settings().public_host().to_owned()),
        )
    }

    /// The host identity of `nodes[index]`; `None` for local nodes.
    pub fn host_identity(&self, index: usize) -> Option<HostIdentity> {
        let node = self.nodes.get(index)?;
        self.host_pins()
            .identity(node.host.as_deref(), node.host_key.as_ref())
    }

    /// The failure domain of `nodes[index]`; `None` for local nodes.
    pub fn failure_domain(&self, index: usize) -> Option<FailureDomain> {
        let node = self.nodes.get(index)?;
        failure_domain(node.failure_domain.as_deref(), self.host_identity(index))
    }

    fn host_pins(&self) -> HostPins {
        HostPins::collect(
            self.nodes
                .iter()
                .map(|node| (node.host.as_deref(), node.host_key.as_ref()))
                .chain(
                    self.edge
                        .iter()
                        .map(|edge| (Some(edge.host.as_str()), Some(&edge.host_key))),
                ),
        )
    }
}

/// Parse `[[grant]]` accounts as canonical I105 literals for `discriminant`,
/// and report a repeated grant under its own index.
fn parse_grants(entries: Vec<GrantEntry>, discriminant: u16, issues: &mut Issues) -> Vec<Grant> {
    // Accounts display under the same discriminant, as the operator wrote them.
    let _scope = ChainDiscriminantGuard::enter(discriminant);
    let mut grants: Vec<(usize, Grant)> = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let account = AccountId::parse_encoded(&entry.account).map_err(|error| {
            ValueError::new(format!(
                "`{}` is not a canonical account id for chain discriminant {discriminant}: {error}",
                entry.account
            ))
        });
        let Some(account) = issues.take(format!("grant[{index}].account"), account) else {
            continue;
        };
        let grant = Grant {
            account,
            permission: entry.permission,
        };
        if let Some((first, _)) = grants.iter().find(|(_, seen)| *seen == grant) {
            issues.push(
                format!("grant[{index}]"),
                format!(
                    "duplicate of grant[{first}] (`{} {}`)",
                    grant.account, grant.permission
                ),
            );
        } else {
            grants.push((index, grant));
        }
    }
    grants.into_iter().map(|(_, grant)| grant).collect()
}

/// Credential ids name token files: 1–64 of `[A-Za-z0-9._-]`, not starting with `.`.
fn check_credential_id(id: &str) -> Result<(), ValueError> {
    let valid = (1..=64).contains(&id.len())
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(ValueError::new(format!(
            "`{id}` must be 1-64 of [A-Za-z0-9._-] and not start with `.`"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = r#"
[network]
name = "dev"
profile = "sora-nexus-v1-qual"
[[node]]
name = "n0"
[[node]]
name = "n1"
[[node]]
name = "n2"
[[node]]
name = "n3"
"#;

    fn parse(text: &str) -> Result<NetworkDefinition, DefinitionError> {
        NetworkDefinition::parse(
            text,
            "/defs/networks/test.toml",
            Some(Path::new("/home/op")),
        )
    }

    #[test]
    fn local_definition_facts() {
        let definition = parse(DEV).unwrap();
        assert_eq!(definition.file, PathBuf::from("/defs/networks/test.toml"));
        assert!(definition.is_local());
        assert_eq!(definition.f(), 1);
        assert_eq!(definition.validators().count(), 4);
        assert_eq!(definition.observers().count(), 0);
        assert_eq!(definition.chain_discriminant(), 369);
        assert_eq!(
            definition.node_ports(3),
            Some(NodePorts {
                p2p: 29340,
                torii: 29083
            })
        );
        assert_eq!(definition.node_ports(4), None);
        assert_eq!(
            definition.advertised_address(0).as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(definition.host_identity(0), None);
        assert_eq!(definition.failure_domain(0), None);
        assert_eq!(definition.retention.journal_max.get(), 2 << 30);
        assert_eq!(definition.monitor.interval.to_string(), "5m");
        assert_eq!(definition.local_settings(), Local::default());
    }

    #[test]
    fn grants_parse_under_the_definition_discriminant() {
        let key: PublicKey =
            "ed01207233BFC89DCBD68C19FDE6CE6158225298EC1131B6A130D1AEB454C1AB5183C0"
                .parse()
                .unwrap();
        let account = {
            let _scope = ChainDiscriminantGuard::enter(369);
            AccountId::new(key).to_string()
        };
        let entry = |account: &str| GrantEntry {
            account: account.to_owned(),
            permission: Permission::CanRegisterDataspace,
        };
        let mut issues = Issues::default();
        let grants = parse_grants(vec![entry(&account), entry("nobody")], 369, &mut issues);
        assert_eq!(grants.len(), 1);
        assert_eq!(issues.0.len(), 1);
        assert_eq!(issues.0[0].key, "grant[1].account");
        let mut issues = Issues::default();
        assert!(parse_grants(vec![entry(&account)], 753, &mut issues).is_empty());

        // A repeat is reported at its own file index, in the operator's literal.
        let mut issues = Issues::default();
        let grants = parse_grants(
            vec![entry("nobody"), entry(&account), entry(&account)],
            369,
            &mut issues,
        );
        assert_eq!(grants.len(), 1);
        let keys: Vec<_> = issues.0.iter().map(|issue| issue.key.as_str()).collect();
        assert_eq!(keys, ["grant[0].account", "grant[2]"]);
        assert_eq!(
            issues.0[1].message,
            format!("duplicate of grant[1] (`{account} CanRegisterDataspace`)")
        );
    }

    #[test]
    fn credential_ids() {
        assert!(check_credential_id("inori-app").is_ok());
        assert!(check_credential_id("A.b_c-1").is_ok());
        for bad in ["", ".hidden", "a/b", &"x".repeat(65)] {
            assert!(check_credential_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn local_ports_overflow_is_reported() {
        let text = format!("{DEV}\n[local]\nbase_p2p_port = 65534\n");
        let error = parse(&text).unwrap_err();
        let keys: Vec<_> = error
            .issues()
            .iter()
            .map(|issue| issue.key.as_str())
            .collect();
        assert!(keys.contains(&"node[2]"), "{keys:?}");
    }
}

//! The dataspace definition, `dataspaces/<name>.toml` (spec §3.3).
//!
//! Inrou is not accepted here in the first release: Soracloud placement targets
//! global validators only, so an `[inrou]` table is an unknown key.

use std::path::{Path, PathBuf};

use iroha_config_base::{
    ReadConfig,
    read::{ConfigReader, FinalWrap, ReadConfig as ReadConfigTrait},
};
use iroha_crypto::PublicKey;
use iroha_data_model::NetworkId;
use iroha_model_base::name::Name;
use iroha_primitives::numeric::Quantity;

use super::{
    CommitteeSource, DataspaceName, DefinitionError, Entries, HostIdentity, HostKey, HostPins,
    Issues, Monitor, NetworkRef, NodePorts, Origin, Placement, Slug, Ssh, Upstream, ValueError,
    Visibility, check_host, check_unique,
    hosts::{
        DEFAULT_P2P_PORT, DEFAULT_TORII_PORT, FailureDomain, Member, check_edge_common,
        check_members, check_ssh, check_ssh_presence, failure_domain, fault_tolerance, placement,
    },
    read_optional, read_source, source_from_file, source_from_str,
};
use crate::verify::finality::MAX_COMMITTEE_MEMBERS;

/// `[dataspace]`: what to register and under which limits.
#[derive(Debug, Clone, PartialEq, Eq, ReadConfig)]
pub struct DataspaceSection {
    /// The SNS dataspace name; it determines the `DataSpaceId`.
    pub name: DataspaceName,
    /// The parent network: its `https` root, a network definition or a card anchor.
    pub network: NetworkRef,
    /// An optional non-interactive pin of the parent's network id.
    pub network_id: Option<NetworkId>,
    /// Who may read the dataspace.
    #[config(default)]
    pub visibility: Visibility,
    /// The owner's 0600 Ed25519 key; the owner account is derived from it.
    pub owner_key: PathBuf,
    /// An optional account alias, `<alias>@<name>`.
    pub account_alias: Option<Name>,
    /// Lease length in years, 1 to 10.
    #[config(default = "1")]
    pub lease_years: u8,
    /// Hard cap, in the parent's fee asset, across all registration writes.
    pub max_fee: Quantity,
    /// Operator keys for the owner's own nodes.
    #[config(default)]
    pub operators: Vec<PublicKey>,
}

/// One `[[committee.node]]`: an owner-run lane validator.
#[derive(Debug, Clone, PartialEq, Eq, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct CommitteeNode {
    /// Unique slug.
    pub name: Slug,
    /// SSH host; absent means a local rehearsal under the parent's supervisor.
    pub host: Option<String>,
    /// The host's pinned key.
    pub host_key: Option<HostKey>,
    /// Advertised P2P host; defaults to `host`.
    pub address: Option<String>,
    /// P2P port; 1337 on remote hosts.
    pub p2p_port: Option<u16>,
    /// Torii port; 8080 on remote hosts.
    pub torii_port: Option<u16>,
    /// Failure domain label; defaults to the host identity.
    pub failure_domain: Option<String>,
}

/// `[committee]`: who signs the dataspace lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committee {
    /// Where the committee comes from.
    pub source: CommitteeSource,
    /// Owner-run nodes; non-empty iff `source = "owner"`.
    pub nodes: Vec<CommitteeNode>,
}

#[derive(ReadConfig)]
struct RawCommittee {
    source: CommitteeSource,
    #[config(default)]
    node: Entries<CommitteeNode>,
}

/// `[edge]` of an owner committee: the lane's public Torii (manifest `torii_url`).
#[derive(Debug, Clone, PartialEq, Eq, ReadConfig)]
pub struct DataspaceEdge {
    /// The edge SSH host.
    pub host: String,
    /// The edge host's pinned key.
    pub host_key: HostKey,
    /// Members are served at `https://<node>.<domain>`.
    pub domain: String,
    /// The certificate path on the edge host.
    pub tls_certificate: PathBuf,
    /// The private key path on the edge host.
    pub tls_private_key: PathBuf,
    /// How the edge reaches member Torii endpoints; only `"mtls"` until P6.
    #[config(default)]
    pub upstream: Upstream,
}

/// A parsed and validated dataspace definition.
#[derive(Debug, Clone)]
pub struct DataspaceDefinition {
    /// The absolute path of the definition file.
    pub file: PathBuf,
    /// `[dataspace]`.
    pub dataspace: DataspaceSection,
    /// `[committee]` and `[[committee.node]]`.
    pub committee: Committee,
    /// `[ssh]`; present iff owner nodes are remote.
    pub ssh: Option<Ssh>,
    /// `[edge]`; owner committees only.
    pub edge: Option<DataspaceEdge>,
    /// `[monitor]`.
    pub monitor: Monitor,
}

struct RawDataspace {
    dataspace: DataspaceSection,
    committee: RawCommittee,
    ssh: Option<Ssh>,
    edge: Option<DataspaceEdge>,
    monitor: Monitor,
}

impl ReadConfigTrait for RawDataspace {
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self> {
        let dataspace = reader.read_nested("dataspace");
        let committee = reader.read_nested("committee");
        let ssh = read_optional(reader, "ssh");
        let edge = read_optional(reader, "edge");
        let monitor = reader.read_nested("monitor");
        FinalWrap::value_fn(move || Self {
            dataspace: dataspace.unwrap(),
            committee: committee.unwrap(),
            ssh: ssh.unwrap(),
            edge: edge.unwrap(),
            monitor: monitor.unwrap(),
        })
    }
}

impl DataspaceDefinition {
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

    fn build(raw: RawDataspace, origin: &Origin) -> Result<Self, DefinitionError> {
        let RawDataspace {
            mut dataspace,
            committee,
            mut ssh,
            edge,
            mut monitor,
        } = raw;
        let mut issues = Issues::default();
        origin.resolve("dataspace.owner_key", &mut dataspace.owner_key, &mut issues);
        if let NetworkRef::Definition(path) | NetworkRef::CardAnchor(path) = &mut dataspace.network
        {
            origin.resolve("dataspace.network", path, &mut issues);
        }
        if let Some(ssh) = ssh.as_mut() {
            origin.resolve("ssh.identity", &mut ssh.identity, &mut issues);
        }
        if let Some(path) = monitor.webhook_file.as_mut() {
            origin.resolve("monitor.webhook_file", path, &mut issues);
        }
        let definition = Self {
            file: origin.file.clone(),
            dataspace,
            committee: Committee {
                source: committee.source,
                nodes: committee.node.0,
            },
            ssh,
            edge,
            monitor,
        };
        definition.validate(&mut issues);
        issues.finish(&origin.file, definition)
    }

    fn validate(&self, issues: &mut Issues) {
        if !(1..=10).contains(&self.dataspace.lease_years) {
            issues.push("dataspace.lease_years", "must be between 1 and 10");
        }
        check_unique(
            "dataspace.operators",
            "key",
            &self.dataspace.operators,
            issues,
        );
        match self.committee.source {
            CommitteeSource::Network => self.check_network_committee(issues),
            CommitteeSource::Owner => self.check_owner_committee(issues),
        }
        if let Some(ssh) = &self.ssh {
            check_ssh(ssh, self.edge.is_some(), issues);
        }
    }

    fn check_network_committee(&self, issues: &mut Issues) {
        const OWNER_ONLY: &str = "only allowed when committee.source = \"owner\"";
        if !self.committee.nodes.is_empty() {
            issues.push("committee.node", OWNER_ONLY);
        }
        if self.ssh.is_some() {
            issues.push("ssh", OWNER_ONLY);
        }
        if self.edge.is_some() {
            issues.push("edge", OWNER_ONLY);
        }
    }

    fn check_owner_committee(&self, issues: &mut Issues) {
        let members: Vec<_> = self
            .committee
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| Member {
                name: &node.name,
                validator: true,
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
            "committee.node",
            &members,
            MAX_COMMITTEE_MEMBERS,
            edge_pin.as_slice(),
            issues,
        );
        check_ssh_presence(placement, self.ssh.as_ref(), issues);
        if placement == Some(Placement::Local)
            && !self.committee.nodes.is_empty()
            && !matches!(self.dataspace.network, NetworkRef::Definition(_))
        {
            // TODO(P6): check at plan time that the parent definition is local too.
            issues.push(
                "dataspace.network",
                "a local rehearsal (no committee node has `host`) runs under a local network \
                 definition's supervisor; point `network` at that definition",
            );
        }
        let Some(edge) = &self.edge else {
            return;
        };
        if placement == Some(Placement::Local) {
            // Pinning local nodes would only lead to "host_key requires host".
            issues.push("edge", "[edge] needs remote committee nodes");
            return;
        }
        for (index, node) in self.committee.nodes.iter().enumerate() {
            let key = format!("committee.node[{index}]");
            if node.host.is_some() && node.host_key.is_none() {
                issues.push(
                    format!("{key}.host_key"),
                    "required when [edge] is present (every host is pinned)",
                );
            }
            // Members are served at `https://<node>.<domain>`.
            let served = check_host(&format!("{}.{}", node.name, edge.domain)).map_err(|_| {
                ValueError::new(format!(
                    "`{}` must be a DNS label to serve https://{}.{}",
                    node.name, node.name, edge.domain
                ))
            });
            issues.take(format!("{key}.name"), served);
        }
        if edge.upstream == Upstream::Private {
            // TODO(P6): committee nodes have no `private_address` (spec §3.3); settle
            // the private upstream for owner committees with the dataspace renderer.
            issues.push(
                "edge.upstream",
                "\"private\" is not supported for owner committees yet; use \"mtls\"",
            );
        }
        check_edge_common(
            &edge.host,
            &edge.domain,
            &[
                ("edge.tls_certificate", edge.tls_certificate.as_path()),
                ("edge.tls_private_key", edge.tls_private_key.as_path()),
            ],
            issues,
        );
    }

    /// Whether the owner committee is a local rehearsal (owner source, no `host`).
    pub fn is_local(&self) -> bool {
        self.committee.source == CommitteeSource::Owner
            && placement(self.committee.nodes.iter().map(|node| node.host.is_some()))
                == Some(Placement::Local)
    }

    /// `f` of the owner committee; `None` when the committee is the parent's validators.
    pub fn f(&self) -> Option<usize> {
        (self.committee.source == CommitteeSource::Owner)
            .then(|| fault_tolerance(self.committee.nodes.len()))
    }

    /// Effective ports of `committee.nodes[index]`: explicit ports, else
    /// 1337/8080 on remote hosts. `None` for a local rehearsal node without
    /// explicit ports.
    // TODO: P6 allocates rehearsal ports after the parent's local nodes.
    pub fn node_ports(&self, index: usize) -> Option<NodePorts> {
        let node = self.committee.nodes.get(index)?;
        let remote = node.host.is_some();
        let pick =
            |explicit: Option<u16>, default: u16| explicit.or_else(|| remote.then_some(default));
        Some(NodePorts {
            p2p: pick(node.p2p_port, DEFAULT_P2P_PORT)?,
            torii: pick(node.torii_port, DEFAULT_TORII_PORT)?,
        })
    }

    /// The host identity of `committee.nodes[index]`; `None` for local nodes.
    pub fn host_identity(&self, index: usize) -> Option<HostIdentity> {
        let node = self.committee.nodes.get(index)?;
        let pins = HostPins::collect(
            self.committee
                .nodes
                .iter()
                .map(|node| (node.host.as_deref(), node.host_key.as_ref()))
                .chain(
                    self.edge
                        .iter()
                        .map(|edge| (Some(edge.host.as_str()), Some(&edge.host_key))),
                ),
        );
        pins.identity(node.host.as_deref(), node.host_key.as_ref())
    }

    /// The failure domain of `committee.nodes[index]`; `None` for local nodes.
    pub fn failure_domain(&self, index: usize) -> Option<FailureDomain> {
        let node = self.committee.nodes.get(index)?;
        failure_domain(node.failure_domain.as_deref(), self.host_identity(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL_OWNER: &str = r#"
[dataspace]
name = "acme"
network = "../networks/dev.toml"
owner_key = "keys/acme-owner.key"
max_fee = "50"

[committee]
source = "owner"
[[committee.node]]
name = "acme-1"
[[committee.node]]
name = "acme-2"
[[committee.node]]
name = "acme-3"
[[committee.node]]
name = "acme-4"
p2p_port = 30001
torii_port = 30002
"#;

    #[test]
    fn local_owner_rehearsal_facts() {
        let definition = DataspaceDefinition::parse(
            LOCAL_OWNER,
            "/defs/dataspaces/acme.toml",
            Some(Path::new("/home/op")),
        )
        .unwrap();
        assert!(definition.is_local());
        assert_eq!(definition.f(), Some(1));
        assert_eq!(
            definition.dataspace.network,
            NetworkRef::Definition(PathBuf::from("/defs/dataspaces/../networks/dev.toml"))
        );
        assert_eq!(
            definition.dataspace.owner_key,
            PathBuf::from("/defs/dataspaces/keys/acme-owner.key")
        );
        assert_eq!(definition.dataspace.visibility, Visibility::Restricted);
        assert_eq!(definition.dataspace.lease_years, 1);
        assert_eq!(definition.node_ports(0), None);
        assert_eq!(
            definition.node_ports(3),
            Some(NodePorts {
                p2p: 30001,
                torii: 30002
            })
        );
        assert_eq!(definition.host_identity(0), None);
        assert_eq!(definition.failure_domain(0), None);
    }

    #[test]
    fn network_committee_has_no_f_and_is_not_local() {
        let text = r#"
[dataspace]
name = "acme"
network = "https://taira.sora.org"
owner_key = "/k"
max_fee = "20"
[committee]
source = "network"
"#;
        let definition = DataspaceDefinition::parse(text, "/d/acme.toml", None).unwrap();
        assert_eq!(definition.f(), None);
        assert!(!definition.is_local());
        assert_eq!(definition.node_ports(0), None);
    }
}

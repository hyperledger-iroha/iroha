//! Host access and placement rules shared by network nodes and owner committee nodes.

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};

use iroha_config_base::{
    ReadConfig,
    read::{ConfigReader, FinalWrap, ReadConfig as ReadConfigTrait},
};

use super::{
    Become, HostKey, Interval, Issues, Slug, SshTarget, check_host, check_origin, check_unique,
    check_user,
};

/// Remote P2P port when a node sets none.
pub const DEFAULT_P2P_PORT: u16 = 1337;
/// Remote Torii port when a node sets none.
pub const DEFAULT_TORII_PORT: u16 = 8080;
/// SSH port when `[ssh]` sets none.
pub const DEFAULT_SSH_PORT: u16 = 22;

const DEFAULT_MONITOR_INTERVAL: Interval = Interval::new(Duration::from_secs(5 * 60));

/// `[ssh]`: how the controller reaches remote hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ssh {
    /// The deploy key (`.pub` or private); it selects the key, resolved locally.
    pub identity: PathBuf,
    /// Use the SSH agent (default). `false` uses an unencrypted key file directly.
    pub agent: bool,
    /// The login user, `root` by default.
    pub user: String,
    /// How the user gains root; must be `sudo` when `user` is not `root`.
    pub r#become: Option<Become>,
    /// The SSH port.
    pub port: u16,
    /// An optional jump host.
    pub jump: Option<SshTarget>,
    /// The jump host's pinned key; required iff `jump` is set.
    pub jump_host_key: Option<HostKey>,
}

impl Ssh {
    /// The effective escalation method.
    pub fn escalation(&self) -> Become {
        self.r#become.unwrap_or(Become::None)
    }
}

// `become` is a Rust keyword, so this section cannot use the derive.
impl ReadConfigTrait for Ssh {
    fn read(reader: &mut ConfigReader) -> FinalWrap<Self> {
        let identity = reader
            .read_parameter(["identity"])
            .value_required()
            .finish();
        let agent = reader
            .read_parameter(["agent"])
            .value_or_else(|| true)
            .finish();
        let user = reader
            .read_parameter(["user"])
            .value_or_else(|| "root".to_owned())
            .finish();
        let r#become = reader.read_parameter(["become"]).value_optional().finish();
        let port = reader
            .read_parameter(["port"])
            .value_or_else(|| DEFAULT_SSH_PORT)
            .finish();
        let jump = reader.read_parameter(["jump"]).value_optional().finish();
        let jump_host_key = reader
            .read_parameter(["jump_host_key"])
            .value_optional()
            .finish();
        FinalWrap::value_fn(move || Self {
            identity: identity.unwrap(),
            agent: agent.unwrap(),
            user: user.unwrap(),
            r#become: r#become.unwrap(),
            port: port.unwrap(),
            jump: jump.unwrap(),
            jump_host_key: jump_host_key.unwrap(),
        })
    }
}

/// `[monitor]`: host watch timers.
#[derive(Debug, Clone, PartialEq, Eq, ReadConfig)]
pub struct Monitor {
    /// A 0600 file holding the alert webhook URL; no webhook when absent.
    pub webhook_file: Option<PathBuf>,
    /// How often the watch timers run.
    #[config(default = "DEFAULT_MONITOR_INTERVAL")]
    pub interval: Interval,
}

/// Effective ports of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodePorts {
    /// The P2P port.
    pub p2p: u16,
    /// The Torii port.
    pub torii: u16,
}

/// How nodes are run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// No node has a `host`: the local driver runs them.
    Local,
    /// Every node has a `host`: the SSH driver runs them.
    Remote,
}

/// A host as counted by the co-location rules. Aliases pinned to one host key
/// are one host.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostIdentity {
    /// A host pinned to this key (possibly under several names).
    Key(HostKey),
    /// An unpinned host name, lowercased.
    Name(String),
}

impl fmt::Display for HostIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => write!(f, "{key}"),
            Self::Name(name) => f.write_str(name),
        }
    }
}

/// A failure domain: an explicit `failure_domain` label, or the host itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureDomain {
    /// An operator-chosen label, for VMs sharing a physical host or disk.
    Named(String),
    /// The node's host identity (the default).
    Host(HostIdentity),
}

impl fmt::Display for FailureDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(name) => f.write_str(name),
            Self::Host(identity) => write!(f, "host {identity}"),
        }
    }
}

/// Host-name to host-key pins, from every pinned host in a definition.
#[derive(Debug, Default)]
pub struct HostPins(BTreeMap<String, HostKey>);

impl HostPins {
    /// Collect pins; the first key seen for a host name wins.
    pub fn collect<'a>(
        hosts: impl IntoIterator<Item = (Option<&'a str>, Option<&'a HostKey>)>,
    ) -> Self {
        let mut pins = BTreeMap::new();
        for (host, key) in hosts {
            if let (Some(host), Some(key)) = (host, key) {
                pins.entry(host.to_ascii_lowercase()).or_insert(*key);
            }
        }
        Self(pins)
    }

    /// The identity of a node's host: its key, a key pinned for the same name
    /// elsewhere in the definition, or the lowercased name. `None` for local nodes.
    pub fn identity(&self, host: Option<&str>, host_key: Option<&HostKey>) -> Option<HostIdentity> {
        if let Some(key) = host_key {
            return Some(HostIdentity::Key(*key));
        }
        let name = host?.to_ascii_lowercase();
        Some(
            self.0
                .get(&name)
                .map_or(HostIdentity::Name(name), |key| HostIdentity::Key(*key)),
        )
    }
}

/// A node as seen by the shared placement rules.
#[derive(Debug, Clone, Copy)]
pub(super) struct Member<'a> {
    pub name: &'a Slug,
    pub validator: bool,
    pub host: Option<&'a str>,
    pub host_key: Option<&'a HostKey>,
    pub address: Option<&'a str>,
    pub failure_domain: Option<&'a str>,
    pub ports: Option<NodePorts>,
}

/// The failure domain of a node given its identity.
pub(super) fn failure_domain(
    explicit: Option<&str>,
    identity: Option<HostIdentity>,
) -> Option<FailureDomain> {
    explicit.map_or_else(
        || identity.map(FailureDomain::Host),
        |name| Some(FailureDomain::Named(name.to_owned())),
    )
}

/// Placement of a node list, given whether each node has a `host`; `None`
/// when it mixes local and remote nodes.
pub(super) fn placement(has_host: impl IntoIterator<Item = bool>) -> Option<Placement> {
    let (mut local, mut remote) = (false, false);
    for hosted in has_host {
        if hosted {
            remote = true;
        } else {
            local = true;
        }
    }
    match (local, remote) {
        (true, true) => None,
        (false, true) => Some(Placement::Remote),
        _ => Some(Placement::Local),
    }
}

/// `f` for `n = 3f + 1` validators.
pub(super) const fn fault_tolerance(validators: usize) -> usize {
    validators.saturating_sub(1) / 3
}

/// Whether `validators` is an exact `3f + 1` roster of 4 to `max` members.
pub(super) const fn is_exact_roster(validators: usize, max: usize) -> bool {
    validators >= 4 && validators <= max && (validators - 1).is_multiple_of(3)
}

/// Apply the shared rules to the node list at `list_key`: unique names, exactly
/// `3f+1` validators from 4 to `max_validators`, uniform placement, host
/// syntax, consistent pins, at most `f` validators per host identity or
/// failure domain (remote only) and distinct ports per host. `extra_pins` are
/// other pinned hosts (the edge).
pub(super) fn check_members(
    list_key: &str,
    members: &[Member<'_>],
    max_validators: usize,
    extra_pins: &[(&str, &str, &HostKey)],
    issues: &mut Issues,
) -> Option<Placement> {
    check_unique(
        list_key,
        "node name",
        members.iter().map(|member| member.name),
        issues,
    );
    let validators = members.iter().filter(|member| member.validator).count();
    let valid_count = is_exact_roster(validators, max_validators);
    if !valid_count {
        issues.push(
            list_key,
            format!(
                "expected exactly 3f+1 validators, from 4 to {max_validators}, found {validators}"
            ),
        );
    }
    for (index, member) in members.iter().enumerate() {
        check_member_syntax(&format!("{list_key}[{index}]"), member, issues);
    }
    check_pin_consistency(list_key, members, extra_pins, issues);
    let placement = placement(members.iter().map(|member| member.host.is_some()));
    if placement.is_none() {
        issues.push(
            list_key,
            "mixes local nodes (no `host`) with remote nodes; set `host` on every node or on none",
        );
    }
    let pins = HostPins::collect(
        members
            .iter()
            .map(|member| (member.host, member.host_key))
            .chain(
                extra_pins
                    .iter()
                    .map(|(_, host, key)| (Some(*host), Some(*key))),
            ),
    );
    if placement == Some(Placement::Remote) && valid_count {
        check_colocation(
            list_key,
            members,
            &pins,
            fault_tolerance(validators),
            issues,
        );
    }
    if placement.is_some() {
        check_ports(list_key, members, &pins, issues);
    }
    placement
}

fn check_member_syntax(key: &str, member: &Member<'_>, issues: &mut Issues) {
    if let Some(host) = member.host {
        issues.take(format!("{key}.host"), check_host(host));
    } else {
        if member.host_key.is_some() {
            issues.push(format!("{key}.host_key"), "requires `host`");
        }
        if member.failure_domain.is_some() {
            issues.push(format!("{key}.failure_domain"), "requires `host`");
        }
    }
    if let Some(address) = member.address {
        issues.take(format!("{key}.address"), check_host(address));
    }
    if member
        .failure_domain
        .is_some_and(|domain| domain.trim().is_empty())
    {
        issues.push(format!("{key}.failure_domain"), "must not be empty");
    }
}

/// One host name must not be pinned to two different keys.
fn check_pin_consistency(
    list_key: &str,
    members: &[Member<'_>],
    extra_pins: &[(&str, &str, &HostKey)],
    issues: &mut Issues,
) {
    let mut seen: BTreeMap<String, (String, HostKey)> = BTreeMap::new();
    let member_pins = members.iter().enumerate().filter_map(|(index, member)| {
        Some((
            format!("{list_key}[{index}]"),
            member.host?,
            member.host_key?,
        ))
    });
    let extra = extra_pins
        .iter()
        .map(|(key, host, host_key)| ((*key).to_owned(), *host, *host_key));
    for (key, host, host_key) in member_pins.chain(extra) {
        let name = host.to_ascii_lowercase();
        match seen.get(&name) {
            Some((first, pinned)) if pinned != host_key => issues.push(
                format!("{key}.host_key"),
                format!("host `{name}` is pinned to a different key in `{first}`"),
            ),
            Some(_) => {}
            None => {
                seen.insert(name, (key, *host_key));
            }
        }
    }
}

/// At most `f` validators per host identity and per failure domain.
fn check_colocation(
    list_key: &str,
    members: &[Member<'_>],
    pins: &HostPins,
    f: usize,
    issues: &mut Issues,
) {
    let mut hosts: BTreeMap<HostIdentity, usize> = BTreeMap::new();
    let mut domains: BTreeMap<FailureDomain, usize> = BTreeMap::new();
    for member in members.iter().filter(|member| member.validator) {
        let identity = pins.identity(member.host, member.host_key);
        if let Some(identity) = identity.clone() {
            *hosts.entry(identity).or_default() += 1;
        }
        if let Some(domain) = failure_domain(member.failure_domain, identity) {
            *domains.entry(domain).or_default() += 1;
        }
    }
    for (identity, count) in hosts.into_iter().filter(|(_, count)| *count > f) {
        issues.push(
            list_key,
            format!("{count} validators share host {identity}; at most f = {f} may"),
        );
    }
    for (domain, count) in domains.into_iter().filter(|(_, count)| *count > f) {
        if let FailureDomain::Named(name) = domain {
            issues.push(
                list_key,
                format!("{count} validators share failure_domain `{name}`; at most f = {f} may"),
            );
        }
    }
}

/// Nodes on one host (all nodes, for local definitions) need distinct, non-zero ports.
fn check_ports(list_key: &str, members: &[Member<'_>], pins: &HostPins, issues: &mut Issues) {
    let mut used: BTreeMap<(Option<HostIdentity>, u16), String> = BTreeMap::new();
    for (index, member) in members.iter().enumerate() {
        let Some(ports) = member.ports else {
            continue;
        };
        let identity = pins.identity(member.host, member.host_key);
        for (field, port) in [("p2p_port", ports.p2p), ("torii_port", ports.torii)] {
            let key = format!("{list_key}[{index}].{field}");
            if port == 0 {
                issues.push(key, "must not be 0");
                continue;
            }
            match used.get(&(identity.clone(), port)) {
                Some(other) => issues.push(key, format!("port {port} is also used by `{other}`")),
                None => {
                    used.insert((identity.clone(), port), key);
                }
            }
        }
    }
}

/// Rules for an `[ssh]` section. `has_edge` refuses `agent = false`.
pub(super) fn check_ssh(ssh: &Ssh, has_edge: bool, issues: &mut Issues) {
    issues.take("ssh.user", check_user(&ssh.user));
    if ssh.user != "root" && ssh.escalation() == Become::None {
        issues.push(
            "ssh.become",
            "must be \"sudo\" when ssh.user is not \"root\"",
        );
    }
    if ssh.jump.is_some() != ssh.jump_host_key.is_some() {
        issues.push("ssh.jump_host_key", "required iff ssh.jump is set");
    }
    if !ssh.agent && has_edge {
        issues.push(
            "ssh.agent",
            "agent = false (an unencrypted key file) is refused for definitions with [edge]",
        );
    }
    if ssh.port == 0 {
        issues.push("ssh.port", "must not be 0");
    }
}

/// `[ssh]` is required iff nodes are remote.
pub(super) fn check_ssh_presence(
    placement: Option<Placement>,
    ssh: Option<&Ssh>,
    issues: &mut Issues,
) {
    match (placement, ssh) {
        (Some(Placement::Remote), None) => {
            issues.push("ssh", "[ssh] is required when nodes have `host`");
        }
        (Some(Placement::Local), Some(_)) => {
            issues.push("ssh", "[ssh] is only allowed when nodes have `host`");
        }
        _ => {}
    }
}

/// Rules shared by the network and dataspace `[edge]` sections.
pub(super) fn check_edge_common(
    host: &str,
    domain: &str,
    remote_paths: &[(&str, &Path)],
    issues: &mut Issues,
) {
    issues.take("edge.host", check_host(host));
    issues.take("edge.domain", check_host(domain));
    for (key, path) in remote_paths {
        if !path.is_absolute() {
            issues.push(*key, "must be an absolute path on the edge host");
        }
    }
}

/// Every `origin` must be an exact, unique web origin.
pub(super) fn check_origins(key: &str, origins: &[String], issues: &mut Issues) {
    for (index, origin) in origins.iter().enumerate() {
        issues.take(format!("{key}[{index}]"), check_origin(origin));
    }
    check_unique(key, "origin", origins, issues);
}

#[cfg(test)]
mod tests {
    use iroha_data_model::block::consensus_v2::{
        MAX_VALIDATORS_PER_HEIGHT, is_valid_committee_size,
    };

    use super::*;

    fn key(byte: u8) -> HostKey {
        HostKey::from_bytes([byte; 32])
    }

    fn slug(name: &str) -> Slug {
        name.parse().unwrap()
    }

    fn member<'a>(
        name: &'a Slug,
        host: Option<&'a str>,
        host_key: Option<&'a HostKey>,
    ) -> Member<'a> {
        Member {
            name,
            validator: true,
            host,
            host_key,
            address: None,
            failure_domain: None,
            ports: Some(NodePorts {
                p2p: DEFAULT_P2P_PORT,
                torii: DEFAULT_TORII_PORT,
            }),
        }
    }

    fn keys_of(issues: &Issues) -> Vec<&str> {
        issues.0.iter().map(|issue| issue.key.as_str()).collect()
    }

    #[test]
    fn pins_make_aliases_one_identity() {
        let pinned = key(1);
        let pins = HostPins::collect([
            (Some("V1.example"), Some(&pinned)),
            (Some("v2.example"), None),
        ]);
        assert_eq!(
            pins.identity(Some("v1.example"), None),
            Some(HostIdentity::Key(pinned))
        );
        assert_eq!(
            pins.identity(Some("V2.Example"), None),
            Some(HostIdentity::Name("v2.example".to_owned()))
        );
        assert_eq!(pins.identity(None, None), None);
        assert_eq!(HostIdentity::Name("h".to_owned()).to_string(), "h");
    }

    #[test]
    fn failure_domains_default_to_the_host() {
        let identity = HostIdentity::Name("h".to_owned());
        assert_eq!(
            failure_domain(None, Some(identity.clone())),
            Some(FailureDomain::Host(identity.clone()))
        );
        assert_eq!(
            failure_domain(Some("rack"), Some(identity)),
            Some(FailureDomain::Named("rack".to_owned()))
        );
        assert_eq!(failure_domain(None, None), None);
        assert_eq!(FailureDomain::Named("rack".to_owned()).to_string(), "rack");
    }

    #[test]
    fn placement_and_fault_tolerance() {
        assert_eq!(placement([false, false]), Some(Placement::Local));
        assert_eq!(placement([true, true]), Some(Placement::Remote));
        assert_eq!(placement([true, false]), None);
        assert_eq!(placement(std::iter::empty()), Some(Placement::Local));
        assert_eq!(fault_tolerance(4), 1);
        assert_eq!(fault_tolerance(7), 2);
        assert_eq!(fault_tolerance(0), 0);
    }

    #[test]
    fn exact_rosters_are_bounded() {
        for validators in 0..=40 {
            assert_eq!(
                is_exact_roster(validators, MAX_VALIDATORS_PER_HEIGHT),
                is_valid_committee_size(validators),
                "{validators}"
            );
        }
        assert!(is_exact_roster(127, 128));
        assert!(!is_exact_roster(130, 128));
        assert!(!is_exact_roster(3, 128));
    }

    #[test]
    fn members_on_distinct_hosts_pass() {
        let names: Vec<_> = (0..4).map(|i| slug(&format!("v{i}"))).collect();
        let keys: Vec<_> = (0..4).map(key).collect();
        let hosts = ["a.example", "b.example", "c.example", "d.example"];
        let members: Vec<_> = (0..4)
            .map(|i| member(&names[i], Some(hosts[i]), Some(&keys[i])))
            .collect();
        let mut issues = Issues::default();
        assert_eq!(
            check_members("node", &members, 4, &[], &mut issues),
            Some(Placement::Remote)
        );
        assert!(issues.0.is_empty(), "{:?}", issues.0);
        check_members("node", &members, 1, &[], &mut issues);
        assert_eq!(keys_of(&issues), ["node"]);
        assert!(issues.0[0].message.contains("from 4 to 1"));
    }

    #[test]
    fn members_report_count_colocation_ports_and_pins() {
        let names: Vec<_> = (0..4).map(|i| slug(&format!("v{i}"))).collect();
        let shared = key(9);
        let members: Vec<_> = names
            .iter()
            .map(|name| member(name, Some("h.example"), Some(&shared)))
            .collect();
        let mut issues = Issues::default();
        check_members(
            "node",
            &members[..3],
            MAX_VALIDATORS_PER_HEIGHT,
            &[("edge", "h.example", &key(8))],
            &mut issues,
        );
        let keys = keys_of(&issues);
        assert!(keys.contains(&"node"), "{keys:?}");
        assert!(keys.contains(&"edge.host_key"), "{keys:?}");

        let mut issues = Issues::default();
        check_members(
            "node",
            &members,
            MAX_VALIDATORS_PER_HEIGHT,
            &[],
            &mut issues,
        );
        assert!(
            issues
                .0
                .iter()
                .any(|issue| issue.message.contains("share host"))
        );
        assert!(keys_of(&issues).contains(&"node[1].p2p_port"));
    }

    #[test]
    fn member_syntax_rules() {
        let name = slug("n0");
        let pinned = key(1);
        let mut local = member(&name, None, Some(&pinned));
        local.failure_domain = Some("rack");
        local.address = Some("bad host");
        let mut issues = Issues::default();
        check_member_syntax("node[0]", &local, &mut issues);
        assert_eq!(
            keys_of(&issues),
            [
                "node[0].host_key",
                "node[0].failure_domain",
                "node[0].address"
            ]
        );
    }

    #[test]
    fn ssh_rules() {
        let mut ssh = Ssh {
            identity: PathBuf::from("/k"),
            agent: false,
            user: "deploy".to_owned(),
            r#become: None,
            port: 0,
            jump: Some("ops@bastion".parse().unwrap()),
            jump_host_key: None,
        };
        assert_eq!(ssh.escalation(), Become::None);
        let mut issues = Issues::default();
        check_ssh(&ssh, true, &mut issues);
        assert_eq!(
            keys_of(&issues),
            ["ssh.become", "ssh.jump_host_key", "ssh.agent", "ssh.port"]
        );
        ssh.r#become = Some(Become::None);
        let mut issues = Issues::default();
        check_ssh(&ssh, false, &mut issues);
        assert_eq!(
            keys_of(&issues),
            ["ssh.become", "ssh.jump_host_key", "ssh.port"]
        );
        ssh.r#become = Some(Become::Sudo);
        assert_eq!(ssh.escalation(), Become::Sudo);
        let mut issues = Issues::default();
        check_ssh(&ssh, false, &mut issues);
        assert_eq!(keys_of(&issues), ["ssh.jump_host_key", "ssh.port"]);
        ssh.user = "root".to_owned();
        ssh.r#become = Some(Become::None);
        let mut issues = Issues::default();
        check_ssh(&ssh, false, &mut issues);
        assert_eq!(keys_of(&issues), ["ssh.jump_host_key", "ssh.port"]);
        let mut issues = Issues::default();
        check_ssh_presence(Some(Placement::Remote), None, &mut issues);
        check_ssh_presence(Some(Placement::Local), Some(&ssh), &mut issues);
        check_ssh_presence(None, None, &mut issues);
        assert_eq!(keys_of(&issues), ["ssh", "ssh"]);
    }

    #[test]
    fn edge_and_origin_rules() {
        let mut issues = Issues::default();
        check_edge_common(
            "edge.example",
            "bad domain",
            &[("edge.tls_certificate", Path::new("relative.pem"))],
            &mut issues,
        );
        check_origins(
            "edge.cors_origins",
            &[
                "https://a.example".to_owned(),
                "https://a.example".to_owned(),
                "x".to_owned(),
            ],
            &mut issues,
        );
        assert_eq!(
            keys_of(&issues),
            [
                "edge.domain",
                "edge.tls_certificate",
                "edge.cors_origins[2]",
                "edge.cors_origins"
            ]
        );
    }
}

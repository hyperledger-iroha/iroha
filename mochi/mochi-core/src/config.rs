//! Network configuration presets, topology metadata, and filesystem helpers.

use std::{
    fmt, fs, io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
};

use iroha_data_model::parameter::system::SumeragiConsensusMode;

const MIN_PEER_COUNT: usize = 1;
const MAX_PEER_COUNT: usize = 7;

/// High-level presets exposed directly to the user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilePreset {
    /// Single peer for rapid prototyping.
    SinglePeer,
    /// Four peers to exercise Sumeragi BFT locally.
    FourPeerBft,
}

impl ProfilePreset {
    /// Default peer count for the preset.
    pub const fn peer_count(self) -> usize {
        match self {
            ProfilePreset::SinglePeer => 1,
            ProfilePreset::FourPeerBft => 4,
        }
    }

    /// Short slug used in filesystem paths.
    pub const fn slug(self) -> &'static str {
        match self {
            ProfilePreset::SinglePeer => "single-peer",
            ProfilePreset::FourPeerBft => "four-peer-bft",
        }
    }

    /// Human-friendly label used in UI surfaces.
    pub const fn label(self) -> &'static str {
        match self {
            ProfilePreset::SinglePeer => "Single Peer",
            ProfilePreset::FourPeerBft => "Four Peer BFT",
        }
    }
}

/// Kagami genesis presets for Iroha 3 networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum GenesisProfile {
    /// Local-only Iroha 3 developer network.
    Iroha3Dev,
    /// Sora Testus public test network.
    Iroha3Testus,
    /// Sora Nexus main network.
    Iroha3Nexus,
}

impl GenesisProfile {
    /// CLI flag value expected by `kagami genesis --profile`.
    pub const fn as_kagami_arg(self) -> &'static str {
        match self {
            GenesisProfile::Iroha3Dev => "iroha3-dev",
            GenesisProfile::Iroha3Testus => "iroha3-testus",
            GenesisProfile::Iroha3Nexus => "iroha3-nexus",
        }
    }

    /// Default chain/collector settings for the profile.
    pub const fn defaults(self) -> GenesisProfileDefaults {
        match self {
            GenesisProfile::Iroha3Dev => GenesisProfileDefaults {
                chain_id: "iroha3-dev.local",
                collectors_k: 1,
                collectors_redundant_send_r: 1,
            },
            GenesisProfile::Iroha3Testus => GenesisProfileDefaults {
                chain_id: "iroha3-testus",
                collectors_k: 3,
                collectors_redundant_send_r: 3,
            },
            GenesisProfile::Iroha3Nexus => GenesisProfileDefaults {
                chain_id: "iroha3-nexus",
                collectors_k: 5,
                collectors_redundant_send_r: 3,
            },
        }
    }

    /// Whether the profile mandates an explicit VRF seed.
    pub const fn requires_seed(self) -> bool {
        matches!(
            self,
            GenesisProfile::Iroha3Testus | GenesisProfile::Iroha3Nexus
        )
    }
}

impl std::str::FromStr for GenesisProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "iroha3-dev" | "iroha3dev" | "iroha3_dev" => Ok(GenesisProfile::Iroha3Dev),
            "iroha3-testus" | "iroha3testus" | "iroha3_testus" => Ok(GenesisProfile::Iroha3Testus),
            "iroha3-nexus" | "iroha3nexus" | "iroha3_nexus" => Ok(GenesisProfile::Iroha3Nexus),
            other => Err(format!("invalid genesis profile `{other}`")),
        }
    }
}

impl fmt::Display for GenesisProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_kagami_arg())
    }
}

/// Derived defaults for a genesis profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenesisProfileDefaults {
    /// Expected chain identifier.
    pub chain_id: &'static str,
    /// Collector quorum size.
    pub collectors_k: u16,
    /// Redundant send fanout.
    pub collectors_redundant_send_r: u8,
}

/// Structural metadata describing how many peers and helper processes MOCHI should manage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkTopology {
    /// Number of peers launched for the profile.
    pub peer_count: usize,
}

impl NetworkTopology {
    /// One peer topology.
    pub const fn single_peer() -> Self {
        Self { peer_count: 1 }
    }

    /// Four peer topology for BFT testing.
    pub const fn four_peer_bft() -> Self {
        Self { peer_count: 4 }
    }
}

impl Default for NetworkTopology {
    fn default() -> Self {
        NetworkTopology::single_peer()
    }
}

/// Aggregated profile data describing topology and consensus mode, optionally tied to a preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProfile {
    /// Optional user-facing preset, when the profile matches a built-in topology.
    pub preset: Option<ProfilePreset>,
    /// Derived topology for the preset.
    pub topology: NetworkTopology,
    /// Consensus mode to seed into the genesis parameters.
    pub consensus_mode: SumeragiConsensusMode,
}

impl NetworkProfile {
    /// Create a profile from a known preset.
    pub fn from_preset(preset: ProfilePreset) -> Self {
        let topology = NetworkTopology {
            peer_count: preset.peer_count(),
        };
        Self {
            preset: Some(preset),
            topology,
            consensus_mode: SumeragiConsensusMode::Permissioned,
        }
    }

    /// Create a custom profile with validation for peer count bounds.
    pub fn custom(
        peer_count: usize,
        consensus_mode: SumeragiConsensusMode,
    ) -> Result<Self, String> {
        let profile = Self {
            preset: None,
            topology: NetworkTopology { peer_count },
            consensus_mode,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Validate the profile against topology bounds and preset defaults.
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_PEER_COUNT..=MAX_PEER_COUNT).contains(&self.topology.peer_count) {
            return Err(format!(
                "peer_count {} must be between {MIN_PEER_COUNT} and {MAX_PEER_COUNT}",
                self.topology.peer_count
            ));
        }
        if let Some(preset) = self.preset {
            let expected = preset.peer_count();
            if self.topology.peer_count != expected {
                return Err(format!(
                    "preset {preset:?} requires peer_count {expected}, got {}",
                    self.topology.peer_count
                ));
            }
        }
        Ok(())
    }

    /// Suggested on-disk slug for the profile.
    pub fn slug(&self) -> String {
        match self.preset {
            Some(preset) => preset.slug().to_owned(),
            None => format!(
                "custom-{}-{}",
                self.topology.peer_count,
                consensus_mode_slug(self.consensus_mode)
            ),
        }
    }

    /// Human-friendly label for the profile.
    pub fn label(&self) -> String {
        match self.preset {
            Some(preset) => preset.label().to_owned(),
            None => format!(
                "Custom ({} peers, {})",
                self.topology.peer_count,
                consensus_mode_label(self.consensus_mode)
            ),
        }
    }
}

impl Default for NetworkProfile {
    fn default() -> Self {
        Self::from_preset(ProfilePreset::SinglePeer)
    }
}

/// Filesystem context describing where generated material should live.
#[derive(Debug, Clone)]
pub struct NetworkPaths {
    /// Root directory for all network artifacts (genesis, configs, logs, Kura, snapshots).
    root: PathBuf,
}

impl NetworkPaths {
    /// Construct paths from an application data root.
    pub fn from_root(root: impl Into<PathBuf>, profile: &NetworkProfile) -> Self {
        let mut path = root.into();
        path.push(profile.slug());
        Self { root: path }
    }

    /// Ensure the directory tree exists for the network root and common subdirectories.
    ///
    /// # Errors
    /// Returns an `io::Error` if any directory creation fails.
    pub fn ensure(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.root())?;
        fs::create_dir_all(self.peers_dir())?;
        fs::create_dir_all(self.genesis_dir())?;
        fs::create_dir_all(self.logs_dir())?;
        fs::create_dir_all(self.snapshots_dir())
    }

    /// Path to the network root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory storing per-peer configuration folders.
    pub fn peers_dir(&self) -> PathBuf {
        self.root.join("peers")
    }

    /// Directory for a specific peer.
    pub fn peer_dir(&self, alias: &str) -> PathBuf {
        self.peers_dir().join(alias)
    }

    /// Directory for genesis manifests and collateral.
    pub fn genesis_dir(&self) -> PathBuf {
        self.root.join("genesis")
    }

    /// Directory storing supervisor logs.
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Directory storing exported snapshots.
    pub fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }
}

/// Deterministic port allocator that hands out incrementing socket ports.
#[derive(Debug, Clone)]
pub struct PortAllocator {
    next: u16,
}

impl PortAllocator {
    /// Create a new allocator starting at `start`.
    pub const fn new(start: u16) -> Self {
        Self { next: start }
    }

    /// Acquire the next available TCP port.
    ///
    /// The allocator probes the loopback interface, returning the first free port inside the
    /// wrapping range starting at the configured base. Callers should ensure the search range is
    /// large enough for the desired topology.
    pub fn allocate(&mut self) -> io::Result<u16> {
        self.allocate_with_probe(Self::port_available)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "exhausted port space while searching for a free socket",
                )
            })
    }

    fn allocate_with_probe<F>(&mut self, mut probe: F) -> Option<u16>
    where
        F: FnMut(u16) -> bool,
    {
        for _ in 0..=u16::MAX {
            let port = self.next;
            self.next = self.next.wrapping_add(1);
            if port == 0 {
                continue;
            }
            if probe(port) {
                return Some(port);
            }
        }
        None
    }

    fn port_available(port: u16) -> bool {
        let v4_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        match Self::try_bind_socket(v4_addr) {
            Ok(()) => return true,
            Err(err) => {
                if err.kind() != io::ErrorKind::AddrNotAvailable {
                    return false;
                }
            }
        }
        let v6_addr = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
        Self::try_bind_socket(v6_addr).is_ok()
    }

    fn try_bind_socket(addr: SocketAddr) -> io::Result<()> {
        TcpListener::bind(addr).map(drop)
    }
}

fn consensus_mode_slug(mode: SumeragiConsensusMode) -> &'static str {
    match mode {
        SumeragiConsensusMode::Permissioned => "permissioned",
        SumeragiConsensusMode::Npos => "npos",
    }
}

fn consensus_mode_label(mode: SumeragiConsensusMode) -> &'static str {
    match mode {
        SumeragiConsensusMode::Permissioned => "permissioned",
        SumeragiConsensusMode::Npos => "npos",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn can_bind_loopback(context: &str) -> bool {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
            Ok(listener) => {
                drop(listener);
                true
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::AddrNotAvailable
                ) =>
            {
                eprintln!("skipping {context}: {err}");
                false
            }
            Err(err) => panic!("{context}: {err}"),
        }
    }

    #[test]
    fn profile_topology_matches_preset() {
        let single = NetworkProfile::from_preset(ProfilePreset::SinglePeer);
        assert_eq!(single.topology.peer_count, 1);
        assert_eq!(single.preset, Some(ProfilePreset::SinglePeer));
        assert_eq!(single.consensus_mode, SumeragiConsensusMode::Permissioned);

        let four = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
        assert_eq!(four.topology.peer_count, 4);
        assert_eq!(four.preset, Some(ProfilePreset::FourPeerBft));
        assert_eq!(four.consensus_mode, SumeragiConsensusMode::Permissioned);
    }

    #[test]
    fn custom_profile_validates_peer_bounds() {
        let err = NetworkProfile::custom(0, SumeragiConsensusMode::Permissioned)
            .expect_err("zero peers should be rejected");
        assert!(err.contains("peer_count"), "unexpected error: {err}");

        let err = NetworkProfile::custom(8, SumeragiConsensusMode::Permissioned)
            .expect_err("too many peers should be rejected");
        assert!(err.contains("peer_count"), "unexpected error: {err}");

        let profile =
            NetworkProfile::custom(7, SumeragiConsensusMode::Npos).expect("valid custom profile");
        assert_eq!(profile.preset, None);
        assert_eq!(profile.topology.peer_count, 7);
    }

    #[test]
    fn custom_profile_slug_and_label_include_consensus_mode() {
        let profile =
            NetworkProfile::custom(3, SumeragiConsensusMode::Npos).expect("valid custom profile");
        assert_eq!(profile.slug(), "custom-3-npos");
        let label = profile.label().to_ascii_lowercase();
        assert!(label.contains("3"), "label should include peer count");
        assert!(
            label.contains("npos"),
            "label should include consensus mode"
        );
    }

    #[test]
    fn genesis_profile_parses_aliases() {
        assert_eq!(
            "iroha3-dev".parse::<GenesisProfile>().unwrap(),
            GenesisProfile::Iroha3Dev
        );
        assert_eq!(
            "iroha3_testus".parse::<GenesisProfile>().unwrap(),
            GenesisProfile::Iroha3Testus
        );
        assert!(
            "unknown".parse::<GenesisProfile>().is_err(),
            "invalid profiles should error"
        );
    }

    #[test]
    fn genesis_profile_defaults_are_available() {
        let nexus = GenesisProfile::Iroha3Nexus.defaults();
        assert_eq!(nexus.chain_id, "iroha3-nexus");
        assert_eq!(nexus.collectors_k, 5);
        assert!(GenesisProfile::Iroha3Nexus.requires_seed());
    }

    #[test]
    fn network_paths_append_profile_slug() {
        let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
        let paths = NetworkPaths::from_root("/tmp/mochi", &profile);
        assert!(paths.root().ends_with(Path::new("four-peer-bft")));
    }

    #[test]
    fn ensure_creates_expected_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        let profile = NetworkProfile::default();
        let paths = NetworkPaths::from_root(temp.path(), &profile);
        paths.ensure().expect("ensure directories");

        assert!(paths.root().exists());
        assert!(paths.peers_dir().exists());
        assert!(paths.genesis_dir().exists());
        assert!(paths.logs_dir().exists());
    }

    #[test]
    fn port_allocator_advances_monotonically() {
        if !can_bind_loopback("port_allocator_advances_monotonically") {
            return;
        }
        let mut allocator = PortAllocator::new(4000);
        assert_eq!(allocator.allocate().unwrap(), 4000);
        assert_eq!(allocator.allocate().unwrap(), 4001);
        assert_eq!(allocator.allocate().unwrap(), 4002);
    }

    #[test]
    fn port_allocator_skips_ports_in_use() {
        use std::{io::ErrorKind, net::TcpListener};

        let (listener, busy_port) = loop {
            let candidate = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
                Ok(listener) => listener,
                Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                    eprintln!("skipping port allocator test: {err}");
                    return;
                }
                Err(err) => panic!("bind busy port: {err}"),
            };
            let port = candidate.local_addr().expect("addr").port();
            let Some(next_port) = port.checked_add(1) else {
                continue;
            };
            match TcpListener::bind((Ipv4Addr::LOCALHOST, next_port)) {
                Ok(guard) => {
                    drop(guard);
                    break (candidate, port);
                }
                Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                    eprintln!("skipping port allocator test: {err}");
                    return;
                }
                Err(_) => continue,
            }
        };

        let mut allocator = PortAllocator::new(busy_port);
        let allocated = allocator.allocate().expect("allocate port");
        assert_eq!(allocated, busy_port.wrapping_add(1));
        drop(listener);
    }

    #[test]
    fn port_allocator_wraps_and_skips_zero() {
        let mut allocator = PortAllocator::new(u16::MAX);
        let mut probed = Vec::new();

        let selected = allocator
            .allocate_with_probe(|port| {
                probed.push(port);
                port == 5
            })
            .expect("probe should return a match before exhausting the search space");

        assert_eq!(
            probed,
            vec![u16::MAX, 1, 2, 3, 4, 5],
            "allocator should wrap past u16::MAX, skip port 0, and continue probing"
        );
        assert_eq!(selected, 5);
    }
}

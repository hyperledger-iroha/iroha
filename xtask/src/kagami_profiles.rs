//! Generate canned Kagami profile bundles (genesis + PoPs + snippets) for Iroha 3 profiles.

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use blake2::{Blake2b512, digest::Digest};
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair};
use iroha_data_model::{parameter::system::SumeragiConsensusMode, peer::PeerId};
use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
use norito::json;

use crate::workspace_root;

#[derive(Debug, Clone)]
pub(crate) struct KagamiProfileOptions {
    pub output: PathBuf,
    pub profiles: Vec<String>,
    pub kagami_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProfileSpec {
    slug: &'static str,
    profile_flag: &'static str,
    chain_id: &'static str,
    min_peers: usize,
    requires_seed: bool,
    collectors_k: u16,
    collectors_r: u8,
}

impl ProfileSpec {
    fn vrf_seed_hex(&self) -> String {
        hex::encode_upper(Hash::new(self.chain_id).as_ref())
    }
}

#[derive(Debug, Clone)]
struct PeerMaterial {
    peer_id: PeerId,
    address: String,
    public_key: String,
    private_key: String,
    pop: Vec<u8>,
    pop_hex: String,
}

type AnyResult<T> = Result<T, Box<dyn Error>>;
// Ed25519 identity used for streaming control-plane in sample configs.
const STREAM_ID_PUBLIC: &str =
    "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
const STREAM_ID_PRIVATE: &str =
    "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83";

pub(crate) fn generate(options: KagamiProfileOptions) -> AnyResult<()> {
    let specs = resolve_requested_profiles(&options.profiles)?;
    let kagami_bin = resolve_kagami_path(options.kagami_override.as_deref())?;
    fs::create_dir_all(&options.output)?;

    for spec in specs {
        write_profile_bundle(&spec, &kagami_bin, &options.output)?;
    }

    Ok(())
}

fn resolve_requested_profiles(names: &[String]) -> AnyResult<Vec<ProfileSpec>> {
    if names.is_empty() {
        return Ok(PROFILES.to_vec());
    }
    let mut out = Vec::new();
    for name in names {
        if name == "all" {
            return Ok(PROFILES.to_vec());
        }
        let Some(spec) = PROFILES.iter().find(|spec| spec.slug == name.as_str()) else {
            return Err(format!(
                "unknown profile `{name}` (expected one of all,{})",
                profile_slug_list()
            )
            .into());
        };
        out.push(*spec);
    }
    Ok(out)
}

fn write_profile_bundle(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    output_root: &Path,
) -> AnyResult<()> {
    let bundle_root = output_root.join(spec.slug);
    if bundle_root.exists() {
        fs::remove_dir_all(&bundle_root)?;
    }
    fs::create_dir_all(&bundle_root)?;

    let genesis_key =
        deterministic_keypair(&format!("{}-genesis-key", spec.slug), Algorithm::Ed25519);
    let genesis_json = generate_genesis(spec, kagami_bin, genesis_key.public_key(), &bundle_root)?;
    let peers = build_peers(spec);
    let patched_genesis = inject_topology(genesis_json, &peers)?;
    let genesis_path = bundle_root.join("genesis.json");
    write_json(&genesis_path, &patched_genesis)?;

    let vrf_seed_hex = if spec.requires_seed {
        Some(spec.vrf_seed_hex())
    } else {
        None
    };
    let verify_out = run_verify(spec, kagami_bin, &genesis_path, vrf_seed_hex.as_deref())?;
    fs::write(bundle_root.join("verify.txt"), verify_out)?;

    let config_text = render_config(spec, &peers, genesis_key.public_key());
    fs::write(bundle_root.join("config.toml"), config_text)?;

    let compose = render_docker_compose(spec, &peers);
    fs::write(bundle_root.join("docker-compose.yml"), compose)?;

    let readme = render_readme(
        spec,
        &peers,
        genesis_key.public_key(),
        vrf_seed_hex.as_deref(),
    );
    fs::write(bundle_root.join("README.md"), readme)?;

    Ok(())
}

fn generate_genesis(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    genesis_public_key: &iroha_crypto::PublicKey,
    workdir: &Path,
) -> AnyResult<RawGenesisTransaction> {
    let mut command = Command::new(kagami_bin);
    command.args([
        "genesis",
        "generate",
        "--profile",
        spec.profile_flag,
        "--ivm-dir",
        ".",
        "--genesis-public-key",
        &genesis_public_key.to_string(),
        "--consensus-mode",
        "npos",
    ]);

    if spec.requires_seed {
        command.args(["--vrf-seed-hex", &spec.vrf_seed_hex()]);
    }

    let output = command
        .current_dir(workdir)
        .output()
        .map_err(|err| format!("failed to run kagami: {err}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "kagami genesis generate failed (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        )
        .into());
    }

    json::from_slice(&output.stdout)
        .map_err(|err| format!("failed to parse genesis JSON: {err}").into())
}

fn inject_topology(
    manifest: RawGenesisTransaction,
    peers: &[PeerMaterial],
) -> AnyResult<RawGenesisTransaction> {
    let consensus_mode = manifest
        .consensus_mode()
        .unwrap_or(SumeragiConsensusMode::Permissioned);
    let topology: Vec<GenesisTopologyEntry> = peers
        .iter()
        .map(|peer| GenesisTopologyEntry::new(peer.peer_id.clone(), peer.pop.clone()))
        .collect();
    let manifest = manifest
        .into_builder()
        .set_topology(topology)
        .build_raw()
        .with_consensus_mode(consensus_mode)
        .with_consensus_meta();
    Ok(manifest)
}

fn write_json(path: &Path, value: &RawGenesisTransaction) -> AnyResult<()> {
    let mut rendered = json::to_json_pretty(value)?;
    rendered.push('\n');
    fs::write(path, rendered)?;
    Ok(())
}

fn run_verify(
    spec: &ProfileSpec,
    kagami_bin: &Path,
    genesis_path: &Path,
    vrf_seed_hex: Option<&str>,
) -> AnyResult<String> {
    let mut command = Command::new(kagami_bin);
    command.args([
        "verify",
        "--profile",
        spec.profile_flag,
        "--genesis",
        genesis_path.to_str().expect("genesis path utf-8"),
    ]);
    if let Some(seed) = vrf_seed_hex {
        command.args(["--vrf-seed-hex", seed]);
    }
    let output = command
        .output()
        .map_err(|err| format!("failed to run kagami verify: {err}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "kagami verify failed (status {:?}):\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn render_config(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    genesis_public_key: &iroha_crypto::PublicKey,
) -> String {
    let node = peers.first().expect("at least one peer per profile");
    let trusted_peers = peers
        .iter()
        .map(|peer| format!("  \"{}@{}\"", peer.public_key, peer.address))
        .collect::<Vec<_>>()
        .join(",\n");
    let trusted_peers_pop = peers
        .iter()
        .map(|peer| {
            format!(
                "  {{ public_key = \"{}\", pop_hex = \"{}\" }}",
                peer.public_key, peer.pop_hex
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        r#"# Sample config for {slug} (generated via cargo xtask kagami-profiles)
chain = "{chain}"
public_key = "{node_pk}"
private_key = "{node_sk}"

trusted_peers = [
{trusted_peers}
]
trusted_peers_pop = [
{trusted_peers_pop}
]

[network]
address = "0.0.0.0:{p2p}"
public_address = "127.0.0.1:{p2p}"

[torii]
address = "0.0.0.0:8080"

[streaming]
identity_public_key = "{stream_pub}"
identity_private_key = "{stream_priv}"

[nexus]
enabled = true
lane_count = 3

[genesis]
public_key = "{genesis_pk}"
"#,
        slug = spec.slug,
        chain = spec.chain_id,
        node_pk = node.public_key,
        node_sk = node.private_key,
        trusted_peers = trusted_peers,
        trusted_peers_pop = trusted_peers_pop,
        p2p = node.address.split(':').next_back().unwrap_or("1337"),
        genesis_pk = genesis_public_key,
        stream_pub = STREAM_ID_PUBLIC,
        stream_priv = STREAM_ID_PRIVATE,
    )
}

fn render_docker_compose(spec: &ProfileSpec, peers: &[PeerMaterial]) -> String {
    let node = peers.first().expect("at least one peer per profile");
    format!(
        r#"version: "3.9"
services:
  iroha-{slug}:
    image: hyperledger/iroha:latest
    command: ["irohad", "--sora", "--config", "/config/config.toml", "--genesis", "/config/genesis.json"]
    volumes:
      - ./config.toml:/config/config.toml:ro
      - ./genesis.json:/config/genesis.json:ro
    ports:
      - "8080:8080"
      - "{p2p}:{p2p}"
"#,
        slug = spec.slug,
        p2p = node.address.split(':').next_back().unwrap_or("1337"),
    )
}

fn render_readme(
    spec: &ProfileSpec,
    peers: &[PeerMaterial],
    genesis_public_key: &iroha_crypto::PublicKey,
    vrf_seed_hex: Option<&str>,
) -> String {
    let peer_rows = peers
        .iter()
        .enumerate()
        .map(|(idx, peer)| {
            format!(
                "- peer {idx}: public_key={pk} address={addr} pop_hex={pop}",
                idx = idx + 1,
                pk = peer.public_key,
                addr = peer.address,
                pop = peer.pop_hex
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let vrf_line = if let Some(seed) = vrf_seed_hex {
        format!("VRF seed (hex): {seed}")
    } else {
        "VRF seed: derived from chain id".to_string()
    };

    format!(
        r#"# {slug} sample bundle

- chain id: {chain}
- collectors: k={k} r={r}
- {vrf_line}
- genesis public key: {genesis_pk}
- peers:
{peer_rows}

Files:
- genesis.json — generated with `kagami genesis generate --profile {profile}` and patched with deterministic topology+PoPs
- verify.txt — stdout from `kagami verify --profile {profile} --genesis genesis.json`
- config.toml — minimal Nexus config matching the topology (ports 8080/1337)
- docker-compose.yml — single-node snippet mounting the config/genesis

Regenerate:
- cargo xtask kagami-profiles --profile {profile}
"#,
        slug = spec.slug,
        chain = spec.chain_id,
        k = spec.collectors_k,
        r = spec.collectors_r,
        vrf_line = vrf_line,
        genesis_pk = genesis_public_key,
        peer_rows = peer_rows,
        profile = spec.profile_flag,
    )
}

fn build_peers(spec: &ProfileSpec) -> Vec<PeerMaterial> {
    (0..spec.min_peers)
        .map(|idx| {
            let seed = format!("{}-peer-{idx}", spec.slug);
            let kp = deterministic_keypair(&seed, Algorithm::BlsNormal);
            let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key())
                .expect("deterministic pop generation");
            let port = 1337 + u16::try_from(idx).unwrap_or(0);
            let address = format!("127.0.0.1:{port}");
            PeerMaterial {
                peer_id: PeerId::from(kp.public_key().clone()),
                address,
                public_key: kp.public_key().to_string(),
                private_key: ExposedPrivateKey(kp.private_key().clone()).to_string(),
                pop: pop.clone(),
                pop_hex: hex::encode(&pop),
            }
        })
        .collect()
}

fn deterministic_keypair(seed_label: &str, algorithm: Algorithm) -> KeyPair {
    let mut hasher = Blake2b512::new();
    hasher.update(seed_label.as_bytes());
    let hash = hasher.finalize();
    let mut seed = Vec::with_capacity(32);
    seed.extend_from_slice(&hash[..32]);
    KeyPair::from_seed(seed, algorithm)
}

fn resolve_kagami_path(override_path: Option<&Path>) -> AnyResult<PathBuf> {
    if let Some(path) = override_path {
        if !path.exists() {
            return Err(format!("kagami override {} does not exist", path.display()).into());
        }
        if !path.is_file() {
            return Err(format!("kagami override {} is not a file", path.display()).into());
        }
        return Ok(path.to_path_buf());
    }

    let target_dir = cargo_target_dir();
    let release_candidate = target_dir
        .join("release")
        .join(format!("kagami{}", std::env::consts::EXE_SUFFIX));
    if release_candidate.exists() {
        return Ok(release_candidate);
    }

    let debug_candidate = target_dir
        .join("debug")
        .join(format!("kagami{}", std::env::consts::EXE_SUFFIX));
    if debug_candidate.exists() {
        return Ok(debug_candidate);
    }

    let status = Command::new("cargo")
        .args(["build", "-p", "iroha_kagami", "--release"])
        .status()?;
    if !status.success() {
        return Err(format!("cargo build -p iroha_kagami --release failed with {status:?}").into());
    }

    let release_candidate = target_dir
        .join("release")
        .join(format!("kagami{}", std::env::consts::EXE_SUFFIX));
    if release_candidate.exists() {
        Ok(release_candidate)
    } else {
        Err(format!(
            "expected kagami binary at {} after build",
            release_candidate.display()
        )
        .into())
    }
}

fn cargo_target_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        let path = PathBuf::from(dir);
        if path.is_absolute() {
            path
        } else {
            workspace_root().join(path)
        }
    } else {
        workspace_root().join("target")
    }
}

const PROFILES: &[ProfileSpec] = &[
    ProfileSpec {
        slug: "iroha3-dev",
        profile_flag: "iroha3-dev",
        chain_id: "iroha3-dev.local",
        min_peers: 1,
        requires_seed: false,
        collectors_k: 1,
        collectors_r: 1,
    },
    ProfileSpec {
        slug: "iroha3-testus",
        profile_flag: "iroha3-testus",
        chain_id: "iroha3-testus",
        min_peers: 4,
        requires_seed: true,
        collectors_k: 3,
        collectors_r: 2,
    },
    ProfileSpec {
        slug: "iroha3-nexus",
        profile_flag: "iroha3-nexus",
        chain_id: "iroha3-nexus",
        min_peers: 4,
        requires_seed: true,
        collectors_k: 5,
        collectors_r: 2,
    },
];

fn profile_slug_list() -> String {
    PROFILES
        .iter()
        .map(|spec| spec.slug)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn stub_genesis() -> RawGenesisTransaction {
        json::from_str(
            r#"{
            "chain": "stub",
            "executor": null,
            "ivm_dir": ".",
            "consensus_mode": "Npos",
            "transactions": [ {} ]
        }"#,
        )
        .expect("stub genesis parses")
    }

    #[test]
    fn peers_are_deterministic_and_populated() {
        let peers = build_peers(&PROFILES[1]);
        assert_eq!(peers.len(), PROFILES[1].min_peers);
        assert!(peers.iter().all(|p| !p.pop_hex.is_empty()));
        assert_eq!(
            peers[0].peer_id.public_key(),
            build_peers(&PROFILES[1])[0].peer_id.public_key()
        );
    }

    #[test]
    fn topology_is_injected_into_genesis() {
        let peers = build_peers(&PROFILES[0]);
        let patched = inject_topology(stub_genesis(), &peers).expect("inject topology");
        let txs = patched.transactions();
        assert_eq!(txs.len(), 1, "stub genesis should carry one transaction");
        let tx0 = &txs[0];
        assert_eq!(
            tx0.topology().len(),
            peers.len(),
            "topology should be populated inside the manifest"
        );
        let pop_count = tx0
            .topology()
            .iter()
            .filter(|entry| entry.pop_hex.as_deref().is_some_and(|hex| !hex.is_empty()))
            .count();
        assert_eq!(
            pop_count,
            peers.len(),
            "pop_hex should be embedded for every topology entry"
        );
        let value = json::to_value(&patched).expect("serialize patched genesis");
        let tx0 = value
            .get("transactions")
            .and_then(norito::json::Value::as_array)
            .and_then(|txs| txs.first())
            .and_then(norito::json::Value::as_object)
            .expect("first transaction present");
        let topo = tx0["topology"].as_array().expect("topology array present");
        assert_eq!(topo.len(), peers.len());
        let first = topo[0].as_object().expect("topology entry object");
        assert!(first.get("pop_hex").is_some(), "pop_hex embedded");
    }

    #[test]
    fn config_contains_expected_keys() {
        let peers = build_peers(&PROFILES[2]);
        let genesis_key = deterministic_keypair("config-genesis", Algorithm::Ed25519);
        let rendered = render_config(&PROFILES[2], &peers, genesis_key.public_key());
        assert!(rendered.contains(PROFILES[2].chain_id));
        assert!(rendered.contains(peers[0].public_key.as_str()));
        assert!(rendered.contains(&genesis_key.public_key().to_string()));
        assert!(rendered.contains(STREAM_ID_PUBLIC));
        assert!(rendered.contains(STREAM_ID_PRIVATE));
    }

    #[test]
    fn readme_carries_profile_metadata() {
        let peers = build_peers(&PROFILES[0]);
        let genesis_key = deterministic_keypair("readme-genesis", Algorithm::Ed25519);
        let readme = render_readme(&PROFILES[0], &peers, genesis_key.public_key(), None);
        assert!(readme.contains(PROFILES[0].slug));
        assert!(readme.contains("Regenerate"));
    }

    #[test]
    fn verify_output_captured_on_success() {
        if std::env::var("XTASK_TEST_KAGAMI_BIN").is_err() {
            return;
        }
        let kagami_path = PathBuf::from(std::env::var("XTASK_TEST_KAGAMI_BIN").unwrap());
        let temp = tempdir().expect("temp dir");
        let genesis_path = temp.path().join("genesis.json");
        let mut rendered = json::to_json_pretty(&stub_genesis()).expect("render stub genesis");
        rendered.push('\n');
        fs::write(&genesis_path, rendered).expect("write stub genesis");
        let out = run_verify(&PROFILES[0], &kagami_path, &genesis_path, None);
        assert!(
            out.is_ok(),
            "verify should succeed when kagami binary is supplied: {out:?}"
        );
    }
}

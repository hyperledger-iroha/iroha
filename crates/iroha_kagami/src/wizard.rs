//! Interactive setup wizard for quickly preparing Iroha/Sora configs.
use crate::{Outcome, RunArgs, tui};
use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Context as _, Result, eyre};
use inquire::{Select, Text};
use iroha_config::parameters::{actual, defaults};
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, KeyPair, PublicKey, bls_normal_pop_prove, bls_normal_pop_verify,
};
use iroha_data_model::peer::{Peer, PeerId};
use iroha_genesis::{read_genesis_manifest_bytes, validate_genesis_manifest_json};
use iroha_primitives::addr::{SocketAddr, SocketAddrHost};
use norito::json::{self, Value as JsonValue};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{BufWriter, Write},
    net::{Ipv4Addr, Ipv6Addr},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    str::FromStr,
};
use toml::{Value as TomlValue, value::Table as TomlTable};
/// Supported network profiles for the wizard.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Profile {
    /// Canonical local single-lane profile.
    Local,
    /// Sora Nexus (mainnet).
    Nexus,
}
impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Profile::Local => write!(f, "Local (single lane)"),
            Profile::Nexus => write!(f, "Sora Nexus (mainnet)"),
        }
    }
}
/// CLI entrypoint for the setup wizard.
#[derive(Debug, ClapArgs, Clone)]
pub struct Args {
    /// Optional preset profile; if omitted, the wizard prompts for one.
    #[arg(long, value_enum)]
    pub profile: Option<Profile>,
    /// Directory where generated config/genesis files will be written.
    #[arg(long, value_name = "PATH", default_value = "wizard-output")]
    pub output_dir: PathBuf,
    /// Run non-interactively, accepting defaults for prompts that are not supplied via flags.
    #[arg(long)]
    pub non_interactive: bool,
    /// Override the default chain identifier.
    #[arg(long, value_name = "CHAIN")]
    pub chain_id: Option<String>,
    /// Override the public P2P host/IP advertised for this peer.
    #[arg(long, value_name = "HOST")]
    pub p2p_host: Option<String>,
    /// Override the public P2P port for this peer.
    #[arg(long, value_name = "PORT")]
    pub p2p_port: Option<u16>,
    /// Override the Torii host/IP advertised for this peer.
    #[arg(long, value_name = "HOST")]
    pub torii_host: Option<String>,
    /// Override the Torii port for this peer.
    #[arg(long, value_name = "PORT")]
    pub torii_port: Option<u16>,
    /// Override the relay mode instead of prompting interactively.
    #[arg(long, value_enum)]
    pub relay_mode: Option<RelayMode>,
    /// Relay hub addresses (`host:port`), repeat once per hub when relay mode uses them.
    #[arg(long = "relay-hub-address", value_name = "HOST:PORT")]
    pub relay_hub_addresses: Vec<String>,
    /// Override the bootstrap peer (`pubkey@host:port`). Comma-separated for multiple entries.
    #[arg(long, value_name = "PEERS")]
    pub trusted_peers: Option<String>,
    /// Comma-separated PoP entries for trusted peers (`pubkey=pop_hex`).
    #[arg(long, value_name = "POPS")]
    pub trusted_peers_pop: Option<String>,
}
#[derive(Clone, Debug)]
struct Answers {
    profile: Profile,
    chain: String,
    p2p_host: String,
    p2p_port: u16,
    torii_host: String,
    torii_port: u16,
    trusted_peers: Vec<String>,
    relay_mode: RelayMode,
    relay_hub_addresses: Vec<String>,
    output_dir: PathBuf,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum RelayMode {
    Disabled,
    Hub,
    Spoke,
    Assist,
}
impl fmt::Display for RelayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelayMode::Disabled => write!(f, "disabled (full mesh)"),
            RelayMode::Hub => write!(f, "hub (static IP)"),
            RelayMode::Spoke => write!(f, "spoke (dial hub only)"),
            RelayMode::Assist => write!(f, "assist (direct + hub fallback)"),
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct ProfileDefaults {
    chain: &'static str,
    p2p_port: u16,
    torii_port: u16,
    host: &'static str,
    trusted_peers: &'static [&'static str],
    config_template: Option<&'static str>,
    genesis_template: &'static str,
}
impl ProfileDefaults {
    fn for_profile(profile: Profile) -> Self {
        match profile {
            Profile::Local => Self {
                chain: "00000000-0000-0000-0000-000000000000",
                p2p_port: 1337,
                torii_port: 8080,
                host: "127.0.0.1",
                trusted_peers: &[],
                config_template: None,
                genesis_template: "defaults/genesis.json",
            },
            Profile::Nexus => Self {
                chain: "00000000-0000-0000-0000-000000000753",
                p2p_port: 1337,
                torii_port: 8080,
                host: "nexus.mof2.sora.org",
                trusted_peers: &[concat!(
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2@",
                    "nexus.mof2.sora.org:1337"
                )],
                config_template: Some("configs/soranexus/nexus/config.toml"),
                genesis_template: "configs/soranexus/nexus/genesis.json",
            },
        }
    }
}
impl<T: Write> RunArgs<T> for Args {
    #[expect(
        clippy::too_many_lines,
        reason = "the guided command keeps generation, validation, persistence, and the final operator handoff in one linear workflow"
    )]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        print_banner();
        let answers = gather_answers(&self)?;
        let keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .wrap_err("failed to generate wizard BLS key pair")?;
        let soranet_transport_keypair = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .wrap_err("failed to generate wizard SoraNet transport key pair")?;
        let streaming_identity_keypair = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .wrap_err("failed to generate wizard streaming identity key pair")?;
        if streaming_identity_keypair.public_key() == soranet_transport_keypair.public_key() {
            return Err(eyre!(
                "wizard streaming and SoraNet transport identities must be distinct"
            ));
        }
        let trusted_pops = resolve_trusted_peers_pop(&self, &answers, &keypair)?;
        tui::status("Generating config and genesis files");
        let (mut config, genesis_template_path) = load_config_template(
            &answers,
            &keypair,
            &soranet_transport_keypair,
            &streaming_identity_keypair,
            &trusted_pops,
        )?;
        apply_overrides(
            &mut config,
            &answers,
            &keypair,
            &soranet_transport_keypair,
            &streaming_identity_keypair,
            &trusted_pops,
        )?;
        let genesis = load_and_patch_genesis(&genesis_template_path, &answers.chain)?;
        fs::create_dir_all(&answers.output_dir)
            .wrap_err("failed to create output directory for wizard artefacts")?;
        let config_path = answers.output_dir.join("config.toml");
        let genesis_path = answers.output_dir.join("genesis.json");
        let mut config_payload = toml::to_string_pretty(&config)
            .wrap_err("failed to serialise config after wizard updates")?;
        // Surface optional networking knobs in the generated config without changing defaults.
        config_payload.push_str(
            r#"

# ---
# P2P advanced options (optional)
#
# [network]
# # Outbound proxy (HTTP CONNECT / SOCKS5):
# p2p_proxy = "http://user:pass@proxy.example.com:8080" # or socks5://...
# p2p_proxy_required = false
# p2p_no_proxy = ["localhost", ".example.com"]
#
# # If p2p_proxy starts with https://, the proxy hop uses TLS (requires iroha_p2p/p2p_tls):
# p2p_proxy_tls_verify = true
# p2p_proxy_tls_pinned_cert_der_base64 = "BASE64_DER"
#
# Notes:
# - When p2p_proxy_required=true, p2p_no_proxy must be empty.
# - When p2p_proxy is https:// and p2p_proxy_tls_verify=true, a pinned cert is required.
# - P2P always serves TLS 1.3 on network.address; there is no plaintext listener or fallback.
"#,
        );
        fs::write(&config_path, config_payload)
            .wrap_err_with(|| format!("failed to write config to {}", config_path.display()))?;
        let genesis_payload = json::to_string_pretty(&genesis)
            .wrap_err("failed to serialise genesis after wizard updates")?;
        validate_genesis_manifest_json(genesis_payload.as_bytes())
            .wrap_err("generated wizard genesis exceeds fixed resource bounds")?;
        fs::write(&genesis_path, genesis_payload)
            .wrap_err_with(|| format!("failed to write genesis to {}", genesis_path.display()))?;
        let guide_path = answers.output_dir.join("README.md");
        let start_command = format!(
            "cd {} && iroha3d {}--config config.toml",
            answers.output_dir.display(),
            if answers.profile == Profile::Nexus {
                "--sora "
            } else {
                ""
            }
        );
        write_wizard_readme(
            &guide_path,
            answers.profile,
            &answers.chain,
            keypair.public_key(),
            &config_path,
            &genesis_path,
            &start_command,
        )?;
        tui::success(format!(
            "Config and reference genesis manifest staged under {}",
            answers.output_dir.display()
        ));
        writeln!(writer, "profile: {}", answers.profile)?;
        writeln!(writer, "chain_id: {}", answers.chain)?;
        writeln!(writer, "generated_public_key: {}", keypair.public_key())?;
        writeln!(writer, "config: {}", config_path.display())?;
        writeln!(writer, "genesis_manifest: {}", genesis_path.display())?;
        writeln!(writer, "guide: {}", guide_path.display())?;
        if answers.profile == Profile::Nexus {
            writeln!(writer, "sora profile: pass --sora when starting iroha3d")?;
        }
        writeln!(
            writer,
            "next: obtain the authoritative genesis.signed.nrt and expected hash; see {}",
            guide_path.display()
        )?;
        Ok(())
    }
}
fn gather_answers(args: &Args) -> Result<Answers> {
    let profile = resolve_profile(args)?;
    let defaults = ProfileDefaults::for_profile(profile);
    let chain = resolve_text(
        "Chain ID",
        args.chain_id.clone(),
        defaults.chain.to_string(),
        args.non_interactive,
    )?;
    let p2p_host = resolve_text(
        "Trusted peer public host",
        args.p2p_host.clone(),
        defaults.host.to_string(),
        args.non_interactive,
    )?;
    let p2p_port = resolve_number(
        "P2P port",
        args.p2p_port,
        defaults.p2p_port,
        args.non_interactive,
    )?;
    let torii_host = resolve_text(
        "Trusted peer Torii host",
        args.torii_host.clone(),
        defaults.host.to_string(),
        args.non_interactive,
    )?;
    let torii_port = resolve_number(
        "Torii port",
        args.torii_port,
        defaults.torii_port,
        args.non_interactive,
    )?;
    let relay_mode = resolve_relay_mode(args.relay_mode, args.non_interactive)?;
    let relay_hub_addresses = if matches!(relay_mode, RelayMode::Spoke | RelayMode::Assist) {
        if args.relay_hub_addresses.is_empty() {
            let raw = resolve_text(
                "Relay hub addresses (comma separated host:port)",
                None,
                format!("{}:{}", defaults.host, defaults.p2p_port),
                args.non_interactive,
            )?;
            raw.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        } else {
            args.relay_hub_addresses.clone()
        }
    } else {
        Vec::new()
    };
    let default_trusted = defaults.trusted_peers.join(", ");
    let trusted_default = args
        .trusted_peers
        .as_deref()
        .unwrap_or(default_trusted.as_str());
    let trusted_prompt = resolve_text(
        "Trusted peers (comma separated pubkey@host:port; PoPs mark validators)",
        args.trusted_peers.clone(),
        trusted_default.to_string(),
        args.non_interactive,
    )?;
    let trusted_peers = trusted_prompt
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    Ok(Answers {
        profile,
        chain,
        p2p_host,
        p2p_port,
        torii_host,
        torii_port,
        trusted_peers,
        relay_mode,
        relay_hub_addresses,
        output_dir: args.output_dir.clone(),
    })
}
fn resolve_trusted_peers_pop(
    args: &Args,
    answers: &Answers,
    keypair: &KeyPair,
) -> Result<BTreeMap<PublicKey, Vec<u8>>> {
    let joins_existing_sora_network = answers.profile == Profile::Nexus;
    if joins_existing_sora_network && args.non_interactive && args.trusted_peers.is_none() {
        return Err(eyre!(
            "non-interactive Sora wizard onboarding requires --trusted-peers with the authoritative full validator roster"
        ));
    }
    if joins_existing_sora_network && args.non_interactive && args.trusted_peers_pop.is_none() {
        return Err(eyre!(
            "non-interactive Sora wizard onboarding requires --trusted-peers-pop matching the authoritative signed genesis roster"
        ));
    }
    let mut peers = sanitize_trusted_peers(&answers.trusted_peers)?;
    let self_peer = format!(
        "{}@{}",
        keypair.public_key(),
        addr_literal(&answers.p2p_host, answers.p2p_port)?
    );
    if !trusted_peers_contain_key(&peers, keypair.public_key())? {
        peers.push(self_peer);
    }
    let peer_ids = parse_trusted_peer_ids(&peers)?;
    let roster_keys: BTreeSet<PublicKey> = peer_ids
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect();
    for pk in &roster_keys {
        if pk.algorithm() != Algorithm::BlsNormal {
            return Err(eyre!("trusted peer {pk} must use a BLS-Normal key"));
        }
    }
    let mut pops = parse_trusted_peers_pop_arg(args.trusted_peers_pop.as_deref())?;
    if !joins_existing_sora_network && !pops.contains_key(keypair.public_key()) {
        let pop = bls_normal_pop_prove(keypair.private_key())
            .wrap_err("failed to build PoP for the local keypair")?;
        pops.insert(keypair.public_key().clone(), pop);
    }
    if joins_existing_sora_network && pops.contains_key(keypair.public_key()) {
        return Err(eyre!(
            "the newly generated Sora peer must start as an observer; its local key must not appear in --trusted-peers-pop"
        ));
    }
    let extras: Vec<_> = pops
        .keys()
        .filter(|pk| !roster_keys.contains(*pk))
        .cloned()
        .collect();
    if !extras.is_empty() {
        return Err(eyre!(
            "trusted_peers_pop contains keys not in trusted_peers: {extras:?}"
        ));
    }
    let missing: Vec<PublicKey> = roster_keys
        .iter()
        .filter(|pk| !pops.contains_key(*pk))
        .cloned()
        .collect();
    if !missing.is_empty() && !args.non_interactive {
        for pk in missing {
            if joins_existing_sora_network && pk == *keypair.public_key() {
                continue;
            }
            let pop = prompt_pop_for_peer(&pk)?;
            pops.insert(pk, pop);
        }
    }
    if joins_existing_sora_network {
        let missing = roster_keys
            .iter()
            .filter(|public_key| {
                *public_key != keypair.public_key() && !pops.contains_key(*public_key)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(eyre!(
                "Sora wizard validator roster is missing authoritative PoPs for: {missing:?}"
            ));
        }
        if !iroha_data_model::block::consensus_v2::is_valid_committee_size(pops.len()) {
            return Err(eyre!(
                "Sora wizard authoritative validator roster must contain an exact 3f + 1 committee of 4, 7, ... 31 PoPs; got {}",
                pops.len()
            ));
        }
    }
    Ok(pops)
}
fn parse_trusted_peer_ids(peers: &[String]) -> Result<Vec<PeerId>> {
    peers
        .iter()
        .map(|entry| {
            let public_key = entry.split_once('@').map_or(entry.as_str(), |(pk, _)| pk);
            PeerId::from_str(public_key)
                .wrap_err_with(|| format!("invalid trusted peer entry: {entry}"))
        })
        .collect()
}
fn parse_trusted_peers_pop_arg(raw: Option<&str>) -> Result<BTreeMap<PublicKey, Vec<u8>>> {
    let mut pops = BTreeMap::new();
    let Some(raw) = raw else {
        return Ok(pops);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(pops);
    }
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (pk_raw, pop_raw) = entry
            .split_once('=')
            .ok_or_else(|| eyre!("trusted_peers_pop entry must be pubkey=pop_hex: {entry}"))?;
        let pk = PublicKey::from_str(pk_raw.trim()).wrap_err_with(|| {
            format!("trusted_peers_pop entry has invalid public key: {pk_raw}")
        })?;
        if pk.algorithm() != Algorithm::BlsNormal {
            return Err(eyre!("trusted_peers_pop entry uses non-BLS key: {pk}"));
        }
        let pop = decode_pop_hex(pop_raw.trim())
            .wrap_err_with(|| format!("trusted_peers_pop entry has invalid hex for {pk}"))?;
        if let Err(err) = bls_normal_pop_verify(&pk, &pop) {
            return Err(eyre!(
                "trusted_peers_pop entry has invalid PoP for {pk}: {err}"
            ));
        }
        if pops.insert(pk, pop).is_some() {
            return Err(eyre!("trusted_peers_pop entry duplicated for {entry}"));
        }
    }
    Ok(pops)
}
fn decode_pop_hex(raw: &str) -> Result<Vec<u8>> {
    let trimmed = raw.trim_start_matches("0x");
    hex::decode(trimmed).wrap_err("invalid PoP hex")
}
fn prompt_pop_for_peer(public_key: &PublicKey) -> Result<Vec<u8>> {
    loop {
        let prompt = format!("PoP for {public_key} (hex)");
        let input = Text::new(&prompt)
            .prompt()
            .wrap_err_with(|| format!("PoP prompt failed for {public_key}"))?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            tui::warn("PoP cannot be empty");
            continue;
        }
        let pop = match decode_pop_hex(trimmed) {
            Ok(pop) => pop,
            Err(err) => {
                tui::warn(format!("invalid PoP hex: {err}"));
                continue;
            }
        };
        if let Err(err) = bls_normal_pop_verify(public_key, &pop) {
            tui::warn(format!("invalid PoP for {public_key}: {err}"));
            continue;
        }
        return Ok(pop);
    }
}
#[allow(clippy::needless_raw_string_hashes)]
fn print_banner() {
    // Retro ASCII splash for a late-80s terminal vibe.
    let banner = r#"
  ________________________________________________________________
 /================================================================\
||  イ ロ ハ     ネ ッ ト ワ ー ク     IROHA NETWORK SETUP        ||
||                                                                ||
||      ⛩  I R O H A   S E T U P   T E R M I N A L  ⛩             ||
||                                                                ||
||  flow: keys → config → genesis → done                          ||
 ||  controls: ENTER to accept, ESC/CTRL+C to abort                ||
 ||                                                                ||
  \================================================================/
  ````````````````````````````````````````````````````````````````
"#;
    eprintln!("{banner}");
}
fn resolve_profile(args: &Args) -> Result<Profile> {
    if let Some(profile) = args.profile {
        return Ok(profile);
    }
    if args.non_interactive {
        return Ok(Profile::Local);
    }
    Select::new(
        "Which profile do you want to set up?",
        vec![Profile::Local, Profile::Nexus],
    )
    .prompt()
    .wrap_err("failed to read profile selection")
}
fn resolve_relay_mode(cli_value: Option<RelayMode>, non_interactive: bool) -> Result<RelayMode> {
    if let Some(mode) = cli_value {
        return Ok(mode);
    }
    if non_interactive {
        return Ok(RelayMode::Disabled);
    }
    Select::new(
        "Relay mode (hub/spoke for constrained topologies; assist adds hub fallback without forcing all peers)",
        vec![
            RelayMode::Disabled,
            RelayMode::Hub,
            RelayMode::Spoke,
            RelayMode::Assist,
        ],
    )
    .prompt()
    .wrap_err("failed to read relay mode selection")
}
fn resolve_text(
    prompt: &str,
    cli_value: Option<String>,
    default: String,
    non_interactive: bool,
) -> Result<String> {
    if let Some(value) = cli_value {
        return Ok(value);
    }
    if non_interactive {
        return Ok(default);
    }
    Text::new(prompt)
        .with_initial_value(&default)
        .prompt()
        .wrap_err_with(|| format!("{prompt} prompt failed"))
}
fn resolve_number(
    prompt: &str,
    cli_value: Option<u16>,
    default: u16,
    non_interactive: bool,
) -> Result<u16> {
    if let Some(value) = cli_value {
        return Ok(value);
    }
    if non_interactive {
        return Ok(default);
    }
    let text = Text::new(prompt)
        .with_initial_value(&default.to_string())
        .prompt()
        .wrap_err_with(|| format!("{prompt} prompt failed"))?;
    text.parse::<u16>()
        .wrap_err_with(|| format!("{prompt} must be a u16 port"))
}
fn write_wizard_readme(
    path: &Path,
    profile: Profile,
    chain_id: &str,
    public_key: &PublicKey,
    config_path: &Path,
    genesis_path: &Path,
    start_command: &str,
) -> Result<()> {
    let profile_prerequisites = match profile {
        Profile::Nexus => {
            "4. Confirm `trusted_peers` and `trusted_peers_pop` are the full operator-authenticated validator roster encoded by the signed genesis; the generated local peer starts as an observer.\n"
        }
        Profile::Local => "",
    };
    let rendered = format!(
        concat!(
            "# Kagami Wizard Output\n\n",
            "- Profile: `{profile}`\n",
            "- Chain ID: `{chain_id}`\n",
            "- Generated public key: `{public_key}`\n",
            "- Config: `{config}`\n",
            "- Reference genesis manifest: `{genesis}`\n\n",
            "This output is staged, not a locally signed replacement for the selected network's genesis.\n\n",
            "## Prerequisites\n\n",
            "1. Obtain the network-authoritative `genesis.signed.nrt` from the operator.\n",
            "2. Obtain its canonical checked NetworkId, derived from the exact signed genesis hash, as `genesis.expected_hash`.\n",
            "3. Verify both artifacts through the network's authenticated distribution channel.\n\n",
            "{profile_prerequisites}\n",
            "`genesis.json` is a reference manifest and must never be used as `genesis.file`.\n\n",
            "## Start after provisioning\n\n",
            "```bash\n",
            "{start_command}\n",
            "```\n",
        ),
        profile = profile,
        chain_id = chain_id,
        public_key = public_key,
        config = config_path.display(),
        genesis = genesis_path.display(),
        profile_prerequisites = profile_prerequisites,
        start_command = start_command,
    );
    fs::write(path, rendered)
        .wrap_err_with(|| format!("failed to write wizard guide to {}", path.display()))
}
fn load_config_template(
    answers: &Answers,
    keypair: &KeyPair,
    soranet_transport_keypair: &KeyPair,
    streaming_identity_keypair: &KeyPair,
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(TomlValue, String)> {
    let defaults = ProfileDefaults::for_profile(answers.profile);
    if let Some(path) = defaults.config_template {
        let config_template_path = resolve_wizard_source_path(path);
        let raw = fs::read_to_string(&config_template_path).wrap_err_with(|| {
            format!(
                "failed to read config template at {}",
                config_template_path.display()
            )
        })?;
        let mut value: TomlValue = toml::from_str(&raw).wrap_err_with(|| {
            format!(
                "failed to parse config template at {}",
                config_template_path.display()
            )
        })?;
        ensure_trusted_peer_list(
            &mut value,
            keypair,
            &answers.trusted_peers,
            &answers.p2p_host,
            answers.p2p_port,
            trusted_pops,
        )?;
        return Ok((
            value,
            resolve_wizard_source_path(defaults.genesis_template)
                .to_string_lossy()
                .into_owned(),
        ));
    }
    let config = build_vanilla_config(
        &answers.chain,
        keypair,
        soranet_transport_keypair,
        streaming_identity_keypair,
        &answers.p2p_host,
        answers.p2p_port,
        &answers.torii_host,
        answers.torii_port,
        &answers.trusted_peers,
        trusted_pops,
    )?;
    Ok((
        config,
        resolve_wizard_source_path(defaults.genesis_template)
            .to_string_lossy()
            .into_owned(),
    ))
}
fn resolve_wizard_source_path(path: &str) -> PathBuf {
    let direct = PathBuf::from(path);
    if direct.is_absolute() || direct.exists() {
        return direct;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(direct)
}
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
fn apply_overrides(
    config: &mut TomlValue,
    answers: &Answers,
    keypair: &KeyPair,
    soranet_transport_keypair: &KeyPair,
    streaming_identity_keypair: &KeyPair,
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<()> {
    set_string(config, "chain", &answers.chain);
    set_string(config, "public_key", &keypair.public_key().to_string());
    set_string(
        config,
        "private_key",
        &ExposedPrivateKey(keypair.private_key().clone()).to_string(),
    );
    set_string(
        config,
        "soranet_transport_public_key",
        &soranet_transport_keypair.public_key().to_string(),
    );
    set_string(
        config,
        "soranet_transport_private_key",
        &ExposedPrivateKey(soranet_transport_keypair.private_key().clone()).to_string(),
    );
    if let TomlValue::Table(root) = config {
        root.remove("private_key_file");
        root.remove("soranet_transport_private_key_file");
    }
    let mut streaming = table(config, "streaming");
    streaming.insert(
        "identity_public_key".into(),
        TomlValue::String(streaming_identity_keypair.public_key().to_string()),
    );
    streaming.insert(
        "identity_private_key".into(),
        TomlValue::String(
            ExposedPrivateKey(streaming_identity_keypair.private_key().clone()).to_string(),
        ),
    );
    streaming.remove("identity_private_key_file");
    set_table(config, "streaming", streaming);
    if answers.profile == Profile::Nexus {
        // `iroha3d --sora` otherwise enables embedded storage. Wizard profiles do not provision
        // the governed gateway-compliance controller required to operate storage safely, so make
        // the operator-authored false explicit and let CLI profile resolution preserve it.
        let mut storage = table(config, "sorafs.storage");
        storage.insert("enabled".into(), TomlValue::Boolean(false));
        set_table(config, "sorafs.storage", storage);
    }
    // trusted peers + ensure self is present
    let mut peers = sanitize_trusted_peers(&answers.trusted_peers)?;
    let self_peer = format!(
        "{}@{}",
        keypair.public_key(),
        addr_literal(&answers.p2p_host, answers.p2p_port)?
    );
    if !trusted_peers_contain_key(&peers, keypair.public_key())? {
        peers.push(self_peer);
    }
    set_array(config, "trusted_peers", peers);
    set_trusted_peers_pop(config, trusted_pops);
    if answers.profile == Profile::Nexus {
        let mut sumeragi = table(config, "sumeragi");
        sumeragi.insert("role".into(), TomlValue::String("observer".into()));
        set_table(config, "sumeragi", sumeragi);
    }
    ensure_sumeragi_body_ingress(config, trusted_pops.len())?;
    let mut network = table(config, "network");
    let network_template = network
        .get("address")
        .and_then(TomlValue::as_str)
        .unwrap_or("");
    network.insert(
        "address".into(),
        TomlValue::String(rewrite_address(
            network_template,
            &answers.p2p_host,
            answers.p2p_port,
        )?),
    );
    let public_network_template = network
        .get("public_address")
        .and_then(TomlValue::as_str)
        .unwrap_or("");
    network.insert(
        "public_address".into(),
        TomlValue::String(rewrite_address(
            public_network_template,
            &answers.p2p_host,
            answers.p2p_port,
        )?),
    );
    let relay_hub_addresses = answers
        .relay_hub_addresses
        .iter()
        .map(|address| canonical_addr_literal(address))
        .collect::<Result<Vec<_>>>()?;
    match answers.relay_mode {
        RelayMode::Disabled => {
            network.remove("relay_mode");
            network.remove("relay_hub_address");
            network.remove("relay_hub_addresses");
        }
        RelayMode::Hub => {
            network.insert("relay_mode".into(), TomlValue::String("hub".to_owned()));
            network.remove("relay_hub_address");
            network.remove("relay_hub_addresses");
        }
        RelayMode::Spoke => {
            network.insert("relay_mode".into(), TomlValue::String("spoke".to_owned()));
            network.remove("relay_hub_address");
            network.insert(
                "relay_hub_addresses".into(),
                TomlValue::Array(
                    relay_hub_addresses
                        .iter()
                        .cloned()
                        .map(TomlValue::String)
                        .collect(),
                ),
            );
        }
        RelayMode::Assist => {
            network.insert("relay_mode".into(), TomlValue::String("assist".to_owned()));
            network.remove("relay_hub_address");
            network.insert(
                "relay_hub_addresses".into(),
                TomlValue::Array(
                    relay_hub_addresses
                        .iter()
                        .cloned()
                        .map(TomlValue::String)
                        .collect(),
                ),
            );
        }
    }
    set_table(config, "network", network);
    let mut torii = table(config, "torii");
    let torii_template = torii
        .get("address")
        .and_then(TomlValue::as_str)
        .unwrap_or("");
    torii.insert(
        "address".into(),
        TomlValue::String(rewrite_address(
            torii_template,
            &answers.torii_host,
            answers.torii_port,
        )?),
    );
    set_table(config, "torii", torii);
    let mut genesis = table(config, "genesis");
    if answers.profile == Profile::Local {
        genesis.insert(
            "public_key".into(),
            TomlValue::String(keypair.public_key().to_string()),
        );
    }
    genesis.insert(
        "file".into(),
        TomlValue::String("genesis.signed.nrt".to_owned()),
    );
    genesis.remove("expected_hash");
    genesis.insert(
        "expected_hash_file".into(),
        TomlValue::String("genesis.expected_hash".to_owned()),
    );
    set_table(config, "genesis", genesis);
    Ok(())
}
fn load_and_patch_genesis(template_path: &str, chain: &str) -> Result<JsonValue> {
    let raw = read_genesis_manifest_bytes(Path::new(template_path))
        .wrap_err_with(|| format!("failed to read bounded genesis template at {template_path}"))?;
    let mut genesis: JsonValue = json::from_slice(&raw)
        .wrap_err_with(|| format!("failed to parse genesis template at {template_path}"))?;
    drop(raw);
    if let Some(chain_slot) = genesis.get_mut("chain") {
        *chain_slot = JsonValue::String(chain.to_string());
    } else if let Some(root) = genesis.as_object_mut() {
        root.insert("chain".to_owned(), JsonValue::String(chain.to_string()));
    }
    Ok(genesis)
}
fn set_string(config: &mut TomlValue, key: &str, value: &str) {
    if let TomlValue::Table(root) = config {
        root.insert(key.to_owned(), TomlValue::String(value.to_owned()));
    }
}
fn set_array(config: &mut TomlValue, key: &str, values: Vec<String>) {
    if let TomlValue::Table(root) = config {
        root.insert(
            key.to_owned(),
            TomlValue::Array(values.into_iter().map(TomlValue::String).collect()),
        );
    }
}
fn set_trusted_peers_pop(config: &mut TomlValue, pops: &BTreeMap<PublicKey, Vec<u8>>) {
    if let TomlValue::Table(root) = config {
        root.insert("trusted_peers_pop".into(), trusted_peers_pop_value(pops));
    }
}
fn trusted_peers_pop_value(pops: &BTreeMap<PublicKey, Vec<u8>>) -> TomlValue {
    let entries = pops
        .iter()
        .map(|(pk, pop)| {
            let mut entry = TomlTable::new();
            entry.insert("public_key".into(), TomlValue::String(pk.to_string()));
            entry.insert("pop_hex".into(), TomlValue::String(hex::encode(pop)));
            TomlValue::Table(entry)
        })
        .collect();
    TomlValue::Array(entries)
}
#[expect(
    clippy::too_many_lines,
    reason = "the geometry verifier keeps all interdependent queue bounds and mutations in one auditable sequence"
)]
fn ensure_sumeragi_body_ingress(config: &mut TomlValue, validator_roster_len: usize) -> Result<()> {
    let mut queues = table(config, "sumeragi.queues");
    let command_capacity = sumeragi_queue_capacity(
        &queues,
        "commands",
        defaults::sumeragi::QUEUE_COMMAND_CAPACITY.get(),
    )?;
    let authenticated_non_validator_sources = sumeragi_queue_capacity(
        &queues,
        "authenticated_non_validator_sources",
        defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get(),
    )?;
    let body_source_bytes = sumeragi_queue_capacity(
        &queues,
        "body_source_bytes",
        defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get(),
    )?;
    let configured_bodies = sumeragi_queue_capacity(
        &queues,
        "bodies",
        defaults::sumeragi::QUEUE_BODY_CAPACITY.get(),
    )?;
    let configured_body_bytes = sumeragi_queue_capacity(
        &queues,
        "body_bytes",
        defaults::sumeragi::QUEUE_BODY_BYTES.get(),
    )?;
    let ingress_required_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
        validator_roster_len,
        authenticated_non_validator_sources,
    )
    .ok_or_else(|| {
        eyre!(
            "wizard Sumeragi body-message capacity overflowed for {validator_roster_len} validators and {authenticated_non_validator_sources} authenticated non-validator sources"
        )
    })?;
    let reply_source_capacity = wizard_reply_source_capacity(config)?;
    if authenticated_non_validator_sources > reply_source_capacity {
        return Err(eyre!(
            "wizard Sumeragi authenticated non-validator source capacity {authenticated_non_validator_sources} exceeds the effective network reply-source capacity {reply_source_capacity}"
        ));
    }
    let remote_trusted_peer_count = wizard_remote_trusted_peer_count(config)?;
    if remote_trusted_peer_count > reply_source_capacity {
        return Err(eyre!(
            "wizard trusted-peer full fanout requires {remote_trusted_peer_count} remote connections, above the effective network connection capacity {reply_source_capacity}"
        ));
    }
    let effect_work_capacity =
        (command_capacity / defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR).max(1);
    let exact_output_required_ownership = reply_source_capacity
        .checked_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)
        .ok_or_else(|| {
            eyre!(
                "wizard Sumeragi exact-output ownership capacity overflowed for {reply_source_capacity} reply sources"
            )
        })?;
    let fixed_exact_output_ownership = effect_work_capacity
        .checked_add(defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP)
        .ok_or_else(|| eyre!("wizard Sumeragi exact-output fixed ownership capacity overflowed"))?;
    let exact_output_required_bodies =
        exact_output_required_ownership.saturating_sub(fixed_exact_output_ownership);
    let required_bodies = ingress_required_bodies.max(exact_output_required_bodies);
    let required_body_bytes = actual::sumeragi_v2_body_ingress_required_byte_capacity(
        validator_roster_len,
        authenticated_non_validator_sources,
        body_source_bytes,
    )
    .ok_or_else(|| {
        eyre!(
            "wizard Sumeragi body-byte capacity overflowed for {validator_roster_len} validators, {authenticated_non_validator_sources} authenticated non-validator sources, and {body_source_bytes} bytes per source"
        )
    })?;
    let effective_bodies = configured_bodies.max(required_bodies);
    let shared_ownership = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
        effect_work_capacity,
        effective_bodies,
    )
    .map_err(|error| eyre!("wizard Sumeragi exact-output geometry is invalid: {error}"))?;
    actual::validate_sumeragi_v2_exact_output_geometry(shared_ownership, reply_source_capacity)
        .map_err(|error| eyre!("wizard Sumeragi exact-output geometry is invalid: {error}"))?;
    actual::sumeragi_v2_lifecycle_capacity_geometry(
        validator_roster_len,
        effect_work_capacity,
        effective_bodies,
        authenticated_non_validator_sources,
    )
    .map_err(|error| eyre!("wizard Sumeragi lifecycle capacity geometry is invalid: {error}"))?;
    let bodies_changed = if required_bodies > configured_bodies {
        queues.insert(
            "bodies".into(),
            TomlValue::Integer(i64::try_from(required_bodies).map_err(|_| {
                eyre!(
                    "wizard Sumeragi body-message capacity {required_bodies} exceeds the TOML integer range"
                )
            })?),
        );
        true
    } else {
        false
    };
    let body_bytes_changed = if required_body_bytes > configured_body_bytes {
        queues.insert(
            "body_bytes".into(),
            TomlValue::Integer(i64::try_from(required_body_bytes).map_err(|_| {
                eyre!(
                    "wizard Sumeragi body-byte capacity {required_body_bytes} exceeds the TOML integer range"
                )
            })?),
        );
        true
    } else {
        false
    };
    if bodies_changed || body_bytes_changed {
        set_table(config, "sumeragi.queues", queues);
    }
    Ok(())
}
fn wizard_reply_source_capacity(config: &TomlValue) -> Result<usize> {
    let network = table(config, "network");
    if let Some(value) = network.get("max_total_connections") {
        let value = value.as_integer().ok_or_else(|| {
            eyre!("wizard template network.max_total_connections must be an integer")
        })?;
        return usize::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                eyre!("wizard template network.max_total_connections must be greater than zero")
            });
    }
    let lane_profile = network
        .get("lane_profile")
        .and_then(TomlValue::as_str)
        .unwrap_or(defaults::network::lane_profile::DEFAULT);
    let lane_profile = actual::LaneProfile::from_label(lane_profile);
    Ok(lane_profile
        .derived_limits()
        .max_total_connections
        .map_or_else(
            || lane_profile.defaults().max_total_connections,
            NonZeroUsize::get,
        ))
}
fn wizard_remote_trusted_peer_count(config: &TomlValue) -> Result<usize> {
    let root = config
        .as_table()
        .ok_or_else(|| eyre!("wizard template root must be a table"))?;
    let local_public_key = root
        .get("public_key")
        .and_then(TomlValue::as_str)
        .ok_or_else(|| eyre!("wizard template public_key must be a string"))?;
    let trusted_peers = root
        .get("trusted_peers")
        .and_then(TomlValue::as_array)
        .ok_or_else(|| eyre!("wizard template trusted_peers must be an array"))?;
    trusted_peers.iter().try_fold(0_usize, |count, peer| {
        let peer = peer
            .as_str()
            .ok_or_else(|| eyre!("wizard template trusted_peers entries must be strings"))?;
        let (public_key, _) = peer.split_once('@').ok_or_else(|| {
            eyre!("wizard template trusted peer `{peer}` must use public_key@address syntax")
        })?;
        if public_key == local_public_key {
            Ok(count)
        } else {
            count
                .checked_add(1)
                .ok_or_else(|| eyre!("wizard trusted-peer remote connection count overflowed"))
        }
    })
}
fn sumeragi_queue_capacity(
    queues: &TomlTable,
    field: &'static str,
    default: usize,
) -> Result<usize> {
    let Some(value) = queues.get(field) else {
        return Ok(default);
    };
    let value = value
        .as_integer()
        .ok_or_else(|| eyre!("wizard template sumeragi.queues.{field} must be an integer"))?;
    let value = usize::try_from(value)
        .map_err(|_| eyre!("wizard template sumeragi.queues.{field} must be greater than zero"))?;
    if value == 0 {
        return Err(eyre!(
            "wizard template sumeragi.queues.{field} must be greater than zero"
        ));
    }
    Ok(value)
}
fn table(config: &TomlValue, path: &str) -> TomlTable {
    let mut table = TomlTable::new();
    let mut current = config;
    for segment in path.split('.') {
        if let TomlValue::Table(child) = current {
            if let Some(next) = child.get(segment) {
                current = next;
            } else {
                return table;
            }
        }
    }
    if let TomlValue::Table(existing) = current {
        table = existing.clone();
    }
    table
}
fn set_table(config: &mut TomlValue, path: &str, table: TomlTable) {
    let mut segments = path.split('.').collect::<Vec<_>>();
    if segments.is_empty() {
        return;
    }
    if let TomlValue::Table(root) = config {
        let last = segments.pop().expect("at least one segment");
        let mut cursor = root;
        for segment in segments {
            cursor = cursor
                .entry(segment.to_owned())
                .or_insert_with(|| TomlValue::Table(TomlTable::new()))
                .as_table_mut()
                .expect("table at path");
        }
        cursor.insert(last.to_owned(), TomlValue::Table(table));
    }
}
fn ensure_trusted_peer_list(
    config: &mut TomlValue,
    keypair: &KeyPair,
    peers: &[String],
    host: &str,
    p2p_port: u16,
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<()> {
    let mut list = sanitize_trusted_peers(peers)?;
    let self_peer = format!("{}@{}", keypair.public_key(), addr_literal(host, p2p_port)?);
    if !trusted_peers_contain_key(&list, keypair.public_key())? {
        list.push(self_peer);
    }
    set_array(config, "trusted_peers", list);
    set_trusted_peers_pop(config, trusted_pops);
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn build_vanilla_config(
    chain: &str,
    keypair: &KeyPair,
    soranet_transport_keypair: &KeyPair,
    streaming_identity_keypair: &KeyPair,
    p2p_host: &str,
    p2p_port: u16,
    torii_host: &str,
    torii_port: u16,
    peers: &[String],
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<TomlValue> {
    let mut root = TomlTable::new();
    root.insert("chain".into(), TomlValue::String(chain.to_owned()));
    root.insert(
        "public_key".into(),
        TomlValue::String(keypair.public_key().to_string()),
    );
    root.insert(
        "private_key".into(),
        TomlValue::String(ExposedPrivateKey(keypair.private_key().clone()).to_string()),
    );
    root.insert(
        "soranet_transport_public_key".into(),
        TomlValue::String(soranet_transport_keypair.public_key().to_string()),
    );
    root.insert(
        "soranet_transport_private_key".into(),
        TomlValue::String(
            ExposedPrivateKey(soranet_transport_keypair.private_key().clone()).to_string(),
        ),
    );
    let mut streaming = TomlTable::new();
    streaming.insert(
        "identity_public_key".into(),
        TomlValue::String(streaming_identity_keypair.public_key().to_string()),
    );
    streaming.insert(
        "identity_private_key".into(),
        TomlValue::String(
            ExposedPrivateKey(streaming_identity_keypair.private_key().clone()).to_string(),
        ),
    );
    root.insert("streaming".into(), TomlValue::Table(streaming));
    root.insert(
        "trusted_peers".into(),
        TomlValue::Array(
            sanitize_trusted_peers(peers)?
                .into_iter()
                .map(TomlValue::String)
                .collect(),
        ),
    );
    root.insert(
        "trusted_peers_pop".into(),
        trusted_peers_pop_value(trusted_pops),
    );
    let mut network = TomlTable::new();
    network.insert(
        "address".into(),
        TomlValue::String(addr_literal(p2p_host, p2p_port)?),
    );
    network.insert(
        "public_address".into(),
        TomlValue::String(addr_literal(p2p_host, p2p_port)?),
    );
    root.insert("network".into(), TomlValue::Table(network));
    let mut torii = TomlTable::new();
    torii.insert(
        "address".into(),
        TomlValue::String(addr_literal(torii_host, torii_port)?),
    );
    root.insert("torii".into(), TomlValue::Table(torii));
    let mut genesis = TomlTable::new();
    genesis.insert(
        "public_key".into(),
        TomlValue::String(keypair.public_key().to_string()),
    );
    genesis.insert(
        "file".into(),
        TomlValue::String("genesis.signed.nrt".to_owned()),
    );
    genesis.insert(
        "expected_hash_file".into(),
        TomlValue::String("genesis.expected_hash".to_owned()),
    );
    root.insert("genesis".into(), TomlValue::Table(genesis));
    let mut nexus = TomlTable::new();
    nexus.insert("lane_count".into(), TomlValue::Integer(1));
    root.insert("nexus".into(), TomlValue::Table(nexus));
    Ok(TomlValue::Table(root))
}
/// Recompute the canonical address literal after overriding its host and port.
fn rewrite_address(_template: &str, host: &str, port: u16) -> Result<String> {
    addr_literal(host, port)
}
/// Render a host and port as a checksummed canonical socket-address literal.
fn addr_literal(host: &str, port: u16) -> Result<String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return Err(eyre!("address host must not be empty"));
    }
    let has_prefix = trimmed.starts_with('[');
    let has_suffix = trimmed.ends_with(']');
    if has_prefix != has_suffix {
        return Err(eyre!("address host has unmatched '[' or ']': `{host}`"));
    }
    let unbracketed = if has_prefix && trimmed.len() >= 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    if unbracketed.is_empty() {
        return Err(eyre!("address host must not be empty"));
    }
    let address = if let Ok(ipv4) = unbracketed.parse::<Ipv4Addr>() {
        SocketAddr::from((ipv4.octets(), port))
    } else if let Ok(ipv6) = unbracketed.parse::<Ipv6Addr>() {
        SocketAddr::from((ipv6.segments(), port))
    } else {
        if unbracketed.contains(':') {
            return Err(eyre!(
                "address host must be a host name or IP literal without a port: `{host}`"
            ));
        }
        SocketAddr::Host(SocketAddrHost {
            host: unbracketed.to_ascii_lowercase().into(),
            port,
        })
    };
    Ok(address.to_literal())
}
/// Parse either a plain socket address or an existing canonical literal and render it canonically.
fn canonical_addr_literal(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(eyre!("socket address must not be empty"));
    }
    let address = if trimmed.starts_with("addr:") {
        let encoded = json::to_string(&trimmed.to_owned())
            .wrap_err("failed to encode socket address literal for validation")?;
        json::from_str::<SocketAddr>(&encoded)
            .wrap_err_with(|| format!("invalid canonical socket address literal `{raw}`"))?
    } else {
        trimmed
            .parse::<SocketAddr>()
            .wrap_err_with(|| format!("invalid socket address `{raw}`"))?
    };
    Ok(address.to_literal())
}
/// Validate trusted-peer entries and render every supplied address canonically.
fn sanitize_trusted_peers(peers: &[String]) -> Result<Vec<String>> {
    let mut normalized_by_key = BTreeMap::<PublicKey, String>::new();
    let mut normalized = Vec::with_capacity(peers.len());
    for entry in peers {
        let peer = Peer::from_str(entry)
            .wrap_err_with(|| format!("invalid trusted peer entry: {entry}"))?;
        let rendered = if entry.contains('@') {
            format!("{}@{}", peer.id().public_key(), peer.address().to_literal())
        } else {
            peer.id().public_key().to_string()
        };
        let public_key = peer.id().public_key().clone();
        if let Some(existing) = normalized_by_key.get(&public_key) {
            if existing == &rendered {
                continue;
            }
            return Err(eyre!(
                "trusted peer public key {public_key} has conflicting entries `{existing}` and `{rendered}`"
            ));
        }
        normalized_by_key.insert(public_key, rendered.clone());
        normalized.push(rendered);
    }
    Ok(normalized)
}
fn trusted_peers_contain_key(peers: &[String], key: &PublicKey) -> Result<bool> {
    for entry in peers {
        let peer = Peer::from_str(entry)
            .wrap_err_with(|| format!("invalid trusted peer entry: {entry}"))?;
        if peer.id().public_key() == key {
            return Ok(true);
        }
    }
    Ok(false)
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::base::toml::TomlSource;
    fn checked_wizard_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("wizard BLS fixture key generation should succeed")
    }
    fn checked_wizard_transport_keypair() -> KeyPair {
        KeyPair::try_from_seed(
            b"iroha:kagami:wizard:test:soranet-transport:v1".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("wizard SoraNet transport fixture key generation should succeed")
    }
    fn checked_wizard_streaming_keypair() -> KeyPair {
        KeyPair::try_from_seed(
            b"iroha:kagami:wizard:test:streaming-identity:v1".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("wizard streaming identity fixture key generation should succeed")
    }
    #[test]
    fn wizard_fixture_key_generation_preserves_bls_algorithm() {
        assert_eq!(
            checked_wizard_bls_keypair().public_key().algorithm(),
            Algorithm::BlsNormal
        );
        assert_eq!(
            checked_wizard_transport_keypair().algorithm(),
            Algorithm::Ed25519
        );
        assert_eq!(
            checked_wizard_streaming_keypair().algorithm(),
            Algorithm::Ed25519
        );
        assert_ne!(
            checked_wizard_transport_keypair().public_key(),
            checked_wizard_streaming_keypair().public_key(),
            "streaming and SoraNet transport fixture identities must be domain-separated"
        );
    }
    #[test]
    fn wizard_non_interactive_defaults_to_local_profile() {
        let args = Args {
            profile: None,
            output_dir: PathBuf::from("out"),
            non_interactive: true,
            chain_id: None,
            p2p_host: None,
            p2p_port: None,
            torii_host: None,
            torii_port: None,
            relay_mode: None,
            relay_hub_addresses: Vec::new(),
            trusted_peers: None,
            trusted_peers_pop: None,
        };
        assert_eq!(
            resolve_profile(&args).expect("non-interactive profile resolution"),
            Profile::Local
        );
    }
    #[test]
    fn wizard_profiles_exclude_public_taira_onboarding() {
        assert_eq!(
            <Profile as clap::ValueEnum>::value_variants(),
            &[Profile::Local, Profile::Nexus]
        );
        assert!(<Profile as clap::ValueEnum>::from_str("taira", false).is_err());
    }
    #[test]
    fn rewrite_address_recomputes_fingerprint() {
        let rewritten = rewrite_address("addr:0.0.0.0:1337#BF18", "1.2.3.4", 9999)
            .expect("rewritten address is valid");
        assert_eq!(
            rewritten,
            addr_literal("1.2.3.4", 9999).expect("expected address is valid")
        );
        assert_ne!(rewritten, "addr:1.2.3.4:9999#BF18");
    }
    #[test]
    fn rewrite_address_plain() {
        assert_eq!(
            rewrite_address("0.0.0.0:8080", "10.0.0.5", 18100).expect("rewritten address is valid"),
            addr_literal("10.0.0.5", 18100).expect("expected address is valid")
        );
    }
    #[test]
    fn trusted_peer_sanitizer_collapses_exact_duplicates_and_rejects_conflicts() {
        let public_key = checked_wizard_bls_keypair().public_key().clone();
        let plain = format!("{public_key}@example.com:1337");
        let canonical = format!(
            "{public_key}@{}",
            addr_literal("example.com", 1337).expect("canonical fixture address")
        );
        assert_eq!(
            sanitize_trusted_peers(&[plain.clone(), canonical])
                .expect("equivalent peer entries should normalize"),
            vec![format!(
                "{public_key}@{}",
                addr_literal("example.com", 1337).expect("canonical fixture address")
            )]
        );
        let conflicting = format!("{public_key}@example.com:1338");
        let error = sanitize_trusted_peers(&[plain, conflicting])
            .expect_err("one public key must not map to conflicting addresses");
        assert!(
            error.to_string().contains("conflicting entries"),
            "unexpected conflict diagnostic: {error:?}"
        );
    }
    #[test]
    fn vanilla_config_has_minimal_sections() {
        let kp = checked_wizard_bls_keypair();
        let transport_kp = checked_wizard_transport_keypair();
        let streaming_kp = checked_wizard_streaming_keypair();
        let pop = bls_normal_pop_prove(kp.private_key()).expect("pop");
        let mut pops = BTreeMap::new();
        pops.insert(kp.public_key().clone(), pop);
        let peer = format!("{}@localhost:1337", kp.public_key());
        let config = build_vanilla_config(
            "chain-x",
            &kp,
            &transport_kp,
            &streaming_kp,
            "localhost",
            1337,
            "localhost",
            8080,
            &[peer],
            &pops,
        )
        .expect("build vanilla wizard config");
        let table = config.as_table().expect("table");
        assert_eq!(
            table.get("chain").and_then(TomlValue::as_str),
            Some("chain-x")
        );
        assert!(table.get("network").is_some());
        assert!(table.get("torii").is_some());
        assert!(table.get("genesis").is_some());
        assert_eq!(
            table
                .get("genesis")
                .and_then(TomlValue::as_table)
                .and_then(|genesis| genesis.get("expected_hash_file"))
                .and_then(TomlValue::as_str),
            Some("genesis.expected_hash"),
            "wizard output must require the operator-provided authoritative hash file"
        );
        assert_eq!(
            table
                .get("genesis")
                .and_then(TomlValue::as_table)
                .and_then(|genesis| genesis.get("file"))
                .and_then(TomlValue::as_str),
            Some("genesis.signed.nrt"),
            "the reference JSON manifest must never be configured as a signed genesis block"
        );
        assert!(table.get("trusted_peers").is_some());
        assert!(table.get("trusted_peers_pop").is_some());
        let nexus = table
            .get("nexus")
            .and_then(TomlValue::as_table)
            .expect("nexus table");
        assert_eq!(
            nexus.get("lane_count").and_then(TomlValue::as_integer),
            Some(1)
        );
        assert!(
            !nexus.contains_key("enabled"),
            "wizard output must not expose the retired Nexus availability switch"
        );
        assert_eq!(
            table
                .get("soranet_transport_public_key")
                .and_then(TomlValue::as_str),
            Some(transport_kp.public_key().to_string().as_str())
        );
        assert_eq!(
            table
                .get("soranet_transport_private_key")
                .and_then(TomlValue::as_str),
            Some(
                ExposedPrivateKey(transport_kp.private_key().clone())
                    .to_string()
                    .as_str()
            )
        );
        assert_ne!(transport_kp.public_key(), kp.public_key());
        assert_ne!(streaming_kp.public_key(), transport_kp.public_key());
    }
    #[test]
    fn wizard_handoff_never_treats_reference_manifest_as_signed_genesis() {
        let directory = tempfile::tempdir().expect("wizard README directory");
        let path = directory.path().join("README.md");
        write_wizard_readme(
            &path,
            Profile::Nexus,
            ProfileDefaults::for_profile(Profile::Nexus).chain,
            checked_wizard_bls_keypair().public_key(),
            Path::new("config.toml"),
            Path::new("genesis.json"),
            "iroha3d --sora --config config.toml",
        )
        .expect("write wizard handoff");
        let rendered = fs::read_to_string(path).expect("read wizard handoff");
        assert!(rendered.contains("genesis.signed.nrt"));
        assert!(rendered.contains("genesis.expected_hash"));
        assert!(rendered.contains("operator-authenticated validator roster"));
        assert!(rendered.contains("iroha3d --sora --config config.toml"));
        assert!(!rendered.contains("--genesis-manifest-json"));
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the geometry scenario proves both required scaling and preservation of larger authored capacities on one config"
    )]
    fn wizard_scales_body_ingress_for_seven_validators_without_shrinking_authored_capacity() {
        let mut local_keypair = None;
        let mut peers = Vec::new();
        let mut pops = BTreeMap::new();
        for index in 0_u8..7 {
            let keypair =
                KeyPair::try_from_seed(vec![0xa0_u8.wrapping_add(index); 32], Algorithm::BlsNormal)
                    .expect("derive deterministic wizard validator fixture");
            let public_key = keypair.public_key().clone();
            pops.insert(
                public_key.clone(),
                bls_normal_pop_prove(keypair.private_key()).expect("derive validator PoP"),
            );
            peers.push(format!(
                "{public_key}@127.0.0.1:{}",
                1337_u16 + u16::from(index)
            ));
            if index == 0 {
                local_keypair = Some(keypair);
            }
        }
        let keypair = local_keypair.expect("fixture includes the local validator");
        let transport_keypair = checked_wizard_transport_keypair();
        let streaming_keypair = checked_wizard_streaming_keypair();
        let mut config = build_vanilla_config(
            "chain-x",
            &keypair,
            &transport_keypair,
            &streaming_keypair,
            "127.0.0.1",
            1337,
            "127.0.0.1",
            8080,
            &peers,
            &pops,
        )
        .expect("build vanilla wizard config");
        let authenticated_non_validator_sources = 5_usize;
        let body_source_bytes = defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get();
        let mut queues = TomlTable::new();
        queues.insert(
            "authenticated_non_validator_sources".into(),
            TomlValue::Integer(
                i64::try_from(authenticated_non_validator_sources).expect("fixture fits TOML"),
            ),
        );
        queues.insert(
            "body_source_bytes".into(),
            TomlValue::Integer(i64::try_from(body_source_bytes).expect("fixture fits TOML")),
        );
        queues.insert("bodies".into(), TomlValue::Integer(1));
        queues.insert("body_bytes".into(), TomlValue::Integer(1));
        set_table(&mut config, "sumeragi.queues", queues);
        let answers = Answers {
            profile: Profile::Local,
            chain: "chain-x".to_owned(),
            p2p_host: "127.0.0.1".to_owned(),
            p2p_port: 1337,
            torii_host: "127.0.0.1".to_owned(),
            torii_port: 8080,
            trusted_peers: peers,
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            output_dir: PathBuf::from("out"),
        };
        apply_overrides(
            &mut config,
            &answers,
            &keypair,
            &transport_keypair,
            &streaming_keypair,
            &pops,
        )
        .expect("scale wizard queue capacity");
        let required = actual::sumeragi_v2_body_ingress_required_byte_capacity(
            7,
            authenticated_non_validator_sources,
            body_source_bytes,
        )
        .expect("fixture capacity is representable");
        let ingress_required_bodies = actual::sumeragi_v2_body_ingress_required_message_capacity(
            7,
            authenticated_non_validator_sources,
        )
        .expect("fixture message capacity is representable");
        let reply_source_capacity = wizard_reply_source_capacity(&config)
            .expect("fixture network reply-source capacity is representable");
        let effect_work_capacity = (defaults::sumeragi::QUEUE_COMMAND_CAPACITY.get()
            / defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR)
            .max(1);
        let exact_output_required_bodies = reply_source_capacity
            .checked_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)
            .and_then(|capacity| {
                capacity
                    .checked_sub(effect_work_capacity + defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP)
            })
            .expect("fixture exact-output capacity is representable");
        let required_bodies = ingress_required_bodies.max(exact_output_required_bodies);
        assert_eq!(
            table(&config, "sumeragi.queues")
                .get("bodies")
                .and_then(TomlValue::as_integer),
            Some(i64::try_from(required_bodies).expect("fixture fits TOML")),
        );
        assert_eq!(
            table(&config, "sumeragi.queues")
                .get("body_bytes")
                .and_then(TomlValue::as_integer),
            Some(i64::try_from(required).expect("fixture fits TOML")),
        );
        let shared_ownership = actual::sumeragi_v2_exact_output_shared_ownership_capacity(
            effect_work_capacity,
            required_bodies,
        )
        .expect("fixture shared ownership is representable");
        actual::validate_sumeragi_v2_exact_output_geometry(shared_ownership, reply_source_capacity)
            .expect("wizard output must satisfy exact-output geometry");
        actual::sumeragi_v2_lifecycle_capacity_geometry(
            7,
            effect_work_capacity,
            required_bodies,
            authenticated_non_validator_sources,
        )
        .expect("wizard output must satisfy lifecycle geometry");
        let authored = required + body_source_bytes;
        let authored_bodies = required_bodies + 7;
        let mut queues = table(&config, "sumeragi.queues");
        queues.insert(
            "bodies".into(),
            TomlValue::Integer(i64::try_from(authored_bodies).expect("fixture fits TOML")),
        );
        queues.insert(
            "body_bytes".into(),
            TomlValue::Integer(i64::try_from(authored).expect("fixture fits TOML")),
        );
        set_table(&mut config, "sumeragi.queues", queues);
        apply_overrides(
            &mut config,
            &answers,
            &keypair,
            &transport_keypair,
            &streaming_keypair,
            &pops,
        )
        .expect("preserve larger authored queue capacity");
        assert_eq!(
            table(&config, "sumeragi.queues")
                .get("bodies")
                .and_then(TomlValue::as_integer),
            Some(i64::try_from(authored_bodies).expect("fixture fits TOML")),
        );
        assert_eq!(
            table(&config, "sumeragi.queues")
                .get("body_bytes")
                .and_then(TomlValue::as_integer),
            Some(i64::try_from(authored).expect("fixture fits TOML")),
        );
        let mut parse_table = config.as_table().expect("wizard config table").clone();
        let genesis = parse_table
            .get_mut("genesis")
            .and_then(TomlValue::as_table_mut)
            .expect("wizard genesis table");
        genesis.remove("expected_hash_file");
        genesis.insert(
            "expected_hash".into(),
            TomlValue::String(
                "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                    .to_owned(),
            ),
        );
        actual::Root::from_toml_source(TomlSource::inline(parse_table))
            .expect("wizard queue scaling must pass canonical config admission");
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the scenario validates the complete generated Nexus template through both canonical admission paths"
    )]
    fn wizard_nexus_profile_template_passes_canonical_and_cli_profile_admission() {
        let keypair = checked_wizard_bls_keypair();
        let transport_keypair = checked_wizard_transport_keypair();
        let streaming_keypair = checked_wizard_streaming_keypair();
        let mut validator_peers = Vec::new();
        let mut pops = BTreeMap::new();
        for index in 0_u8..4 {
            let validator =
                KeyPair::try_from_seed(vec![0xd0_u8.wrapping_add(index); 32], Algorithm::BlsNormal)
                    .expect("derive authoritative validator fixture");
            pops.insert(
                validator.public_key().clone(),
                bls_normal_pop_prove(validator.private_key())
                    .expect("derive authoritative validator PoP"),
            );
            validator_peers.push(validator.public_key().clone());
        }
        {
            let profile = Profile::Nexus;
            let defaults = ProfileDefaults::for_profile(profile);
            let answers = Answers {
                profile,
                chain: defaults.chain.to_owned(),
                p2p_host: defaults.host.to_owned(),
                p2p_port: defaults.p2p_port,
                torii_host: defaults.host.to_owned(),
                torii_port: defaults.torii_port,
                trusted_peers: validator_peers
                    .iter()
                    .enumerate()
                    .map(|(index, public_key)| {
                        format!(
                            "{public_key}@{}:{}",
                            defaults.host,
                            defaults.p2p_port
                                + u16::try_from(index).expect("fixture port offset fits")
                        )
                    })
                    .collect(),
                relay_mode: RelayMode::Disabled,
                relay_hub_addresses: Vec::new(),
                output_dir: PathBuf::from("out"),
            };
            let (mut config, _) = load_config_template(
                &answers,
                &keypair,
                &transport_keypair,
                &streaming_keypair,
                &pops,
            )
            .unwrap_or_else(|error| panic!("load {profile} wizard template: {error:?}"));
            apply_overrides(
                &mut config,
                &answers,
                &keypair,
                &transport_keypair,
                &streaming_keypair,
                &pops,
            )
            .unwrap_or_else(|error| panic!("apply {profile} wizard overrides: {error:?}"));
            let root = config.as_table_mut().expect("wizard config table");
            assert!(!root.contains_key("private_key_file"));
            assert!(!root.contains_key("soranet_transport_private_key_file"));
            assert_eq!(
                root.get("sumeragi")
                    .and_then(TomlValue::as_table)
                    .and_then(|sumeragi| sumeragi.get("role"))
                    .and_then(TomlValue::as_str),
                Some("observer")
            );
            let configured_pops = root
                .get("trusted_peers_pop")
                .and_then(TomlValue::as_array)
                .expect("wizard trusted-peer PoP roster");
            assert_eq!(configured_pops.len(), 4);
            assert!(configured_pops.iter().all(|entry| {
                entry
                    .as_table()
                    .and_then(|entry| entry.get("public_key"))
                    .and_then(TomlValue::as_str)
                    != Some(keypair.public_key().to_string().as_str())
            }));
            let streaming = root
                .get("streaming")
                .and_then(TomlValue::as_table)
                .expect("wizard streaming table");
            assert!(!streaming.contains_key("identity_private_key_file"));
            assert_eq!(
                streaming
                    .get("identity_public_key")
                    .and_then(TomlValue::as_str),
                Some(streaming_keypair.public_key().to_string().as_str())
            );
            assert_eq!(
                root.get("sorafs")
                    .and_then(TomlValue::as_table)
                    .and_then(|sorafs| sorafs.get("storage"))
                    .and_then(TomlValue::as_table)
                    .and_then(|storage| storage.get("enabled"))
                    .and_then(TomlValue::as_bool),
                Some(false),
                "wizard must explicitly keep embedded storage disabled when `--sora` is applied"
            );
            let genesis = root
                .get_mut("genesis")
                .and_then(TomlValue::as_table_mut)
                .expect("wizard genesis table");
            assert_eq!(
                genesis
                    .get("expected_hash_file")
                    .and_then(TomlValue::as_str),
                Some("genesis.expected_hash")
            );
            genesis.remove("expected_hash_file");
            genesis.insert(
                "expected_hash".into(),
                TomlValue::String(
                    "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                        .to_owned(),
                ),
            );
            let mut actual = actual::Root::from_toml_source(TomlSource::inline(root.clone()))
                .unwrap_or_else(|error| panic!("{profile} wizard config admission: {error:?}"));
            let configured_storage_enabled = actual.torii.sorafs_storage.enabled;
            actual.apply_sora_profile();
            // Mirror `iroha3d` CLI resolution: an explicit authored value survives profile
            // defaults. The raw assertion above proves the wizard emitted that explicit value.
            actual.torii.sorafs_storage.enabled = configured_storage_enabled;
            assert!(!actual.torii.sorafs_storage.enabled);
        }
    }
    #[test]
    fn genesis_chain_is_patched() {
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        fs::write(tmp.path(), r#"{"chain":"old","transactions":[]}"#).expect("write");
        let path = tmp.path().display().to_string();
        let genesis = load_and_patch_genesis(&path, "new-chain").expect("genesis");
        assert_eq!(
            genesis
                .get("chain")
                .and_then(JsonValue::as_str)
                .unwrap_or(""),
            "new-chain"
        );
    }
    #[test]
    fn trusted_peers_pop_missing_non_interactive_marks_peer_observer() {
        let keypair = checked_wizard_bls_keypair();
        let other = checked_wizard_bls_keypair();
        let answers = Answers {
            profile: Profile::Local,
            chain: "chain-x".to_string(),
            p2p_host: "127.0.0.1".to_string(),
            p2p_port: 1337,
            torii_host: "127.0.0.1".to_string(),
            torii_port: 8080,
            trusted_peers: vec![format!("{}@127.0.0.1:1338", other.public_key())],
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            output_dir: PathBuf::from("out"),
        };
        let args = Args {
            profile: None,
            output_dir: PathBuf::from("out"),
            non_interactive: true,
            chain_id: None,
            p2p_host: None,
            p2p_port: None,
            torii_host: None,
            torii_port: None,
            relay_mode: None,
            relay_hub_addresses: Vec::new(),
            trusted_peers: None,
            trusted_peers_pop: None,
        };
        let result = resolve_trusted_peers_pop(&args, &answers, &keypair);
        let pops =
            result.expect("missing remote PoPs should leave peers as non-validator trusted peers");
        assert!(
            pops.contains_key(keypair.public_key()),
            "local validator PoP should be generated"
        );
        assert!(
            !pops.contains_key(other.public_key()),
            "remote trusted peer without PoP should not be promoted into the validator roster"
        );
    }
    #[test]
    fn sora_wizard_requires_authoritative_roster_and_keeps_local_peer_observer() {
        let local = checked_wizard_bls_keypair();
        let mut trusted_peers = Vec::new();
        let mut pop_entries = Vec::new();
        for index in 0_u8..4 {
            let validator =
                KeyPair::try_from_seed(vec![0xe0_u8.wrapping_add(index); 32], Algorithm::BlsNormal)
                    .expect("derive authoritative validator fixture");
            trusted_peers.push(format!(
                "{}@127.0.0.1:{}",
                validator.public_key(),
                13_337_u16 + u16::from(index)
            ));
            let pop = bls_normal_pop_prove(validator.private_key())
                .expect("derive authoritative validator PoP");
            pop_entries.push(format!("{}={}", validator.public_key(), hex::encode(pop)));
        }
        let answers = Answers {
            profile: Profile::Nexus,
            chain: ProfileDefaults::for_profile(Profile::Nexus)
                .chain
                .to_owned(),
            p2p_host: "127.0.0.1".to_owned(),
            p2p_port: 1_337,
            torii_host: "127.0.0.1".to_owned(),
            torii_port: 8_080,
            trusted_peers: trusted_peers.clone(),
            relay_mode: RelayMode::Disabled,
            relay_hub_addresses: Vec::new(),
            output_dir: PathBuf::from("out"),
        };
        let mut args = Args {
            profile: Some(Profile::Nexus),
            output_dir: PathBuf::from("out"),
            non_interactive: true,
            chain_id: None,
            p2p_host: None,
            p2p_port: None,
            torii_host: None,
            torii_port: None,
            relay_mode: None,
            relay_hub_addresses: Vec::new(),
            trusted_peers: None,
            trusted_peers_pop: Some(pop_entries.join(",")),
        };
        let error = resolve_trusted_peers_pop(&args, &answers, &local)
            .expect_err("non-interactive onboarding must require an explicit peer roster");
        assert!(error.to_string().contains("--trusted-peers"));
        args.trusted_peers = Some(trusted_peers.join(","));
        let pops = resolve_trusted_peers_pop(&args, &answers, &local)
            .expect("authoritative four-validator roster");
        assert_eq!(pops.len(), 4);
        assert!(!pops.contains_key(local.public_key()));
    }
    #[test]
    fn public_taira_bundle_uses_expected_network_identity() {
        const EXPECTED_TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
        const EXPECTED_TAIRA_CHAIN_DISCRIMINANT: i64 = 369;
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config_path = repo_root.join("configs/soranexus/taira/config.toml");
        let config_text = fs::read_to_string(&config_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", config_path.display()));
        let config: TomlValue = toml::from_str(&config_text)
            .unwrap_or_else(|err| panic!("parse {}: {err}", config_path.display()));
        assert_eq!(
            config.get("chain").and_then(TomlValue::as_str),
            Some(EXPECTED_TAIRA_CHAIN_ID),
            "public Taira config.toml must keep the shipped live chain id"
        );
        assert_eq!(
            config
                .get("chain_discriminant")
                .and_then(TomlValue::as_integer),
            Some(EXPECTED_TAIRA_CHAIN_DISCRIMINANT),
            "public Taira config.toml must keep the shipped address discriminant"
        );
        let genesis_path = repo_root.join("configs/soranexus/taira/genesis.json");
        let genesis_text = fs::read_to_string(&genesis_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", genesis_path.display()));
        let genesis: JsonValue = json::from_str(&genesis_text)
            .unwrap_or_else(|err| panic!("parse {}: {err}", genesis_path.display()));
        assert_eq!(
            genesis.get("chain").and_then(JsonValue::as_str),
            Some(EXPECTED_TAIRA_CHAIN_ID),
            "public Taira genesis.json must match the shipped live chain id"
        );
        assert!(
            config_text.contains("testu"),
            "public Taira config.toml must render testnet i105 literals"
        );
        assert!(
            !config_text.contains("sorau"),
            "public Taira config.toml must not leak mainnet i105 literals"
        );
        assert!(
            genesis_text.contains("testu"),
            "public Taira genesis.json must render testnet i105 literals"
        );
        assert!(
            !genesis_text.contains("sorau"),
            "public Taira genesis.json must not leak mainnet i105 literals"
        );
        let first_tx_instructions = genesis
            .get("transactions")
            .and_then(JsonValue::as_array)
            .and_then(|items| items.first())
            .and_then(|tx| tx.get("instructions"))
            .and_then(JsonValue::as_array)
            .expect("public Taira genesis.json must include bootstrap instructions");
        let xor_asset_definition_id = first_tx_instructions
            .iter()
            .find(|instruction| {
                instruction
                    .get("SetAssetDefinitionAlias")
                    .and_then(|binding| binding.get("alias"))
                    .and_then(JsonValue::as_str)
                    == Some("xor#universal")
            })
            .and_then(|instruction| instruction.get("SetAssetDefinitionAlias"))
            .and_then(|binding| binding.get("asset_definition_id"))
            .and_then(JsonValue::as_str)
            .expect("public Taira genesis.json must bind xor#universal");
        let xor_universal = first_tx_instructions
            .iter()
            .find(|instruction| {
                instruction
                    .get("Register")
                    .and_then(|register| register.get("AssetDefinition"))
                    .and_then(|asset| asset.get("id"))
                    .and_then(JsonValue::as_str)
                    == Some(xor_asset_definition_id)
            })
            .expect("public Taira genesis.json must register the canonical xor asset");
        let confidential_policy = xor_universal
            .get("Register")
            .and_then(|register| register.get("AssetDefinition"))
            .and_then(|asset| asset.get("confidential_policy"));
        assert!(
            confidential_policy.is_none(),
            "asset registration must not bypass canonical confidential verifier activation"
        );
    }
}

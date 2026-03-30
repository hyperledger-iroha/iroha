//! Interactive setup wizard for quickly preparing Iroha/Sora configs.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Context as _, Result, eyre};
use inquire::{Select, Text};
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, KeyPair, PublicKey, bls_normal_pop_prove, bls_normal_pop_verify,
};
use iroha_data_model::peer::PeerId;
use norito::json::{self, Value as JsonValue};
use toml::{Value as TomlValue, value::Table as TomlTable};

use crate::{Outcome, RunArgs, tui};

/// Supported network profiles for the wizard.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Profile {
    /// Vanilla single-lane Iroha 2 style network (no Sora profile needed).
    Iroha2,
    /// Sora Nexus (mainnet).
    Nexus,
    /// Sora Taira (testnet).
    Taira,
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Profile::Iroha2 => write!(f, "Iroha2 (single lane)"),
            Profile::Nexus => write!(f, "Sora Nexus (mainnet)"),
            Profile::Taira => write!(f, "Sora Taira (testnet)"),
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
            Profile::Iroha2 => Self {
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
                trusted_peers: &[
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2@nexus.mof2.sora.org:1337",
                ],
                config_template: Some("configs/soranexus/nexus/config.toml"),
                genesis_template: "configs/soranexus/nexus/genesis.json",
            },
            Profile::Taira => Self {
                chain: "809574f5-fee7-5e69-bfcf-52451e42d50f",
                p2p_port: 1337,
                torii_port: 18080,
                host: "taira-validator-1.sora.org",
                trusted_peers: &[
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2@taira-validator-1.sora.org:1337",
                ],
                config_template: Some("configs/soranexus/taira/config.toml"),
                genesis_template: "configs/soranexus/taira/genesis.json",
            },
        }
    }
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        print_banner();
        let answers = gather_answers(&self)?;
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let trusted_pops = resolve_trusted_peers_pop(&self, &answers, &keypair)?;

        tui::status("Generating config and genesis files");
        let (mut config, genesis_template_path) =
            load_config_template(&answers, &keypair, &trusted_pops)?;
        apply_overrides(&mut config, &answers, &keypair, &trusted_pops)?;
        let genesis = load_and_patch_genesis(&genesis_template_path, &answers.chain)?;

        fs::create_dir_all(&answers.output_dir)
            .wrap_err("failed to create output directory for wizard artefacts")?;
        let config_path = answers.output_dir.join("config.toml");
        let genesis_path = answers.output_dir.join("genesis.json");

        let sorafs_dir = answers.output_dir.join("sorafs_admission");
        if matches!(answers.profile, Profile::Nexus | Profile::Taira) {
            fs::create_dir_all(&sorafs_dir)
                .wrap_err("failed to create sorafs admission directory")?;
        }

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
# # TLS-over-TCP to peers (requires iroha_p2p/p2p_tls):
# tls_enabled = false
# tls_fallback_to_plain = false
# tls_listen_address = "addr:0.0.0.0:1337"
# tls_inbound_only = false
#
# Notes:
# - When p2p_proxy_required=true, p2p_no_proxy must be empty.
# - When p2p_proxy is https:// and p2p_proxy_tls_verify=true, a pinned cert is required.
# - tls_inbound_only=true disables the plaintext P2P listener; inbound connections require TLS.
"#,
        );
        fs::write(&config_path, config_payload)
            .wrap_err_with(|| format!("failed to write config to {}", config_path.display()))?;

        let genesis_payload = json::to_string_pretty(&genesis)
            .wrap_err("failed to serialise genesis after wizard updates")?;
        fs::write(&genesis_path, genesis_payload)
            .wrap_err_with(|| format!("failed to write genesis to {}", genesis_path.display()))?;
        let guide_path = answers.output_dir.join("README.md");
        let next_command = format!(
            "cd {} && irohad {}--config {} --genesis-manifest-json {}",
            answers.output_dir.display(),
            if matches!(answers.profile, Profile::Nexus | Profile::Taira) {
                "--sora "
            } else {
                ""
            },
            config_path.display(),
            genesis_path.display()
        );
        write_wizard_readme(
            &guide_path,
            answers.profile,
            &answers.chain,
            keypair.public_key(),
            &config_path,
            &genesis_path,
            &next_command,
        )?;

        tui::success(format!(
            "Config and genesis ready under {}",
            answers.output_dir.display()
        ));
        writeln!(writer, "profile: {}", answers.profile)?;
        writeln!(writer, "chain_id: {}", answers.chain)?;
        writeln!(writer, "generated_public_key: {}", keypair.public_key())?;
        writeln!(writer, "config: {}", config_path.display())?;
        writeln!(writer, "genesis: {}", genesis_path.display())?;
        writeln!(writer, "guide: {}", guide_path.display())?;
        if matches!(answers.profile, Profile::Nexus | Profile::Taira) {
            writeln!(writer, "sora profile: pass --sora when starting irohad")?;
        }
        writeln!(writer, "next: {next_command}")?;
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
        "Trusted peers (comma separated pubkey@host:port; PoP required)",
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
    let mut peers = sanitize_trusted_peers(&answers.trusted_peers);
    let self_peer = format!(
        "{}@{}",
        keypair.public_key(),
        addr_literal(&answers.p2p_host, answers.p2p_port)
    );
    if !peers.iter().any(|p| p == &self_peer) {
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
    if !pops.contains_key(keypair.public_key()) {
        let pop = bls_normal_pop_prove(keypair.private_key())
            .wrap_err("failed to build PoP for the local keypair")?;
        pops.insert(keypair.public_key().clone(), pop);
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
    if !missing.is_empty() {
        if args.non_interactive {
            return Err(eyre!(
                "trusted_peers_pop missing PoPs for peers: {missing:?}"
            ));
        }
        for pk in missing {
            let pop = prompt_pop_for_peer(&pk)?;
            pops.insert(pk, pop);
        }
    }

    Ok(pops)
}

fn parse_trusted_peer_ids(peers: &[String]) -> Result<Vec<PeerId>> {
    peers
        .iter()
        .map(|entry| {
            PeerId::from_str(entry).wrap_err_with(|| format!("invalid trusted peer entry: {entry}"))
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
        return Ok(Profile::Iroha2);
    }

    Select::new(
        "Which profile do you want to set up?",
        vec![Profile::Iroha2, Profile::Nexus, Profile::Taira],
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
    next_command: &str,
) -> Result<()> {
    let rendered = format!(
        concat!(
            "# Kagami Wizard Output\n\n",
            "- Profile: `{profile}`\n",
            "- Chain ID: `{chain_id}`\n",
            "- Generated public key: `{public_key}`\n",
            "- Config: `{config}`\n",
            "- Genesis: `{genesis}`\n\n",
            "## Next step\n\n",
            "```bash\n",
            "{next_command}\n",
            "```\n",
        ),
        profile = profile,
        chain_id = chain_id,
        public_key = public_key,
        config = config_path.display(),
        genesis = genesis_path.display(),
        next_command = next_command,
    );
    fs::write(path, rendered)
        .wrap_err_with(|| format!("failed to write wizard guide to {}", path.display()))
}

fn load_config_template(
    answers: &Answers,
    keypair: &KeyPair,
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(TomlValue, String)> {
    let defaults = ProfileDefaults::for_profile(answers.profile);
    if let Some(path) = defaults.config_template {
        let raw = fs::read_to_string(path)
            .wrap_err_with(|| format!("failed to read config template at {path}"))?;
        let mut value: TomlValue = toml::from_str(&raw)
            .wrap_err_with(|| format!("failed to parse config template at {path}"))?;
        ensure_trusted_peer_list(
            &mut value,
            keypair,
            &answers.trusted_peers,
            &answers.p2p_host,
            answers.p2p_port,
            trusted_pops,
        );
        return Ok((value, defaults.genesis_template.to_string()));
    }

    let config = build_vanilla_config(
        &answers.chain,
        keypair,
        &answers.p2p_host,
        answers.p2p_port,
        &answers.torii_host,
        answers.torii_port,
        &answers.trusted_peers,
        trusted_pops,
    );
    Ok((config, defaults.genesis_template.to_string()))
}

#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
fn apply_overrides(
    config: &mut TomlValue,
    answers: &Answers,
    keypair: &KeyPair,
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<()> {
    set_string(config, "chain", &answers.chain);
    set_string(config, "public_key", &keypair.public_key().to_string());
    set_string(
        config,
        "private_key",
        &ExposedPrivateKey(keypair.private_key().clone()).to_string(),
    );

    // trusted peers + ensure self is present
    let mut peers = sanitize_trusted_peers(&answers.trusted_peers);
    let self_peer = format!(
        "{}@{}",
        keypair.public_key(),
        addr_literal(&answers.p2p_host, answers.p2p_port)
    );
    if !peers.iter().any(|p| p == &self_peer) {
        peers.push(self_peer);
    }
    set_array(config, "trusted_peers", peers);
    set_trusted_peers_pop(config, trusted_pops);

    let mut network = table(config, "network");
    let network_template = network
        .get("address")
        .and_then(TomlValue::as_str)
        .unwrap_or("");
    network.insert(
        "address".into(),
        TomlValue::String(rewrite_address_with_literal(
            network_template,
            &answers.p2p_host,
            answers.p2p_port,
        )),
    );
    let public_network_template = network
        .get("public_address")
        .and_then(TomlValue::as_str)
        .unwrap_or("");
    network.insert(
        "public_address".into(),
        TomlValue::String(rewrite_address_with_literal(
            public_network_template,
            &answers.p2p_host,
            answers.p2p_port,
        )),
    );
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
                    answers
                        .relay_hub_addresses
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
                    answers
                        .relay_hub_addresses
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
        TomlValue::String(rewrite_address_with_literal(
            torii_template,
            &answers.torii_host,
            answers.torii_port,
        )),
    );

    if matches!(answers.profile, Profile::Nexus | Profile::Taira) {
        let mut sorafs = table(config, "torii.sorafs");
        sorafs.insert(
            "admission_envelopes_dir".into(),
            TomlValue::String("sorafs_admission".into()),
        );
        torii.insert("sorafs".into(), TomlValue::Table(sorafs));
    }
    set_table(config, "torii", torii);

    let mut genesis = table(config, "genesis");
    genesis.insert(
        "public_key".into(),
        TomlValue::String(keypair.public_key().to_string()),
    );
    genesis.insert("file".into(), TomlValue::String("genesis.json".to_owned()));
    set_table(config, "genesis", genesis);

    Ok(())
}

fn load_and_patch_genesis(template_path: &str, chain: &str) -> Result<JsonValue> {
    let raw = fs::read_to_string(template_path)
        .wrap_err_with(|| format!("failed to read genesis template at {template_path}"))?;
    let mut genesis: JsonValue = json::from_str(&raw)
        .wrap_err_with(|| format!("failed to parse genesis template at {template_path}"))?;
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
) {
    let mut list = sanitize_trusted_peers(peers);
    let self_peer = format!("{}@{}", keypair.public_key(), addr_literal(host, p2p_port));
    if !list.iter().any(|p| p == &self_peer) {
        list.push(self_peer);
    }
    set_array(config, "trusted_peers", list);
    set_trusted_peers_pop(config, trusted_pops);
}

#[allow(clippy::too_many_arguments)]
fn build_vanilla_config(
    chain: &str,
    keypair: &KeyPair,
    p2p_host: &str,
    p2p_port: u16,
    torii_host: &str,
    torii_port: u16,
    peers: &[String],
    trusted_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> TomlValue {
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
        "trusted_peers".into(),
        TomlValue::Array(peers.iter().map(|p| TomlValue::String(p.clone())).collect()),
    );
    root.insert(
        "trusted_peers_pop".into(),
        trusted_peers_pop_value(trusted_pops),
    );

    let mut network = TomlTable::new();
    network.insert(
        "address".into(),
        TomlValue::String(addr_literal(p2p_host, p2p_port)),
    );
    network.insert(
        "public_address".into(),
        TomlValue::String(addr_literal(p2p_host, p2p_port)),
    );
    root.insert("network".into(), TomlValue::Table(network));

    let mut torii = TomlTable::new();
    torii.insert(
        "address".into(),
        TomlValue::String(addr_literal(torii_host, torii_port)),
    );
    root.insert("torii".into(), TomlValue::Table(torii));

    let mut genesis = TomlTable::new();
    genesis.insert(
        "public_key".into(),
        TomlValue::String(keypair.public_key().to_string()),
    );
    genesis.insert("file".into(), TomlValue::String("genesis.json".to_owned()));
    root.insert("genesis".into(), TomlValue::Table(genesis));

    let mut nexus = TomlTable::new();
    nexus.insert("enabled".into(), TomlValue::Boolean(false));
    nexus.insert("lane_count".into(), TomlValue::Integer(1));
    root.insert("nexus".into(), TomlValue::Table(nexus));

    TomlValue::Table(root)
}

/// Preserve optional address prefixes/suffixes while overriding host/port.
fn rewrite_address(template: &str, host: &str, port: u16) -> String {
    let (prefix, rest) = template
        .strip_prefix("addr:")
        .map_or(("", template), |stripped| ("addr:", stripped));
    let (_body, suffix) = rest
        .split_once('#')
        .map_or((rest, ""), |(addr, tag)| (addr, tag));
    if suffix.is_empty() {
        format!("{prefix}{host}:{port}")
    } else {
        format!("{prefix}{host}:{port}#{suffix}")
    }
}

/// Ensure address uses `addr:` literal so DNS hosts parse cleanly.
fn rewrite_address_with_literal(template: &str, host: &str, port: u16) -> String {
    let rendered = rewrite_address(template, host, port);
    if rendered.starts_with("addr:") {
        rendered
    } else {
        format!("addr:{rendered}")
    }
}

/// Render a SocketAddr with the `addr:` literal to support DNS hosts.
fn addr_literal(host: &str, port: u16) -> String {
    if host.starts_with("addr:") {
        format!("{host}:{port}")
    } else {
        format!("addr:{host}:{port}")
    }
}

/// Add `addr:` to DNS trusted peers so they parse as socket addresses.
fn sanitize_trusted_peers(peers: &[String]) -> Vec<String> {
    peers
        .iter()
        .map(|p| {
            let mut parts = p.splitn(2, '@');
            let pk = parts.next().unwrap_or_default();
            let rest = parts.next();
            match rest {
                None | Some("") => p.clone(),
                Some(addr) if addr.starts_with("addr:") => p.clone(),
                Some(addr) => {
                    let needs_literal = addr.chars().any(char::is_alphabetic);
                    if needs_literal {
                        format!("{pk}@addr:{addr}")
                    } else {
                        p.clone()
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_address_preserves_fingerprint() {
        assert_eq!(
            rewrite_address("addr:0.0.0.0:1337#BF18", "1.2.3.4", 9999),
            "addr:1.2.3.4:9999#BF18"
        );
    }

    #[test]
    fn rewrite_address_plain() {
        assert_eq!(
            rewrite_address("0.0.0.0:8080", "10.0.0.5", 18100),
            "10.0.0.5:18100"
        );
    }

    #[test]
    fn vanilla_config_has_minimal_sections() {
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let pop = bls_normal_pop_prove(kp.private_key()).expect("pop");
        let mut pops = BTreeMap::new();
        pops.insert(kp.public_key().clone(), pop);
        let peer = format!("{}@localhost:1337", kp.public_key());
        let config = build_vanilla_config(
            "chain-x",
            &kp,
            "localhost",
            1337,
            "localhost",
            8080,
            &[peer],
            &pops,
        );
        let table = config.as_table().expect("table");
        assert_eq!(
            table.get("chain").and_then(TomlValue::as_str),
            Some("chain-x")
        );
        assert!(table.get("network").is_some());
        assert!(table.get("torii").is_some());
        assert!(table.get("genesis").is_some());
        assert!(table.get("trusted_peers").is_some());
        assert!(table.get("trusted_peers_pop").is_some());
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
    fn trusted_peers_pop_missing_non_interactive_fails() {
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let other = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let answers = Answers {
            profile: Profile::Iroha2,
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
        assert!(result.is_err());
    }

    #[test]
    fn public_taira_bundle_uses_expected_network_identity() {
        const EXPECTED_TAIRA_CHAIN_ID: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
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
        let xor_universal = first_tx_instructions
            .iter()
            .find(|instruction| {
                instruction
                    .get("Register")
                    .and_then(|register| register.get("AssetDefinition"))
                    .and_then(|asset| asset.get("id"))
                    .and_then(JsonValue::as_str)
                    == Some("xor#universal")
            })
            .expect("public Taira genesis.json must register xor#universal");
        let confidential_mode = xor_universal
            .get("Register")
            .and_then(|register| register.get("AssetDefinition"))
            .and_then(|asset| asset.get("confidential_policy"))
            .and_then(|policy| policy.get("mode"))
            .and_then(JsonValue::as_str);
        assert_eq!(
            confidential_mode,
            Some("Convertible"),
            "public Taira genesis.json must keep xor#universal shield-capable for wallet shielding"
        );
    }
}

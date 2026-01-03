//! Interactive setup wizard for quickly preparing Iroha/Sora configs.

use std::{
    fmt, fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Context as _, Result};
use inquire::{Select, Text};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
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
    /// Sora Testus (testnet).
    Testus,
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Profile::Iroha2 => write!(f, "Iroha2 (single lane)"),
            Profile::Nexus => write!(f, "Sora Nexus (mainnet)"),
            Profile::Testus => write!(f, "Sora Testus (testnet)"),
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
    /// Override the bootstrap peer (`pubkey@host:port`). Comma-separated for multiple entries.
    #[arg(long, value_name = "PEERS")]
    pub trusted_peers: Option<String>,
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
    relay_hub: Option<String>,
    output_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelayMode {
    Disabled,
    Hub,
    Spoke,
}

impl fmt::Display for RelayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelayMode::Disabled => write!(f, "disabled (full mesh)"),
            RelayMode::Hub => write!(f, "hub (static IP)"),
            RelayMode::Spoke => write!(f, "spoke (dial hub only)"),
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
                    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@nexus.mof2.sora.org:1337",
                ],
                config_template: Some("configs/soranexus/nexus/config.toml"),
                genesis_template: "configs/soranexus/nexus/genesis.json",
            },
            Profile::Testus => Self {
                chain: "809574f5-fee7-5e69-bfcf-52451e42d50f",
                p2p_port: 1337,
                torii_port: 18080,
                host: "testus.mof3.sora.org",
                trusted_peers: &[
                    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@testus.mof3.sora.org:1337",
                ],
                config_template: Some("configs/soranexus/testus/config.toml"),
                genesis_template: "configs/soranexus/testus/genesis.json",
            },
        }
    }
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        print_banner();
        let answers = gather_answers(&self)?;
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);

        tui::status("Generating config and genesis files");
        let (mut config, genesis_template_path) = load_config_template(&answers, &keypair)?;
        apply_overrides(&mut config, &answers, &keypair)?;
        let genesis = load_and_patch_genesis(&genesis_template_path, &answers.chain)?;

        fs::create_dir_all(&answers.output_dir)
            .wrap_err("failed to create output directory for wizard artefacts")?;
        let config_path = answers.output_dir.join("config.toml");
        let genesis_path = answers.output_dir.join("genesis.json");

        let sorafs_dir = answers.output_dir.join("sorafs_admission");
        if matches!(answers.profile, Profile::Nexus | Profile::Testus) {
            fs::create_dir_all(&sorafs_dir)
                .wrap_err("failed to create sorafs admission directory")?;
        }

        let config_payload = toml::to_string_pretty(&config)
            .wrap_err("failed to serialise config after wizard updates")?;
        fs::write(&config_path, config_payload)
            .wrap_err_with(|| format!("failed to write config to {}", config_path.display()))?;

        let genesis_payload = json::to_string_pretty(&genesis)
            .wrap_err("failed to serialise genesis after wizard updates")?;
        fs::write(&genesis_path, genesis_payload)
            .wrap_err_with(|| format!("failed to write genesis to {}", genesis_path.display()))?;

        tui::success(format!(
            "Config and genesis ready under {}",
            answers.output_dir.display()
        ));
        writeln!(writer, "config: {}", config_path.display())?;
        writeln!(writer, "genesis: {}", genesis_path.display())?;
        if matches!(answers.profile, Profile::Nexus | Profile::Testus) {
            writeln!(writer, "sora profile: pass --sora when starting irohad")?;
        }
        writeln!(
            writer,
            "next: irohad {} --config {} --genesis-manifest-json {}",
            if matches!(answers.profile, Profile::Nexus | Profile::Testus) {
                "--sora"
            } else {
                ""
            },
            config_path.display(),
            genesis_path.display()
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
        None,
        defaults.host.to_string(),
        args.non_interactive,
    )?;
    let p2p_port = resolve_number("P2P port", defaults.p2p_port, args.non_interactive)?;
    let torii_host = resolve_text(
        "Trusted peer Torii host",
        None,
        defaults.host.to_string(),
        args.non_interactive,
    )?;
    let torii_port = resolve_number("Torii port", defaults.torii_port, args.non_interactive)?;

    let relay_mode = resolve_relay_mode(args.non_interactive)?;
    let relay_hub = if matches!(relay_mode, RelayMode::Spoke) {
        Some(resolve_text(
            "Relay hub address (host:port)",
            None,
            format!("{}:{}", defaults.host, defaults.p2p_port),
            args.non_interactive,
        )?)
    } else {
        None
    };

    let default_trusted = defaults.trusted_peers.join(", ");
    let trusted_default = args
        .trusted_peers
        .as_deref()
        .unwrap_or(default_trusted.as_str());
    let trusted_prompt = resolve_text(
        "Trusted peers (comma separated pubkey@host:port)",
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
        relay_hub,
        output_dir: args.output_dir.clone(),
    })
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
        vec![Profile::Iroha2, Profile::Nexus, Profile::Testus],
    )
    .prompt()
    .wrap_err("failed to read profile selection")
}

fn resolve_relay_mode(non_interactive: bool) -> Result<RelayMode> {
    if non_interactive {
        return Ok(RelayMode::Disabled);
    }
    Select::new(
        "Relay mode (use hub/spoke when only one static IP is available)",
        vec![RelayMode::Disabled, RelayMode::Hub, RelayMode::Spoke],
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

fn resolve_number(prompt: &str, default: u16, non_interactive: bool) -> Result<u16> {
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

fn load_config_template(answers: &Answers, keypair: &KeyPair) -> Result<(TomlValue, String)> {
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
    );
    Ok((config, defaults.genesis_template.to_string()))
}

#[allow(clippy::unnecessary_wraps)]
fn apply_overrides(config: &mut TomlValue, answers: &Answers, keypair: &KeyPair) -> Result<()> {
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
        }
        RelayMode::Hub => {
            network.insert("relay_mode".into(), TomlValue::String("hub".to_owned()));
            network.remove("relay_hub_address");
        }
        RelayMode::Spoke => {
            network.insert("relay_mode".into(), TomlValue::String("spoke".to_owned()));
            if let Some(hub) = &answers.relay_hub {
                network.insert("relay_hub_address".into(), TomlValue::String(hub.clone()));
            }
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

    if matches!(answers.profile, Profile::Nexus | Profile::Testus) {
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
) {
    let mut list = sanitize_trusted_peers(peers);
    let self_peer = format!("{}@{}", keypair.public_key(), addr_literal(host, p2p_port));
    if !list.iter().any(|p| p == &self_peer) {
        list.push(self_peer);
    }
    set_array(config, "trusted_peers", list);
}

fn build_vanilla_config(
    chain: &str,
    keypair: &KeyPair,
    p2p_host: &str,
    p2p_port: u16,
    torii_host: &str,
    torii_port: u16,
    peers: &[String],
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
        let kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let config = build_vanilla_config(
            "chain-x",
            &kp,
            "localhost",
            1337,
            "localhost",
            8080,
            &["peer1@localhost:1337".to_string()],
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
}

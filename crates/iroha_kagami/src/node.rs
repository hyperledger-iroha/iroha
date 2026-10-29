//! Prepare one node directory that `iroha3d` reads through `data_dir`.
//!
//! `kagami node init` writes the node's three private keys into the fixed
//! `<dir>/secrets/` layout the daemon completes from `data_dir`, plus a starter
//! `config.toml` that names only this node's identity and listeners. State paths, the
//! SoraNet transport and streaming identities, and DA/SoraFS storage all derive
//! from `data_dir`, so the starter config carries none of them. Everything the network
//! shares comes from the `network.toml` that `kagami network create` writes, which the
//! starter config includes with `extends`.
use crate::{Outcome, RunArgs, tui};
use clap::{Args as ClapArgs, Subcommand};
use color_eyre::eyre::{WrapErr as _, eyre};
use iroha_config::parameters::{defaults::data_dir as layout, user::ConfigSocketAddr};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey};
use iroha_primitives::addr::SocketAddr;
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};
use zeroize::Zeroizing;

/// The starter configuration file written beside `secrets/`.
const CONFIG_FILE: &str = "config.toml";

/// Prepare a node directory for `iroha3d --config <DIR>/config.toml`.
#[derive(Subcommand)]
pub enum NodeCommand {
    /// Create a node's validator, SoraNet transport and streaming keys plus a starter config.
    Init(InitArgs),
}

impl<T: Write> RunArgs<T> for NodeCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::Init(args) => args.run(writer),
        }
    }
}

/// Arguments of `kagami node init`.
#[derive(ClapArgs)]
pub struct InitArgs {
    /// Node directory: keys go to `<DIR>/secrets/`, state to `<DIR>/state/`.
    ///
    /// It is created owner-only when missing; an existing `secrets/` or `config.toml`
    /// is never replaced.
    #[arg(long, value_name = "DIR")]
    dir: PathBuf,
    /// P2P address other peers dial (`host:port`); the node binds the same port on all
    /// interfaces in the address's IP family (IPv4 for a DNS name).
    #[arg(long, value_name = "HOST:PORT", default_value = "127.0.0.1:1337")]
    public_address: ConfigSocketAddr,
    /// Torii API listener (`host:port`).
    #[arg(long, value_name = "HOST:PORT", default_value = "127.0.0.1:8080")]
    api_address: ConfigSocketAddr,
}

impl<T: Write> RunArgs<T> for InitArgs {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let public_address = self.public_address.into_inner();
        let api_address = self.api_address.into_inner();
        if public_address.port() == 0 || api_address.port() == 0 {
            return Err(eyre!("P2P and Torii addresses must name nonzero ports"));
        }
        if public_address
            .to_string()
            .parse::<std::net::SocketAddr>()
            .is_ok_and(|address| address.ip().is_unspecified())
        {
            return Err(eyre!(
                "P2P public address must not be a wildcard; use the address other peers dial"
            ));
        }
        if public_address.port() == api_address.port() {
            return Err(eyre!("P2P and Torii ports must be distinct"));
        }
        let node = iroha_fs::OwnerDirectory::open_or_create(&self.dir)
            .wrap_err("prepare the owner-only node directory")?;
        for existing in [layout::SECRETS_DIR, CONFIG_FILE] {
            if node.path().join(existing).exists() {
                return Err(eyre!(
                    "{} already exists; choose a fresh node directory",
                    node.path().join(existing).display()
                ));
            }
        }
        tui::status("Generating validator, SoraNet transport and streaming keys");
        let keys = NodeKeys::generate()?;
        let validator = private_record(&keys.validator)?;
        let transport = private_record(&keys.transport)?;
        let streaming = private_record(&keys.streaming)?;
        let secrets = node
            .publish_private_child(
                layout::SECRETS_DIR,
                &[
                    (layout::VALIDATOR_KEY, validator.as_slice()),
                    (layout::TRANSPORT_KEY, transport.as_slice()),
                    (layout::STREAMING_KEY, streaming.as_slice()),
                ],
            )
            .wrap_err("publish the node key directory; reconcile it before retrying")?;
        let config = starter_config(keys.validator.public_key(), &public_address, &api_address);
        node.write_atomic(
            CONFIG_FILE,
            config.as_bytes(),
            iroha_fs::PublishMode::CreateNew,
        )
        .wrap_err("write the starter node configuration")?;
        tui::success(format!("Node directory ready at {}", node.path().display()));
        let config_path = node.path().join(CONFIG_FILE);
        writeln!(writer, "node_dir: {}", node.path().display())?;
        writeln!(writer, "config: {}", config_path.display())?;
        writeln!(writer, "secrets: {}", secrets.path().display())?;
        writeln!(writer, "public_key: {}", keys.validator.public_key())?;
        writeln!(writer, "pop_hex: {}", keys.pop_hex)?;
        writeln!(
            writer,
            "trusted_peer: {}@{public_address}",
            keys.validator.public_key()
        )?;
        writeln!(
            writer,
            "validator: {}@{public_address}={}",
            keys.validator.public_key(),
            keys.pop_hex
        )?;
        writeln!(
            writer,
            "next: send the validator line to whoever creates the network \
             (`kagami network create --validator ...`), copy the network.toml, \
             genesis.signed.nrt, genesis.expected_hash and nexus-amx-context.v1.bin it writes \
             into {}, then run \
             `iroha3d --config {} --check-config`",
            node.path().display(),
            config_path.display()
        )?;
        Ok(())
    }
}

/// The three key pairs one validator needs.
struct NodeKeys {
    validator: KeyPair,
    pop_hex: String,
    transport: KeyPair,
    streaming: KeyPair,
}

impl NodeKeys {
    fn generate() -> color_eyre::Result<Self> {
        let validator = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .wrap_err("generate the BLS validator key")?;
        let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
            .wrap_err("prove possession of the validator key")?;
        let transport = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .wrap_err("generate the SoraNet transport key")?;
        let streaming = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .wrap_err("generate the streaming identity key")?;
        if transport.public_key() == streaming.public_key() {
            return Err(eyre!(
                "SoraNet transport and streaming identities must differ"
            ));
        }
        Ok(Self {
            validator,
            pop_hex: hex::encode(pop),
            transport,
            streaming,
        })
    }
}

/// One canonical private multihash and a newline, the record the daemon reads.
fn private_record(key_pair: &KeyPair) -> color_eyre::Result<Zeroizing<Vec<u8>>> {
    let encoded = Zeroizing::new(
        ExposedPrivateKey(key_pair.private_key().clone())
            .try_to_multihash_string()
            .wrap_err("encode the private key")?,
    );
    let mut record = Zeroizing::new(Vec::with_capacity(encoded.len() + 1));
    record.extend_from_slice(encoded.as_bytes());
    record.push(b'\n');
    Ok(record)
}

/// A node file that names only this node's identity and listeners; `network.toml` supplies
/// what the network shares and `data_dir = "."` derives the rest.
fn starter_config(
    public_key: &PublicKey,
    public_address: &SocketAddr,
    api_address: &SocketAddr,
) -> String {
    let bind_host = match public_address {
        SocketAddr::Ipv6(_) => "[::]",
        SocketAddr::Ipv4(_) | SocketAddr::Host(_) => "0.0.0.0",
    };
    let p2p_port = public_address.port();
    format!(
        r#"# Node configuration created by `kagami node init`.
#
# Keys live in ./secrets/ and node state in ./state/. `data_dir = "."` derives every
# state path and the SoraNet transport and streaming identities from this directory,
# so no DA, SoraFS, SoraNet or streaming settings are needed.
#
# network.toml beside this file supplies what every node of the network shares: the
# chain, each validator with its proof of possession, [genesis] and the consensus-bound
# sections the signed genesis binds. `kagami network create` writes it together with
# genesis.signed.nrt, genesis.expected_hash and nexus-amx-context.v1.bin; copy all four
# into this directory. The context file gives field-level configuration diagnostics.
# Validate with: iroha3d --config config.toml --check-config
extends = "network.toml"
data_dir = "."
public_key = "{public_key}"

[network]
address = "{bind_host}:{p2p_port}"
public_address = "{public_address}"

[torii]
address = "{api_address}"
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr as _;

    /// Run `node init` and return its printed summary.
    fn init(dir: &std::path::Path) -> color_eyre::Result<String> {
        let mut output = BufWriter::new(Vec::new());
        InitArgs {
            dir: dir.to_path_buf(),
            public_address: ConfigSocketAddr::from_str("10.0.0.5:1338").unwrap(),
            api_address: ConfigSocketAddr::from_str("127.0.0.1:8081").unwrap(),
        }
        .run(&mut output)?;
        Ok(String::from_utf8(output.into_inner()?)?)
    }

    #[test]
    fn init_writes_the_data_dir_key_layout_and_a_starter_config() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().canonicalize().unwrap().join("node");
        let summary = init(&dir).expect("fresh node directory");
        let secrets = dir.join(layout::SECRETS_DIR);
        let read_key = |name: &str| {
            let text = std::fs::read_to_string(secrets.join(name)).unwrap();
            assert!(
                text.ends_with('\n'),
                "{name} is one newline-terminated record"
            );
            iroha_crypto::PrivateKey::from_str(text.trim_end()).unwrap()
        };
        let validator = KeyPair::from_private_key(read_key(layout::VALIDATOR_KEY)).unwrap();
        assert_eq!(
            validator.public_key().try_algorithm().unwrap(),
            Algorithm::BlsNormal
        );
        for name in [layout::TRANSPORT_KEY, layout::STREAMING_KEY] {
            assert_eq!(read_key(name).algorithm(), Algorithm::Ed25519, "{name}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            for name in [
                layout::VALIDATOR_KEY,
                layout::TRANSPORT_KEY,
                layout::STREAMING_KEY,
            ] {
                let mode = std::fs::metadata(secrets.join(name))
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o077, 0, "{name} is owner-only");
            }
        }
        let config: toml::Table =
            toml::from_str(&std::fs::read_to_string(dir.join(CONFIG_FILE)).unwrap()).unwrap();
        assert_eq!(config["extends"].as_str(), Some("network.toml"));
        assert_eq!(config["data_dir"].as_str(), Some("."));
        assert_eq!(
            config["public_key"].as_str(),
            Some(validator.public_key().to_string().as_str())
        );
        assert_eq!(config["network"]["address"].as_str(), Some("0.0.0.0:1338"));
        assert_eq!(
            config["network"]["public_address"].as_str(),
            Some("10.0.0.5:1338")
        );
        assert_eq!(config["torii"]["address"].as_str(), Some("127.0.0.1:8081"));
        for absent in ["private_key", "streaming", "sorafs", "tiered_state", "kura"] {
            assert!(
                !config.contains_key(absent),
                "{absent} derives from data_dir"
            );
        }
        for shared in [
            "chain",
            "trusted_peers",
            "trusted_peers_pop",
            "genesis",
            "nexus",
        ] {
            assert!(
                !config.contains_key(shared),
                "{shared} comes from network.toml"
            );
        }
        // The printed roster entry is exactly what `kagami network create --validator` takes.
        let entry = summary
            .lines()
            .find_map(|line| line.strip_prefix("validator: "))
            .expect("roster entry");
        let parsed = entry
            .parse::<iroha_deploy::localnet::NetworkValidator>()
            .expect("node init prints a valid roster entry");
        assert_eq!(parsed.public_key(), validator.public_key());
        assert_eq!(parsed.address().to_string(), "10.0.0.5:1338");
    }

    #[test]
    fn init_never_replaces_existing_keys_or_config() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().canonicalize().unwrap().join("node");
        init(&dir).expect("fresh node directory");
        let before =
            std::fs::read(dir.join(layout::SECRETS_DIR).join(layout::VALIDATOR_KEY)).unwrap();
        assert!(init(&dir).is_err());
        assert_eq!(
            std::fs::read(dir.join(layout::SECRETS_DIR).join(layout::VALIDATOR_KEY)).unwrap(),
            before
        );
    }

    #[test]
    fn init_binds_ipv6_for_an_ipv6_public_address() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().canonicalize().unwrap().join("node");
        InitArgs {
            dir: dir.clone(),
            public_address: "[2001:db8::7]:1337".parse().unwrap(),
            api_address: "[::1]:8080".parse().unwrap(),
        }
        .run(&mut BufWriter::new(Vec::new()))
        .expect("IPv6 node init");
        let config: toml::Table =
            toml::from_str(&std::fs::read_to_string(dir.join(CONFIG_FILE)).unwrap()).unwrap();
        assert_eq!(config["network"]["address"].as_str(), Some("[::]:1337"));
        assert_eq!(
            config["network"]["public_address"].as_str(),
            Some("[2001:db8::7]:1337")
        );
        assert_eq!(config["torii"]["address"].as_str(), Some("[::1]:8080"));
    }

    #[test]
    fn init_rejects_unusable_addresses_before_creating_keys() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("node");
        for (public_address, api_address, reason) in [
            ("0.0.0.0:1337", "127.0.0.1:8080", "wildcard"),
            ("[::]:1337", "127.0.0.1:8080", "wildcard"),
            ("127.0.0.1:0", "127.0.0.1:8080", "nonzero"),
            ("127.0.0.1:1337", "127.0.0.1:0", "nonzero"),
        ] {
            let error = InitArgs {
                dir: dir.clone(),
                public_address: public_address.parse().unwrap(),
                api_address: api_address.parse().unwrap(),
            }
            .run(&mut BufWriter::new(Vec::new()))
            .expect_err("the address cannot identify a usable node endpoint");
            assert!(error.to_string().contains(reason), "{error}");
            assert!(!dir.exists(), "invalid endpoints must not publish keys");
        }
    }

    #[test]
    fn init_rejects_shared_p2p_and_api_ports() {
        let root = tempfile::tempdir().unwrap();
        let error = InitArgs {
            dir: root.path().join("node"),
            public_address: ConfigSocketAddr::from_str("127.0.0.1:1337").unwrap(),
            api_address: ConfigSocketAddr::from_str("127.0.0.1:1337").unwrap(),
        }
        .run(&mut BufWriter::new(Vec::new()))
        .unwrap_err();
        assert!(error.to_string().contains("distinct"));
        assert!(!root.path().join("node").exists());
    }
}

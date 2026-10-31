//! Assemble a network from validators that hold their own keys.
//!
//! `kagami network create` takes the `validator:` line that `kagami node init` prints on each
//! host and writes the signed genesis, its expected hash and public key, and `network.toml`,
//! the shared configuration each node's `config.toml` includes with `extends`. Genesis
//! composition, signing and output custody belong to [`iroha_deploy::network::generate_network`].
use crate::{Outcome, RunArgs, localnet::ConsensusModeArg, tui};
use clap::{Args as ClapArgs, Subcommand};
use iroha_deploy::network::{
    DEFAULT_NETWORK_TORII_URL, NetworkOptions, NetworkValidator, generate_network,
};
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

/// Create a network whose validators hold their own keys.
#[derive(Subcommand)]
pub enum NetworkCommand {
    /// Sign genesis for `kagami node init` validators and write the files every node shares
    Create(CreateArgs),
}

impl<T: Write> RunArgs<T> for NetworkCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::Create(args) => args.run(writer),
        }
    }
}

/// Arguments of `kagami network create`.
#[derive(ClapArgs)]
pub struct CreateArgs {
    /// One genesis validator, exactly as `kagami node init` prints it on the validator's host.
    ///
    /// Repeat once per validator. The roster must be an exact Sumeragi `3f + 1` committee
    /// (4, 7, 10, ...) of distinct BLS-normal keys, each with a valid proof of possession and
    /// a distinct `HOST:PORT` that the other validators dial.
    #[arg(
        long = "validator",
        value_name = "PUBLIC_KEY@HOST:PORT=POP_HEX",
        required = true,
        value_parser = parse_validator
    )]
    validators: Vec<NetworkValidator>,
    /// Output directory, created owner-only; an existing directory must be empty.
    #[arg(long, value_name = "DIR")]
    out_dir: PathBuf,
    /// Canonical chain identifier; a fresh random UUID when omitted.
    #[arg(long, value_name = "CHAIN_ID")]
    chain_id: Option<String>,
    /// Account-address chain prefix written into genesis and the shared configuration.
    ///
    /// Omit it to use the node default.
    #[arg(long, value_name = "PREFIX")]
    chain_discriminant: Option<u16>,
    /// Consensus mode committed in genesis.
    #[arg(long, value_enum, value_name = "MODE", default_value = "permissioned")]
    consensus_mode: ConsensusModeArg,
    /// Torii API that the operator's `client.toml` dials; any validator's Torii works.
    #[arg(long, value_name = "URL", default_value = DEFAULT_NETWORK_TORII_URL)]
    torii_url: String,
}

/// Keep the complete roster-entry diagnostic for clap.
fn parse_validator(value: &str) -> Result<NetworkValidator, String> {
    value
        .parse()
        .map_err(|error: color_eyre::Report| error.to_string())
}

impl<T: Write> RunArgs<T> for CreateArgs {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        tui::status(format!(
            "Signing genesis for {} validators with a fresh genesis key",
            self.validators.len()
        ));
        generate_network(
            &NetworkOptions {
                validators: self.validators,
                chain_id: self.chain_id,
                chain_discriminant: self.chain_discriminant,
                consensus_mode: self.consensus_mode.into(),
                torii_url: Some(self.torii_url),
                out_dir: self.out_dir,
            },
            writer,
        )?;
        tui::success("Network ready to distribute");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InitArgs;
    use clap::Parser as _;
    use iroha_config::{
        node_config::{NodeConfigOptions, NodeFile, open_node_config},
        parameters::actual,
    };
    use iroha_data_model::prelude::NetworkId;
    use std::{fs, path::Path};

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: NetworkCommand,
    }

    #[derive(clap::Parser)]
    struct InitCli {
        #[command(flatten)]
        init: InitArgs,
    }

    /// `kagami node init` for one validator; returns its directory and printed `validator:` line.
    fn node_init(root: &Path, index: u16) -> (PathBuf, String) {
        let dir = root.join(format!("node{index}"));
        let mut output = BufWriter::new(Vec::new());
        InitCli::try_parse_from([
            "init",
            "--dir",
            dir.to_str().unwrap(),
            "--public-address",
            &format!("127.0.0.1:{}", 31_337 + index),
            "--api-address",
            &format!("127.0.0.1:{}", 38_080 + index),
        ])
        .unwrap()
        .init
        .run(&mut output)
        .expect("node init");
        let output = String::from_utf8(output.into_inner().unwrap()).unwrap();
        let validator = output
            .lines()
            .find_map(|line| line.strip_prefix("validator: "))
            .expect("node init prints its roster entry")
            .to_owned();
        (dir, validator)
    }

    fn create(arguments: &[&str]) -> Outcome {
        let mut argv = vec!["kagami-network", "create"];
        argv.extend_from_slice(arguments);
        let TestCli {
            command: NetworkCommand::Create(args),
        } = TestCli::try_parse_from(argv)?;
        args.run(&mut BufWriter::new(Vec::new()))
    }

    fn load_node(config: &Path) -> actual::Root {
        let node = open_node_config(
            NodeFile::Path(config.to_path_buf()),
            NodeConfigOptions::default(),
        )
        .expect("node config layout");
        let (user, _) = node.read().expect("read node config");
        user.parse().expect("node config validates")
    }

    #[test]
    fn create_requires_validators_and_an_output_directory() {
        assert!(TestCli::try_parse_from(["kagami-network", "create", "--out-dir", "net"]).is_err());
        assert!(
            TestCli::try_parse_from(["kagami-network", "create", "--validator", "x"]).is_err(),
            "a malformed roster entry fails while parsing arguments"
        );
        let error = TestCli::try_parse_from([
            "kagami-network",
            "create",
            "--out-dir",
            "net",
            "--validator",
            "ea0130ab@127.0.0.1:1337",
        ])
        .err()
        .expect("an entry without a proof of possession is refused");
        assert!(error.to_string().contains("POP_HEX"), "{error}");
    }

    #[test]
    fn node_init_directories_join_the_created_network_through_extends() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let nodes = (0..4)
            .map(|index| node_init(&root, index))
            .collect::<Vec<_>>();
        let out_dir = root.join("network");
        let mut arguments = vec![
            "--out-dir".to_owned(),
            out_dir.to_str().unwrap().to_owned(),
            "--chain-id".to_owned(),
            "kagami-network-test".to_owned(),
        ];
        for (_, validator) in &nodes {
            arguments.extend(["--validator".to_owned(), validator.clone()]);
        }
        create(&arguments.iter().map(String::as_str).collect::<Vec<_>>())
            .expect("create the network from node init identities");
        let expected_hash = fs::read_to_string(out_dir.join("genesis.expected_hash"))
            .unwrap()
            .trim_end()
            .parse::<NetworkId>()
            .unwrap()
            .into_genesis_hash();
        let configs = nodes
            .iter()
            .map(|(dir, _)| {
                for name in [
                    "network.toml",
                    "genesis.signed.nrt",
                    "genesis.expected_hash",
                    "nexus-amx-context.v1.bin",
                ] {
                    fs::copy(out_dir.join(name), dir.join(name)).unwrap();
                }
                let config = load_node(&dir.join("config.toml"));
                assert_eq!(config.common.chain.as_str(), "kagami-network-test");
                assert_eq!(config.genesis.expected_hash, expected_hash);
                assert_eq!(config.common.trusted_peers.value().others.len(), 3);
                assert_eq!(config.common.trusted_peers.value().pops.len(), 4);
                config
            })
            .collect::<Vec<_>>();
        // The node files that `node init` wrote, extended by the shared file, reproduce the
        // exact consensus commitments the genesis was signed with.
        let bound =
            iroha_genesis::RawGenesisTransaction::from_path(out_dir.join("genesis.json")).unwrap();
        let signed =
            iroha_genesis::read_signed_genesis(&out_dir.join("genesis.signed.nrt")).unwrap();
        let (nexus, execution) =
            crate::genesis::staged_signed_sumeragi_context_hashes(&bound, &signed, &configs[0])
                .expect("restage genesis with a node's own configuration");
        let context = bound.sumeragi_context_parameters();
        assert_eq!(
            nexus,
            iroha_crypto::Hash::prehashed(context.nexus_amx_context_hash)
        );
        assert_eq!(
            execution,
            iroha_crypto::Hash::prehashed(context.execution_policy_hash)
        );
        assert!(
            create(&arguments.iter().map(String::as_str).collect::<Vec<_>>()).is_err(),
            "an existing network directory is never replaced"
        );
    }

    #[test]
    fn create_rejects_an_incomplete_committee_before_writing_anything() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let out_dir = root.join("network");
        let mut arguments = vec!["--out-dir".to_owned(), out_dir.to_str().unwrap().to_owned()];
        for index in 0..3 {
            arguments.extend(["--validator".to_owned(), node_init(&root, index).1]);
        }
        let error = create(&arguments.iter().map(String::as_str).collect::<Vec<_>>())
            .expect_err("three validators are not a 3f+1 committee");
        assert!(error.to_string().contains("3f+1"), "{error}");
        assert!(!out_dir.exists());
    }
}

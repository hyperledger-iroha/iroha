//! CLI syntax for developer localnet bundles.
//!
//! Sora networks (Nexus, the public Taira chain and the private dataspace presets) come from
//! `kagami sora-network generate`.
use crate::{Outcome, RunArgs};
use clap::{Args as ClapArgs, ValueEnum};
use iroha_data_model::parameter::system::SumeragiConsensusMode;
use iroha_deploy::localnet::*;
use std::{
    io::{BufWriter, Write},
    num::NonZeroU16,
    path::PathBuf,
};
use zeroize::Zeroizing;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsensusModeArg {
    Permissioned,
    Npos,
}
impl From<ConsensusModeArg> for SumeragiConsensusMode {
    fn from(value: ConsensusModeArg) -> Self {
        match value {
            ConsensusModeArg::Permissioned => SumeragiConsensusMode::Permissioned,
            ConsensusModeArg::Npos => SumeragiConsensusMode::Npos,
        }
    }
}
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalnetPerfProfileArg {
    #[value(name = "10k-permissioned")]
    Throughput10kPermissioned,
    #[value(name = "10k-npos")]
    Throughput10kNpos,
}
impl From<LocalnetPerfProfileArg> for LocalnetPerfProfile {
    fn from(value: LocalnetPerfProfileArg) -> Self {
        match value {
            LocalnetPerfProfileArg::Throughput10kPermissioned => {
                LocalnetPerfProfile::Throughput10kPermissioned
            }
            LocalnetPerfProfileArg::Throughput10kNpos => LocalnetPerfProfile::Throughput10kNpos,
        }
    }
}
/// Generate a bare-metal developer localnet (no Docker): genesis, per-peer configs, start/stop scripts.
///
/// Sora networks, including the public Taira chain, come from `kagami sora-network generate`.
#[derive(ClapArgs)]
pub struct Args {
    /// Number of peers to generate (minimum four).
    #[arg(long, short, value_name = "COUNT", default_value_t = NonZeroU16::new(4).unwrap())]
    peers: NonZeroU16,
    /// Optional UTF-8 seed for deterministic development keys.
    ///
    /// Omit this option to generate independent keys from operating-system entropy.
    #[arg(long, short)]
    seed: Option<String>,
    /// Canonical developer chain identifier written into genesis, peer configs, and the client
    /// config. Public chain identities are rejected.
    #[arg(long, value_name = "CHAIN_ID", default_value = DEFAULT_CHAIN_ID)]
    chain_id: String,
    /// Account-address chain prefix written into genesis and client/peer configs.
    #[arg(long, value_name = "PREFIX")]
    chain_discriminant: Option<u16>,
    /// Apply a localnet performance profile (10k TPS / 1s finality presets).
    #[arg(long, value_enum, value_name = "PROFILE")]
    perf_profile: Option<LocalnetPerfProfileArg>,
    /// Host to bind P2P and Torii listeners to (host/IP only, no port).
    #[arg(long, default_value = DEFAULT_BIND_HOST, value_name = "HOST")]
    bind_host: String,
    /// Host to advertise to peers and use for client Torii URL (host/IP only, no port).
    #[arg(long, default_value = DEFAULT_PUBLIC_HOST, value_name = "HOST")]
    public_host: String,
    /// Base Torii API port (per-peer increments by 1).
    #[arg(long, default_value_t = 8080)]
    base_api_port: u16,
    /// Base P2P port (per-peer increments by 1).
    #[arg(long, default_value_t = 1337)]
    base_p2p_port: u16,
    /// Output directory for configs/genesis/scripts.
    #[arg(long, short, value_name = "DIR")]
    out_dir: PathBuf,
    /// Extra accounts to pre-register (in wonderland).
    #[arg(long, default_value_t = 0)]
    extra_accounts: u16,
    /// Register the optional sample asset and mint to the default account.
    /// The built-in localnet asset is always emitted.
    #[arg(long, default_value_t = false)]
    sample_asset: bool,
    /// Register additional asset definition IDs owned by the generated client signer.
    /// Repeat the flag to register more than one asset definition. A localnet reserve is minted
    /// to the generated client signer for each requested asset definition.
    #[arg(long, value_name = "ASSET_DEFINITION_ID")]
    asset_definition_id: Vec<String>,
    /// Override the immutable signed block cadence in milliseconds.
    /// Leave unset to use the one-second localnet cadence.
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    block_cadence_ms: Option<u64>,
    /// Consensus mode to emit in genesis/configs.
    /// Defaults to `permissioned`; perf profiles select their own mode.
    #[arg(long, value_enum, value_name = "MODE")]
    consensus_mode: Option<ConsensusModeArg>,
}
fn resolve_requested_consensus_mode(
    explicit_mode: Option<ConsensusModeArg>,
    perf_profile: Option<LocalnetPerfProfile>,
) -> SumeragiConsensusMode {
    explicit_mode.map_or_else(
        || {
            perf_profile.map_or(SumeragiConsensusMode::Permissioned, |profile| {
                profile.consensus_mode()
            })
        },
        SumeragiConsensusMode::from,
    )
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            peers,
            seed,
            chain_id,
            chain_discriminant,
            perf_profile,
            bind_host,
            public_host,
            base_api_port,
            base_p2p_port,
            out_dir,
            extra_accounts,
            sample_asset,
            asset_definition_id,
            block_cadence_ms,
            consensus_mode,
        } = self;
        // Own the CLI seed in a zeroizing guard before any fallible request
        // validation. Once validation succeeds, `LocalnetOptions` takes over
        // the same custody obligation through its `Drop` implementation.
        let mut seed = seed.map(Zeroizing::new);
        let perf_profile = perf_profile.map(LocalnetPerfProfile::from);
        let consensus_mode = resolve_requested_consensus_mode(consensus_mode, perf_profile);
        let mut assets = if sample_asset {
            vec![AssetSpec {
                id: localnet_sample_asset_literal(),
                name: LOCALNET_SAMPLE_ASSET_NAME.to_owned(),
                alias: None,
                owned_by: localnet_client_account_id(),
                mint_to: localnet_client_account_id(),
                quantity: 100,
            }]
        } else {
            vec![]
        };
        for asset_definition_id in asset_definition_id {
            assets.push(requested_localnet_asset_spec(&asset_definition_id)?);
        }
        let opts = LocalnetOptions {
            perf_profile,
            peers,
            seed: seed.as_mut().map(|seed| std::mem::take(&mut **seed)),
            bind_host,
            public_host,
            base_api_port,
            base_p2p_port,
            out_dir,
            extra_accounts,
            assets,
            consensus_mode,
            block_cadence_ms,
        };
        generate_localnet_with_chain(&opts, writer, Some(&chain_id), chain_discriminant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    use iroha_config::{base::toml::TomlSource, parameters::actual};
    use iroha_data_model::prelude::NetworkId;
    use iroha_deploy::genesis::profile::{PUBLIC_NEXUS_CHAIN_ID, PUBLIC_TAIRA_CHAIN_ID};
    use iroha_genesis::{RawGenesisTransaction, read_signed_genesis};
    use std::fs;

    #[derive(clap::Parser)]
    struct TestArgs {
        #[command(flatten)]
        localnet: Args,
    }

    #[test]
    fn localnet_cli_rejects_sora_selectors_and_retired_aliases() {
        for sora_only in [
            ["--sora-profile", "nexus"],
            ["--private-dataspace", "cbuae"],
            ["--profile", "dataspace"],
        ] {
            let mut args = vec![
                "kagami-localnet-test",
                "--out-dir",
                "/tmp/kagami-localnet-test",
            ];
            args.extend(sora_only);
            assert!(
                TestArgs::try_parse_from(args).is_err(),
                "developer localnets must not accept Sora option {}",
                sora_only[0]
            );
        }
        for retired in ["throughput-10k-permissioned", "throughput-10k-npos"] {
            assert!(
                TestArgs::try_parse_from([
                    "kagami-localnet-test",
                    "--out-dir",
                    "/tmp/kagami-localnet-test",
                    "--perf-profile",
                    retired,
                ])
                .is_err(),
                "first-release CLI must reject retired performance alias {retired}"
            );
        }
    }

    #[test]
    fn localnet_cli_rejects_public_chain_ids_without_partial_output() {
        let parent = tempfile::tempdir().expect("create public-chain refusal parent");
        let private = iroha_fs::PrivateDirectory::open_or_create(parent.path().join("private"))
            .expect("create private public-chain refusal parent");
        for (index, chain) in [PUBLIC_TAIRA_CHAIN_ID, PUBLIC_NEXUS_CHAIN_ID]
            .into_iter()
            .enumerate()
        {
            let output = private.path().join(format!("public{index}"));
            let parsed = TestArgs::try_parse_from([
                "kagami-localnet-test",
                "--out-dir",
                output.to_str().expect("UTF-8 temporary path"),
                "--chain-id",
                chain,
            ])
            .expect("parse public chain id");
            let error = parsed
                .localnet
                .run(&mut BufWriter::new(Vec::new()))
                .expect_err("a developer localnet cannot use a public chain identity");
            assert!(
                error.to_string().contains("kagami sora-network generate"),
                "the refusal names the Sora network command: {error}"
            );
            assert!(
                !output.exists(),
                "a refused public chain identity must not create output"
            );
        }
    }

    #[test]
    fn localnet_perf_profile_keeps_matching_consensus_mode() {
        let mode =
            resolve_requested_consensus_mode(None, Some(LocalnetPerfProfile::Throughput10kNpos));
        assert_eq!(mode, SumeragiConsensusMode::Npos);
    }
    #[test]
    fn localnet_defaults_to_permissioned_without_perf_preset() {
        let mode = resolve_requested_consensus_mode(None, None);
        assert_eq!(mode, SumeragiConsensusMode::Permissioned);
    }

    #[test]
    fn localnet_chain_discriminant_cli_generates_matching_native_artifacts() {
        let parent = tempfile::tempdir().expect("create isolated localnet parent");
        let private = iroha_fs::PrivateDirectory::open_or_create(parent.path().join("private"))
            .expect("create private localnet parent");
        let output = private.path().join("isolated");
        let parsed = TestArgs::try_parse_from([
            "kagami-localnet-test",
            "--out-dir",
            output.to_str().expect("UTF-8 temporary path"),
            "--chain-discriminant",
            "369",
            "--seed",
            "isolated-native-chain-prefix",
            "--bind-host",
            "127.0.0.1",
            "--public-host",
            "127.0.0.1",
        ])
        .expect("parse explicit native chain prefix");
        assert_eq!(parsed.localnet.chain_id, DEFAULT_CHAIN_ID);
        assert_eq!(parsed.localnet.chain_discriminant, Some(369));
        parsed
            .localnet
            .run(&mut BufWriter::new(Vec::new()))
            .expect("generate isolated localnet with native prefix");

        let manifest = RawGenesisTransaction::from_path(output.join("genesis.json"))
            .expect("parse generated native genesis");
        assert_eq!(manifest.chain_id().to_string(), DEFAULT_CHAIN_ID);
        assert_eq!(manifest.chain_discriminant(), 369);
        let signed = read_signed_genesis(&output.join("genesis.signed.nrt"))
            .expect("decode generated signed genesis");
        signed
            .validate_output_merkle_cache()
            .expect("signed genesis has authenticated execution outputs");
        for index in 0..signed.network_entrypoint_count() {
            let (_, result) = signed
                .network_output_at(u32::try_from(index).expect("genesis index fits u32"))
                .expect("every genesis input has its Network output");
            assert!(result.result.is_ok(), "genesis input {index} must apply");
        }
        let expected_hash = fs::read_to_string(output.join(GENESIS_EXPECTED_HASH_FILE))
            .expect("read exact signed genesis identity")
            .trim()
            .parse::<NetworkId>()
            .expect("parse exact signed genesis identity");
        assert_eq!(signed.hash(), expected_hash.into_genesis_hash());

        let client = fs::read_to_string(output.join("client.toml"))
            .expect("read generated client")
            .parse::<toml::Table>()
            .expect("parse generated client");
        assert_eq!(
            client.get("chain").and_then(toml::Value::as_str),
            Some(DEFAULT_CHAIN_ID)
        );
        assert_eq!(
            client["account"]["chain_discriminant"].as_integer(),
            Some(369)
        );
        for index in 0..4 {
            let path = output.join(format!("peer{index}.toml"));
            let peer = fs::read_to_string(&path)
                .expect("read generated peer")
                .parse::<toml::Table>()
                .expect("parse generated peer");
            assert_eq!(
                peer.get("chain").and_then(toml::Value::as_str),
                Some(DEFAULT_CHAIN_ID)
            );
            assert_eq!(
                peer.get("chain_discriminant")
                    .and_then(toml::Value::as_integer),
                Some(369)
            );
            assert!(
                peer["torii"].get("account_onboarding").is_none(),
                "developer localnets generate no onboarding authority"
            );
            let authority = peer["gov"]["bond_escrow_account"]
                .as_str()
                .expect("governance account literal in the chain prefix");
            assert!(
                iroha_data_model::account::address::AccountAddress::parse_encoded(
                    authority,
                    Some(369)
                )
                .is_ok()
            );
            assert!(
                iroha_data_model::account::address::AccountAddress::parse_encoded(
                    authority,
                    Some(753)
                )
                .is_err()
            );
            let config = actual::Root::from_toml_source(
                TomlSource::from_file(&path).expect("read native peer config"),
            )
            .expect("native config accepts generated genesis and account prefix");
            assert_eq!(config.genesis.expected_hash, signed.hash());
            assert!(
                !peer["soracloud_runtime"]
                    .get("production_mode")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false)
            );
        }
    }
}

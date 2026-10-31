//! CLI syntax for Sora network bundles: Nexus, the public Taira chain and the private dataspace
//! presets. Developer networks come from `kagami localnet`.
use crate::{Outcome, RunArgs};
use clap::{Args as ClapArgs, Subcommand, ValueEnum};
use color_eyre::eyre::{Result, eyre};
use iroha_deploy::sora_network::{
    DEFAULT_BIND_HOST, DEFAULT_CHAIN_ID, DEFAULT_PUBLIC_HOST, NetworkOptions, SoraProfile,
    generate_network_with_chain, validate_beacon_launch,
};
use std::{
    io::{BufWriter, Write},
    num::NonZeroU16,
    path::PathBuf,
};
use zeroize::Zeroizing;

/// Generate and validate Sora network bundles.
#[derive(Subcommand)]
pub enum Command {
    /// Generate a Sora network bundle without starting validators.
    Generate(Args),
    /// Validate an isolated native beacon configuration and opaque descriptor handoff.
    ValidateBeaconLaunch(ValidateBeaconLaunchArgs),
}

impl<T: Write> RunArgs<T> for Command {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::Generate(args) => args.run(writer),
            Self::ValidateBeaconLaunch(args) => args.run(writer),
        }
    }
}

/// Native validation of one isolated post-DKG Taira peer launch.
#[derive(ClapArgs)]
pub struct ValidateBeaconLaunchArgs {
    /// Absolute directory containing the unchanged generated peer0..3 configurations.
    #[arg(long)]
    network_dir: PathBuf,
    /// Zero-based generated peer index.
    #[arg(long, value_parser = clap::value_parser!(u16).range(0..=3))]
    peer_index: u16,
    /// Separate native configuration in the private run's beacon/seat-N directory.
    #[arg(long)]
    beacon_config: PathBuf,
    /// Owner-only native beacon credential beside the separate configuration.
    #[arg(long)]
    beacon_credential: PathBuf,
}

impl<T: Write> RunArgs<T> for ValidateBeaconLaunchArgs {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let selection = validate_beacon_launch(
            &self.network_dir,
            self.peer_index,
            &self.beacon_config,
            &self.beacon_credential,
        )?;
        writer.write_all(&norito::json::to_vec(&selection)?)?;
        writeln!(writer)?;
        Ok(())
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoraProfileArg {
    /// Dataspace-oriented defaults; `--private-dataspace` selects a restricted preset.
    Dataspace,
    /// Public dataspace rules (Nexus); the public Taira chain uses this profile.
    Nexus,
}
/// Canonical restricted-dataspace presets supported by the Sora network generator.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateDataspaceArg {
    /// State Bank of Pakistan dataspace (id 10, lane 3).
    Sbp,
    /// Central Bank of the UAE dataspace (id 12, lane 4).
    Cbuae,
    /// Bank of Papua New Guinea local dataspace (id 8648377547929788715, lane 5).
    Bpng,
}
fn resolve_sora_profile(
    profile: SoraProfileArg,
    private_dataspace: Option<PrivateDataspaceArg>,
) -> Result<SoraProfile> {
    match (profile, private_dataspace) {
        (SoraProfileArg::Nexus, None) => Ok(SoraProfile::Nexus),
        (SoraProfileArg::Dataspace, None) => Ok(SoraProfile::Dataspace),
        (SoraProfileArg::Dataspace, Some(PrivateDataspaceArg::Sbp)) => Ok(SoraProfile::PrivateSbp),
        (SoraProfileArg::Dataspace, Some(PrivateDataspaceArg::Cbuae)) => {
            Ok(SoraProfile::PrivateCbuae)
        }
        (SoraProfileArg::Dataspace, Some(PrivateDataspaceArg::Bpng)) => {
            Ok(SoraProfile::PrivateBpng)
        }
        (SoraProfileArg::Nexus, Some(_)) => Err(eyre!(
            "`--private-dataspace` requires `--profile dataspace`"
        )),
    }
}
/// Generate a bare-metal Sora network (no Docker): genesis, per-peer configs, start/stop scripts.
///
/// Sora networks always run `NPoS`. The public Taira chain identity selects the Taira layout.
#[derive(ClapArgs)]
pub struct Args {
    /// Sora network profile; `nexus` enforces public dataspace rules.
    #[arg(long, value_enum, value_name = "PROFILE")]
    profile: SoraProfileArg,
    /// Select an exact restricted dataspace preset for the `dataspace` profile.
    #[arg(long, value_enum, value_name = "DATASPACE")]
    private_dataspace: Option<PrivateDataspaceArg>,
    /// Number of validators to generate (an exact `3f + 1` committee, minimum four).
    #[arg(long, short, value_name = "COUNT", default_value_t = NonZeroU16::new(4).unwrap())]
    peers: NonZeroU16,
    /// Optional UTF-8 seed for deterministic keys.
    ///
    /// Omit this option to generate independent keys from operating-system entropy.
    #[arg(long, short)]
    seed: Option<String>,
    /// Canonical chain identifier written into genesis, peer configs, and the client config.
    #[arg(long, value_name = "CHAIN_ID", default_value = DEFAULT_CHAIN_ID)]
    chain_id: String,
    /// Account-address chain prefix written into genesis and client/peer configs.
    /// Public chain identities retain their fixed prefix.
    #[arg(long, value_name = "PREFIX")]
    chain_discriminant: Option<u16>,
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
    /// Override the immutable signed block cadence in milliseconds.
    /// Leave unset to use the one-second default cadence.
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u64).range(1..))]
    block_cadence_ms: Option<u64>,
}
impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            profile,
            private_dataspace,
            peers,
            seed,
            chain_id,
            chain_discriminant,
            bind_host,
            public_host,
            base_api_port,
            base_p2p_port,
            out_dir,
            block_cadence_ms,
        } = self;
        // Own the CLI seed in a zeroizing guard before any fallible request
        // validation. Once validation succeeds, `NetworkOptions` takes over
        // the same custody obligation through its `Drop` implementation.
        let mut seed = seed.map(Zeroizing::new);
        let sora_profile = resolve_sora_profile(profile, private_dataspace)?;
        let opts = NetworkOptions {
            sora_profile,
            peers,
            seed: seed.as_mut().map(|seed| std::mem::take(&mut **seed)),
            bind_host,
            public_host,
            base_api_port,
            base_p2p_port,
            out_dir,
            block_cadence_ms,
        };
        generate_network_with_chain(&opts, writer, Some(&chain_id), chain_discriminant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;
    use iroha_deploy::{
        genesis::profile::{
            PUBLIC_NEXUS_CHAIN_ID, PUBLIC_TAIRA_CHAIN_ID, known_chain_discriminant_for_chain_id,
        },
        sora_network::resolve_network_chain_id,
    };

    #[derive(clap::Parser)]
    struct TestArgs {
        #[command(flatten)]
        generate: Args,
    }

    #[test]
    fn beacon_launch_cli_requires_all_explicit_native_selectors() {
        #[derive(clap::Parser)]
        struct TestArgs {
            #[command(flatten)]
            launch: ValidateBeaconLaunchArgs,
        }
        let values = [
            "validate-beacon-launch",
            "--network-dir",
            "/private/run/network",
            "--peer-index",
            "0",
            "--beacon-config",
            "/private/run/beacon/seat-1/beacon.toml",
            "--beacon-credential",
            "/private/run/beacon/seat-1/iroha-global-beacon-partial-signer-v1.norito",
        ];
        let parsed = TestArgs::try_parse_from(values).unwrap();
        assert_eq!(parsed.launch.peer_index, 0);
        for missing in [1, 3, 5, 7] {
            let mut absent = values.to_vec();
            absent.drain(missing..missing + 2);
            assert!(TestArgs::try_parse_from(absent).is_err());
        }
        let mut invalid = values;
        invalid[4] = "4";
        assert!(TestArgs::try_parse_from(invalid).is_err());
        let mut extra = values.to_vec();
        extra.extend(["--runtime-toggle", "true"]);
        assert!(TestArgs::try_parse_from(extra).is_err());
    }

    #[test]
    fn profile_selector_is_typed_and_fail_closed() {
        for (name, expected) in [
            ("sbp", SoraProfile::PrivateSbp),
            ("cbuae", SoraProfile::PrivateCbuae),
            ("bpng", SoraProfile::PrivateBpng),
        ] {
            let parsed = TestArgs::try_parse_from([
                "kagami-sora-network-test",
                "--out-dir",
                "/tmp/kagami-sora-network-test",
                "--profile",
                "dataspace",
                "--private-dataspace",
                name,
            ])
            .expect("parse typed private dataspace selector");
            assert_eq!(
                resolve_sora_profile(parsed.generate.profile, parsed.generate.private_dataspace)
                    .expect("resolve typed private dataspace selector"),
                expected
            );
        }
        for (name, expected) in [
            ("nexus", SoraProfile::Nexus),
            ("dataspace", SoraProfile::Dataspace),
        ] {
            let parsed = TestArgs::try_parse_from([
                "kagami-sora-network-test",
                "--out-dir",
                "/tmp/kagami-sora-network-test",
                "--profile",
                name,
            ])
            .expect("parse Sora profile");
            assert_eq!(
                resolve_sora_profile(parsed.generate.profile, parsed.generate.private_dataspace)
                    .expect("resolve Sora profile"),
                expected
            );
        }
        assert!(
            TestArgs::try_parse_from([
                "kagami-sora-network-test",
                "--out-dir",
                "/tmp/kagami-sora-network-test",
                "--private-dataspace",
                "cbuae",
            ])
            .is_err(),
            "a Sora network requires an explicit profile"
        );
        assert!(
            resolve_sora_profile(SoraProfileArg::Nexus, Some(PrivateDataspaceArg::Cbuae)).is_err(),
            "private dataspace selection must reject the public Nexus profile"
        );
        for retired in ["dataspaces", "public", "sora-nexus", "nexus-public"] {
            assert!(
                TestArgs::try_parse_from([
                    "kagami-sora-network-test",
                    "--out-dir",
                    "/tmp/kagami-sora-network-test",
                    "--profile",
                    retired,
                ])
                .is_err(),
                "first-release CLI must reject retired profile alias {retired}"
            );
        }
        for developer_only in [
            ["--consensus-mode", "permissioned"],
            ["--perf-profile", "10k-npos"],
            ["--extra-accounts", "1"],
        ] {
            let mut args = vec![
                "kagami-sora-network-test",
                "--out-dir",
                "/tmp/kagami-sora-network-test",
                "--profile",
                "nexus",
            ];
            args.extend(developer_only);
            assert!(
                TestArgs::try_parse_from(args).is_err(),
                "Sora networks reject developer-only option {}",
                developer_only[0]
            );
        }
    }

    #[test]
    fn generator_accepts_the_canonical_taira_chain_id() {
        let parsed = TestArgs::try_parse_from([
            "kagami-sora-network-test",
            "--out-dir",
            "/tmp/kagami-sora-network-test",
            "--profile",
            "nexus",
            "--chain-id",
            PUBLIC_TAIRA_CHAIN_ID,
        ])
        .expect("parse explicit Taira chain id");
        assert_eq!(
            resolve_network_chain_id(Some(&parsed.generate.chain_id))
                .expect("resolve explicit Taira chain id"),
            PUBLIC_TAIRA_CHAIN_ID
        );
        assert!(resolve_network_chain_id(Some("   ")).is_err());
        assert!(resolve_network_chain_id(Some("iroha3-taira")).is_err());
        assert!(resolve_network_chain_id(Some("iroha3-nexus")).is_err());
        assert!(resolve_network_chain_id(Some("cbdc16")).is_err());
        assert!(
            resolve_network_chain_id(Some(PUBLIC_NEXUS_CHAIN_ID))
                .expect_err("a generated network cannot impersonate mainnet")
                .to_string()
                .contains("operator")
        );
        for padded in [
            format!(" {PUBLIC_TAIRA_CHAIN_ID}"),
            format!("{PUBLIC_TAIRA_CHAIN_ID} "),
            format!("{PUBLIC_TAIRA_CHAIN_ID}\n"),
            format!("\t{PUBLIC_TAIRA_CHAIN_ID}"),
        ] {
            assert!(
                resolve_network_chain_id(Some(&padded)).is_err(),
                "padded chain identity must fail rather than normalize: {padded:?}"
            );
        }
    }

    #[test]
    fn public_chain_prefix_conflicts_leave_no_partial_output() {
        let parent = tempfile::tempdir().expect("create public-chain validation parent");
        let private = iroha_fs::PrivateDirectory::open_or_create(parent.path().join("private"))
            .expect("create private public-chain validation parent");
        for (index, chain) in [PUBLIC_TAIRA_CHAIN_ID, PUBLIC_NEXUS_CHAIN_ID]
            .into_iter()
            .enumerate()
        {
            let output = private.path().join(format!("conflict{index}"));
            let wrong = known_chain_discriminant_for_chain_id(chain)
                .unwrap()
                .wrapping_add(1)
                .to_string();
            let parsed = TestArgs::try_parse_from([
                "kagami-sora-network-test",
                "--out-dir",
                output.to_str().unwrap(),
                "--chain-id",
                chain,
                "--chain-discriminant",
                &wrong,
                "--profile",
                "nexus",
            ])
            .expect("parse typed public-chain prefix conflict");
            assert!(
                parsed
                    .generate
                    .run(&mut BufWriter::new(Vec::new()))
                    .is_err()
            );
            assert!(
                !output.exists(),
                "invalid public prefix must not create output"
            );
        }
        assert!(
            TestArgs::try_parse_from([
                "kagami-sora-network-test",
                "--out-dir",
                "/tmp/unused-chain-prefix-test",
                "--profile",
                "nexus",
                "--chain-discriminant",
                "65536",
            ])
            .is_err()
        );
    }
}

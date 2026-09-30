//! CLI syntax for the shared native localnet generator.
use crate::{Outcome, RunArgs};
use clap::{Args as ClapArgs, ValueEnum};
use color_eyre::eyre::{Result, eyre};
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
pub(crate) enum SoraProfileArg {
    Dataspace,
    Nexus,
}
/// Canonical restricted-dataspace presets supported by the localnet generator.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivateDataspaceArg {
    /// State Bank of Pakistan dataspace (id 10, lane 3).
    Sbp,
    /// Central Bank of the UAE dataspace (id 12, lane 4).
    Cbuae,
    /// Bank of Papua New Guinea local dataspace (id 8648377547929788715, lane 5).
    Bpng,
}
impl From<SoraProfileArg> for SoraProfile {
    fn from(value: SoraProfileArg) -> Self {
        match value {
            SoraProfileArg::Dataspace => SoraProfile::Dataspace,
            SoraProfileArg::Nexus => SoraProfile::Nexus,
        }
    }
}
fn resolve_sora_profile(
    profile: Option<SoraProfileArg>,
    private_dataspace: Option<PrivateDataspaceArg>,
) -> Result<Option<SoraProfile>> {
    match (profile, private_dataspace) {
        (None, None) => Ok(None),
        (Some(profile), None) => Ok(Some(profile.into())),
        (Some(SoraProfileArg::Dataspace), Some(PrivateDataspaceArg::Sbp)) => {
            Ok(Some(SoraProfile::PrivateSbp))
        }
        (Some(SoraProfileArg::Dataspace), Some(PrivateDataspaceArg::Cbuae)) => {
            Ok(Some(SoraProfile::PrivateCbuae))
        }
        (Some(SoraProfileArg::Dataspace), Some(PrivateDataspaceArg::Bpng)) => {
            Ok(Some(SoraProfile::PrivateBpng))
        }
        (Some(SoraProfileArg::Nexus) | None, Some(_)) => Err(eyre!(
            "`--private-dataspace` requires `--sora-profile dataspace`"
        )),
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
/// Generate a bare-metal local network (no Docker): genesis, per-peer configs, start/stop scripts.
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
    /// Canonical chain identifier written into genesis, peer configs, and the client config.
    #[arg(long, value_name = "CHAIN_ID", default_value = DEFAULT_CHAIN_ID)]
    chain_id: String,
    /// Enable Sora profile defaults; `nexus` enforces public dataspace rules (NPoS).
    /// Requires at least 4 peers.
    #[arg(long, value_enum, value_name = "PROFILE")]
    sora_profile: Option<SoraProfileArg>,
    /// Select an exact restricted dataspace preset for the `dataspace` Sora profile.
    #[arg(long, value_enum, value_name = "DATASPACE", requires = "sora_profile")]
    private_dataspace: Option<PrivateDataspaceArg>,
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
    /// The built-in KAGEMUSHA V1 asset is always emitted.
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
    /// Defaults to `permissioned`.
    /// Sora profile localnets and perf profiles require `npos`.
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
            sora_profile,
            private_dataspace,
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
        let sora_profile = resolve_sora_profile(sora_profile, private_dataspace)?;
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
            sora_profile,
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
        generate_localnet_with_chain(&opts, writer, Some(&chain_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_deploy::genesis::profile::{PUBLIC_NEXUS_CHAIN_ID, PUBLIC_TAIRA_CHAIN_ID};
    #[test]
    fn private_dataspace_cli_selector_is_typed_and_fail_closed() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct TestArgs {
            #[command(flatten)]
            localnet: Args,
        }
        for (name, expected) in [
            ("sbp", SoraProfile::PrivateSbp),
            ("cbuae", SoraProfile::PrivateCbuae),
            ("bpng", SoraProfile::PrivateBpng),
        ] {
            let parsed = TestArgs::try_parse_from([
                "kagami-localnet-test",
                "--out-dir",
                "/tmp/kagami-localnet-test",
                "--sora-profile",
                "dataspace",
                "--private-dataspace",
                name,
            ])
            .expect("parse typed private dataspace selector");
            assert_eq!(
                resolve_sora_profile(
                    parsed.localnet.sora_profile,
                    parsed.localnet.private_dataspace,
                )
                .expect("resolve typed private dataspace selector"),
                Some(expected)
            );
        }
        assert!(
            TestArgs::try_parse_from([
                "kagami-localnet-test",
                "--out-dir",
                "/tmp/kagami-localnet-test",
                "--private-dataspace",
                "cbuae",
            ])
            .is_err(),
            "private dataspace selection must require an explicit Sora profile"
        );
        assert!(
            resolve_sora_profile(
                Some(SoraProfileArg::Nexus),
                Some(PrivateDataspaceArg::Cbuae),
            )
            .is_err(),
            "private dataspace selection must reject the public Nexus profile"
        );
        for retired in ["dataspaces", "public", "sora-nexus", "nexus-public"] {
            assert!(
                TestArgs::try_parse_from([
                    "kagami-localnet-test",
                    "--out-dir",
                    "/tmp/kagami-localnet-test",
                    "--sora-profile",
                    retired,
                ])
                .is_err(),
                "first-release CLI must reject retired profile alias {retired}"
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
    fn localnet_cli_accepts_an_explicit_canonical_chain_id() {
        use clap::Parser as _;
        #[derive(clap::Parser)]
        struct TestArgs {
            #[command(flatten)]
            localnet: Args,
        }
        let parsed = TestArgs::try_parse_from([
            "kagami-localnet-test",
            "--out-dir",
            "/tmp/kagami-localnet-test",
            "--chain-id",
            PUBLIC_TAIRA_CHAIN_ID,
        ])
        .expect("parse explicit localnet chain id");
        assert_eq!(
            resolve_localnet_chain_id(Some(&parsed.localnet.chain_id))
                .expect("resolve explicit localnet chain id"),
            PUBLIC_TAIRA_CHAIN_ID
        );
        assert!(resolve_localnet_chain_id(Some("   ")).is_err());
        assert!(resolve_localnet_chain_id(Some("iroha3-taira")).is_err());
        assert!(resolve_localnet_chain_id(Some("iroha3-nexus")).is_err());
        assert!(resolve_localnet_chain_id(Some("cbdc16")).is_err());
        assert!(
            resolve_localnet_chain_id(Some(PUBLIC_NEXUS_CHAIN_ID))
                .expect_err("disposable localnet cannot impersonate mainnet")
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
                resolve_localnet_chain_id(Some(&padded)).is_err(),
                "padded chain identity must fail rather than normalize: {padded:?}"
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
    fn localnet_defaults_to_permissioned_without_profile_or_perf_preset() {
        let mode = resolve_requested_consensus_mode(None, None);
        assert_eq!(mode, SumeragiConsensusMode::Permissioned);
    }
}

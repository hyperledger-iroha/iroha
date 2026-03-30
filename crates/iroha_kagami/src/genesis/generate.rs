use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use color_eyre::eyre::WrapErr as _;
use iroha_core::sumeragi::network_topology::redundant_send_r_from_len;
use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    parameter::{
        Parameter, Parameters,
        custom::{CustomParameter, CustomParameterId},
        system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
    },
    prelude::*,
};
use iroha_executor_data_model::permission::{
    account::CanRegisterAccount, domain::CanRegisterDomain, parameter::CanSetParameters,
};
use iroha_genesis::{GenesisBuilder, ManifestCrypto, RawGenesisTransaction};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, CARPENTER_ID, gen_account_in};
use iroha_version::BuildLine;

use crate::{
    Outcome, RunArgs,
    genesis::profile::{
        GenesisProfile, ProfileDefaults, known_chain_discriminant_for_chain_id, parse_vrf_seed_hex,
        profile_defaults, profile_requires_npos, resolve_vrf_seed,
    },
    tui,
};

/// Generate a genesis configuration and standard-output in JSON format
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Optional profile: picks Iroha3 defaults for dev/taira/nexus (sets chain id, DA/RBC, collector knobs).
    #[clap(long, value_enum, value_name = "PROFILE")]
    profile: Option<GenesisProfile>,
    /// Optional explicit chain id (overrides profile default).
    #[clap(long, value_name = "CHAIN_ID")]
    chain_id: Option<ChainId>,
    /// Optional VRF seed (hex, 32 bytes). Required for `iroha3-taira`/`iroha3-nexus`
    /// when NPoS is selected; ignored for permissioned manifests.
    #[clap(long, value_name = "HEX")]
    vrf_seed_hex: Option<String>,
    /// Optional path (relative to output) to the executor bytecode file (.to).
    /// If omitted, no executor upgrade is included in genesis.
    #[clap(long, value_name = "PATH")]
    executor: Option<PathBuf>,
    /// Relative path from the directory of output file to the directory that contains IVM bytecode libraries
    #[clap(long, value_name = "PATH")]
    ivm_dir: PathBuf,
    #[clap(long, value_name = "MULTI_HASH")]
    genesis_public_key: PublicKey,
    #[clap(subcommand)]
    mode: Option<Mode>,
    /// Optional: set the custom parameter `ivm_gas_limit_per_block` (u64) in genesis so all peers agree on the block gas budget.
    /// If omitted, a sensible default (1,680,000) is applied.
    #[clap(long, value_name = "U64")]
    ivm_gas_limit_per_block: Option<u64>,
    /// Select the consensus mode snapshot to seed in the genesis parameters
    /// (public dataspace requires NPoS; other Iroha3 dataspaces may use permissioned or NPoS;
    /// Iroha2 defaults to permissioned).
    #[clap(long, value_enum, value_name = "MODE")]
    consensus_mode: Option<ConsensusModeArg>,
    /// Optional future consensus mode to stage behind `--mode-activation-height`
    /// (Iroha2 only; Iroha3 disallows staged cutovers).
    #[clap(long, value_enum, value_name = "MODE")]
    next_consensus_mode: Option<ConsensusModeArg>,
    /// Optional: set the block height at which `next_mode` should activate (requires `--next-consensus-mode`).
    #[clap(long, value_name = "HEIGHT")]
    mode_activation_height: Option<u64>,
    /// Override cryptography snapshot fields in the generated manifest.
    #[clap(flatten)]
    crypto: CryptoArgs,
}

#[derive(ClapArgs, Debug, Clone, Default)]
struct CryptoArgs {
    /// Toggle the OpenSSL-backed SM preview helpers in the generated manifest.
    #[clap(long, value_name = "BOOL")]
    sm_openssl_preview: Option<bool>,
    /// Override the default hash advertised in the manifest.
    #[clap(long, value_name = "HASH")]
    default_hash: Option<String>,
    /// Replace the allowed signing algorithms (repeat flag to supply multiple values).
    #[clap(long = "allowed-signing", value_name = "ALGO", value_enum)]
    allowed_signing: Vec<AlgorithmArg>,
    /// Override the fallback SM2 distinguishing identifier.
    #[clap(long, value_name = "DISTID")]
    sm2_distid_default: Option<String>,
    /// Override the allowed curve identifiers (repeat flag to supply multiple values).
    #[clap(long = "allowed-curve-id", value_name = "CURVE_ID")]
    allowed_curve_ids: Vec<u8>,
}

impl CryptoArgs {
    fn into_manifest_crypto(self) -> color_eyre::Result<ManifestCrypto> {
        let mut crypto = ManifestCrypto::default();

        if !self.allowed_signing.is_empty() {
            crypto.allowed_signing = self
                .allowed_signing
                .into_iter()
                .map(Algorithm::from)
                .collect();
        }

        if let Some(flag) = self.sm_openssl_preview {
            crypto.sm_openssl_preview = flag;
        }

        if let Some(hash) = self.default_hash {
            crypto.default_hash = hash;
        }

        if let Some(distid) = self.sm2_distid_default {
            crypto.sm2_distid_default = distid;
        }

        if !self.allowed_curve_ids.is_empty() {
            crypto.allowed_curve_ids = self.allowed_curve_ids;
        }

        crypto.allowed_signing.sort();
        crypto.allowed_signing.dedup();
        if !crypto
            .allowed_signing
            .iter()
            .any(|algo| matches!(algo, Algorithm::Ed25519))
        {
            crypto.allowed_signing.insert(0, Algorithm::Ed25519);
        }

        crypto.validate()?;
        Ok(crypto)
    }
}

#[derive(ValueEnum, Clone, Debug)]
enum AlgorithmArg {
    Ed25519,
    Secp256k1,
    #[cfg(feature = "sm")]
    Sm2,
}

impl From<AlgorithmArg> for Algorithm {
    fn from(value: AlgorithmArg) -> Self {
        match value {
            AlgorithmArg::Ed25519 => Algorithm::Ed25519,
            AlgorithmArg::Secp256k1 => Algorithm::Secp256k1,
            #[cfg(feature = "sm")]
            AlgorithmArg::Sm2 => Algorithm::Sm2,
        }
    }
}

#[derive(Subcommand, Debug, Clone, Copy, Default)]
pub enum Mode {
    /// Generate default genesis
    #[default]
    Default,
    /// Generate synthetic genesis with the specified number of domains, accounts and assets.
    ///
    /// Synthetic mode is useful when we need a semi-realistic genesis for stress-testing
    /// Iroha's startup times as well as being able to just start an Iroha network and have
    /// instructions that represent a typical blockchain after migration.
    Synthetic {
        /// Number of domains in synthetic genesis.
        #[clap(long, default_value_t)]
        domains: u64,
        /// Number of accounts per domains in synthetic genesis.
        /// The total number of accounts would be `domains * accounts_per_domain`.
        #[clap(long, default_value_t)]
        accounts_per_domain: u64,
        /// Number of asset definitions per domain in synthetic genesis.
        /// The total number of asset definitions would be `domains * asset_definitions_per_domain`.
        #[clap(long, default_value_t)]
        asset_definitions_per_domain: u64,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ConsensusModeArg {
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

#[derive(Debug)]
struct ResolvedGenesisSettings {
    chain: ChainId,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    profile_vrf_seed: Option<[u8; 32]>,
}

fn validate_consensus_cutover(
    next_consensus_mode: Option<SumeragiConsensusMode>,
    mode_activation_height: Option<u64>,
) -> color_eyre::Result<()> {
    match (next_consensus_mode, mode_activation_height) {
        (Some(_), None) => {
            return Err(color_eyre::eyre::eyre!(
                "`--next-consensus-mode` requires `--mode-activation-height` to stage a cutover"
            ));
        }
        (None, Some(_)) => {
            return Err(color_eyre::eyre::eyre!(
                "`--mode-activation-height` requires `--next-consensus-mode` to stage a cutover"
            ));
        }
        _ => {}
    }

    if let Some(height) = mode_activation_height
        && height == 0
    {
        return Err(color_eyre::eyre::eyre!(
            "`--mode-activation-height` must be greater than zero"
        ));
    }

    Ok(())
}

fn apply_profile_overrides(
    profile: GenesisProfile,
    chain_id: Option<&ChainId>,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    vrf_seed_override: Option<[u8; 32]>,
    ivm_gas_limit_per_block: Option<u64>,
    defaults: &ProfileDefaults,
) -> color_eyre::Result<ResolvedGenesisSettings> {
    if let Some(explicit_chain) = chain_id
        && explicit_chain != &defaults.chain_id
    {
        return Err(color_eyre::eyre::eyre!(
            "profile {profile:?} expects chain id `{}`; drop or align the `--chain-id` override",
            defaults.chain_id
        ));
    }

    if profile_requires_npos(profile) && !matches!(consensus_mode, SumeragiConsensusMode::Npos) {
        return Err(color_eyre::eyre::eyre!(
            "profile {profile:?} targets the public dataspace; use `--consensus-mode npos`"
        ));
    }

    if let Some(next) = next_consensus_mode {
        return Err(color_eyre::eyre::eyre!(
            "profile {profile:?} disallows staged cutovers; remove `--next-consensus-mode {next:?}`"
        ));
    }

    if let Some(gas_limit) = ivm_gas_limit_per_block
        && gas_limit != 1_680_000
    {
        return Err(color_eyre::eyre::eyre!(
            "profile {profile:?} pins `ivm_gas_limit_per_block` to 1_680_000; drop the override"
        ));
    }

    let chain = defaults.chain_id.clone();
    let wants_npos_seed = matches!(consensus_mode, SumeragiConsensusMode::Npos)
        || matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos));
    let profile_vrf_seed = if wants_npos_seed {
        Some(resolve_vrf_seed(profile, &chain, vrf_seed_override)?)
    } else {
        None
    };

    Ok(ResolvedGenesisSettings {
        chain,
        consensus_mode,
        next_consensus_mode,
        profile_vrf_seed,
    })
}

fn resolve_profile_settings(
    profile: Option<GenesisProfile>,
    chain_id: Option<&ChainId>,
    profile_defaults: Option<&ProfileDefaults>,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    vrf_seed_override: Option<[u8; 32]>,
    ivm_gas_limit_per_block: Option<u64>,
) -> color_eyre::Result<ResolvedGenesisSettings> {
    let mut chain = chain_id
        .cloned()
        .or_else(|| profile_defaults.map(|d| d.chain_id.clone()))
        .unwrap_or_else(|| ChainId::from("00000000-0000-0000-0000-000000000000"));
    let mut consensus_mode = consensus_mode;
    let mut next_consensus_mode = next_consensus_mode;
    let profile_vrf_seed = if let Some(profile) = profile {
        let defaults = profile_defaults.expect("profile defaults available when profile is set");
        let overrides = apply_profile_overrides(
            profile,
            chain_id,
            consensus_mode,
            next_consensus_mode,
            vrf_seed_override,
            ivm_gas_limit_per_block,
            defaults,
        )?;
        chain = overrides.chain;
        consensus_mode = overrides.consensus_mode;
        next_consensus_mode = overrides.next_consensus_mode;
        overrides.profile_vrf_seed
    } else {
        None
    };

    Ok(ResolvedGenesisSettings {
        chain,
        consensus_mode,
        next_consensus_mode,
        profile_vrf_seed,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_genesis_for_mode(
    mode: Mode,
    builder: GenesisBuilder,
    genesis_public_key: &PublicKey,
    ivm_gas_limit_per_block: Option<u64>,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    mode_activation_height: Option<u64>,
    profile_defaults: Option<&ProfileDefaults>,
    resolved_vrf_seed: Option<[u8; 32]>,
    build_line: BuildLine,
) -> color_eyre::Result<RawGenesisTransaction> {
    match mode {
        Mode::Default => generate_default(
            builder,
            genesis_public_key,
            ivm_gas_limit_per_block,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
            profile_defaults,
            resolved_vrf_seed,
            build_line,
        ),
        Mode::Synthetic {
            domains,
            accounts_per_domain,
            asset_definitions_per_domain,
        } => generate_synthetic(
            builder,
            genesis_public_key,
            ivm_gas_limit_per_block,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
            domains,
            accounts_per_domain,
            asset_definitions_per_domain,
            profile_defaults,
            resolved_vrf_seed,
            build_line,
        ),
    }
}

fn format_profile_summary(
    profile: GenesisProfile,
    summary_chain: &ChainId,
    profile_defaults: Option<&ProfileDefaults>,
    genesis: &RawGenesisTransaction,
    resolved_vrf_seed: Option<[u8; 32]>,
) -> String {
    let summary_fingerprint = genesis.consensus_fingerprint().unwrap_or("n/a");
    let collectors_k = profile_defaults.map_or(0, |d| d.collectors_k);
    let collectors_r = profile_defaults.map_or(0, |d| d.collectors_redundant_send_r);
    let vrf_seed_hex = resolved_vrf_seed.map_or_else(|| "n/a".to_string(), hex::encode_upper);
    let params = genesis.effective_parameters();
    let sumeragi = params.sumeragi();
    let da_enabled = sumeragi.da_enabled();
    format!(
        "kagami profile summary: profile={:?} chain_id={} da_enabled={} collectors_k={} redundant_send_r={} vrf_seed={} consensus_fingerprint={} kagami_version={}",
        profile,
        summary_chain,
        da_enabled,
        collectors_k,
        collectors_r,
        vrf_seed_hex,
        summary_fingerprint,
        env!("CARGO_PKG_VERSION")
    )
}

fn validate_vrf_seed_usage(
    resolved_vrf_seed: Option<[u8; 32]>,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
) -> color_eyre::Result<()> {
    if resolved_vrf_seed.is_some()
        && !matches!(consensus_mode, SumeragiConsensusMode::Npos)
        && !matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos))
    {
        return Err(color_eyre::eyre::eyre!(
            "`--vrf-seed-hex` applies only to NPoS consensus manifests"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusPolicy {
    /// Allow either permissioned or NPoS consensus.
    Any,
    /// Require NPoS (public dataspace rule).
    PublicDataspace,
}

pub fn validate_consensus_mode_for_line(
    build_line: BuildLine,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    policy: ConsensusPolicy,
) -> color_eyre::Result<()> {
    if matches!(policy, ConsensusPolicy::PublicDataspace)
        && consensus_mode != SumeragiConsensusMode::Npos
    {
        return Err(color_eyre::eyre::eyre!(
            "public dataspace requires `--consensus-mode npos` (permissioned is private-only)"
        ));
    }
    if build_line.is_iroha3() && next_consensus_mode.is_some() {
        return Err(color_eyre::eyre::eyre!(
            "Iroha3 does not support staged consensus cutovers; drop `--next-consensus-mode`"
        ));
    }
    Ok(())
}

impl<T: Write> RunArgs<T> for Args {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            profile,
            chain_id,
            vrf_seed_hex,
            executor,
            ivm_dir,
            genesis_public_key,
            mode,
            ivm_gas_limit_per_block,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
            crypto,
        } = self;

        let mode = mode.unwrap_or_default();
        let mode_label = match &mode {
            Mode::Default => "default genesis manifest",
            Mode::Synthetic { .. } => "synthetic genesis manifest",
        };
        tui::status(format!("Building {mode_label}"));

        let profile_defaults = profile.map(profile_defaults);
        let vrf_seed_override = vrf_seed_hex
            .map(|hex| parse_vrf_seed_hex(&hex))
            .transpose()
            .wrap_err("invalid --vrf-seed-hex")?;

        let profile_is_some = profile.is_some();
        let build_line = if profile_is_some {
            BuildLine::Iroha3
        } else {
            build_line_from_env()
        };

        let consensus_mode = consensus_mode.map_or_else(
            || {
                if build_line.is_iroha3() {
                    SumeragiConsensusMode::Npos
                } else {
                    SumeragiConsensusMode::Permissioned
                }
            },
            SumeragiConsensusMode::from,
        );
        let next_consensus_mode = next_consensus_mode.map(SumeragiConsensusMode::from);

        validate_consensus_cutover(next_consensus_mode, mode_activation_height)?;

        let crypto = crypto.into_manifest_crypto()?;

        let resolved = resolve_profile_settings(
            profile,
            chain_id.as_ref(),
            profile_defaults.as_ref(),
            consensus_mode,
            next_consensus_mode,
            vrf_seed_override,
            ivm_gas_limit_per_block,
        )?;
        let chain = resolved.chain;
        let consensus_mode = resolved.consensus_mode;
        let next_consensus_mode = resolved.next_consensus_mode;
        let profile_vrf_seed = resolved.profile_vrf_seed;

        let resolved_vrf_seed = profile_vrf_seed.or(vrf_seed_override);
        validate_vrf_seed_usage(resolved_vrf_seed, consensus_mode, next_consensus_mode)?;

        let summary_chain = chain.clone();
        let consensus_policy = match profile {
            Some(profile) if profile_requires_npos(profile) => ConsensusPolicy::PublicDataspace,
            _ => ConsensusPolicy::Any,
        };
        validate_consensus_mode_for_line(
            build_line,
            consensus_mode,
            next_consensus_mode,
            consensus_policy,
        )?;
        let builder = match executor {
            Some(path) => GenesisBuilder::new(chain, path, ivm_dir),
            None => GenesisBuilder::new_without_executor(chain, ivm_dir),
        }
        .with_crypto(crypto);
        let genesis = build_genesis_for_mode(
            mode,
            builder,
            &genesis_public_key,
            ivm_gas_limit_per_block,
            consensus_mode,
            next_consensus_mode,
            mode_activation_height,
            profile_defaults.as_ref(),
            resolved_vrf_seed,
            build_line,
        )?;
        let _chain_discriminant = profile_defaults
            .as_ref()
            .and_then(|defaults| defaults.chain_discriminant)
            .or_else(|| known_chain_discriminant_for_chain_id(summary_chain.as_str()))
            .map(ChainDiscriminantGuard::enter);
        let json = norito::json::to_json_pretty(&genesis)?;
        writeln!(writer, "{json}").wrap_err("failed to write serialized genesis to the buffer")?;
        if let Some(profile) = profile {
            let summary = format_profile_summary(
                profile,
                &summary_chain,
                profile_defaults.as_ref(),
                &genesis,
                resolved_vrf_seed,
            );
            eprintln!("{summary}");
        }
        tui::success("Genesis manifest generated");
        Ok(())
    }
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn generate_default(
    builder: GenesisBuilder,
    genesis_public_key: &PublicKey,
    ivm_gas_limit_per_block: Option<u64>,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    mode_activation_height: Option<u64>,
    profile_defaults: Option<&ProfileDefaults>,
    profile_vrf_seed: Option<[u8; 32]>,
    da_rbc_line: BuildLine,
) -> color_eyre::Result<RawGenesisTransaction> {
    let genesis_account_id = AccountId::new(genesis_public_key.clone());
    let meta = Metadata::default();
    let wonderland_name: Name = "wonderland".parse()?;
    let wonderland_domain = DomainId::new(wonderland_name.clone());
    let garden_of_live_flowers_name: Name = "garden_of_live_flowers".parse()?;
    let garden_of_live_flowers_domain = DomainId::new(garden_of_live_flowers_name.clone());
    let rose_asset_definition_id =
        AssetDefinitionId::new(wonderland_domain.clone(), "rose".parse()?);
    let cabbage_asset_definition_id =
        AssetDefinitionId::new(garden_of_live_flowers_domain.clone(), "cabbage".parse()?);

    let mut builder = builder
        .domain_with_metadata(wonderland_name, meta.clone())
        .account_with_metadata(ALICE_ID.signatory().clone(), meta.clone())
        .asset("rose".parse()?, NumericSpec::default())
        .finish_domain()
        .domain(garden_of_live_flowers_name)
        .account(CARPENTER_ID.signatory().clone())
        .asset("cabbage".parse()?, NumericSpec::default())
        .finish_domain();

    let mint = Mint::asset_numeric(
        13u32,
        AssetId::new(rose_asset_definition_id.clone(), ALICE_ID.clone()),
    );
    let mint_cabbage = Mint::asset_numeric(
        44u32,
        AssetId::new(cabbage_asset_definition_id, ALICE_ID.clone()),
    );
    let register_account_permission = Permission::new(
        <CanRegisterAccount as iroha_executor_data_model::permission::Permission>::name(),
        Json::from_string_unchecked(format!("{{\"domain\":\"{}\"}}", wonderland_domain)),
    );
    let grant_permission_to_set_parameters =
        Grant::account_permission(CanSetParameters, ALICE_ID.clone());
    let grant_permission_to_register_domains =
        Grant::account_permission(CanRegisterDomain, ALICE_ID.clone());
    let grant_permission_to_manage_soracloud = Grant::account_permission(
        Permission::new("CanManageSoracloud".into(), Json::new(())),
        ALICE_ID.clone(),
    );
    let grant_permission_to_register_accounts =
        Grant::account_permission(register_account_permission, ALICE_ID.clone());
    let transfer_rose_ownership = Transfer::asset_definition(
        genesis_account_id.clone(),
        rose_asset_definition_id,
        ALICE_ID.clone(),
    );

    let mut parameters = Parameters::default();
    if let Some(defaults) = profile_defaults {
        parameters.sumeragi.collectors_k = defaults.collectors_k;
        parameters.sumeragi.collectors_redundant_send_r = defaults.collectors_redundant_send_r;
    }
    apply_da_rbc_policy_for_line(&mut parameters, da_rbc_line);
    let active_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos);
    let wants_npos_defaults =
        active_npos || matches!(next_consensus_mode, Some(SumeragiConsensusMode::Npos));
    if wants_npos_defaults {
        if profile_defaults.is_none() {
            parameters.sumeragi.collectors_k = 3;
            parameters.sumeragi.collectors_redundant_send_r = redundant_send_r_from_len(4);
        }
        let defaults = profile_vrf_seed.map_or_else(SumeragiNposParameters::default, |seed| {
            SumeragiNposParameters::default().with_epoch_seed(seed)
        });
        parameters.set_parameter(Parameter::Custom(defaults.into()));
    }
    // Pin block-level gas limit for IVM across peers via a custom parameter.
    // Name: "ivm_gas_limit_per_block", payload: JSON u64 (1_680_000)
    let gas_param_id = CustomParameterId::new("ivm_gas_limit_per_block".parse()?);
    let gas_param_val = ivm_gas_limit_per_block.unwrap_or(1_680_000u64);
    let gas_param = CustomParameter::new(gas_param_id, Json::new(gas_param_val));
    for parameter in parameters.parameters() {
        builder = builder.append_parameter(parameter);
    }
    // Persist overrides via structured parameters so manifests stay canonical.
    builder = builder.append_parameter(Parameter::Custom(gas_param));
    if let Some(mode) = next_consensus_mode {
        builder = builder.append_parameter(Parameter::Sumeragi(SumeragiParameter::NextMode(mode)));
    }
    if let (Some(height), Some(_)) = (mode_activation_height, next_consensus_mode) {
        builder = builder.append_parameter(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(height),
        ));
    }

    // Use transaction-oriented API: separate initial registrations from
    // subsequent state updates.
    builder = builder
        .next_transaction()
        .append_instruction(mint)
        .append_instruction(mint_cabbage)
        .append_instruction(transfer_rose_ownership)
        .append_instruction(grant_permission_to_set_parameters)
        .append_instruction(grant_permission_to_register_domains)
        .append_instruction(grant_permission_to_manage_soracloud)
        .append_instruction(grant_permission_to_register_accounts);

    let manifest = builder.build_raw().with_consensus_mode(consensus_mode);
    // Enrich with consensus metadata and fingerprint for operator visibility.
    Ok(manifest.with_consensus_meta())
}

#[cfg(test)]
mod da_tests {
    use iroha_data_model::parameter::custom::CustomParameterId;
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    use super::*;

    #[test]
    fn synthetic_genesis_includes_consensus_metadata() {
        let builder = GenesisBuilder::new_without_executor(
            ChainId::from("synthetic-meta"),
            PathBuf::from("."),
        );
        let manifest = generate_synthetic(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
            0,
            0,
            0,
            None,
            None,
            BuildLine::Iroha3,
        )
        .expect("generate synthetic genesis manifest");

        assert!(
            manifest.consensus_mode().is_some(),
            "synthetic manifest should advertise consensus_mode"
        );
        assert!(
            manifest.consensus_fingerprint().is_some(),
            "synthetic manifest should carry consensus_fingerprint"
        );
        assert!(
            manifest.bls_domain().is_some(),
            "synthetic manifest should include bls_domain"
        );
        assert!(
            !manifest.wire_proto_versions().is_empty(),
            "synthetic manifest should list supported wire_proto_versions"
        );
    }

    #[test]
    fn default_genesis_includes_activation_height_when_requested() {
        let builder =
            GenesisBuilder::new_without_executor(ChainId::from("mode-cutover"), PathBuf::from("."));
        let manifest = generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            Some(SumeragiConsensusMode::Npos),
            Some(5),
            None,
            None,
            BuildLine::Iroha3,
        )
        .expect("generate default genesis manifest");

        let params = manifest.effective_parameters();
        assert_eq!(
            params.sumeragi().next_mode(),
            Some(SumeragiConsensusMode::Npos),
            "genesis should set next_mode"
        );
        assert_eq!(
            params.sumeragi().mode_activation_height(),
            Some(5),
            "genesis should include mode_activation_height when requested"
        );
    }

    #[test]
    fn profile_vrf_seed_is_applied() {
        let defaults = profile_defaults(GenesisProfile::Iroha3Dev);
        let builder =
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from("."));
        let seed = [9u8; 32];
        let manifest = generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
            Some(&defaults),
            Some(seed),
            BuildLine::Iroha3,
        )
        .expect("generate genesis with profile seed");

        let params = manifest.effective_parameters();
        let npos_param_id = SumeragiNposParameters::parameter_id();
        let npos = params
            .custom()
            .get(&npos_param_id)
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters should be present");
        assert_eq!(npos.epoch_seed(), seed);
    }

    #[test]
    fn profile_collectors_and_da_are_applied() {
        let defaults = profile_defaults(GenesisProfile::Iroha3Nexus);
        let builder =
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from("."));
        let seed = [5u8; 32];
        let manifest = generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
            Some(&defaults),
            Some(seed),
            BuildLine::Iroha3,
        )
        .expect("generate profile manifest");

        let params = manifest.effective_parameters();
        assert_eq!(params.sumeragi().collectors_k(), defaults.collectors_k);
        assert_eq!(
            params.sumeragi().collectors_redundant_send_r(),
            defaults.collectors_redundant_send_r
        );
        assert!(params.sumeragi().da_enabled());

        let gas_param_id = CustomParameterId::new("ivm_gas_limit_per_block".parse().unwrap());
        let gas_param = params
            .custom()
            .get(&gas_param_id)
            .expect("gas limit parameter should be present");
        let limit = gas_param
            .payload()
            .try_into_any::<u64>()
            .expect("gas limit should be a u64");
        assert_eq!(limit, 1_680_000);
    }
}

#[cfg(test)]
mod profile_cli_tests {
    use std::io::{BufWriter, Write};

    use iroha_data_model::parameter::system::{SumeragiConsensusMode, SumeragiNposParameters};
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    use super::*;

    fn base_profile_args(profile: GenesisProfile) -> Args {
        Args {
            profile: Some(profile),
            chain_id: None,
            vrf_seed_hex: None,
            executor: None,
            ivm_dir: PathBuf::from("."),
            genesis_public_key: SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
            mode: None,
            ivm_gas_limit_per_block: None,
            consensus_mode: Some(ConsensusModeArg::Npos),
            next_consensus_mode: None,
            mode_activation_height: None,
            crypto: CryptoArgs::default(),
        }
    }

    fn run_and_parse(args: Args) -> color_eyre::Result<RawGenesisTransaction> {
        let json = run_to_string(args)?;
        let manifest: RawGenesisTransaction =
            norito::json::from_str(&json).map_err(|err| color_eyre::eyre::eyre!(err))?;
        Ok(manifest)
    }

    fn run_to_string(args: Args) -> color_eyre::Result<String> {
        let mut buf = BufWriter::new(Vec::new());
        args.run(&mut buf)?;
        buf.flush().expect("flush buffer");
        let bytes = buf.into_inner().expect("buffer into inner");
        Ok(String::from_utf8(bytes).expect("utf8 output"))
    }

    #[test]
    fn profile_rejects_permissioned_mode() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Nexus);
        args.consensus_mode = Some(ConsensusModeArg::Permissioned);

        let err = run_and_parse(args).expect_err("public dataspace should demand NPoS");
        assert!(
            err.to_string().contains("public dataspace"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn profile_rejects_staged_cutover() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Dev);
        args.next_consensus_mode = Some(ConsensusModeArg::Npos);
        args.mode_activation_height = Some(10);

        let err = run_and_parse(args).expect_err("staged cutover should be rejected");
        assert!(
            err.to_string().contains("disallows staged cutovers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dev_profile_allows_permissioned_mode() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Dev);
        args.consensus_mode = Some(ConsensusModeArg::Permissioned);

        let manifest = run_and_parse(args).expect("permissioned should be accepted");
        assert_eq!(
            manifest.consensus_mode(),
            Some(SumeragiConsensusMode::Permissioned)
        );
    }

    #[test]
    fn profile_rejects_chain_override_mismatch() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Dev);
        args.chain_id = Some(ChainId::from("other-chain"));

        let err = run_and_parse(args).expect_err("chain mismatch should be rejected");
        assert!(
            err.to_string()
                .contains("expects chain id `iroha3-dev.local`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn profile_rejects_non_default_gas_limit() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Dev);
        args.ivm_gas_limit_per_block = Some(42);

        let err = run_and_parse(args).expect_err("gas limit override should be rejected");
        assert!(
            err.to_string()
                .contains("pins `ivm_gas_limit_per_block` to 1_680_000"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn vrf_seed_only_allowed_for_npos_manifests() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Dev);
        args.profile = None;
        args.consensus_mode = Some(ConsensusModeArg::Permissioned);
        args.vrf_seed_hex = Some(hex::encode([1u8; 32]));

        let err = run_and_parse(args).expect_err("vrf seed should be rejected for permissioned");
        assert!(
            err.to_string()
                .contains("`--vrf-seed-hex` applies only to NPoS consensus manifests"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn defaults_to_npos_on_iroha3_build_line() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Dev);
        args.profile = None;
        args.consensus_mode = None;
        args.next_consensus_mode = None;

        let manifest = run_and_parse(args).expect("default should build");
        assert_eq!(manifest.consensus_mode(), Some(SumeragiConsensusMode::Npos));
    }

    #[test]
    fn dev_profile_derives_vrf_seed() {
        let defaults = profile_defaults(GenesisProfile::Iroha3Dev);
        let manifest = run_and_parse(base_profile_args(GenesisProfile::Iroha3Dev))
            .expect("dev profile should succeed");

        let params = manifest.effective_parameters();
        let npos_param_id = SumeragiNposParameters::parameter_id();
        let npos = params
            .custom()
            .get(&npos_param_id)
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters should be present");
        let expected_seed = crate::genesis::profile::derive_vrf_seed_from_chain(&defaults.chain_id);
        assert_eq!(npos.epoch_seed(), expected_seed);
    }

    #[test]
    fn taira_profile_requires_explicit_seed() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Taira);
        args.vrf_seed_hex = None;

        let err = run_and_parse(args).expect_err("taira profile should require seed");
        assert!(
            err.to_string().contains("vrf-seed-hex"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn taira_profile_applies_explicit_seed() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Taira);
        let seed = [0x11u8; 32];
        args.vrf_seed_hex = Some(hex::encode(seed));

        let manifest = run_and_parse(args).expect("taira profile with seed should succeed");
        let params = manifest.effective_parameters();
        let npos_param_id = SumeragiNposParameters::parameter_id();
        let npos = params
            .custom()
            .get(&npos_param_id)
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("npos parameters should be present");
        assert_eq!(npos.epoch_seed(), seed);
    }

    #[test]
    fn taira_profile_renders_test_prefix_account_literals() {
        let mut args = base_profile_args(GenesisProfile::Iroha3Taira);
        args.vrf_seed_hex = Some(hex::encode([0x22u8; 32]));

        let json = run_to_string(args).expect("taira profile should render");
        assert!(
            json.contains("\"testu"),
            "taira profile JSON should contain testnet i105 literals: {json}"
        );
        assert!(
            !json.contains("\"sorau"),
            "taira profile JSON must not leak mainnet i105 literals: {json}"
        );
    }
}

#[cfg(test)]
mod helper_tests {
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    use super::*;

    #[test]
    fn validate_consensus_cutover_rejects_missing_pairs() {
        let err = validate_consensus_cutover(Some(SumeragiConsensusMode::Npos), None)
            .expect_err("cutover should require activation height");
        assert!(
            err.to_string().contains("`--mode-activation-height`"),
            "unexpected error: {err}"
        );

        let err = validate_consensus_cutover(None, Some(10))
            .expect_err("activation height should require next mode");
        assert!(
            err.to_string().contains("`--next-consensus-mode`"),
            "unexpected error: {err}"
        );

        let err = validate_consensus_cutover(Some(SumeragiConsensusMode::Npos), Some(0))
            .expect_err("activation height should reject zero");
        assert!(
            err.to_string().contains("must be greater than zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_consensus_cutover_accepts_valid_inputs() {
        assert!(validate_consensus_cutover(None, None).is_ok());
        assert!(validate_consensus_cutover(Some(SumeragiConsensusMode::Npos), Some(7)).is_ok());
    }

    #[test]
    fn apply_profile_overrides_rejects_chain_mismatch() {
        let profile = GenesisProfile::Iroha3Dev;
        let defaults = profile_defaults(profile);
        let err = apply_profile_overrides(
            profile,
            Some(&ChainId::from("other-chain")),
            SumeragiConsensusMode::Npos,
            None,
            None,
            None,
            &defaults,
        )
        .expect_err("chain mismatch should be rejected");
        assert!(
            err.to_string().contains("expects chain id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn apply_profile_overrides_apply_defaults_and_seed() {
        let profile = GenesisProfile::Iroha3Dev;
        let defaults = profile_defaults(profile);
        let overrides = apply_profile_overrides(
            profile,
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
            None,
            &defaults,
        )
        .expect("profile overrides should succeed");

        assert_eq!(overrides.chain, defaults.chain_id);
        assert_eq!(overrides.consensus_mode, SumeragiConsensusMode::Npos);
        assert!(overrides.next_consensus_mode.is_none());
        let expected_seed =
            resolve_vrf_seed(profile, &defaults.chain_id, None).expect("resolve seed");
        assert_eq!(overrides.profile_vrf_seed, Some(expected_seed));
    }

    #[test]
    fn apply_profile_overrides_allows_permissioned_mode_for_dev() {
        let profile = GenesisProfile::Iroha3Dev;
        let defaults = profile_defaults(profile);
        let overrides = apply_profile_overrides(
            profile,
            None,
            SumeragiConsensusMode::Permissioned,
            None,
            None,
            None,
            &defaults,
        )
        .expect("permissioned dev profile should succeed");

        assert_eq!(overrides.chain, defaults.chain_id);
        assert_eq!(
            overrides.consensus_mode,
            SumeragiConsensusMode::Permissioned
        );
        assert!(overrides.profile_vrf_seed.is_none());
    }

    #[test]
    fn resolve_profile_settings_uses_chain_override_when_no_profile() {
        let chain_id = ChainId::from("explicit-chain");
        let resolved = resolve_profile_settings(
            None,
            Some(&chain_id),
            None,
            SumeragiConsensusMode::Permissioned,
            None,
            None,
            None,
        )
        .expect("settings should resolve");

        assert_eq!(resolved.chain, chain_id);
        assert_eq!(resolved.consensus_mode, SumeragiConsensusMode::Permissioned);
        assert!(resolved.next_consensus_mode.is_none());
        assert!(resolved.profile_vrf_seed.is_none());
    }

    #[test]
    fn resolve_profile_settings_applies_profile_defaults() {
        let profile = GenesisProfile::Iroha3Dev;
        let defaults = profile_defaults(profile);
        let resolved = resolve_profile_settings(
            Some(profile),
            None,
            Some(&defaults),
            SumeragiConsensusMode::Npos,
            None,
            None,
            None,
        )
        .expect("profile settings should resolve");

        assert_eq!(resolved.chain, defaults.chain_id);
        assert_eq!(resolved.consensus_mode, SumeragiConsensusMode::Npos);
        assert!(resolved.next_consensus_mode.is_none());
        assert!(resolved.profile_vrf_seed.is_some());
    }

    #[test]
    fn resolve_profile_settings_allows_permissioned_without_seed() {
        let profile = GenesisProfile::Iroha3Dev;
        let defaults = profile_defaults(profile);
        let resolved = resolve_profile_settings(
            Some(profile),
            None,
            Some(&defaults),
            SumeragiConsensusMode::Permissioned,
            None,
            None,
            None,
        )
        .expect("permissioned profile settings should resolve");

        assert_eq!(resolved.chain, defaults.chain_id);
        assert_eq!(resolved.consensus_mode, SumeragiConsensusMode::Permissioned);
        assert!(resolved.next_consensus_mode.is_none());
        assert!(resolved.profile_vrf_seed.is_none());
    }

    #[test]
    fn build_genesis_for_mode_handles_default_and_synthetic() {
        let chain_id = ChainId::from("mode-build");
        let builder = GenesisBuilder::new_without_executor(chain_id.clone(), PathBuf::from("."));
        let manifest = build_genesis_for_mode(
            Mode::Default,
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Permissioned,
            None,
            None,
            None,
            None,
            BuildLine::Iroha3,
        )
        .expect("default manifest should build");
        assert_eq!(
            manifest.consensus_mode(),
            Some(SumeragiConsensusMode::Permissioned)
        );

        let builder = GenesisBuilder::new_without_executor(chain_id, PathBuf::from("."));
        let manifest = build_genesis_for_mode(
            Mode::Synthetic {
                domains: 0,
                accounts_per_domain: 0,
                asset_definitions_per_domain: 0,
            },
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Permissioned,
            None,
            None,
            None,
            None,
            BuildLine::Iroha3,
        )
        .expect("synthetic manifest should build");
        assert_eq!(
            manifest.consensus_mode(),
            Some(SumeragiConsensusMode::Permissioned)
        );
    }

    #[test]
    fn format_profile_summary_includes_expected_fields() {
        let profile = GenesisProfile::Iroha3Dev;
        let defaults = profile_defaults(profile);
        let builder =
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from("."));
        let manifest = generate_default(
            builder,
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
            Some(&defaults),
            None,
            BuildLine::Iroha3,
        )
        .expect("manifest should build");
        let seed = [0xAB; 32];
        let summary = format_profile_summary(
            profile,
            &defaults.chain_id,
            Some(&defaults),
            &manifest,
            Some(seed),
        );
        assert!(summary.contains("profile=Iroha3Dev"));
        assert!(summary.contains("chain_id=iroha3-dev.local"));
        assert!(summary.contains("da_enabled=true"));
        assert!(summary.contains("collectors_k=1"));
        assert!(summary.contains("redundant_send_r=1"));
        let seed_hex = hex::encode_upper(seed);
        assert!(summary.contains(&format!("vrf_seed={seed_hex}")));
    }

    #[test]
    fn validate_vrf_seed_usage_rejects_permissioned() {
        let err =
            validate_vrf_seed_usage(Some([1u8; 32]), SumeragiConsensusMode::Permissioned, None)
                .expect_err("permissioned mode should reject vrf seed");
        assert!(
            err.to_string().contains("vrf-seed-hex"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_vrf_seed_usage_allows_npos() {
        assert!(
            validate_vrf_seed_usage(Some([2u8; 32]), SumeragiConsensusMode::Npos, None).is_ok()
        );
    }

    #[test]
    fn validate_consensus_mode_for_line_allows_permissioned_on_iroha3_without_policy() {
        assert!(
            validate_consensus_mode_for_line(
                BuildLine::Iroha3,
                SumeragiConsensusMode::Permissioned,
                None,
                ConsensusPolicy::Any
            )
            .is_ok()
        );
    }

    #[test]
    fn validate_consensus_mode_for_line_requires_npos_for_public_dataspace() {
        let err = validate_consensus_mode_for_line(
            BuildLine::Iroha3,
            SumeragiConsensusMode::Permissioned,
            None,
            ConsensusPolicy::PublicDataspace,
        )
        .expect_err("public dataspace should require npos");
        assert!(
            err.to_string().contains("public dataspace"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_consensus_mode_for_line_rejects_staged_cutover_on_iroha3() {
        let err = validate_consensus_mode_for_line(
            BuildLine::Iroha3,
            SumeragiConsensusMode::Npos,
            Some(SumeragiConsensusMode::Npos),
            ConsensusPolicy::Any,
        )
        .expect_err("iroha3 should reject staged cutover");
        assert!(
            err.to_string().contains("staged consensus cutovers"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_consensus_mode_for_line_allows_npos_on_iroha3() {
        assert!(
            validate_consensus_mode_for_line(
                BuildLine::Iroha3,
                SumeragiConsensusMode::Npos,
                None,
                ConsensusPolicy::PublicDataspace
            )
            .is_ok()
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_synthetic(
    builder: GenesisBuilder,
    genesis_public_key: &PublicKey,
    ivm_gas_limit_per_block: Option<u64>,
    consensus_mode: SumeragiConsensusMode,
    next_consensus_mode: Option<SumeragiConsensusMode>,
    mode_activation_height: Option<u64>,
    domains: u64,
    accounts_per_domain: u64,
    asset_definitions_per_domain: u64,
    profile_defaults: Option<&ProfileDefaults>,
    profile_vrf_seed: Option<[u8; 32]>,
    da_rbc_line: BuildLine,
) -> color_eyre::Result<RawGenesisTransaction> {
    // Synthetic genesis extends the default one with additional transactions
    // describing synthetic domains and assets.
    let default_genesis = generate_default(
        builder,
        genesis_public_key,
        ivm_gas_limit_per_block,
        consensus_mode,
        next_consensus_mode,
        mode_activation_height,
        profile_defaults,
        profile_vrf_seed,
        da_rbc_line,
    )?;
    let mut builder = default_genesis.into_builder().next_transaction();

    for domain in 0..domains {
        let domain_id: DomainId = format!("domain_{domain}").parse()?;
        builder = builder.append_instruction(Register::domain(Domain::new(domain_id.clone())));

        let mut synthetic_asset_definitions = Vec::new();
        for asset_definition in 0..asset_definitions_per_domain {
            let asset_name_literal = format!("asset_{asset_definition}");
            let asset_name: Name = asset_name_literal.parse()?;
            let asset_definition_id = AssetDefinitionId::new(domain_id.clone(), asset_name);
            synthetic_asset_definitions.push(asset_definition_id.clone());
            builder = builder.append_instruction(Register::asset_definition(
                AssetDefinition::new(asset_definition_id, NumericSpec::default())
                    .with_name(asset_name_literal),
            ));
        }

        for _ in 0..accounts_per_domain {
            let (account_id, _account_keypair) = gen_account_in(&domain_id);
            builder = builder.append_instruction(Register::account(
                Account::new(account_id.clone()),
            ));

            for asset_definition_id in &synthetic_asset_definitions {
                let mint = Mint::asset_numeric(
                    13u32,
                    AssetId::new(asset_definition_id.clone(), account_id.clone()),
                );
                builder = builder.append_instruction(mint);
            }
        }
    }

    let manifest = builder.build_raw().with_consensus_mode(consensus_mode);
    Ok(manifest.with_consensus_meta())
}

/// Resolve the build line from the binary name or `IROHA_BUILD_LINE` override.
pub fn build_line_from_env() -> BuildLine {
    const OVERRIDE_ENV: &str = "IROHA_BUILD_LINE";
    if let Ok(val) = std::env::var(OVERRIDE_ENV) {
        match val.to_ascii_lowercase().as_str() {
            "iroha2" | "i2" | "2" => return BuildLine::Iroha2,
            "iroha3" | "i3" | "3" => return BuildLine::Iroha3,
            other => eprintln!(
                "warning: {OVERRIDE_ENV}={other} is not a valid build line (expected iroha2/iroha3); falling back to binary name"
            ),
        }
    }
    BuildLine::from_bin_name(env!("CARGO_BIN_NAME"))
}

#[allow(dead_code)]
fn apply_da_rbc_policy_for_line(params: &mut Parameters, line: BuildLine) {
    params.sumeragi.da_enabled = line.is_iroha3();
}

#[cfg(test)]
mod tests {
    use std::io::BufWriter;

    use iroha_data_model::isi::GrantBox;
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    use super::*;

    #[test]
    fn da_rbc_policy_tracks_build_line() {
        let mut params = Parameters::default();
        apply_da_rbc_policy_for_line(&mut params, BuildLine::Iroha3);
        assert!(params.sumeragi.da_enabled);

        let mut params_i2 = Parameters::default();
        params_i2.sumeragi.da_enabled = true;
        apply_da_rbc_policy_for_line(&mut params_i2, BuildLine::Iroha2);
        assert!(!params_i2.sumeragi.da_enabled);
    }

    #[test]
    fn profile_defaults_assign_chain_and_collectors() {
        let dev = profile_defaults(GenesisProfile::Iroha3Dev);
        assert_eq!(dev.chain_id, ChainId::from("iroha3-dev.local"));
        assert_eq!(dev.chain_discriminant, None);
        assert_eq!(dev.collectors_k, 1);
        assert_eq!(dev.collectors_redundant_send_r, 1);

        let taira = profile_defaults(GenesisProfile::Iroha3Taira);
        assert_eq!(taira.chain_id, ChainId::from("iroha3-taira"));
        assert_eq!(taira.chain_discriminant, Some(369));
        assert_eq!(taira.collectors_k, 3);
        assert_eq!(taira.collectors_redundant_send_r, 3);

        let nexus = profile_defaults(GenesisProfile::Iroha3Nexus);
        assert_eq!(nexus.chain_id, ChainId::from("iroha3-nexus"));
        assert_eq!(nexus.chain_discriminant, Some(753));
        assert_eq!(nexus.collectors_k, 5);
        assert_eq!(nexus.collectors_redundant_send_r, 3);
    }

    #[test]
    fn generated_genesis_grants_alice_soracloud_management_permission() {
        let mut output = BufWriter::new(Vec::new());
        Args {
            profile: Some(GenesisProfile::Iroha3Dev),
            chain_id: None,
            vrf_seed_hex: None,
            executor: None,
            ivm_dir: PathBuf::from("."),
            genesis_public_key: SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
            mode: None,
            ivm_gas_limit_per_block: None,
            consensus_mode: Some(ConsensusModeArg::Permissioned),
            next_consensus_mode: None,
            mode_activation_height: None,
            crypto: CryptoArgs::default(),
        }
        .run(&mut output)
        .expect("genesis generation should succeed");
        output.flush().expect("flush generated manifest");
        let manifest: RawGenesisTransaction =
            norito::json::from_slice(&output.into_inner().expect("manifest bytes"))
                .expect("generated manifest should parse");
        let saw_permission = manifest.instructions().any(|instruction| {
            let Some(GrantBox::Permission(grant)) = instruction.as_any().downcast_ref::<GrantBox>()
            else {
                return false;
            };
            grant.destination == *ALICE_ID && grant.object.name() == "CanManageSoracloud"
        });
        assert!(
            saw_permission,
            "generated genesis should grant ALICE_ID CanManageSoracloud"
        );
    }
}

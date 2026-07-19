use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use color_eyre::eyre::WrapErr as _;
use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    asset::{AssetDefinitionAlias, definition::AssetConfidentialPolicy},
    parameter::{
        Parameter, Parameters,
        custom::{CustomParameter, CustomParameterId},
        system::{SumeragiConsensusMode, SumeragiNposParameters},
    },
    prelude::*,
};
use iroha_executor_data_model::permission::{
    account::CanRegisterAccount, parameter::CanSetParameters,
};
use iroha_genesis::{GenesisBuilder, ManifestCrypto, RawGenesisTransaction};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, CARPENTER_ID, gen_account_in};
use iroha_version::BuildLine;

use crate::{
    Outcome, RunArgs,
    genesis::profile::{
        GenesisProfile, PUBLIC_XOR_ALIAS, PUBLIC_XOR_DOMAIN, ProfileDefaults,
        known_chain_discriminant_for_chain_id, parse_vrf_seed_hex, profile_defaults,
        profile_requires_npos, resolve_public_xor_asset_definition_id, resolve_vrf_seed,
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
    /// Canonical public XOR asset definition id (Base58). Required for `iroha3-nexus`
    /// NPoS manifests; `iroha3-taira` defaults to its live XOR id.
    #[clap(long, value_name = "BASE58")]
    xor_asset_definition_id: Option<String>,
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
    profile_vrf_seed: Option<[u8; 32]>,
    public_xor_asset_definition_id: Option<AssetDefinitionId>,
}

fn apply_profile_overrides(
    profile: GenesisProfile,
    chain_id: Option<&ChainId>,
    consensus_mode: SumeragiConsensusMode,
    vrf_seed_override: Option<[u8; 32]>,
    xor_asset_definition_id: Option<&str>,
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

    if let Some(gas_limit) = ivm_gas_limit_per_block
        && gas_limit != 1_680_000
    {
        return Err(color_eyre::eyre::eyre!(
            "profile {profile:?} pins `ivm_gas_limit_per_block` to 1_680_000; drop the override"
        ));
    }

    let chain = defaults.chain_id.clone();
    let wants_npos_seed = matches!(consensus_mode, SumeragiConsensusMode::Npos);
    let profile_vrf_seed = if wants_npos_seed {
        Some(resolve_vrf_seed(profile, &chain, vrf_seed_override)?)
    } else {
        None
    };

    Ok(ResolvedGenesisSettings {
        chain,
        consensus_mode,
        profile_vrf_seed,
        public_xor_asset_definition_id: resolve_public_xor_asset_definition_id(
            Some(profile),
            xor_asset_definition_id,
            wants_npos_seed,
        )?,
    })
}

fn resolve_profile_settings(
    profile: Option<GenesisProfile>,
    chain_id: Option<&ChainId>,
    profile_defaults: Option<&ProfileDefaults>,
    consensus_mode: SumeragiConsensusMode,
    vrf_seed_override: Option<[u8; 32]>,
    xor_asset_definition_id: Option<&str>,
    ivm_gas_limit_per_block: Option<u64>,
) -> color_eyre::Result<ResolvedGenesisSettings> {
    let mut chain = chain_id
        .cloned()
        .or_else(|| profile_defaults.map(|d| d.chain_id.clone()))
        .unwrap_or_else(|| ChainId::from("00000000-0000-0000-0000-000000000000"));
    let mut consensus_mode = consensus_mode;
    let mut public_xor_asset_definition_id = None;
    let profile_vrf_seed = if let Some(profile) = profile {
        let defaults = profile_defaults.expect("profile defaults available when profile is set");
        let overrides = apply_profile_overrides(
            profile,
            chain_id,
            consensus_mode,
            vrf_seed_override,
            xor_asset_definition_id,
            ivm_gas_limit_per_block,
            defaults,
        )?;
        chain = overrides.chain;
        consensus_mode = overrides.consensus_mode;
        public_xor_asset_definition_id = overrides.public_xor_asset_definition_id;
        overrides.profile_vrf_seed
    } else {
        None
    };

    if profile.is_none() {
        let wants_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos);
        public_xor_asset_definition_id =
            resolve_public_xor_asset_definition_id(profile, xor_asset_definition_id, wants_npos)?;
    }

    Ok(ResolvedGenesisSettings {
        chain,
        consensus_mode,
        profile_vrf_seed,
        public_xor_asset_definition_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_genesis_for_mode(
    mode: Mode,
    builder: GenesisBuilder,
    genesis_public_key: &PublicKey,
    ivm_gas_limit_per_block: Option<u64>,
    consensus_mode: SumeragiConsensusMode,
    profile_defaults: Option<&ProfileDefaults>,
    resolved_vrf_seed: Option<[u8; 32]>,
) -> color_eyre::Result<RawGenesisTransaction> {
    let genesis = match mode {
        Mode::Default => generate_default(
            builder,
            genesis_public_key,
            ivm_gas_limit_per_block,
            consensus_mode,
            profile_defaults,
            resolved_vrf_seed,
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
            domains,
            accounts_per_domain,
            asset_definitions_per_domain,
            profile_defaults,
            resolved_vrf_seed,
        ),
    }?;

    Ok(apply_npos_crypto_overrides(genesis, consensus_mode))
}

fn apply_npos_crypto_overrides(
    genesis: RawGenesisTransaction,
    consensus_mode: SumeragiConsensusMode,
) -> RawGenesisTransaction {
    let npos_bootstrap = matches!(consensus_mode, SumeragiConsensusMode::Npos);
    if !npos_bootstrap {
        return genesis;
    }

    let mut crypto = genesis.crypto().clone();
    if !crypto
        .allowed_signing
        .iter()
        .any(|algo| matches!(algo, Algorithm::BlsNormal))
    {
        crypto.allowed_signing.push(Algorithm::BlsNormal);
    }
    crypto.allowed_signing.sort();
    crypto.allowed_signing.dedup();
    crypto.allowed_curve_ids = crypto
        .allowed_signing
        .iter()
        .filter_map(|algo| {
            iroha_data_model::account::curve::CurveId::try_from_algorithm(*algo).ok()
        })
        .map(iroha_data_model::account::curve::CurveId::as_u8)
        .collect();
    crypto.allowed_curve_ids.sort_unstable();
    crypto.allowed_curve_ids.dedup();

    genesis.into_builder().with_crypto(crypto).build_raw()
}

fn append_public_xor_binding(
    genesis: RawGenesisTransaction,
    asset_definition_id: &AssetDefinitionId,
) -> color_eyre::Result<RawGenesisTransaction> {
    let public_xor_domain = DomainId::parse_fully_qualified(PUBLIC_XOR_DOMAIN)?;
    let public_xor_alias: AssetDefinitionAlias = PUBLIC_XOR_ALIAS.parse()?;

    let mut has_domain = false;
    let mut has_asset_definition = false;
    let mut alias_bound = false;
    for instruction in genesis.instructions() {
        if let Some(register) = instruction.as_any().downcast_ref::<Register<Domain>>() {
            has_domain |= register.object.id == public_xor_domain;
            continue;
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<Register<AssetDefinition>>()
        {
            has_asset_definition |= register.object.id == *asset_definition_id;
            continue;
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::register::RegisterBox>()
        {
            match register {
                iroha_data_model::isi::register::RegisterBox::Domain(register) => {
                    has_domain |= register.object.id == public_xor_domain;
                }
                iroha_data_model::isi::register::RegisterBox::AssetDefinition(register) => {
                    has_asset_definition |= register.object.id == *asset_definition_id;
                }
                _ => {}
            }
            continue;
        }
        if let Some(bind) = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::asset_alias::SetAssetDefinitionAlias>(
        ) && bind.alias.as_ref() == Some(&public_xor_alias)
        {
            if bind.asset_definition_id != *asset_definition_id {
                return Err(color_eyre::eyre::eyre!(
                    "public XOR alias `{PUBLIC_XOR_ALIAS}` is already bound to `{}`, expected `{asset_definition_id}`",
                    bind.asset_definition_id
                ));
            }
            alias_bound = true;
        }
    }

    if has_domain && has_asset_definition && alias_bound {
        return Ok(genesis);
    }

    let mut builder = genesis.into_builder().next_transaction();
    if !has_domain {
        builder = builder.append_instruction(Register::domain(Domain::new(public_xor_domain)));
    }
    if !has_asset_definition {
        let definition = AssetDefinition::new(asset_definition_id.clone(), NumericSpec::default())
            .with_name("xor".to_owned())
            .confidential_policy(AssetConfidentialPolicy::convertible())
            .with_metadata(Metadata::default());
        builder = builder.append_instruction(Register::asset_definition(definition));
    }
    if !alias_bound {
        builder = builder.append_instruction(
            iroha_data_model::isi::asset_alias::SetAssetDefinitionAlias::bind(
                asset_definition_id.clone(),
                public_xor_alias,
                None,
            ),
        );
    }

    Ok(builder.build_raw().with_consensus_meta())
}

fn format_profile_summary(
    profile: GenesisProfile,
    summary_chain: &ChainId,
    profile_defaults: Option<&ProfileDefaults>,
    genesis: &RawGenesisTransaction,
    resolved_vrf_seed: Option<[u8; 32]>,
) -> String {
    let summary_fingerprint = genesis
        .consensus_fingerprint()
        .map_or_else(|| "n/a".to_owned(), |fingerprint| fingerprint.to_string());
    let vrf_seed_hex = resolved_vrf_seed.map_or_else(|| "n/a".to_string(), hex::encode_upper);
    format!(
        "kagami profile summary: profile={:?} chain_id={} block_cadence_ms={} vrf_seed={} consensus_fingerprint={} kagami_version={}",
        profile,
        summary_chain,
        profile_defaults.map_or(100, |defaults| defaults.block_cadence_ms.get()),
        vrf_seed_hex,
        summary_fingerprint,
        env!("CARGO_PKG_VERSION")
    )
}

fn validate_vrf_seed_usage(
    resolved_vrf_seed: Option<[u8; 32]>,
    consensus_mode: SumeragiConsensusMode,
) -> color_eyre::Result<()> {
    if resolved_vrf_seed.is_some() && !matches!(consensus_mode, SumeragiConsensusMode::Npos) {
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
    policy: ConsensusPolicy,
) -> color_eyre::Result<()> {
    if matches!(policy, ConsensusPolicy::PublicDataspace)
        && consensus_mode != SumeragiConsensusMode::Npos
    {
        return Err(color_eyre::eyre::eyre!(
            "public dataspace requires `--consensus-mode npos` (permissioned is private-only)"
        ));
    }
    let _ = build_line;
    Ok(())
}

impl<T: Write> RunArgs<T> for Args {
    #[allow(clippy::too_many_lines)]
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            profile,
            chain_id,
            vrf_seed_hex,
            xor_asset_definition_id,
            executor,
            ivm_dir,
            genesis_public_key,
            mode,
            ivm_gas_limit_per_block,
            consensus_mode,
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
        let crypto = crypto.into_manifest_crypto()?;

        let resolved = resolve_profile_settings(
            profile,
            chain_id.as_ref(),
            profile_defaults.as_ref(),
            consensus_mode,
            vrf_seed_override,
            xor_asset_definition_id.as_deref(),
            ivm_gas_limit_per_block,
        )?;
        let chain = resolved.chain;
        let consensus_mode = resolved.consensus_mode;
        let profile_vrf_seed = resolved.profile_vrf_seed;
        let public_xor_asset_definition_id = resolved.public_xor_asset_definition_id;

        let resolved_vrf_seed = profile_vrf_seed.or(vrf_seed_override);
        validate_vrf_seed_usage(resolved_vrf_seed, consensus_mode)?;

        let summary_chain = chain.clone();
        let consensus_policy = match profile {
            Some(profile) if profile_requires_npos(profile) => ConsensusPolicy::PublicDataspace,
            _ => ConsensusPolicy::Any,
        };
        validate_consensus_mode_for_line(build_line, consensus_mode, consensus_policy)?;
        let builder = match executor {
            Some(path) => GenesisBuilder::new(chain, path, ivm_dir),
            None => GenesisBuilder::new_without_executor(chain, ivm_dir),
        }
        .with_crypto(crypto);
        let mut genesis = build_genesis_for_mode(
            mode,
            builder,
            &genesis_public_key,
            ivm_gas_limit_per_block,
            consensus_mode,
            profile_defaults.as_ref(),
            resolved_vrf_seed,
        )?;
        if let Some(asset_definition_id) = public_xor_asset_definition_id.as_ref() {
            genesis = append_public_xor_binding(genesis, asset_definition_id)?;
        }
        let chain_discriminant = profile_defaults
            .as_ref()
            .and_then(|defaults| defaults.chain_discriminant)
            .or_else(|| known_chain_discriminant_for_chain_id(summary_chain.as_str()))
            .unwrap_or_else(iroha_data_model::account::address::chain_discriminant);
        let genesis = genesis.with_chain_discriminant(chain_discriminant);
        let _chain_discriminant = ChainDiscriminantGuard::enter(chain_discriminant);
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
    profile_defaults: Option<&ProfileDefaults>,
    profile_vrf_seed: Option<[u8; 32]>,
) -> color_eyre::Result<RawGenesisTransaction> {
    let genesis_account_id = AccountId::new(genesis_public_key.clone());
    let meta = Metadata::default();
    let wonderland_name: Name = "wonderland".parse()?;
    let universal_dataspace: Name = "universal".parse()?;
    let wonderland_domain =
        DomainId::try_new(wonderland_name.as_ref(), universal_dataspace.as_ref())?;
    let garden_of_live_flowers_name: Name = "garden_of_live_flowers".parse()?;
    let garden_of_live_flowers_domain = DomainId::try_new(
        garden_of_live_flowers_name.as_ref(),
        universal_dataspace.as_ref(),
    )?;
    let rose_asset_definition_id =
        AssetDefinitionId::new(wonderland_domain.clone(), "rose".parse()?);
    let cabbage_asset_definition_id =
        AssetDefinitionId::new(garden_of_live_flowers_domain.clone(), "cabbage".parse()?);

    let mut builder = builder
        .domain_with_metadata(wonderland_domain.clone(), meta.clone())
        .account_with_metadata(ALICE_ID.signatory().clone(), meta.clone())
        .asset("rose".parse()?, NumericSpec::default())
        .finish_domain()
        .domain(garden_of_live_flowers_domain.clone())
        .account(CARPENTER_ID.signatory().clone())
        .asset("cabbage".parse()?, NumericSpec::default())
        .finish_domain();

    let mint = Mint::asset_quantity(
        13u32,
        AssetId::new(rose_asset_definition_id.clone(), ALICE_ID.clone()),
    );
    let mint_cabbage = Mint::asset_quantity(
        44u32,
        AssetId::new(cabbage_asset_definition_id, ALICE_ID.clone()),
    );
    let register_account_permission = Permission::new(
        <CanRegisterAccount as iroha_executor_data_model::permission::Permission>::name(),
        Json::from_string_unchecked(format!("{{\"domain\":\"{}\"}}", wonderland_domain)),
    );
    let grant_permission_to_set_parameters =
        Grant::account_permission(CanSetParameters, ALICE_ID.clone());
    let grant_permission_to_manage_soracloud = Grant::account_permission(
        Permission::new("CanManageSoracloud".into(), Json::new(())),
        ALICE_ID.clone(),
    );
    let grant_permission_to_manage_verifying_keys = Grant::account_permission(
        Permission::new("CanManageVerifyingKeys".into(), Json::new(())),
        genesis_account_id.clone(),
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
        builder = builder.with_block_cadence_ms(defaults.block_cadence_ms);
    }
    let active_npos = matches!(consensus_mode, SumeragiConsensusMode::Npos);
    if active_npos {
        let seed = profile_vrf_seed.ok_or_else(|| {
            color_eyre::eyre::eyre!("NPoS genesis requires an explicit or profile-derived VRF seed")
        })?;
        let defaults = SumeragiNposParameters::default().with_epoch_seed(seed);
        defaults
            .validate()
            .map_err(|error| color_eyre::eyre::eyre!(error))?;
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
    builder = builder
        .next_transaction()
        .append_instruction(grant_permission_to_manage_verifying_keys);

    // Use transaction-oriented API: separate initial registrations from
    // subsequent state updates.
    builder = builder
        .next_transaction()
        .append_instruction(mint)
        .append_instruction(mint_cabbage)
        .append_instruction(transfer_rose_ownership)
        .append_instruction(grant_permission_to_set_parameters)
        .append_instruction(grant_permission_to_manage_soracloud)
        .append_instruction(grant_permission_to_register_accounts);

    let manifest = builder.build_raw().with_consensus_mode(consensus_mode);
    // Enrich with consensus metadata and fingerprint for operator visibility.
    Ok(manifest.with_consensus_meta())
}

#[cfg(test)]
mod consensus_manifest_tests {
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;

    use super::*;

    #[test]
    fn synthetic_npos_genesis_has_canonical_metadata() {
        let manifest = generate_synthetic(
            GenesisBuilder::new_without_executor(
                ChainId::from("synthetic-meta"),
                PathBuf::from("."),
            ),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            0,
            0,
            0,
            None,
            Some([7; 32]),
        )
        .expect("generate synthetic NPoS genesis");

        assert_eq!(manifest.consensus_mode(), SumeragiConsensusMode::Npos);
        assert!(manifest.consensus_fingerprint().is_some());
        assert_eq!(
            manifest.wire_protocol_version(),
            u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION)
        );
    }

    #[test]
    fn profile_cadence_and_seed_are_signed() {
        let defaults = profile_defaults(GenesisProfile::Iroha3Dev);
        let seed = [9; 32];
        let manifest = generate_default(
            GenesisBuilder::new_without_executor(defaults.chain_id.clone(), PathBuf::from(".")),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            Some(&defaults),
            Some(seed),
        )
        .expect("generate profiled NPoS genesis");
        let parameters = manifest
            .effective_parameters()
            .expect("generated manifest has one structured parameter block");
        let npos = parameters
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("signed NPoS parameters");

        assert_eq!(
            parameters.sumeragi().block_cadence_ms(),
            defaults.block_cadence_ms
        );
        assert_eq!(npos.epoch_seed(), seed);
    }

    #[test]
    fn npos_genesis_rejects_missing_seed() {
        let error = generate_default(
            GenesisBuilder::new_without_executor(
                ChainId::from("missing-npos-seed"),
                PathBuf::from("."),
            ),
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(),
            None,
            SumeragiConsensusMode::Npos,
            None,
            None,
        )
        .expect_err("NPoS genesis without a seed must fail closed");
        assert!(error.to_string().contains("VRF seed"));
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_synthetic(
    builder: GenesisBuilder,
    genesis_public_key: &PublicKey,
    ivm_gas_limit_per_block: Option<u64>,
    consensus_mode: SumeragiConsensusMode,
    domains: u64,
    accounts_per_domain: u64,
    asset_definitions_per_domain: u64,
    profile_defaults: Option<&ProfileDefaults>,
    profile_vrf_seed: Option<[u8; 32]>,
) -> color_eyre::Result<RawGenesisTransaction> {
    // Synthetic genesis extends the default one with additional transactions
    // describing synthetic domains and assets.
    let default_genesis = generate_default(
        builder,
        genesis_public_key,
        ivm_gas_limit_per_block,
        consensus_mode,
        profile_defaults,
        profile_vrf_seed,
    )?;
    let mut builder = default_genesis.into_builder().next_transaction();

    for domain in 0..domains {
        let domain_id = DomainId::try_new(format!("domain_{domain}"), "universal")?;
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
            builder =
                builder.append_instruction(Register::account(Account::new(account_id.clone())));

            for asset_definition_id in &synthetic_asset_definitions {
                let mint = Mint::asset_quantity(
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

//! Testnet-only Taira Kagemusha release and genesis bootstrap helpers.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write as _,
    num::NonZeroU64,
    path::{Path, PathBuf},
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::{
    smartcontracts::isi::offline::{
        KagemushaReleaseCatalogV4, isi::production_offline_device_attestation_policy_v1,
    },
    zk::confidential_v2::{
        confidential_transfer_v2_vk_record, confidential_unshield_v3_vk_record,
        kagemusha_topup_shield_v2_vk_record,
    },
};
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    ChainId,
    account::{Account, AccountId, ParsedAccountId, address::ChainDiscriminantGuard},
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    block::consensus_v2::{ConsensusMode, ValidatorPower},
    isi::{
        Grant, GrantBox, InstructionBox, Mint, MintBox, Register, RegisterBox,
        asset_alias::SetAssetDefinitionAlias,
        offline::ActivateKagemushaRecursiveReleaseV4,
        verifying_keys::{self, RegisterVerifyingKey},
        zk::{RegisterZkAsset, ZkAssetMode},
    },
    offline::{
        KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1, KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2, KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2, KAGEMUSHA_VERIFIER_NAMESPACE,
        KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4, KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
        KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2, KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
        KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2, KagemushaTopUpFinalityRosterArtifactV2,
        KagemushaTopUpFinalityRosterWindowV2, OfflineAuthenticatedArtifactSet,
        kagemusha_recursive_spend_release_sha256, offline_escrow_account_id,
    },
    peer::PeerId,
    permission::Permission,
    proof::{VerifyingKeyId, VerifyingKeyRecord},
};
use iroha_genesis::RawGenesisTransaction;
use iroha_primitives::{json::Json, numeric::Quantity};
use norito::json::Value as JsonValue;

use super::{Outcome, Result, write_new_durable_file};

const PUBLIC_TAIRA_CHAIN_ID: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const TAIRA_RELEASE_GENERATION_V4: &str = "production-gate-real-artifacts-v4";
const TAIRA_RELEASE_ACTIVATION_HEIGHT_V4: u64 = 2;
const TAIRA_RELEASE_MINIMUM_WITHDRAWAL_HEIGHT_V4: u64 = 1_000_000;
const DEFAULT_TAIRA_RELEASE_WITHDRAWAL_HEIGHT_V4: u64 = 1_000_000_000;
const LEGACY_TAIRA_BLOCK_CADENCE_MS: u64 = 1_000;
const PUBLIC_TAIRA_BLOCK_CADENCE_MS: u64 = 4_000;
const PUBLIC_TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
const PUBLIC_TAIRA_OFFLINE_ASSET_ID: &str = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv";
const PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS: &str = "ds#boi.is";
const PUBLIC_TAIRA_OFFLINE_ASSET_SCALE: u32 = 2;
const PUBLIC_TAIRA_FEE_ASSET_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const LEGACY_TAIRA_OFFLINE_ASSET_NAME: &str = "sbd";
const LEGACY_TAIRA_OFFLINE_ASSET_ALIAS: &str = "sbd#cbsi";
const PUBLIC_TAIRA_OFFLINE_ASSET_NAME: &str = "ds";
const RELEASE_POLICY_FILE_NAME: &str = "release-policy-v1.norito";
const RELEASE_CATALOG_DIRECTORY_NAME: &str = "catalog";

/// Build the exact public Taira top-up finality roster consumed by release generation.
#[derive(Debug, ClapArgs)]
pub(super) struct PrepareReleaseRosterV4Args {
    /// One rendered validator config containing the complete trusted-peers PoP roster.
    #[arg(long)]
    validator_config: PathBuf,
    /// First excluded height for release issuance and roster authentication.
    #[arg(long, default_value_t = DEFAULT_TAIRA_RELEASE_WITHDRAWAL_HEIGHT_V4)]
    withdrawal_height: u64,
    /// New private file receiving the canonical Norito roster artifact.
    #[arg(long)]
    output: PathBuf,
}

/// Append the complete authenticated offline-cash state to a fresh Taira genesis.
#[derive(Debug, ClapArgs)]
pub(super) struct PrepareTestnetBootstrapV4Args {
    /// Fresh canonical Taira unsigned genesis manifest.
    #[arg(long)]
    genesis: PathBuf,
    /// Exact release bundle containing `release-policy-v1.norito` and `catalog/<digest>`.
    #[arg(long)]
    release_bundle: PathBuf,
    /// I105 account used to sign and execute the genesis block.
    #[arg(long)]
    genesis_authority: String,
    /// Runtime account whose private key signs Torii offline commands.
    #[arg(long)]
    command_authority: String,
    /// XOR amount minted to the command authority for mandatory readiness and fees.
    #[arg(long, default_value = "1000000")]
    fee_mint: String,
    /// Apple App ID prefix, normally the Developer Team ID.
    #[arg(long)]
    ios_team_id: String,
    /// Production iOS bundle identifier.
    #[arg(long)]
    ios_bundle_id: String,
    /// Allowed App Attest validation category; repeat for additional categories.
    #[arg(long, required = true)]
    ios_validation_category: Vec<u32>,
    /// Allowed production app bundle version; repeat for additional versions.
    #[arg(long, required = true)]
    ios_bundle_version: Vec<String>,
    /// Android application package name.
    #[arg(long)]
    android_package_name: String,
    /// Android signing-certificate SHA-256; repeat for signer rotation.
    #[arg(long, value_parser = parse_sha256, required = true)]
    android_signing_certificate_sha256: Vec<[u8; 32]>,
    /// New private path receiving the complete unsigned offline-enabled genesis.
    #[arg(long)]
    output: PathBuf,
    /// New external JSON path receiving the exact operator-reviewed release identity.
    #[arg(long)]
    operator_identity_output: PathBuf,
}

fn parse_sha256(value: &str) -> std::result::Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("SHA-256 must be exactly 64 hexadecimal characters".to_owned());
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(value, &mut digest).map_err(|_| "invalid SHA-256".to_owned())?;
    Ok(digest)
}

fn json_string_field<'a>(
    fields: &'a norito::json::Map,
    key: &str,
    context: &str,
) -> Result<&'a str> {
    fields
        .get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| eyre!("{context}.{key} must be a string"))
}

fn migrate_legacy_taira_asset_to_digital_shekel(manifest: &mut JsonValue) -> Result<()> {
    let transactions = manifest
        .get_mut("transactions")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| eyre!("fresh Taira genesis is missing its transactions array"))?;
    let mut registration_count = 0_usize;
    let mut alias_count = 0_usize;

    for (transaction_index, transaction) in transactions.iter_mut().enumerate() {
        let transaction = transaction
            .as_object_mut()
            .ok_or_else(|| eyre!("genesis transaction {transaction_index} is not an object"))?;
        let Some(instructions) = transaction.get_mut("instructions") else {
            continue;
        };
        let instructions = instructions.as_array_mut().ok_or_else(|| {
            eyre!("genesis transaction {transaction_index}.instructions is not an array")
        })?;

        for (instruction_index, instruction) in instructions.iter_mut().enumerate() {
            let instruction_context =
                format!("transactions[{transaction_index}].instructions[{instruction_index}]");
            let instruction = instruction
                .as_object_mut()
                .ok_or_else(|| eyre!("{instruction_context} is not an object"))?;

            if let Some(register) = instruction.get_mut("Register") {
                let register = register
                    .as_object_mut()
                    .ok_or_else(|| eyre!("{instruction_context}.Register is not an object"))?;
                if let Some(definition) = register.get_mut("AssetDefinition") {
                    let definition = definition.as_object_mut().ok_or_else(|| {
                        eyre!("{instruction_context}.Register.AssetDefinition is not an object")
                    })?;
                    let id = json_string_field(
                        definition,
                        "id",
                        &format!("{instruction_context}.Register.AssetDefinition"),
                    )?
                    .to_owned();
                    if id == PUBLIC_TAIRA_OFFLINE_ASSET_ID {
                        registration_count += 1;
                        if registration_count != 1 {
                            bail!(
                                "fresh Taira genesis contains duplicate `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}` registrations"
                            );
                        }
                        let name = json_string_field(
                            definition,
                            "name",
                            &format!("{instruction_context}.Register.AssetDefinition"),
                        )?;
                        if name != LEGACY_TAIRA_OFFLINE_ASSET_NAME {
                            bail!(
                                "canonical Taira asset registration must have the clean legacy name `{LEGACY_TAIRA_OFFLINE_ASSET_NAME}`, got `{name}`"
                            );
                        }
                        definition.insert(
                            "name".to_owned(),
                            JsonValue::String(PUBLIC_TAIRA_OFFLINE_ASSET_NAME.to_owned()),
                        );
                        let metadata = definition
                            .get_mut("metadata")
                            .and_then(JsonValue::as_object_mut)
                            .ok_or_else(|| {
                                eyre!(
                                    "{instruction_context}.Register.AssetDefinition.metadata is not an object"
                                )
                            })?;
                        for (key, expected) in [
                            ("currency_code", "SBD"),
                            ("display_code", "e-SBD"),
                            ("display_name", "Digital Solomon Islands Dollar"),
                        ] {
                            let actual = json_string_field(
                                metadata,
                                key,
                                &format!("{instruction_context}.Register.AssetDefinition.metadata"),
                            )?;
                            if actual != expected {
                                bail!(
                                    "clean legacy Taira asset metadata `{key}` must be `{expected}`, got `{actual}`"
                                );
                            }
                        }
                        metadata.insert(
                            "currency_code".to_owned(),
                            JsonValue::String("DS".to_owned()),
                        );
                        metadata.insert(
                            "display_code".to_owned(),
                            JsonValue::String("DS".to_owned()),
                        );
                        metadata.insert(
                            "display_name".to_owned(),
                            JsonValue::String("Digital Shekel".to_owned()),
                        );
                        metadata.insert(
                            "iso_currency_code".to_owned(),
                            JsonValue::String("ILS".to_owned()),
                        );
                        metadata.insert("symbol".to_owned(), JsonValue::String("₪".to_owned()));
                    }
                }
            }

            if let Some(set_alias) = instruction.get_mut("SetAssetDefinitionAlias") {
                let set_alias = set_alias.as_object_mut().ok_or_else(|| {
                    eyre!("{instruction_context}.SetAssetDefinitionAlias is not an object")
                })?;
                let asset_definition_id = json_string_field(
                    set_alias,
                    "asset_definition_id",
                    &format!("{instruction_context}.SetAssetDefinitionAlias"),
                )?
                .to_owned();
                let alias = json_string_field(
                    set_alias,
                    "alias",
                    &format!("{instruction_context}.SetAssetDefinitionAlias"),
                )?
                .to_owned();
                if alias == PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
                    && asset_definition_id != PUBLIC_TAIRA_OFFLINE_ASSET_ID
                {
                    bail!(
                        "fresh Taira genesis already binds `{PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS}` to the wrong asset `{asset_definition_id}`"
                    );
                }
                if asset_definition_id == PUBLIC_TAIRA_OFFLINE_ASSET_ID {
                    alias_count += 1;
                    if alias_count != 1 {
                        bail!(
                            "fresh Taira genesis contains multiple aliases for `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}`"
                        );
                    }
                    if alias != LEGACY_TAIRA_OFFLINE_ASSET_ALIAS {
                        bail!(
                            "canonical Taira asset must have the clean legacy alias `{LEGACY_TAIRA_OFFLINE_ASSET_ALIAS}`, got `{alias}`"
                        );
                    }
                    set_alias.insert(
                        "alias".to_owned(),
                        JsonValue::String(PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS.to_owned()),
                    );
                }
            }
        }
    }

    if registration_count != 1 {
        bail!(
            "fresh Taira genesis must contain exactly one `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}` registration"
        );
    }
    if alias_count != 1 {
        bail!(
            "fresh Taira genesis must contain exactly one `{LEGACY_TAIRA_OFFLINE_ASSET_ALIAS}` binding for `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}`"
        );
    }
    Ok(())
}

#[derive(Debug)]
struct PublicValidatorWithPop {
    validator: ValidatorPower,
    pop: [u8; 96],
}

fn parse_public_validator_roster(config: &toml::Value) -> Result<Vec<PublicValidatorWithPop>> {
    let entries = config
        .get("trusted_peers_pop")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            eyre!("validator config is missing the top-level trusted_peers_pop array")
        })?;
    if entries.len() < 4 {
        bail!("Taira release roster requires at least four validators");
    }

    let mut validators = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let table = entry
            .as_table()
            .ok_or_else(|| eyre!("trusted_peers_pop[{index}] must be an inline TOML table"))?;
        let public_key_literal = table
            .get("public_key")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| eyre!("trusted_peers_pop[{index}].public_key is missing"))?;
        let public_key = PublicKey::from_str(public_key_literal)
            .wrap_err_with(|| format!("trusted_peers_pop[{index}].public_key is invalid"))?;
        if !matches!(
            public_key.try_algorithm(),
            Ok(iroha_crypto::Algorithm::BlsNormal)
        ) {
            bail!("trusted_peers_pop[{index}].public_key is not BLS normal");
        }
        let pop_hex = table
            .get("pop_hex")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| eyre!("trusted_peers_pop[{index}].pop_hex is missing"))?;
        if pop_hex.len() != 192 || !pop_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("trusted_peers_pop[{index}].pop_hex must be exactly 96 bytes of hexadecimal");
        }
        let mut pop = [0_u8; 96];
        hex::decode_to_slice(pop_hex, &mut pop)
            .wrap_err_with(|| format!("trusted_peers_pop[{index}].pop_hex is invalid"))?;
        iroha_crypto::bls_normal_pop_verify(&public_key, &pop)
            .wrap_err_with(|| format!("trusted_peers_pop[{index}] has an invalid BLS PoP"))?;
        validators.push(PublicValidatorWithPop {
            validator: ValidatorPower {
                validator: PeerId::new(public_key),
                power: 1,
            },
            pop,
        });
    }
    validators
        .sort_unstable_by(|left, right| left.validator.validator.cmp(&right.validator.validator));
    if validators
        .windows(2)
        .any(|pair| pair[0].validator.validator == pair[1].validator.validator)
    {
        bail!("trusted_peers_pop contains a duplicate validator");
    }
    Ok(validators)
}

fn taira_release_roster_v4(
    validators: Vec<PublicValidatorWithPop>,
    withdrawal_height: u64,
) -> Result<KagemushaTopUpFinalityRosterArtifactV2> {
    if withdrawal_height <= TAIRA_RELEASE_ACTIVATION_HEIGHT_V4 {
        bail!(
            "Taira release withdrawal height must be greater than {}",
            TAIRA_RELEASE_ACTIVATION_HEIGHT_V4
        );
    }
    let (validator_set, validator_set_pops): (Vec<_>, Vec<_>) = validators
        .into_iter()
        .map(|entry| (entry.validator, entry.pop))
        .unzip();
    let roster = KagemushaTopUpFinalityRosterArtifactV2 {
        version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
        chain_id: ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
        artifact_generation: TAIRA_RELEASE_GENERATION_V4.to_owned(),
        windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
            activates_at_height: TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
            withdraws_at_height: withdrawal_height,
            consensus_mode: ConsensusMode::Npos,
            validator_set,
            validator_set_pops,
        }],
    };
    roster
        .validate()
        .map_err(|error| eyre!("rendered Taira finality roster is invalid: {error}"))?;
    Ok(roster)
}

pub(super) fn prepare_release_roster_v4<T: std::io::Write>(
    args: PrepareReleaseRosterV4Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    let config_bytes = super::read_external_bounded(
        &args.validator_config,
        2 * 1024 * 1024,
        "rendered Taira validator config",
    )?;
    let config_text = std::str::from_utf8(&config_bytes)
        .wrap_err("rendered Taira validator config is not UTF-8")?;
    let config: toml::Value =
        toml::from_str(config_text).wrap_err("failed to decode rendered validator config")?;
    let validators = parse_public_validator_roster(&config)?;
    let validator_count = validators.len();
    let roster = taira_release_roster_v4(validators, args.withdrawal_height)?;
    let bytes = norito::to_bytes(&roster).wrap_err("failed to encode Taira release roster")?;
    let sha256 = kagemusha_recursive_spend_release_sha256(&bytes);
    write_new_durable_file(&args.output, &bytes)?;
    writeln!(
        writer,
        "{{\"status\":\"prepared\",\"chain_id\":\"{}\",\"generation\":\"{}\",\"activation_height\":{},\"withdrawal_height\":{},\"validator_count\":{},\"roster_sha256\":\"{}\",\"output\":\"{}\"}}",
        PUBLIC_TAIRA_CHAIN_ID,
        TAIRA_RELEASE_GENERATION_V4,
        TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
        args.withdrawal_height,
        validator_count,
        hex::encode(sha256),
        args.output.display(),
    )?;
    Ok(())
}

fn parse_taira_account(raw: &str) -> Result<AccountId> {
    AccountId::parse_encoded(raw)
        .map(ParsedAccountId::into_account_id)
        .map_err(|error| eyre!("invalid Taira account literal `{raw}`: {error}"))
}

fn sole_release_manifest_sha256(catalog_root: &Path) -> Result<[u8; 32]> {
    let mut digests = Vec::new();
    for entry in fs::read_dir(catalog_root).wrap_err("failed to enumerate release catalog")? {
        let entry = entry.wrap_err("failed to inspect release catalog entry")?;
        let file_type = entry
            .file_type()
            .wrap_err("failed to inspect release catalog entry type")?;
        if !file_type.is_dir() || file_type.is_symlink() {
            bail!("Taira release catalog root may contain only manifest-digest directories");
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| eyre!("release catalog directory name is not UTF-8"))?;
        if name != name.to_ascii_lowercase() {
            bail!("release catalog manifest digest must be lowercase");
        }
        digests.push(parse_sha256(&name).map_err(|error| eyre!(error))?);
    }
    match digests.as_slice() {
        [digest] => Ok(*digest),
        _ => bail!("Taira release catalog must contain exactly one authenticated release"),
    }
}

fn taira_base_verifier_records(
    activation_height: u64,
) -> Result<[(VerifyingKeyId, VerifyingKeyRecord); 3]> {
    let make_record = |role: &str,
                       build: fn(&str, u32) -> std::result::Result<VerifyingKeyRecord, String>|
     -> Result<(VerifyingKeyId, VerifyingKeyRecord)> {
        let id = VerifyingKeyId::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND, role);
        let mut record = build(role, 1).map_err(|error| eyre!(error))?;
        record.namespace = KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.activation_height = Some(activation_height);
        record.withdraw_height = None;
        Ok((id, record))
    };
    Ok([
        make_record(
            KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2,
            confidential_transfer_v2_vk_record,
        )?,
        make_record(
            KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
            kagemusha_topup_shield_v2_vk_record,
        )?,
        make_record(
            KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
            confidential_unshield_v3_vk_record,
        )?,
    ])
}

#[derive(Debug)]
struct TairaGenesisInventory {
    accounts: BTreeSet<AccountId>,
    asset_scales: BTreeMap<AssetDefinitionId, Option<u32>>,
    asset_names: BTreeMap<AssetDefinitionId, String>,
    asset_mints: Vec<(AssetId, Quantity)>,
    verifier_ids: BTreeSet<VerifyingKeyId>,
    verifier_circuit_versions: BTreeSet<(String, u32)>,
    zk_assets: BTreeSet<AssetDefinitionId>,
    grants: BTreeSet<(AccountId, Permission)>,
    ds_alias_binding: Option<AssetDefinitionId>,
    has_recursive_activation: bool,
}

impl TairaGenesisInventory {
    fn from_genesis(genesis: &RawGenesisTransaction) -> Result<Self> {
        let ds_alias: AssetDefinitionAlias = PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
            .parse()
            .expect("static Taira offline alias");
        let mut inventory = Self {
            accounts: BTreeSet::new(),
            asset_scales: BTreeMap::new(),
            asset_names: BTreeMap::new(),
            asset_mints: Vec::new(),
            verifier_ids: BTreeSet::new(),
            verifier_circuit_versions: BTreeSet::new(),
            zk_assets: BTreeSet::new(),
            grants: BTreeSet::new(),
            ds_alias_binding: None,
            has_recursive_activation: false,
        };

        for instruction in genesis.instructions() {
            if instruction
                .as_any()
                .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
                .is_some()
            {
                inventory.has_recursive_activation = true;
                continue;
            }
            if let Some(register) = instruction.as_any().downcast_ref::<RegisterVerifyingKey>() {
                inventory.verifier_ids.insert(register.id.clone());
                inventory
                    .verifier_circuit_versions
                    .insert((register.record.circuit_id.clone(), register.record.version));
                continue;
            }
            if let Some(register) = instruction.as_any().downcast_ref::<RegisterZkAsset>() {
                inventory.zk_assets.insert(register.asset().clone());
                continue;
            }
            if let Some(set_alias) = instruction
                .as_any()
                .downcast_ref::<SetAssetDefinitionAlias>()
            {
                if set_alias.alias().as_ref() == Some(&ds_alias) {
                    inventory.ds_alias_binding = Some(set_alias.asset_definition_id().clone());
                }
                continue;
            }
            if let Some(grant) = instruction.as_any().downcast_ref::<GrantBox>() {
                if let GrantBox::Permission(grant) = grant {
                    inventory
                        .grants
                        .insert((grant.destination().clone(), grant.object().clone()));
                }
                continue;
            }
            if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>() {
                inventory
                    .asset_mints
                    .push((mint.destination().clone(), mint.object().clone()));
                continue;
            }
            let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() else {
                continue;
            };
            match register {
                RegisterBox::Account(register) => {
                    inventory.accounts.insert(register.object().id.clone());
                }
                RegisterBox::AssetDefinition(register) => {
                    let definition = register.object();
                    inventory
                        .asset_scales
                        .insert(definition.id.clone(), definition.spec.scale());
                    inventory
                        .asset_names
                        .insert(definition.id.clone(), definition.name.clone());
                }
                _ => {}
            }
        }
        Ok(inventory)
    }
}

fn register_account_if_missing(
    accounts: &mut BTreeSet<AccountId>,
    instructions: &mut Vec<InstructionBox>,
    account_id: AccountId,
) -> bool {
    if !accounts.insert(account_id.clone()) {
        return false;
    }
    instructions.push(Register::account(Account::new(account_id)).into());
    true
}

fn has_nonzero_online_backing_source(
    asset_mints: &[(AssetId, Quantity)],
    asset_definition_id: &AssetDefinitionId,
    escrow_account: &AccountId,
) -> bool {
    asset_mints.iter().any(|(asset_id, quantity)| {
        asset_id.definition() == asset_definition_id
            && asset_id.account() != escrow_account
            && !quantity.is_zero()
    })
}

fn validate_source_block_cadence_ms(block_cadence_ms: u64) -> Result<()> {
    if matches!(
        block_cadence_ms,
        LEGACY_TAIRA_BLOCK_CADENCE_MS | PUBLIC_TAIRA_BLOCK_CADENCE_MS
    ) {
        return Ok(());
    }
    bail!(
        "source genesis has unexpected signed block cadence {block_cadence_ms} ms; expected legacy {LEGACY_TAIRA_BLOCK_CADENCE_MS} ms or target {PUBLIC_TAIRA_BLOCK_CADENCE_MS} ms"
    )
}

fn unit_permission(name: &'static str) -> Permission {
    Permission::new(name.into(), Json::new(()))
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct TairaOperatorVerifierV1 {
    backend: String,
    name: String,
    version: u32,
    circuit_id: String,
    commitment: String,
    public_inputs_schema_hash: String,
    max_proof_bytes: u32,
    activation_height: u64,
    withdrawal_height: Option<u64>,
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct TairaOperatorVerifierIdentityV1 {
    active_transfer_verifier: TairaOperatorVerifierV1,
    active_topup_shield_verifier: TairaOperatorVerifierV1,
    active_unshield_verifier: TairaOperatorVerifierV1,
    active_recursive_step_eq_verifier: TairaOperatorVerifierV1,
    active_recursive_step_ep_verifier: TairaOperatorVerifierV1,
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct TairaOperatorReleaseIdentityV1 {
    cash_handoff_capability: String,
    required_bridge_abi_version: u32,
    max_hops: u32,
    asset_definition_id: String,
    asset_scale: u32,
    artifact_set: OfflineAuthenticatedArtifactSet,
    verifiers: TairaOperatorVerifierIdentityV1,
}

fn project_operator_verifier(
    id: VerifyingKeyId,
    record: &VerifyingKeyRecord,
    max_proof_bytes: u32,
    activation_height: u64,
    withdrawal_height: Option<u64>,
) -> TairaOperatorVerifierV1 {
    TairaOperatorVerifierV1 {
        backend: id.backend.as_str().to_owned(),
        name: id.name,
        version: record.version,
        circuit_id: record.circuit_id.clone(),
        commitment: hex::encode(record.commitment),
        public_inputs_schema_hash: hex::encode(record.public_inputs_schema_hash),
        max_proof_bytes,
        activation_height,
        withdrawal_height,
    }
}

fn validate_operator_verifier_distinctness(verifiers: [&TairaOperatorVerifierV1; 5]) -> Result<()> {
    let ids = verifiers
        .iter()
        .map(|verifier| (&verifier.backend, &verifier.name))
        .collect::<BTreeSet<_>>();
    let commitments = verifiers
        .iter()
        .map(|verifier| &verifier.commitment)
        .collect::<BTreeSet<_>>();
    let schema_hashes = verifiers
        .iter()
        .map(|verifier| &verifier.public_inputs_schema_hash)
        .collect::<BTreeSet<_>>();
    if ids.len() != 5 || commitments.len() != 5 || schema_hashes.len() != 5 {
        bail!("operator release identity does not contain five cryptographically distinct roles");
    }
    Ok(())
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct TairaBootstrapReport {
    status: String,
    chain_id: String,
    asset_definition_id: String,
    asset_name: String,
    asset_alias: String,
    asset_scale: u32,
    iso_currency_code: String,
    escrow_account: String,
    manifest_sha256: String,
    activation_height: u64,
    withdrawal_height: u64,
    verifier_version: u32,
    command_authority: String,
    fee_asset_definition_id: String,
    fee_mint: String,
    online_backing_source_ready: bool,
    source_block_cadence_ms: u64,
    effective_block_cadence_ms: u64,
    instruction_count: usize,
    instructions_hash: String,
    release_policy: String,
    artifact_root: String,
    output: String,
    operator_identity_output: String,
}

#[allow(clippy::too_many_lines)]
pub(super) fn prepare_testnet_bootstrap_v4<T: std::io::Write>(
    args: PrepareTestnetBootstrapV4Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    iroha_genesis::init_instruction_registry();
    if args.output == args.operator_identity_output {
        bail!("genesis and operator identity outputs must be distinct paths");
    }
    let genesis_bytes = super::read_external_bounded(
        &args.genesis,
        64 * 1024 * 1024,
        "fresh Taira genesis manifest",
    )?;
    let mut genesis_value: JsonValue = norito::json::from_slice(&genesis_bytes)
        .wrap_err("failed to decode fresh Taira genesis JSON")?;
    migrate_legacy_taira_asset_to_digital_shekel(&mut genesis_value)?;
    let genesis: RawGenesisTransaction = norito::json::value::from_value(genesis_value)
        .wrap_err("failed to decode migrated Taira genesis manifest")?;
    if genesis.chain_id().as_str() != PUBLIC_TAIRA_CHAIN_ID {
        bail!(
            "genesis chain must be canonical public Taira `{PUBLIC_TAIRA_CHAIN_ID}`, got `{}`",
            genesis.chain_id()
        );
    }
    if genesis.chain_discriminant() != PUBLIC_TAIRA_CHAIN_DISCRIMINANT {
        bail!(
            "genesis chain discriminant must be {PUBLIC_TAIRA_CHAIN_DISCRIMINANT}, got {}",
            genesis.chain_discriminant()
        );
    }
    let source_block_cadence_ms = genesis
        .effective_parameters()
        .wrap_err("failed to resolve source Taira genesis parameters")?
        .sumeragi
        .block_cadence_ms
        .get();
    validate_source_block_cadence_ms(source_block_cadence_ms)?;
    let _discriminant = ChainDiscriminantGuard::enter(PUBLIC_TAIRA_CHAIN_DISCRIMINANT);
    let genesis_authority = parse_taira_account(&args.genesis_authority)?;
    let command_authority = parse_taira_account(&args.command_authority)?;
    let fee_mint = Quantity::from_str(&args.fee_mint)
        .wrap_err("fee mint must be a canonical non-negative quantity")?;
    if fee_mint.is_zero() {
        bail!("fee mint must be greater than zero");
    }

    let release_bundle = args
        .release_bundle
        .canonicalize()
        .wrap_err("failed to canonicalize Taira release bundle")?;
    let release_policy = release_bundle.join(RELEASE_POLICY_FILE_NAME);
    let artifact_root = release_bundle.join(RELEASE_CATALOG_DIRECTORY_NAME);
    let manifest_sha256 = sole_release_manifest_sha256(&artifact_root)?;
    let catalog = KagemushaReleaseCatalogV4::load(&release_policy, &artifact_root)
        .map_err(|error| eyre!(error))?;
    let activation = catalog
        .build_activation(manifest_sha256, 1)
        .map_err(|error| eyre!(error))?;
    let manifest = &activation.release_record.manifest;
    if manifest.chain_id.as_str() != PUBLIC_TAIRA_CHAIN_ID
        || manifest.asset.to_string() != PUBLIC_TAIRA_OFFLINE_ASSET_ID
        || manifest.asset_scale != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
        || manifest.activation_height != TAIRA_RELEASE_ACTIVATION_HEIGHT_V4
        || manifest.withdrawal_height < TAIRA_RELEASE_MINIMUM_WITHDRAWAL_HEIGHT_V4
    {
        bail!(
            "authenticated release must target canonical Taira asset `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}` at scale {PUBLIC_TAIRA_OFFLINE_ASSET_SCALE}, activate at height {TAIRA_RELEASE_ACTIVATION_HEIGHT_V4}, and retain a material validity window"
        );
    }
    let release_withdrawal_height = manifest.withdrawal_height;

    let policy_evaluation_time_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .wrap_err("system clock is before the Unix epoch")?
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    let policy = production_offline_device_attestation_policy_v1(
        args.ios_team_id,
        args.ios_bundle_id,
        args.ios_validation_category,
        args.ios_bundle_version,
        args.android_package_name,
        args.android_signing_certificate_sha256,
        policy_evaluation_time_ms,
    )
    .map_err(|error| eyre!(error))?;

    let asset_definition_id =
        AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset ID");
    let fee_asset_definition_id =
        AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_FEE_ASSET_ID)
            .expect("static Taira fee asset ID");
    let asset_alias: AssetDefinitionAlias = PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
        .parse()
        .expect("static Taira offline alias");
    let escrow_account =
        offline_escrow_account_id(&ChainId::from(PUBLIC_TAIRA_CHAIN_ID), &asset_definition_id);
    let mut inventory = TairaGenesisInventory::from_genesis(&genesis)?;
    // `irohad` creates the genesis signer account in the otherwise-empty
    // world before it executes height one. Treat that implicit account as
    // existing so this transaction cannot duplicate its registration.
    if !inventory.accounts.insert(genesis_authority.clone()) {
        bail!(
            "source genesis explicitly registers its signer account; irohad seeds that account before height one, so the explicit registration would be a duplicate"
        );
    }
    match inventory.asset_scales.get(&asset_definition_id) {
        Some(Some(PUBLIC_TAIRA_OFFLINE_ASSET_SCALE)) => {}
        Some(scale) => bail!(
            "canonical Taira offline asset has wrong fixed scale: expected {PUBLIC_TAIRA_OFFLINE_ASSET_SCALE}, got {scale:?}"
        ),
        None => bail!(
            "canonical Taira offline asset `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}` is not registered"
        ),
    }
    match inventory.asset_names.get(&asset_definition_id) {
        Some(name) if name == PUBLIC_TAIRA_OFFLINE_ASSET_NAME => {}
        Some(name) => bail!(
            "canonical Taira offline asset has wrong name after migration: expected `{PUBLIC_TAIRA_OFFLINE_ASSET_NAME}`, got `{name}`"
        ),
        None => bail!(
            "canonical Taira offline asset `{PUBLIC_TAIRA_OFFLINE_ASSET_ID}` has no registered name"
        ),
    }
    if !inventory
        .asset_scales
        .contains_key(&fee_asset_definition_id)
    {
        bail!("canonical Taira fee asset `{PUBLIC_TAIRA_FEE_ASSET_ID}` is not registered");
    }
    if inventory.has_recursive_activation {
        bail!(
            "source genesis already contains a recursive-release activation; reset from clean genesis"
        );
    }
    if let Some(existing) = &inventory.ds_alias_binding {
        if existing != &asset_definition_id {
            bail!(
                "source genesis binds `{PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS}` to wrong asset `{existing}`"
            );
        }
    }
    if inventory.zk_assets.contains(&asset_definition_id) {
        bail!("source genesis already registers the Taira offline asset as a ZK asset");
    }
    if !has_nonzero_online_backing_source(
        &inventory.asset_mints,
        &asset_definition_id,
        &escrow_account,
    ) {
        bail!(
            "canonical Taira offline asset has no non-zero online source liquidity; at least one funded non-escrow account is required for an immediate backed top-up"
        );
    }

    let base_verifiers = taira_base_verifier_records(TAIRA_RELEASE_ACTIVATION_HEIGHT_V4)?;
    let target_ids = base_verifiers
        .iter()
        .map(|(id, _)| id.clone())
        .chain([
            activation.step_eq_verifier_key_id.clone(),
            activation.step_ep_verifier_key_id.clone(),
        ])
        .collect::<BTreeSet<_>>();
    if let Some(existing) = target_ids
        .iter()
        .find(|id| inventory.verifier_ids.contains(*id))
    {
        bail!("source genesis already registers required verifier `{existing:?}`");
    }
    if let Some((_, record)) = base_verifiers.iter().find(|(_, record)| {
        inventory
            .verifier_circuit_versions
            .contains(&(record.circuit_id.clone(), record.version))
    }) {
        bail!(
            "source genesis already registers circuit `{}` version {}",
            record.circuit_id,
            record.version
        );
    }

    let mut instructions = Vec::<InstructionBox>::new();
    register_account_if_missing(
        &mut inventory.accounts,
        &mut instructions,
        genesis_authority.clone(),
    );
    register_account_if_missing(
        &mut inventory.accounts,
        &mut instructions,
        command_authority.clone(),
    );
    register_account_if_missing(
        &mut inventory.accounts,
        &mut instructions,
        escrow_account.clone(),
    );
    for (permission, destination) in [
        (
            unit_permission("CanManageVerifyingKeys"),
            genesis_authority.clone(),
        ),
        (
            unit_permission("CanActivateKagemushaRecursiveReleaseV4"),
            genesis_authority.clone(),
        ),
        (
            unit_permission("CanManageOfflineDeviceAttestationPolicy"),
            genesis_authority.clone(),
        ),
        (
            unit_permission("CanManageOfflineEscrow"),
            command_authority.clone(),
        ),
    ] {
        if inventory
            .grants
            .insert((destination.clone(), permission.clone()))
        {
            instructions.push(Grant::account_permission(permission, destination).into());
        }
    }
    if inventory.ds_alias_binding.is_none() {
        instructions.push(
            SetAssetDefinitionAlias::bind(asset_definition_id.clone(), asset_alias, None).into(),
        );
    }
    let transfer_id = base_verifiers[0].0.clone();
    let topup_id = base_verifiers[1].0.clone();
    let unshield_id = base_verifiers[2].0.clone();
    let transfer_operator_verifier = project_operator_verifier(
        transfer_id.clone(),
        &base_verifiers[0].1,
        base_verifiers[0].1.max_proof_bytes,
        TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
        None,
    );
    let topup_operator_verifier = project_operator_verifier(
        topup_id.clone(),
        &base_verifiers[1].1,
        base_verifiers[1].1.max_proof_bytes,
        TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
        None,
    );
    let unshield_operator_verifier = project_operator_verifier(
        unshield_id.clone(),
        &base_verifiers[2].1,
        base_verifiers[2].1.max_proof_bytes,
        TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
        None,
    );
    let recursive_step_eq_operator_verifier = project_operator_verifier(
        VerifyingKeyId::new(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
            KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
        ),
        &activation.step_eq_verifier_record,
        manifest.max_proof_bytes,
        manifest.activation_height,
        Some(manifest.withdrawal_height),
    );
    let recursive_step_ep_operator_verifier = project_operator_verifier(
        VerifyingKeyId::new(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
            KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
        ),
        &activation.step_ep_verifier_record,
        manifest.max_proof_bytes,
        manifest.activation_height,
        Some(manifest.withdrawal_height),
    );
    validate_operator_verifier_distinctness([
        &transfer_operator_verifier,
        &topup_operator_verifier,
        &unshield_operator_verifier,
        &recursive_step_eq_operator_verifier,
        &recursive_step_ep_operator_verifier,
    ])?;
    let operator_identity = TairaOperatorReleaseIdentityV1 {
        cash_handoff_capability: KAGEMUSHA_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        required_bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        max_hops: KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
        asset_definition_id: PUBLIC_TAIRA_OFFLINE_ASSET_ID.to_owned(),
        asset_scale: PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
        artifact_set: OfflineAuthenticatedArtifactSet {
            generation: manifest.generation.clone(),
            manifest_sha256: hex::encode(manifest_sha256),
            release_policy_sha256: hex::encode(activation.configured_policy_sha256),
            release_attestation_sha256: hex::encode(manifest.release_attestation_sha256),
            activation_height: manifest.activation_height,
            withdrawal_height: manifest.withdrawal_height,
            max_proof_bytes: manifest.max_proof_bytes,
            asset_scale: manifest.asset_scale,
        },
        verifiers: TairaOperatorVerifierIdentityV1 {
            active_transfer_verifier: transfer_operator_verifier,
            active_topup_shield_verifier: topup_operator_verifier,
            active_unshield_verifier: unshield_operator_verifier,
            active_recursive_step_eq_verifier: recursive_step_eq_operator_verifier,
            active_recursive_step_ep_verifier: recursive_step_ep_operator_verifier,
        },
    };
    for (id, record) in base_verifiers {
        instructions.push(verifying_keys::RegisterVerifyingKey { id, record }.into());
    }
    instructions.push(
        RegisterZkAsset::new(
            asset_definition_id.clone(),
            ZkAssetMode::Hybrid,
            true,
            true,
            Some(transfer_id),
            Some(unshield_id),
            Some(topup_id),
        )
        .into(),
    );
    instructions.push(
        Mint::asset_quantity(
            fee_mint.clone(),
            AssetId::new(fee_asset_definition_id.clone(), command_authority.clone()),
        )
        .into(),
    );
    instructions.push(ActivateKagemushaRecursiveReleaseV4::new(activation, policy).into());

    let instructions_hash = HashOf::new(&instructions);
    let instruction_count = instructions.len();
    let mut builder = genesis
        .into_builder()
        .with_block_cadence_ms(
            NonZeroU64::new(PUBLIC_TAIRA_BLOCK_CADENCE_MS)
                .expect("static Taira cadence is non-zero"),
        )
        .next_transaction();
    for instruction in instructions {
        builder = builder.append_instruction(instruction);
    }
    let output_genesis = builder.build_raw();
    let effective_block_cadence_ms = output_genesis
        .effective_parameters()
        .wrap_err("failed to resolve generated Taira genesis parameters")?
        .sumeragi
        .block_cadence_ms
        .get();
    if effective_block_cadence_ms != PUBLIC_TAIRA_BLOCK_CADENCE_MS {
        bail!(
            "generated Taira genesis did not freeze the required {PUBLIC_TAIRA_BLOCK_CADENCE_MS} ms cadence"
        );
    }
    let output_genesis = output_genesis.with_consensus_meta();
    let output_json = norito::json::to_json_pretty(&output_genesis)
        .wrap_err("failed to encode offline-enabled Taira genesis")?;
    let operator_identity_json = norito::json::to_json_pretty(&operator_identity)
        .wrap_err("failed to encode operator-reviewed Taira release identity")?;
    write_new_durable_file(&args.output, output_json.as_bytes())?;
    write_new_durable_file(
        &args.operator_identity_output,
        operator_identity_json.as_bytes(),
    )?;

    let report = TairaBootstrapReport {
        status: "prepared".to_owned(),
        chain_id: PUBLIC_TAIRA_CHAIN_ID.to_owned(),
        asset_definition_id: PUBLIC_TAIRA_OFFLINE_ASSET_ID.to_owned(),
        asset_name: PUBLIC_TAIRA_OFFLINE_ASSET_NAME.to_owned(),
        asset_alias: PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS.to_owned(),
        asset_scale: PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
        iso_currency_code: "ILS".to_owned(),
        escrow_account: escrow_account.to_string(),
        manifest_sha256: hex::encode(manifest_sha256),
        activation_height: TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
        withdrawal_height: release_withdrawal_height,
        verifier_version: 1,
        command_authority: command_authority.to_string(),
        fee_asset_definition_id: PUBLIC_TAIRA_FEE_ASSET_ID.to_owned(),
        fee_mint: fee_mint.to_string(),
        online_backing_source_ready: true,
        source_block_cadence_ms,
        effective_block_cadence_ms,
        instruction_count,
        instructions_hash: instructions_hash.to_string(),
        release_policy: release_policy.display().to_string(),
        artifact_root: artifact_root.display().to_string(),
        output: args.output.display().to_string(),
        operator_identity_output: args.operator_identity_output.display().to_string(),
    };
    writeln!(
        writer,
        "{}",
        norito::json::to_string(&report).wrap_err("failed to encode Taira bootstrap report")?
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn test_account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive test account");
        AccountId::new(key_pair.public_key().clone())
    }

    fn legacy_taira_asset_json() -> JsonValue {
        norito::json::from_str(
            r#"{
                "transactions": [{
                    "instructions": [
                        {
                            "Register": {
                                "AssetDefinition": {
                                    "id": "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
                                    "name": "sbd",
                                    "metadata": {
                                        "currency_code": "SBD",
                                        "display_code": "e-SBD",
                                        "display_name": "Digital Solomon Islands Dollar"
                                    }
                                }
                            }
                        },
                        {
                            "SetAssetDefinitionAlias": {
                                "alias": "sbd#cbsi",
                                "asset_definition_id": "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
                                "lease_expiry_ms": null
                            }
                        }
                    ]
                }]
            }"#,
        )
        .expect("decode legacy Taira asset fixture")
    }

    #[test]
    fn legacy_taira_asset_is_migrated_to_exact_digital_shekel_identity() {
        let mut manifest = legacy_taira_asset_json();
        migrate_legacy_taira_asset_to_digital_shekel(&mut manifest)
            .expect("migrate the unique clean legacy asset");
        let encoded = norito::json::to_string(&manifest).expect("encode migrated fixture");

        for expected in [
            r#""name":"ds""#,
            r#""currency_code":"DS""#,
            r#""display_code":"DS""#,
            r#""display_name":"Digital Shekel""#,
            r#""iso_currency_code":"ILS""#,
            r#""symbol":"₪""#,
            r#""alias":"ds#boi.is""#,
        ] {
            assert!(
                encoded.contains(expected),
                "migrated asset is missing {expected}: {encoded}"
            );
        }
        assert!(!encoded.contains("sbd#cbsi"));
        assert!(!encoded.contains("Digital Solomon Islands Dollar"));
    }

    #[test]
    fn legacy_taira_asset_migration_rejects_unreviewed_metadata() {
        let mut manifest = legacy_taira_asset_json();
        let encoded = norito::json::to_string(&manifest)
            .expect("encode legacy fixture")
            .replace("\"currency_code\":\"SBD\"", "\"currency_code\":\"ILS\"");
        manifest = norito::json::from_str(&encoded).expect("decode altered fixture");
        let error = migrate_legacy_taira_asset_to_digital_shekel(&mut manifest)
            .expect_err("unreviewed source identity must fail closed");
        assert!(error.to_string().contains("currency_code"));
    }

    #[test]
    fn checked_in_taira_genesis_migrates_without_changing_backing_supply() {
        iroha_genesis::init_instruction_registry();
        let _discriminant = ChainDiscriminantGuard::enter(PUBLIC_TAIRA_CHAIN_DISCRIMINANT);
        let mut manifest: JsonValue = norito::json::from_str(include_str!(
            "../../../../configs/soranexus/taira/genesis.json"
        ))
        .expect("decode checked-in clean Taira genesis");
        migrate_legacy_taira_asset_to_digital_shekel(&mut manifest)
            .expect("migrate checked-in Taira asset identity");
        let genesis: RawGenesisTransaction = norito::json::value::from_value(manifest)
            .expect("decode migrated checked-in Taira genesis");
        let inventory =
            TairaGenesisInventory::from_genesis(&genesis).expect("inventory migrated genesis");
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let escrow = offline_escrow_account_id(&ChainId::from(PUBLIC_TAIRA_CHAIN_ID), &definition);

        assert_eq!(
            inventory.asset_names.get(&definition).map(String::as_str),
            Some(PUBLIC_TAIRA_OFFLINE_ASSET_NAME)
        );
        assert_eq!(
            inventory.asset_scales.get(&definition),
            Some(&Some(PUBLIC_TAIRA_OFFLINE_ASSET_SCALE))
        );
        assert_eq!(inventory.ds_alias_binding.as_ref(), Some(&definition));
        assert!(has_nonzero_online_backing_source(
            &inventory.asset_mints,
            &definition,
            &escrow,
        ));
    }

    #[test]
    fn sha256_parser_accepts_only_exact_hex() {
        assert_eq!(parse_sha256(&"a5".repeat(32)), Ok([0xa5; 32]));
        assert!(parse_sha256(&"a5".repeat(31)).is_err());
        assert!(parse_sha256(&format!("{}zz", "a5".repeat(31))).is_err());
    }

    #[test]
    fn base_verifiers_are_distinct_and_height_two_active() {
        let records = taira_base_verifier_records(TAIRA_RELEASE_ACTIVATION_HEIGHT_V4)
            .expect("canonical built-in verifier keys");
        assert_eq!(
            records
                .iter()
                .map(|(id, _)| id)
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        assert_eq!(
            records
                .iter()
                .map(|(_, record)| record.commitment)
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        for (_, record) in records {
            assert_eq!(record.namespace, KAGEMUSHA_VERIFIER_NAMESPACE);
            assert_eq!(
                record.activation_height,
                Some(TAIRA_RELEASE_ACTIVATION_HEIGHT_V4)
            );
            assert!(record.withdraw_height.is_none());
            assert!(record.key.is_some());
        }
    }

    #[test]
    fn release_roster_rejects_height_two_withdrawal() {
        let error = taira_release_roster_v4(Vec::new(), TAIRA_RELEASE_ACTIVATION_HEIGHT_V4)
            .expect_err("an empty issuance window must fail");
        assert!(error.to_string().contains("withdrawal height"));
    }

    #[test]
    fn missing_recovery_and_command_accounts_are_registered_once() {
        let recovery = test_account(0x31);
        let command = test_account(0x32);
        let mut accounts = BTreeSet::new();
        let mut instructions = Vec::new();

        assert!(register_account_if_missing(
            &mut accounts,
            &mut instructions,
            recovery.clone(),
        ));
        assert!(register_account_if_missing(
            &mut accounts,
            &mut instructions,
            command.clone(),
        ));
        assert!(!register_account_if_missing(
            &mut accounts,
            &mut instructions,
            recovery.clone(),
        ));
        assert!(!register_account_if_missing(
            &mut accounts,
            &mut instructions,
            command.clone(),
        ));

        assert_eq!(accounts, BTreeSet::from([recovery, command]));
        assert_eq!(instructions.len(), 2);
    }

    #[test]
    fn implicit_genesis_signer_is_not_registered_again() {
        let signer = test_account(0x33);
        let command = signer.clone();
        let distinct_command = test_account(0x34);
        let mut accounts = BTreeSet::from([signer.clone()]);
        let mut instructions = Vec::new();

        assert!(!register_account_if_missing(
            &mut accounts,
            &mut instructions,
            signer,
        ));
        assert!(!register_account_if_missing(
            &mut accounts,
            &mut instructions,
            command,
        ));
        assert!(register_account_if_missing(
            &mut accounts,
            &mut instructions,
            distinct_command.clone(),
        ));
        assert_eq!(instructions.len(), 1);
        assert!(accounts.contains(&distinct_command));
    }

    #[test]
    fn backing_source_must_be_nonzero_and_outside_escrow() {
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let escrow = test_account(0x41);
        let holder = test_account(0x42);
        let zero_holder = AssetId::new(definition.clone(), holder.clone());
        let escrow_asset = AssetId::new(definition.clone(), escrow.clone());
        let funded_holder = AssetId::new(definition.clone(), holder);

        assert!(!has_nonzero_online_backing_source(
            &[(zero_holder, Quantity::zero())],
            &definition,
            &escrow,
        ));
        assert!(!has_nonzero_online_backing_source(
            &[(escrow_asset, Quantity::from(100_u32))],
            &definition,
            &escrow,
        ));
        assert!(has_nonzero_online_backing_source(
            &[(funded_holder, Quantity::from(100_u32))],
            &definition,
            &escrow,
        ));
    }

    #[test]
    fn taira_cadence_accepts_legacy_input_but_rejects_unknown_conflicts() {
        validate_source_block_cadence_ms(LEGACY_TAIRA_BLOCK_CADENCE_MS)
            .expect("canonical v20 source cadence may be upgraded");
        validate_source_block_cadence_ms(PUBLIC_TAIRA_BLOCK_CADENCE_MS)
            .expect("already-upgraded source cadence remains canonical");
        let error =
            validate_source_block_cadence_ms(2_000).expect_err("unknown cadence must fail closed");
        assert!(
            error
                .to_string()
                .contains("unexpected signed block cadence")
        );
    }

    #[test]
    fn genesis_builder_freezes_effective_taira_cadence_at_four_seconds() {
        let _discriminant = ChainDiscriminantGuard::enter(PUBLIC_TAIRA_CHAIN_DISCRIMINANT);
        let source = iroha_genesis::GenesisBuilder::new_without_executor(
            ChainId::from(PUBLIC_TAIRA_CHAIN_ID),
            ".",
        )
        .with_block_cadence_ms(
            NonZeroU64::new(LEGACY_TAIRA_BLOCK_CADENCE_MS).expect("legacy cadence is non-zero"),
        )
        .build_raw();
        assert_eq!(
            source
                .effective_parameters()
                .expect("source parameters")
                .sumeragi
                .block_cadence_ms
                .get(),
            LEGACY_TAIRA_BLOCK_CADENCE_MS,
        );

        let upgraded = source
            .into_builder()
            .with_block_cadence_ms(
                NonZeroU64::new(PUBLIC_TAIRA_BLOCK_CADENCE_MS).expect("target cadence is non-zero"),
            )
            .build_raw();
        assert_eq!(
            upgraded
                .effective_parameters()
                .expect("upgraded parameters")
                .sumeragi
                .block_cadence_ms
                .get(),
            PUBLIC_TAIRA_BLOCK_CADENCE_MS,
        );
    }

    #[test]
    fn canonical_taira_escrow_derivation_matches_deployment_binding() {
        let _discriminant = ChainDiscriminantGuard::enter(PUBLIC_TAIRA_CHAIN_DISCRIMINANT);
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let escrow = offline_escrow_account_id(&ChainId::from(PUBLIC_TAIRA_CHAIN_ID), &definition);
        assert_eq!(
            escrow.to_string(),
            "testuﾛ1Nｿyｵn2PHﾕG6VxﾊﾁpﾏR1uｼM8JｻXBpYcﾆﾎRKjAWvｾALWT5T",
        );
    }
}

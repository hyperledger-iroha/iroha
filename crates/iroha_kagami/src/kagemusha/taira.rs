//! Testnet-only Taira Kagemusha release and genesis bootstrap helpers.
use super::{Outcome, Result, publish_new_durable_file};
use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::zk::confidential_v2::{
    confidential_transfer_v2_vk_record, confidential_unshield_v3_vk_record,
    kagemusha_topup_shield_v2_vk_record,
};
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::{Account, AccountId, ParsedAccountId, address::ChainDiscriminantGuard},
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    block::consensus_v2::{ConsensusMode, ValidatorPower},
    isi::{
        Burn, BurnBox, Grant, GrantBox, InstructionBox, Mint, MintBox, Register, RegisterBox,
        Transfer, TransferBox,
        asset_alias::SetAssetDefinitionAlias,
        governance::RegisterCitizen,
        nexus::{
            ActivateFeeSponsorProgramRevision, CreateFeeSponsorProgram,
            EnrollFeeSponsorBeneficiary, FundFeeSponsorProgram, StageFeeSponsorProgramRevision,
        },
        offline::ActivateKagemushaRecursiveReleaseV4,
        verifying_keys::{self, RegisterVerifyingKey},
        zk::RegisterZkAsset,
    },
    offline::{
        KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND, KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
        KAGEMUSHA_VERIFIER_NAMESPACE, KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
        KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4, KAGEMUSHA_VERIFIER_ROLE_TOPUP_SHIELD_V2,
        KAGEMUSHA_VERIFIER_ROLE_TRANSFER_V2, KAGEMUSHA_VERIFIER_ROLE_UNSHIELD_V2,
        KagemushaTopUpFinalityRosterArtifactV2, KagemushaTopUpFinalityRosterWindowV2,
        kagemusha_recursive_spend_release_sha256,
    },
    peer::PeerId,
    permission::Permission,
    proof::{VerifyingKeyId, VerifyingKeyRecord},
};
use iroha_genesis::{
    GENESIS_MANIFEST_JSON_MAX_BYTES_V1, RawGenesisTransaction, validate_genesis_manifest_json,
};
use iroha_primitives::{json::Json, numeric::Quantity};
use norito::json::Value as JsonValue;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write as _,
    num::NonZeroU64,
    path::PathBuf,
    str::FromStr as _,
};
const PUBLIC_TAIRA_CHAIN_NAME: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const TAIRA_RELEASE_GENERATION_V4: &str = "production-gate-real-artifacts-v4";
const TAIRA_RELEASE_ACTIVATION_HEIGHT_V4: u64 = 2;
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
/// Build the exact public Taira top-up finality roster consumed by release generation.
#[derive(Debug, ClapArgs)]
pub(super) struct PrepareReleaseRosterV4Args {
    /// One rendered validator config containing the complete trusted-peers PoP roster.
    #[arg(long)]
    validator_config: PathBuf,
    /// Exact genesis-derived network identity whose finality votes the roster authenticates.
    #[arg(long)]
    network_id: NetworkId,
    /// First excluded height for release issuance and roster authentication.
    #[arg(long, default_value_t = DEFAULT_TAIRA_RELEASE_WITHDRAWAL_HEIGHT_V4)]
    withdrawal_height: u64,
    /// New private file receiving the canonical Norito roster artifact.
    #[arg(long)]
    output: PathBuf,
}
/// Append only network-independent offline-cash prerequisites to a fresh Taira genesis.
///
/// The exact release roster, recursive release, governed device policy, and
/// deterministic escrow are intentionally excluded. They bind the signed
/// genesis hash and must be prepared after this output is signed.
#[derive(Debug, ClapArgs)]
pub(super) struct PrepareTestnetBaseGenesisV4Args {
    /// Fresh canonical Taira unsigned genesis manifest.
    #[arg(long)]
    genesis: PathBuf,
    /// I105 account used to sign and execute the genesis block.
    #[arg(long)]
    genesis_authority: String,
    /// Runtime account whose private key signs Torii offline commands.
    #[arg(long)]
    command_authority: String,
    /// XOR amount minted to the command authority for transaction fees.
    #[arg(long, default_value = "1000000")]
    fee_mint: String,
    /// New private path receiving the unsigned Taira base genesis.
    #[arg(long)]
    output: PathBuf,
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
    network_id: NetworkId,
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
        network_id,
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
    let roster = taira_release_roster_v4(args.network_id, validators, args.withdrawal_height)?;
    let bytes = norito::to_bytes(&roster).wrap_err("failed to encode Taira release roster")?;
    let sha256 = kagemusha_recursive_spend_release_sha256(&bytes);
    publish_new_durable_file(writer, &args.output, &bytes)?;
    writeln!(
        writer,
        "{{\"status\":\"prepared\",\"chain_name\":\"{}\",\"network_id\":\"{}\",\"generation\":\"{}\",\"activation_height\":{},\"withdrawal_height\":{},\"validator_count\":{},\"roster_sha256\":\"{}\",\"output\":\"{}\"}}",
        PUBLIC_TAIRA_CHAIN_NAME,
        args.network_id,
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
    online_backing_balances: BTreeMap<AssetId, Quantity>,
    verifier_ids: BTreeSet<VerifyingKeyId>,
    verifier_circuit_versions: BTreeSet<(String, u32)>,
    zk_assets: BTreeSet<AssetDefinitionId>,
    grants: BTreeSet<(AccountId, Permission)>,
    ds_alias_binding: Option<AssetDefinitionId>,
    has_recursive_activation: bool,
}
impl TairaGenesisInventory {
    fn from_genesis(
        genesis: &RawGenesisTransaction,
        online_backing_definition: &AssetDefinitionId,
    ) -> Result<Self> {
        let ds_alias: AssetDefinitionAlias = PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
            .parse()
            .expect("static Taira offline alias");
        let mut inventory = Self {
            accounts: BTreeSet::new(),
            asset_scales: BTreeMap::new(),
            asset_names: BTreeMap::new(),
            online_backing_balances: BTreeMap::new(),
            verifier_ids: BTreeSet::new(),
            verifier_circuit_versions: BTreeSet::new(),
            zk_assets: BTreeSet::new(),
            grants: BTreeSet::new(),
            ds_alias_binding: None,
            has_recursive_activation: false,
        };
        for instruction in genesis.instructions() {
            apply_online_backing_balance_instruction(
                &mut inventory.online_backing_balances,
                online_backing_definition,
                instruction,
            )?;
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
fn set_online_backing_balance(
    balances: &mut BTreeMap<AssetId, Quantity>,
    asset_id: AssetId,
    quantity: Quantity,
) {
    if quantity.is_zero() {
        balances.remove(&asset_id);
    } else {
        balances.insert(asset_id, quantity);
    }
}
fn apply_online_backing_balance_instruction(
    balances: &mut BTreeMap<AssetId, Quantity>,
    online_backing_definition: &AssetDefinitionId,
    instruction: &InstructionBox,
) -> Result<()> {
    if let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() {
        if let MintBox::Asset(mint) = mint
            && mint.destination().definition() == online_backing_definition
        {
            let destination = mint.destination().clone();
            let next = balances
                .get(&destination)
                .cloned()
                .unwrap_or_else(Quantity::zero)
                .checked_add(mint.object())
                .wrap_err("online backing mint overflows the canonical quantity range")?;
            set_online_backing_balance(balances, destination, next);
        }
        return Ok(());
    }
    if let Some(burn) = instruction.as_any().downcast_ref::<BurnBox>() {
        if let BurnBox::Asset(burn) = burn
            && burn.destination().definition() == online_backing_definition
        {
            let destination = burn.destination().clone();
            let next = balances
                .get(&destination)
                .cloned()
                .ok_or_else(|| eyre!("online backing burn references an absent source balance"))?
                .checked_sub(burn.object())
                .wrap_err("online backing burn exceeds the derived source balance")?;
            set_online_backing_balance(balances, destination, next);
        }
        return Ok(());
    }
    if let Some(transfer) = instruction.as_any().downcast_ref::<TransferBox>() {
        if let TransferBox::Asset(transfer) = transfer
            && transfer.source().definition() == online_backing_definition
        {
            let source = transfer.source().clone();
            let destination = AssetId::new(
                online_backing_definition.clone(),
                transfer.destination().clone(),
            );
            let source_after = balances
                .get(&source)
                .cloned()
                .ok_or_else(|| {
                    eyre!("online backing transfer references an absent source balance")
                })?
                .checked_sub(transfer.object())
                .wrap_err("online backing transfer exceeds the derived source balance")?;
            if source == destination {
                let restored = source_after
                    .checked_add(transfer.object())
                    .wrap_err("online backing self-transfer overflows the source quantity")?;
                set_online_backing_balance(balances, source, restored);
            } else {
                let destination_after = balances
                    .get(&destination)
                    .cloned()
                    .unwrap_or_else(Quantity::zero)
                    .checked_add(transfer.object())
                    .wrap_err("online backing transfer overflows the destination quantity")?;
                set_online_backing_balance(balances, source, source_after);
                set_online_backing_balance(balances, destination, destination_after);
            }
        }
        return Ok(());
    }
    if instruction
        .as_any()
        .downcast_ref::<RegisterBox>()
        .is_some_and(|register| matches!(register, RegisterBox::Trigger(_)))
    {
        bail!("cannot prove final online backing liquidity across a registered executable trigger");
    }
    if let Some(fund) = instruction.as_any().downcast_ref::<FundFeeSponsorProgram>() {
        if fund.asset_definition_id() == online_backing_definition {
            bail!(
                "cannot prove online backing liquidity across a fee-sponsor vault funding instruction for the backing asset"
            );
        }
        return Ok(());
    }
    if instruction
        .as_any()
        .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>()
        .is_some()
        || instruction
            .as_any()
            .downcast_ref::<RegisterVerifyingKey>()
            .is_some()
        || instruction
            .as_any()
            .downcast_ref::<RegisterZkAsset>()
            .is_some()
        || instruction
            .as_any()
            .downcast_ref::<SetAssetDefinitionAlias>()
            .is_some()
        || instruction.as_any().downcast_ref::<GrantBox>().is_some()
        || instruction.as_any().downcast_ref::<RegisterBox>().is_some()
        || instruction
            .as_any()
            .downcast_ref::<RegisterCitizen>()
            .is_some()
        || instruction
            .as_any()
            .downcast_ref::<CreateFeeSponsorProgram>()
            .is_some()
        || instruction
            .as_any()
            .downcast_ref::<StageFeeSponsorProgramRevision>()
            .is_some()
        || instruction
            .as_any()
            .downcast_ref::<ActivateFeeSponsorProgramRevision>()
            .is_some()
        || instruction
            .as_any()
            .downcast_ref::<EnrollFeeSponsorBeneficiary>()
            .is_some()
    {
        return Ok(());
    }
    bail!("cannot prove final online backing liquidity across an unsupported genesis instruction")
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
    asset_balances: &BTreeMap<AssetId, Quantity>,
    asset_definition_id: &AssetDefinitionId,
) -> bool {
    asset_balances.iter().any(|(asset_id, quantity)| {
        asset_id.definition() == asset_definition_id && !quantity.is_zero()
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
struct TairaBaseGenesisReport {
    status: String,
    chain_name: String,
    asset_definition_id: String,
    asset_name: String,
    asset_alias: String,
    asset_scale: u32,
    iso_currency_code: String,
    planned_activation_height: u64,
    exact_network_release_required: bool,
    exact_network_escrow_required: bool,
    command_authority: String,
    fee_asset_definition_id: String,
    fee_mint: String,
    online_backing_source_ready: bool,
    source_block_cadence_ms: u64,
    effective_block_cadence_ms: u64,
    instruction_count: usize,
    instructions_hash: String,
    output: String,
}
#[allow(clippy::too_many_lines)]
pub(super) fn prepare_testnet_base_genesis_v4<T: std::io::Write>(
    args: PrepareTestnetBaseGenesisV4Args,
    writer: &mut std::io::BufWriter<T>,
) -> Outcome {
    iroha_genesis::init_instruction_registry();
    let genesis_bytes = super::read_external_bounded(
        &args.genesis,
        GENESIS_MANIFEST_JSON_MAX_BYTES_V1,
        "fresh Taira genesis manifest",
    )?;
    validate_genesis_manifest_json(&genesis_bytes)
        .wrap_err("fresh Taira genesis manifest exceeds fixed resource bounds")?;
    let mut genesis_value: JsonValue = norito::json::from_slice(&genesis_bytes)
        .wrap_err("failed to decode fresh Taira genesis JSON")?;
    drop(genesis_bytes);
    migrate_legacy_taira_asset_to_digital_shekel(&mut genesis_value)?;
    let genesis: RawGenesisTransaction = norito::json::value::from_value(genesis_value)
        .wrap_err("failed to decode migrated Taira genesis manifest")?;
    if genesis.chain_id().as_str() != PUBLIC_TAIRA_CHAIN_NAME {
        bail!(
            "genesis chain name must be canonical public Taira `{PUBLIC_TAIRA_CHAIN_NAME}`, got `{}`",
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
    let asset_definition_id =
        AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset ID");
    let fee_asset_definition_id =
        AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_FEE_ASSET_ID)
            .expect("static Taira fee asset ID");
    let asset_alias: AssetDefinitionAlias = PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
        .parse()
        .expect("static Taira offline alias");
    let mut inventory = TairaGenesisInventory::from_genesis(&genesis, &asset_definition_id)?;
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
    if !has_nonzero_online_backing_source(&inventory.online_backing_balances, &asset_definition_id)
    {
        bail!(
            "canonical Taira offline asset has no non-zero online source liquidity; at least one funded account is required before the exact-network escrow is materialized"
        );
    }
    let base_verifiers = taira_base_verifier_records(TAIRA_RELEASE_ACTIVATION_HEIGHT_V4)?;
    let target_ids = base_verifiers
        .iter()
        .map(|(id, _)| id.clone())
        .chain([
            VerifyingKeyId::new(
                KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
                KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
            ),
            VerifyingKeyId::new(
                KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
                KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
            ),
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
    let topup_id = base_verifiers[1].0.clone();
    let unshield_id = base_verifiers[2].0.clone();
    for (id, record) in base_verifiers {
        instructions.push(verifying_keys::RegisterVerifyingKey { id, record }.into());
    }
    instructions.push(
        RegisterZkAsset::new(
            asset_definition_id.clone(),
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
        .wrap_err("failed to encode Taira base genesis")?;
    validate_genesis_manifest_json(output_json.as_bytes())
        .wrap_err("generated Taira base genesis exceeds fixed resource bounds")?;
    publish_new_durable_file(writer, &args.output, output_json.as_bytes())?;
    let report = TairaBaseGenesisReport {
        status: "base_genesis_prepared".to_owned(),
        chain_name: PUBLIC_TAIRA_CHAIN_NAME.to_owned(),
        asset_definition_id: PUBLIC_TAIRA_OFFLINE_ASSET_ID.to_owned(),
        asset_name: PUBLIC_TAIRA_OFFLINE_ASSET_NAME.to_owned(),
        asset_alias: PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS.to_owned(),
        asset_scale: PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
        iso_currency_code: "ILS".to_owned(),
        planned_activation_height: TAIRA_RELEASE_ACTIVATION_HEIGHT_V4,
        exact_network_release_required: true,
        exact_network_escrow_required: true,
        command_authority: command_authority.to_string(),
        fee_asset_definition_id: PUBLIC_TAIRA_FEE_ASSET_ID.to_owned(),
        fee_mint: fee_mint.to_string(),
        online_backing_source_ready: true,
        source_block_cadence_ms,
        effective_block_cadence_ms,
        instruction_count,
        instructions_hash: instructions_hash.to_string(),
        output: args.output.display().to_string(),
    };
    writeln!(
        writer,
        "{}",
        norito::json::to_string(&report).wrap_err("failed to encode Taira base-genesis report")?
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId,
        events::execute_trigger::ExecuteTriggerEventFilter,
        offline::offline_escrow_account_id,
        trigger::{
            Trigger, TriggerId,
            action::{Action, Repeats},
        },
    };
    fn test_network_id(seed: impl AsRef<[u8]>) -> NetworkId {
        NetworkId::from_genesis_hash(
            HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(Hash::new(seed)),
        )
    }
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
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let inventory = TairaGenesisInventory::from_genesis(&genesis, &definition)
            .expect("inventory migrated genesis");
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
            &inventory.online_backing_balances,
            &definition,
        ));
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
        let network_id =
            "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
                .parse()
                .expect("deterministic Taira test network identity");
        let error =
            taira_release_roster_v4(network_id, Vec::new(), TAIRA_RELEASE_ACTIVATION_HEIGHT_V4)
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
    fn backing_source_must_be_nonzero_before_exact_escrow_derivation() {
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let holder = test_account(0x42);
        let zero_holder = AssetId::new(definition.clone(), holder.clone());
        let funded_holder = AssetId::new(definition.clone(), holder);
        let zero_balances = BTreeMap::from([(zero_holder, Quantity::zero())]);
        let funded_balances = BTreeMap::from([(funded_holder, Quantity::from(100_u32))]);
        assert!(!has_nonzero_online_backing_source(
            &zero_balances,
            &definition,
        ));
        assert!(has_nonzero_online_backing_source(
            &funded_balances,
            &definition,
        ));
    }
    #[test]
    fn backing_source_uses_final_ordered_balance_not_historical_mints() {
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let source = AssetId::new(definition.clone(), test_account(0x43));
        let destination_account = test_account(0x44);
        let destination = AssetId::new(definition.clone(), destination_account.clone());
        let mut balances = BTreeMap::new();
        for instruction in [
            InstructionBox::from(Mint::asset_quantity(100_u32, source.clone())),
            InstructionBox::from(Transfer::asset_quantity(
                source.clone(),
                40_u32,
                destination_account,
            )),
            InstructionBox::from(Burn::asset_quantity(60_u32, source.clone())),
        ] {
            apply_online_backing_balance_instruction(&mut balances, &definition, &instruction)
                .expect("ordered transparent balance instruction is supported");
        }
        assert_eq!(balances.get(&source), None);
        assert_eq!(balances.get(&destination), Some(&Quantity::from(40_u32)));
        assert!(has_nonzero_online_backing_source(&balances, &definition));

        let final_burn = InstructionBox::from(Burn::asset_quantity(40_u32, destination));
        apply_online_backing_balance_instruction(&mut balances, &definition, &final_burn)
            .expect("final burn is supported");
        assert!(
            !has_nonzero_online_backing_source(&balances, &definition),
            "a fully burned historical mint must not produce a green backing result"
        );
    }
    #[test]
    fn backing_balance_underflow_fails_without_mutation() {
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let holder = AssetId::new(definition.clone(), test_account(0x45));
        let mut balances = BTreeMap::from([(holder.clone(), Quantity::from(5_u32))]);
        let before = balances.clone();
        let burn = InstructionBox::from(Burn::asset_quantity(6_u32, holder));
        let error = apply_online_backing_balance_instruction(&mut balances, &definition, &burn)
            .expect_err("derived backing balance underflow must fail closed");
        assert!(
            error
                .to_string()
                .contains("exceeds the derived source balance")
        );
        assert_eq!(balances, before);
    }
    #[test]
    fn backing_self_transfer_checks_source_balance_without_mutation() {
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let holder = test_account(0x46);
        let source = AssetId::new(definition.clone(), holder.clone());
        let mut balances = BTreeMap::from([(source.clone(), Quantity::from(5_u32))]);
        let before = balances.clone();
        let transfer = InstructionBox::from(Transfer::asset_quantity(source, 6_u32, holder));
        let error = apply_online_backing_balance_instruction(&mut balances, &definition, &transfer)
            .expect_err("an underfunded self-transfer must fail closed");
        assert!(
            error
                .to_string()
                .contains("exceeds the derived source balance")
        );
        assert_eq!(balances, before);
    }
    #[test]
    fn backing_projection_rejects_registered_executable_triggers() {
        let definition = AssetDefinitionId::parse_address_literal(PUBLIC_TAIRA_OFFLINE_ASSET_ID)
            .expect("static Taira asset definition");
        let authority = test_account(0x47);
        let source = AssetId::new(definition.clone(), authority.clone());
        let trigger_id: TriggerId = "backing_balance_mutator"
            .parse()
            .expect("static trigger identifier");
        let action = Action::new(
            vec![InstructionBox::from(Burn::asset_quantity(1_u32, source))],
            Repeats::Indefinitely,
            authority.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(authority),
        )
        .expect("trigger action fixture is structurally valid");
        let register = InstructionBox::from(Register::trigger(Trigger::new(trigger_id, action)));
        let mut balances = BTreeMap::new();
        let error = apply_online_backing_balance_instruction(&mut balances, &definition, &register)
            .expect_err("executable genesis triggers must fail closed");
        assert!(error.to_string().contains("executable trigger"));
        assert!(balances.is_empty());
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
            ChainId::from(PUBLIC_TAIRA_CHAIN_NAME),
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
        let network_id = test_network_id(b"canonical-taira-escrow-test-network");
        let escrow = offline_escrow_account_id(&network_id, &definition);
        assert_eq!(escrow, offline_escrow_account_id(&network_id, &definition));
        assert_ne!(
            escrow,
            offline_escrow_account_id(
                &test_network_id(b"canonical-taira-other-escrow-test-network"),
                &definition,
            )
        );
    }
}

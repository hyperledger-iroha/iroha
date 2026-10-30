//! Signed genesis for developer localnets.
//!
//! Genesis registers the client signer and funds it in XOR, the fee asset; grants the client
//! contract, alias, parameter and account-registration rights; and bootstraps the validator
//! committee (genesis topology for permissioned networks, staked public-lane validators for
//! `NPoS`). The storage layer adds its service accounts, provider admission and publication
//! namespace. Genesis is signed against the parsed peer configuration so the node accepts it.

use super::service_authorities::{GeneratedAuthorities, publication_client};
use super::*;
use iroha_config::parameters::actual;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{PublicLaneStakingPolicy, RootFeePolicy, SumeragiGenesisContextParameters},
    },
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    isi::{
        GrantBox, RegisterBox, SetAssetDefinitionAlias,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    nexus::PublicLaneMonetaryPlanV1,
    parameter::{
        Parameter, Parameters,
        custom::{CustomParameter, CustomParameterId},
        system::SumeragiNposParameters,
    },
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias, CanRegisterAccount},
    governance::CanEnactGovernance,
    nexus::CanPublishSpaceDirectoryManifest,
    parameter::{CanSetHijiriParameters, CanSetParameters},
    query::CanReadAllLedgerData,
    smart_contract::{CanGrantSmartContractCodeManagement, CanManageSmartContractCode},
};
use iroha_genesis::{
    GenesisBuilder, GenesisTopologyEntry, RawGenesisTransaction, SIGNED_GENESIS_MAX_BYTES_V1,
    read_signed_genesis, validate_genesis_manifest_json,
};
use iroha_model_base::{
    metadata::Metadata,
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, NumericSpec, Quantity},
};
use std::num::NonZeroU64;

const UNIVERSAL_DOMAIN: &str = "universal.universal";
const NEXUS_DOMAIN: &str = "nexus.universal";
const IVM_DOMAIN: &str = "ivm.universal";
/// Domain separator of the keyless protocol custody account (gas, fees, stake escrow, slashes).
const GAS_ACCOUNT_DOMAIN: &[u8] = b"iroha:localnet:gas-custody:v1";
const FEE_ASSET_SCALE: u32 = 9;
/// XOR minted to the client signer for fees and developer workloads.
const CLIENT_BALANCE: u64 = 1_000_000_000;
/// XOR minted to the genesis authority so genesis-signed administration can pay fees.
const GENESIS_FEE_BALANCE: u64 = 10;
const BLOCK_MAX_TRANSACTIONS: u64 = 10_000;
const IVM_GAS_LIMIT_PER_BLOCK: u64 = 50_000_000;
const IVM_GAS_UNITS_PER_GAS: u64 = 1;
/// Self bond and fee balance of every `NPoS` validator.
const STAKE_AMOUNT: u64 = 10_000;

/// Everything genesis is derived from.
pub(super) struct Inputs<'a> {
    pub(super) request: &'a Request<'a>,
    pub(super) out_dir: &'a Path,
    pub(super) peers: &'a [Peer],
    pub(super) genesis_public_key: &'a PublicKey,
    pub(super) genesis_private_key: ExposedPrivateKey,
    pub(super) client: &'a AccountId,
    pub(super) config: &'a actual::Root,
    /// The storage layer, when the generation provisions one.
    pub(super) authorities: Option<&'a GeneratedAuthorities>,
}

/// Build, sign and publish genesis: `genesis.json`, `genesis.signed.nrt`,
/// `genesis.expected_hash` and the genesis key files. Returns the signed block hash.
pub(super) fn write(inputs: Inputs<'_>) -> Result<HashOf<BlockHeader>> {
    let manifest = build(&inputs)?
        .with_consensus_mode(inputs.request.opts.consensus_mode)
        .with_consensus_meta()?;
    sign_and_publish(manifest, inputs)
}

fn build(inputs: &Inputs<'_>) -> Result<RawGenesisTransaction> {
    let request = inputs.request;
    let opts = request.opts;
    let chain_id = request
        .chain_id
        .parse::<ChainId>()
        .wrap_err("localnet chain id must be canonical")?;
    let npos = opts.consensus_mode == SumeragiConsensusMode::Npos;
    let genesis_account = AccountId::new(inputs.genesis_public_key.clone());
    let custody = gas_account_id(inputs.genesis_public_key);
    let builder = GenesisBuilder::new_without_executor(
        chain_id.clone(),
        PathBuf::from("."),
        CryptoAdmissionPolicyV1::new(vec![
            iroha_crypto::Algorithm::Ed25519,
            iroha_crypto::Algorithm::Secp256k1,
            iroha_crypto::Algorithm::BlsNormal,
        ])
        .expect("explicit canonical genesis producer policy"),
    )
    .with_sumeragi_context_parameters(SumeragiGenesisContextParameters::recommended());
    let mut genesis = crate::genesis::generate_default(
        builder,
        inputs.genesis_public_key,
        None,
        opts.consensus_mode,
        None,
        npos.then(|| npos_epoch_seed(&chain_id)),
        staking_policy(&custody)?,
    )?;
    let assets = assets_for_client(&opts.assets, inputs.client);
    if opts.extra_accounts > 0 || !assets.is_empty() {
        genesis = extend(
            genesis,
            &genesis_account,
            opts.seed.as_ref().map(String::as_bytes),
            opts.extra_accounts,
            &assets,
        )?;
    }
    genesis = register_client_and_fees(genesis, &genesis_account, inputs.client)?;
    if let Some(authorities) = inputs.authorities {
        genesis = authorities.append_genesis(genesis, inputs.client, &genesis_account)?;
    }
    // The managed engine recognizes a plain developer network by this one client marker.
    genesis =
        service_authorities::append_profile(genesis, request.service_profile(), inputs.client)?;
    genesis = apply_parameter_overrides(
        genesis,
        opts.peers,
        Some(request.block_cadence_ms),
        opts.consensus_mode,
    )?;
    genesis = genesis
        .into_builder()
        .append_parameter(Parameter::Custom(CustomParameter::new(
            custom_parameter_id("ivm_gas_tech_account_id"),
            Json::new(custody.to_string()),
        )))
        .append_parameter(Parameter::Custom(
            root_fee_policy(npos || request.chain_discriminant.is_some(), &custody)
                .into_custom_parameter()?,
        ))
        .build_raw()?;
    genesis = grant_client_permissions(genesis, &genesis_account, inputs.client)?;
    genesis = set_topology(genesis, inputs.peers)?;
    genesis = if npos {
        let stake = stake_amount(&genesis.effective_parameters()?)?;
        bootstrap_npos(genesis, inputs.peers, &custody, inputs.client, &stake)?
    } else {
        register_permissioned_support_accounts(genesis, inputs.peers, &custody)?
    };
    if inputs.authorities.is_some() {
        genesis = publication_client::append_namespace(genesis, &genesis_account, inputs.client)?;
    }
    ensure!(
        genesis
            .crypto_admission_policy()
            .allows(iroha_crypto::Algorithm::BlsNormal),
        "localnet genesis validator account controllers require explicitly allowing bls_normal"
    );
    Ok(genesis)
}

fn sign_and_publish(
    manifest: RawGenesisTransaction,
    inputs: Inputs<'_>,
) -> Result<HashOf<BlockHeader>> {
    let Inputs {
        request,
        out_dir,
        genesis_public_key,
        genesis_private_key,
        config,
        authorities,
        ..
    } = inputs;
    let json_path = out_dir.join("genesis.json");
    let signed_path = out_dir.join("genesis.signed.nrt");
    let discriminant = request
        .chain_discriminant
        .unwrap_or_else(iroha_data_model::account::address::chain_discriminant);
    let _discriminant = ChainDiscriminantGuard::enter(discriminant);
    let manifest = manifest.with_chain_discriminant(discriminant);
    let json = norito::json::to_json_pretty(&manifest)?;
    validate_genesis_manifest_json(json.as_bytes())
        .wrap_err("generated genesis.json exceeds fixed resource bounds")?;
    custody::write(&json_path, json).wrap_err("failed to write genesis.json")?;
    // Sign the exact persisted manifest so genesis.json and genesis.signed.nrt stay aligned
    // even where custom JSON payloads change key order across a JSON round trip.
    let persisted = RawGenesisTransaction::from_path(&json_path)
        .wrap_err("failed to reload persisted genesis.json before signing")?;
    let key_pair = KeyPair::new(genesis_public_key.clone(), genesis_private_key.0.clone())
        .wrap_err("make genesis key pair")?;
    let (bound, block, _nexus_context) =
        crate::genesis::staging::bind_and_sign_staged_sumeragi_context_with_preimage(
            persisted,
            &key_pair,
            Some(config),
            Some(iroha_core::da::proof_policy_bundle(
                &config.nexus.lane_config,
            )),
            iroha_core::state::default_genesis_confidential_policy_hash(),
            // Storage plans are signed for this creation time; genesis must match it.
            authorities.map(GeneratedAuthorities::creation_time_ms),
        )
        .wrap_err("stage and sign genesis block")?;
    let mut bound_json =
        norito::json::to_json_pretty(&bound).wrap_err("encode bound genesis manifest")?;
    bound_json.push('\n');
    validate_genesis_manifest_json(bound_json.as_bytes())
        .wrap_err("bound genesis.json exceeds fixed resource bounds")?;
    custody::replace(&json_path, bound_json).wrap_err("write bound genesis.json")?;
    let expected_hash = block.0.hash();
    let framed = block.0.encode_wire().wrap_err("frame genesis block")?;
    ensure!(
        framed.len() <= SIGNED_GENESIS_MAX_BYTES_V1,
        "generated signed genesis body is {} bytes, exceeding the {SIGNED_GENESIS_MAX_BYTES_V1}-byte first-release limit",
        framed.len()
    );
    custody::write(&signed_path, &framed)?;
    let decoded = read_signed_genesis(&signed_path)
        .wrap_err("read and decode the generated signed genesis body")?;
    ensure!(
        decoded.hash() == expected_hash,
        "generated signed genesis body hashes to {}, expected {expected_hash}",
        decoded.hash()
    );
    write_owner_only(
        &out_dir.join(GENESIS_EXPECTED_HASH_FILE),
        format!("{}\n", NetworkId::from_genesis_hash(expected_hash)).as_bytes(),
    )?;
    let private_key = Zeroizing::new(
        genesis_private_key
            .try_to_multihash_string()
            .wrap_err("encode genesis private key")?,
    );
    let mut private_record = Zeroizing::new(Vec::with_capacity(private_key.len() + 1));
    private_record.extend_from_slice(private_key.as_bytes());
    private_record.push(b'\n');
    write_owner_only(&out_dir.join(GENESIS_PRIVATE_KEY_FILE), &private_record)?;
    custody::write(
        out_dir.join(GENESIS_PUBLIC_KEY_FILE),
        format!("{genesis_public_key}\n"),
    )
    .wrap_err("write genesis public-key file")?;
    Ok(expected_hash)
}

/// Keyless protocol custody: no signing key exists for it, even with a public genesis key.
fn gas_account_id(genesis_public_key: &PublicKey) -> AccountId {
    AccountId::new(iroha_crypto::derive_non_signing_ed25519_public_key(
        GAS_ACCOUNT_DOMAIN,
        &[genesis_public_key.to_string().as_bytes()],
    ))
}

fn xor_asset_definition_id() -> AssetDefinitionId {
    localnet_xor_asset_definition_id()
}

/// The compiled developer recipe, with its escrow and slash sink in protocol custody.
fn staking_policy(custody: &AccountId) -> Result<PublicLaneStakingPolicy> {
    use iroha_config::profile::{Profile, ProfileId};
    let mut policy = Profile::compiled(ProfileId::IrohaDevV1)?
        .genesis_recipe()
        .public_lane_staking
        .clone();
    policy.validate()?;
    ensure!(
        policy.stake_asset_id == xor_asset_definition_id(),
        "the developer recipe must stake the localnet XOR asset"
    );
    policy.stake_escrow_account_id = custody.clone();
    policy.slash_sink_account_id = custody.clone();
    policy.validate()?;
    Ok(policy)
}

fn npos_epoch_seed(chain_id: &ChainId) -> [u8; 32] {
    let mut seed: [u8; 32] =
        Hash::new(format!("iroha:localnet:npos-epoch-seed:v1:{chain_id}")).into();
    if seed == [0; 32] {
        seed[0] = 1;
    }
    seed
}

fn root_fee_policy(generated_custody: bool, custody: &AccountId) -> RootFeePolicy {
    let mut policy = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    policy.fee_asset_id = xor_asset_definition_id();
    if generated_custody {
        policy.fee_sink_account_id = custody.clone();
        policy.sponsor_vault_custody_account_id = custody.clone();
    }
    policy
}

fn custom_parameter_id(name: &str) -> CustomParameterId {
    CustomParameterId::new(
        name.parse()
            .expect("constant custom parameter name is valid"),
    )
}

/// What genesis already registers, so appended phases never register an object twice.
struct Registrations {
    domains: BTreeSet<DomainId>,
    accounts: BTreeSet<AccountId>,
    asset_definitions: BTreeSet<AssetDefinitionId>,
}

impl Registrations {
    fn of(manifest: &RawGenesisTransaction) -> Self {
        let mut registrations = Self {
            domains: BTreeSet::new(),
            accounts: BTreeSet::new(),
            asset_definitions: BTreeSet::new(),
        };
        for instruction in manifest.instructions() {
            match instruction.as_any().downcast_ref::<RegisterBox>() {
                Some(RegisterBox::Domain(register)) => {
                    registrations.domains.insert(register.object.id.clone());
                }
                Some(RegisterBox::Account(register)) => {
                    registrations.accounts.insert(register.object.id.clone());
                }
                Some(RegisterBox::AssetDefinition(register)) => {
                    registrations
                        .asset_definitions
                        .insert(register.object.id.clone());
                }
                _ => {}
            }
        }
        registrations
    }
}

fn xor_definition() -> iroha_data_model::asset::NewAssetDefinition {
    AssetDefinition::new(
        xor_asset_definition_id(),
        "XOR".to_owned(),
        NumericSpec::fractional(FEE_ASSET_SCALE),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .with_metadata(Metadata::default())
}

/// Register extra accounts and requested assets in the bootstrap phase `generate_default` leaves
/// open. Scoped alias domains get their own transactions, as staged genesis requires.
fn extend(
    genesis: RawGenesisTransaction,
    genesis_account: &AccountId,
    seed: Option<&[u8]>,
    extra_accounts: u16,
    assets: &[AssetSpec],
) -> Result<RawGenesisTransaction> {
    let mut registrations = Registrations::of(&genesis);
    let bootstrap_index =
        genesis.transactions().len().checked_sub(1).ok_or_else(|| {
            eyre!("localnet asset extension requires a bootstrap permission phase")
        })?;
    let bootstrap = &genesis.transactions()[bootstrap_index];
    ensure!(
        !bootstrap.instructions().is_empty()
            && bootstrap.instructions().iter().all(|instruction| {
                matches!(
                    instruction.as_any().downcast_ref::<GrantBox>(),
                    Some(GrantBox::Permission(grant)) if grant.destination() == genesis_account
                )
            }),
        "localnet asset extension requires the generated authority's bootstrap permission phase"
    );
    let mut current_length = bootstrap.instructions().len();
    let mut lengths = Vec::new();
    let mut builder = genesis.into_builder();
    for index in 0..extra_accounts {
        let account = ephemeral_identity(seed, format!("acct{index}").as_bytes())
            .wrap_err_with(|| format!("failed to generate localnet extra account key {index}"))?
            .account_id;
        if registrations.accounts.insert(account.clone()) {
            builder = builder.append_instruction(Register::account(Account::new(account)));
            current_length += 1;
        }
    }
    for asset in assets {
        for account in [&asset.owned_by, &asset.mint_to] {
            if registrations.accounts.insert(account.clone()) {
                builder =
                    builder.append_instruction(Register::account(Account::new(account.clone())));
                current_length += 1;
            }
        }
        let definition_id = AssetDefinitionId::parse_address_literal(&asset.id)
            .wrap_err("invalid asset definition id")?;
        builder = builder.append_instruction(Register::asset_definition(
            AssetDefinition::new(
                definition_id.clone(),
                asset.name.clone(),
                NumericSpec::default(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .with_metadata(Metadata::default()),
        ));
        current_length += 1;
        if let Some(alias) = asset.alias.as_deref() {
            let alias = alias
                .parse::<iroha_data_model::asset::AssetDefinitionAlias>()
                .wrap_err("invalid asset definition alias")?;
            let scoped = alias.dataspace_segment() != "universal";
            if let Some(domain_name) = alias.domain_segment() {
                let domain = DomainId::try_new(domain_name, alias.dataspace_segment())?;
                if registrations.domains.insert(domain.clone()) {
                    if scoped && current_length > 0 {
                        lengths.push(current_length);
                        current_length = 0;
                    }
                    builder = builder.append_instruction(Register::domain(Domain::new(domain)));
                    current_length += 1;
                    if scoped {
                        lengths.push(current_length);
                        current_length = 0;
                    }
                }
            }
            builder = builder.append_instruction(SetAssetDefinitionAlias::bind(
                definition_id.clone(),
                alias,
                None,
            ));
            current_length += 1;
        }
        if asset.quantity > 0 {
            builder = builder.append_instruction(Mint::asset_quantity(
                asset.quantity,
                AssetId::new(definition_id.clone(), asset.mint_to.clone()),
            ));
            current_length += 1;
        }
        if asset.owned_by != *genesis_account {
            builder = builder.append_instruction(Transfer::asset_definition(
                genesis_account.clone(),
                definition_id,
                asset.owned_by.clone(),
            ));
            current_length += 1;
        }
    }
    if current_length > 0 {
        lengths.push(current_length);
    }
    let manifest = builder.build_raw()?;
    if lengths.is_empty() {
        return Ok(manifest);
    }
    manifest.partition_instruction_only_transaction(bootstrap_index, &lengths)
}

/// Register the client and the XOR fee asset (`xor#universal`), and fund the client and the
/// genesis authority, continuing the open bootstrap phase.
fn register_client_and_fees(
    genesis: RawGenesisTransaction,
    genesis_account: &AccountId,
    client: &AccountId,
) -> Result<RawGenesisTransaction> {
    let mut registrations = Registrations::of(&genesis);
    let universal = DomainId::parse_fully_qualified(UNIVERSAL_DOMAIN)?;
    let xor = xor_asset_definition_id();
    let mut builder = genesis.into_builder();
    if registrations.accounts.insert(client.clone()) {
        builder = builder.append_instruction(Register::account(Account::new(client.clone())));
    }
    if registrations.domains.insert(universal.clone()) {
        builder = builder.append_instruction(Register::domain(Domain::new(universal)));
    }
    if registrations.asset_definitions.insert(xor.clone()) {
        builder = builder.append_instruction(Register::asset_definition(xor_definition()));
    }
    builder = builder.append_instruction(SetAssetDefinitionAlias::bind(
        xor.clone(),
        crate::genesis::PUBLIC_XOR_ALIAS
            .parse()
            .expect("canonical XOR alias"),
        None,
    ));
    builder = builder.append_instruction(Mint::asset_quantity(
        GENESIS_FEE_BALANCE,
        AssetId::new(xor.clone(), genesis_account.clone()),
    ));
    builder = builder.append_instruction(Mint::asset_quantity(
        CLIENT_BALANCE,
        AssetId::new(xor, client.clone()),
    ));
    builder.build_raw()
}

/// Contract, alias, parameter and account-registration rights for the developer client.
fn grant_client_permissions(
    genesis: RawGenesisTransaction,
    genesis_account: &AccountId,
    client: &AccountId,
) -> Result<RawGenesisTransaction> {
    let manage_verifying_keys = Permission::new("CanManageVerifyingKeys".into(), Json::new(()));
    let grants = [
        (Permission::from(CanEnactGovernance), client),
        (CanManageSmartContractCode.into(), client),
        (CanGrantSmartContractCodeManagement.into(), client),
        (CanSetParameters.into(), client),
        (CanSetHijiriParameters.into(), client),
        (CanReadAllLedgerData.into(), client),
        (
            Permission::new("CanManageSoracloud".into(), Json::new(())),
            client,
        ),
        (manage_verifying_keys.clone(), genesis_account),
        (manage_verifying_keys, client),
        (
            CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
            }
            .into(),
            client,
        ),
        (
            CanPublishSpaceDirectoryManifest {
                dataspace: DataSpaceId::UNIVERSAL,
            }
            .into(),
            client,
        ),
        (
            CanRegisterAccount {
                domain: DomainId::parse_fully_qualified(UNIVERSAL_DOMAIN)?,
            }
            .into(),
            client,
        ),
    ];
    let mut granted = genesis
        .instructions()
        .filter_map(
            |instruction| match instruction.as_any().downcast_ref::<GrantBox>()? {
                GrantBox::Permission(grant) => {
                    Some((grant.destination().clone(), grant.object().clone()))
                }
                _ => None,
            },
        )
        .collect::<BTreeSet<_>>();
    let mut builder = genesis.into_builder();
    for (permission, destination) in grants {
        if granted.insert((destination.clone(), permission.clone())) {
            builder = builder
                .append_instruction(Grant::account_permission(permission, destination.clone()));
        }
    }
    builder.build_raw()
}

fn set_topology(genesis: RawGenesisTransaction, peers: &[Peer]) -> Result<RawGenesisTransaction> {
    let mut topology = peers
        .iter()
        .map(|peer| {
            GenesisTopologyEntry::new(PeerId::new(peer.public_key.clone()), peer.pop.clone())
        })
        .collect::<Vec<_>>();
    topology.sort_by(|left, right| left.peer.cmp(&right.peer));
    genesis
        .into_builder()
        .next_transaction()
        .set_topology(topology)
        .build_raw()
}

/// Apply the developer chain parameters: block cadence and size, IVM gas, and `NPoS` sizing.
fn apply_parameter_overrides(
    genesis: RawGenesisTransaction,
    peers: NonZeroU16,
    block_cadence_ms: Option<u64>,
    consensus_mode: SumeragiConsensusMode,
) -> Result<RawGenesisTransaction> {
    let npos = consensus_mode == SumeragiConsensusMode::Npos;
    let mut parameters = genesis
        .effective_parameters()
        .wrap_err("generated localnet genesis must have one structured parameter block")?;
    // into_builder retains the original parameter snapshot: append only changed parameters,
    // since replaying unchanged authority would duplicate staking.
    let original = parameters.parameters().collect::<Vec<_>>();
    parameters.block.max_transactions =
        NonZeroU64::new(BLOCK_MAX_TRANSACTIONS).expect("block max transactions is nonzero");
    if let Some(cadence) = block_cadence_ms {
        parameters.sumeragi.block_cadence_ms =
            NonZeroU64::new(cadence).expect("validated non-zero block cadence");
    }
    if npos {
        let mut npos_parameters = parameters
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .map(SumeragiNposParameters::from_custom_parameter)
            .transpose()?
            .flatten()
            .unwrap_or_default();
        // The signed election ceiling matches the generated roster.
        npos_parameters.max_validators = u32::from(peers.get());
        npos_parameters.min_self_bond = 1_u64.into();
        npos_parameters.epoch_seed = npos_epoch_seed(genesis.chain_id());
        parameters.set_parameter(Parameter::Custom(npos_parameters.into_custom_parameter()));
    }
    parameters.set_parameter(Parameter::Custom(CustomParameter::new(
        custom_parameter_id("ivm_gas_limit_per_block"),
        Json::new(IVM_GAS_LIMIT_PER_BLOCK),
    )));
    if npos {
        let xor = xor_asset_definition_id().to_string();
        parameters.set_parameter(Parameter::Custom(CustomParameter::new(
            custom_parameter_id("ivm_gas_accepted_assets"),
            Json::new(vec![xor.clone()]),
        )));
        parameters.set_parameter(Parameter::Custom(CustomParameter::new(
            custom_parameter_id("ivm_gas_units_per_gas"),
            Json::from_str_norito(&format!(
                concat!(
                    r#"[{{"asset":"{asset}","liquidity_profile":"tier2","#,
                    r#""twap_local_per_xor":"1","units_per_gas":{units},"#,
                    r#""volatility_class":"stable"}}]"#
                ),
                asset = xor,
                units = IVM_GAS_UNITS_PER_GAS
            ))
            .expect("localnet gas-rate payload must be valid JSON"),
        )));
    }
    let mut builder = genesis.into_builder();
    if let Some(cadence) = block_cadence_ms {
        builder = builder
            .with_block_cadence_ms(NonZeroU64::new(cadence).expect("validated non-zero cadence"));
    }
    for parameter in parameters
        .parameters()
        .filter(|parameter| !original.contains(parameter))
    {
        builder = builder.append_parameter(parameter);
    }
    builder.build_raw()
}

/// The staking policy genesis already authored, checked against the `NPoS` XOR pin.
fn bootstrap_staking_policy(genesis: &RawGenesisTransaction) -> Result<PublicLaneStakingPolicy> {
    let _chain = crate::genesis::staging::staged_genesis_chain_discriminant(genesis);
    let parameters = genesis.effective_parameters()?;
    let policy = PublicLaneStakingPolicy::from_parameters(&parameters)?;
    if genesis.consensus_mode() == SumeragiConsensusMode::Npos {
        let npos = parameters
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .map(SumeragiNposParameters::from_custom_parameter)
            .transpose()?
            .flatten()
            .ok_or_else(|| eyre!("localnet NPoS bootstrap requires original NPoS parameters"))?;
        ensure!(
            policy.stake_asset_id == npos.xor_asset_definition_id
                && policy.stake_asset_id == xor_asset_definition_id(),
            "localnet staking policy and NPoS XOR pin must name the localnet XOR asset"
        );
    }
    Ok(policy)
}

fn stake_amount(parameters: &Parameters) -> Result<Quantity> {
    let requested = Quantity::from(STAKE_AMOUNT);
    let min_self_bond = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .map(SumeragiNposParameters::from_custom_parameter)
        .transpose()?
        .flatten()
        .map_or_else(|| requested.clone(), |npos| npos.min_self_bond);
    Ok(requested.max(min_self_bond).max(Quantity::from(1_u64)))
}

/// Permissioned networks register the Nexus support accounts and every validator account but
/// stake nothing: the committee is the genesis roster.
fn register_permissioned_support_accounts(
    genesis: RawGenesisTransaction,
    peers: &[Peer],
    custody: &AccountId,
) -> Result<RawGenesisTransaction> {
    let policy = bootstrap_staking_policy(&genesis)?;
    let nexus = DomainId::parse_fully_qualified(NEXUS_DOMAIN)?;
    let mut registrations = Registrations::of(&genesis);
    let mut builder = genesis.into_builder().next_transaction();
    if registrations.domains.insert(nexus.clone()) {
        builder = builder.append_instruction(Register::domain(Domain::new(nexus)));
    }
    let validators = peers
        .iter()
        .map(|peer| AccountId::new(peer.public_key.clone()));
    for account in [
        custody.clone(),
        policy.stake_escrow_account_id.clone(),
        policy.slash_sink_account_id.clone(),
    ]
    .into_iter()
    .chain(validators)
    {
        if registrations.accounts.insert(account.clone()) {
            builder = builder.append_instruction(Register::account(Account::new(account)));
        }
    }
    builder.build_raw()
}

/// `NPoS` networks fund, register and activate every validator on the single public lane.
fn bootstrap_npos(
    genesis: RawGenesisTransaction,
    peers: &[Peer],
    custody: &AccountId,
    client: &AccountId,
    stake: &Quantity,
) -> Result<RawGenesisTransaction> {
    let policy = bootstrap_staking_policy(&genesis)?;
    let xor = xor_asset_definition_id();
    let mut registrations = Registrations::of(&genesis);
    let mut builder = genesis.into_builder().next_transaction();
    for domain in [NEXUS_DOMAIN, IVM_DOMAIN, UNIVERSAL_DOMAIN] {
        let domain = DomainId::parse_fully_qualified(domain)?;
        if registrations.domains.insert(domain.clone()) {
            builder = builder.append_instruction(Register::domain(Domain::new(domain)));
        }
    }
    for account in [
        custody,
        &policy.stake_escrow_account_id,
        &policy.slash_sink_account_id,
        client,
    ] {
        if registrations.accounts.insert(account.clone()) {
            builder = builder.append_instruction(Register::account(Account::new(account.clone())));
        }
    }
    for definition in [&policy.stake_asset_id, &xor] {
        if registrations.asset_definitions.insert(definition.clone()) {
            builder = builder.append_instruction(Register::asset_definition(
                AssetDefinition::new(
                    definition.clone(),
                    "XOR".to_owned(),
                    NumericSpec::fractional(FEE_ASSET_SCALE),
                    iroha_data_model::asset::AssetBalancePolicy::Global,
                    None,
                )
                .with_metadata(Metadata::default()),
            ));
        }
    }
    // One public lane: the self bond covers it, and a fee balance covers validator upkeep.
    let lanes = Numeric::from(1_u64);
    let stake_mint = stake
        .try_mul_decimal(&lanes)
        .map_err(|error| eyre!("localnet validator stake mint amount overflow: {error}"))?;
    for peer in peers {
        let validator = AccountId::new(peer.public_key.clone());
        if registrations.accounts.insert(validator.clone()) {
            builder =
                builder.append_instruction(Register::account(Account::new(validator.clone())));
        }
        builder = builder.append_instruction(Mint::asset_quantity(
            stake_mint.clone(),
            AssetId::new(policy.stake_asset_id.clone(), validator.clone()),
        ));
        builder = builder.append_instruction(Mint::asset_quantity(
            stake.clone(),
            AssetId::new(xor.clone(), validator),
        ));
    }
    for peer in peers {
        let validator = AccountId::new(peer.public_key.clone());
        builder = builder.append_instruction(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            peer_id: PeerId::from(peer.public_key.clone()),
            stake_account: validator.clone(),
            initial_stake: stake.clone(),
            metadata: Metadata::default(),
            monetary_plan: PublicLaneMonetaryPlanV1::genesis_registration(
                AssetId::new(policy.stake_asset_id.clone(), validator.clone()),
                AssetId::new(
                    policy.stake_asset_id.clone(),
                    policy.stake_escrow_account_id.clone(),
                ),
                stake.clone(),
            ),
        });
        builder = builder.append_instruction(ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator,
        });
    }
    builder.build_raw()
}

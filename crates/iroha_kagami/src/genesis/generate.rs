use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use color_eyre::eyre::WrapErr as _;
use iroha_data_model::{isi::InstructionBox, parameter::Parameters, prelude::*};
use iroha_executor_data_model::permission::{
    domain::CanRegisterDomain, parameter::CanSetParameters, peer::CanManagePeers,
};
use iroha_genesis::{
    GenesisBuilder, GenesisWasmAction, GenesisWasmTrigger, RawGenesisTransaction, GENESIS_DOMAIN_ID,
};
use iroha_test_samples::{gen_account_in, ALICE_ID, BOB_ID, CARPENTER_ID};

use crate::{Outcome, RunArgs};

const AIRDROP_KEY: &str = "airdrop";
const DEFAULT_AIRDROP: u32 = 100;

/// Generate a genesis configuration and standard-output in JSON format
#[derive(Parser, Debug, Clone)]
pub struct Args {
    /// Relative path from the directory of output file to the executor.wasm file
    #[clap(long, value_name = "PATH")]
    executor: PathBuf,
    /// Relative path from the directory of output file to the directory that contains *.wasm libraries
    #[clap(long, value_name = "PATH")]
    wasm_dir: PathBuf,
    #[clap(long, value_name = "MULTI_HASH")]
    genesis_public_key: PublicKey,
    #[clap(subcommand)]
    mode: Option<Mode>,
}

#[derive(Subcommand, Debug, Clone, Default)]
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
        /// The total number of accounts would be `domains * assets_per_domain`.
        #[clap(long, default_value_t)]
        accounts_per_domain: u64,
        /// Number of assets per domains in synthetic genesis.
        /// The total number of assets would be `domains * assets_per_domain`.
        #[clap(long, default_value_t)]
        assets_per_domain: u64,
    },
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let Self {
            executor,
            wasm_dir,
            genesis_public_key,
            mode,
        } = self;

        let chain = ChainId::from("00000000-0000-0000-0000-000000000000");
        let builder = GenesisBuilder::new(chain, executor, wasm_dir);
        let genesis = match mode.unwrap_or_default() {
            Mode::Default => generate_default(builder, genesis_public_key),
            Mode::Synthetic {
                domains,
                accounts_per_domain,
                assets_per_domain,
            } => generate_synthetic(
                builder,
                genesis_public_key,
                domains,
                accounts_per_domain,
                assets_per_domain,
            ),
        }?;
        writeln!(writer, "{}", serde_json::to_string_pretty(&genesis)?)
            .wrap_err("failed to write serialized genesis to the buffer")
    }
}

#[allow(clippy::too_many_lines)]
pub fn generate_default(
    builder: GenesisBuilder,
    genesis_public_key: PublicKey,
) -> color_eyre::Result<RawGenesisTransaction> {
    let genesis_account_id = AccountId::new(GENESIS_DOMAIN_ID.clone(), genesis_public_key);
    let mut meta = Metadata::default();
    meta.insert("key".parse()?, Json::new("value"));

    let mut builder = builder
        .domain_with_metadata("wonderland".parse()?, meta.clone())
        .account_with_metadata(ALICE_ID.signatory().clone(), meta.clone())
        .account_with_metadata(BOB_ID.signatory().clone(), meta)
        .asset("rose".parse()?, AssetType::Numeric(NumericSpec::default()))
        .finish_domain()
        .domain("garden_of_live_flowers".parse()?)
        .account(CARPENTER_ID.signatory().clone())
        .asset(
            "cabbage".parse()?,
            AssetType::Numeric(NumericSpec::default()),
        )
        .finish_domain();

    let mint = Mint::asset_numeric(
        13u32,
        AssetId::new("rose#wonderland".parse()?, ALICE_ID.clone()),
    );
    let mint_cabbage = Mint::asset_numeric(
        44u32,
        AssetId::new("cabbage#garden_of_live_flowers".parse()?, ALICE_ID.clone()),
    );
    let grant_permission_to_set_parameters =
        Grant::account_permission(CanSetParameters, ALICE_ID.clone());
    let grant_permission_to_register_domains =
        Grant::account_permission(CanRegisterDomain, ALICE_ID.clone());
    let transfer_rose_ownership = Transfer::asset_definition(
        genesis_account_id.clone(),
        "rose#wonderland".parse()?,
        ALICE_ID.clone(),
    );
    let transfer_wonderland_ownership = Transfer::domain(
        genesis_account_id.clone(),
        "wonderland".parse()?,
        ALICE_ID.clone(),
    );
    let set_rose_airdrop = SetKeyValue::asset_definition(
        "rose#wonderland".parse()?,
        AIRDROP_KEY.parse()?,
        DEFAULT_AIRDROP,
    );

    let parameters = Parameters::default();

    for parameter in parameters.parameters() {
        builder = builder.append_parameter(parameter);
    }

    let instructions: [InstructionBox; 7] = [
        mint.into(),
        mint_cabbage.into(),
        transfer_rose_ownership.into(),
        transfer_wonderland_ownership.into(),
        grant_permission_to_set_parameters.into(),
        grant_permission_to_register_domains.into(),
        set_rose_airdrop.into(),
    ];

    for isi in instructions
        .into_iter()
        .chain(extension_from_test_genesis())
    {
        builder = builder.append_instruction(isi);
    }

    let airdrop = GenesisWasmTrigger::new(
        AIRDROP_KEY.parse().unwrap(),
        GenesisWasmAction::new(
            "trigger_airdrop.wasm",
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            AccountEventFilter::new().for_events(AccountEventSet::Created),
        ),
    );
    builder = builder.append_wasm_trigger(airdrop);

    Ok(builder.build_raw())
}

/// For testnet only
fn extension_from_test_genesis() -> impl Iterator<Item = InstructionBox> {
    use iroha_executor_data_model::permission::{
        asset::CanMintAssetWithDefinition, domain::CanUnregisterDomain,
        executor::CanUpgradeExecutor, role::CanManageRoles,
    };

    let rose_definition_id = "rose#wonderland".parse::<AssetDefinitionId>().unwrap();
    let grant_modify_rose_permission = Grant::account_permission(
        CanMintAssetWithDefinition {
            asset_definition: rose_definition_id.clone(),
        },
        ALICE_ID.clone(),
    );
    let grant_manage_peers_permission = Grant::account_permission(CanManagePeers, ALICE_ID.clone());
    let grant_manage_roles_permission = Grant::account_permission(CanManageRoles, ALICE_ID.clone());
    let grant_unregister_wonderland_domain = Grant::account_permission(
        CanUnregisterDomain {
            domain: "wonderland".parse().unwrap(),
        },
        ALICE_ID.clone(),
    );
    let grant_upgrade_executor_permission =
        Grant::account_permission(CanUpgradeExecutor, ALICE_ID.clone());

    [
        grant_modify_rose_permission,
        grant_manage_peers_permission,
        grant_manage_roles_permission,
        grant_unregister_wonderland_domain,
        grant_upgrade_executor_permission,
    ]
    .into_iter()
    .map(Into::into)
}

fn generate_synthetic(
    builder: GenesisBuilder,
    genesis_public_key: PublicKey,
    domains: u64,
    accounts_per_domain: u64,
    assets_per_domain: u64,
) -> color_eyre::Result<RawGenesisTransaction> {
    // Synthetic genesis is extension of default one
    let default_genesis = generate_default(builder, genesis_public_key)?;
    let mut builder = default_genesis.into_builder();

    for domain in 0..domains {
        let domain_id: DomainId = format!("domain_{domain}").parse()?;
        builder = builder.append_instruction(Register::domain(Domain::new(domain_id.clone())));

        for asset in 0..assets_per_domain {
            let asset_definition_id: AssetDefinitionId =
                format!("asset_{asset}#{domain_id}").parse()?;
            builder = builder.append_instruction(Register::asset_definition(AssetDefinition::new(
                asset_definition_id,
                AssetType::Numeric(NumericSpec::default()),
            )));
        }

        for _ in 0..accounts_per_domain {
            let (account_id, _account_keypair) = gen_account_in(&domain_id);
            builder =
                builder.append_instruction(Register::account(Account::new(account_id.clone())));

            // FIXME: Should `assets_per_domain` be renamed to `asset_definitions_per_domain`?
            //        https://github.com/hyperledger-iroha/iroha/issues/3508
            for asset in 0..assets_per_domain {
                let mint = Mint::asset_numeric(
                    13u32,
                    AssetId::new(
                        format!("asset_{asset}#domain_{domain}").parse()?,
                        account_id.clone(),
                    ),
                );
                builder = builder.append_instruction(mint);
            }
        }
    }

    Ok(builder.build_raw())
}

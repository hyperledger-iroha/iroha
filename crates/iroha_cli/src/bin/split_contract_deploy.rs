//! Split contract deploy helper for oversized public deploy envelopes.

use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    account_address::parse_account_address,
    client::Client,
    config::{Config, LoadPath},
    data_model::{
        isi::smart_contract_code::{
            ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
        metadata::Metadata,
        name::Name,
        prelude::*,
        smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY,
        transaction::TransactionBuilder,
    },
};
use iroha_crypto::{KeyPair, PrivateKey};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    authority: String,
    #[arg(long)]
    private_key: String,
    #[arg(long)]
    code_file: PathBuf,
    #[arg(long)]
    contract_address: String,
    #[arg(long, default_value = "universal")]
    dataspace: String,
    #[arg(long)]
    deploy_nonce: u64,
    #[arg(long, default_value_t = 753)]
    chain_discriminant: u16,
}

fn sign_and_submit(
    client: &Client,
    authority: &AccountId,
    private_key: &PrivateKey,
    instructions: impl IntoIterator<Item = InstructionBox>,
) -> Result<HashOf<SignedTransaction>> {
    let tx = TransactionBuilder::new(client.chain.clone(), authority.clone())
        .with_instructions(instructions)
        .with_metadata(Metadata::default())
        .sign(private_key);
    client.submit_transaction_blocking(&tx)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(LoadPath::Explicit(&args.config))
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err_with(|| format!("load config {}", args.config.display()))?;
    let client = Client::new(config);
    let authority = parse_account_address(&args.authority, Some(args.chain_discriminant))
        .wrap_err("failed to parse --authority as canonical account address")?
        .address
        .to_account_id()
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err("failed to decode --authority")?;
    let private_key: PrivateKey = args
        .private_key
        .parse()
        .wrap_err("failed to parse --private-key")?;
    let signer = KeyPair::from(private_key.clone());
    let contract_address: iroha::data_model::smart_contract::ContractAddress = args
        .contract_address
        .parse()
        .wrap_err("failed to parse --contract-address")?;

    let code =
        fs::read(&args.code_file).wrap_err_with(|| format!("read {}", args.code_file.display()))?;
    let verified = ivm::verify_contract_artifact(&code)
        .map_err(|err| eyre!("verify contract artifact: {err}"))?;
    let manifest = verified.manifest.signed(&signer);
    let code_hash = verified.code_hash;
    let nonce_key = Name::from_str(CONTRACT_DEPLOY_NONCE_METADATA_KEY)
        .expect("static contract deploy nonce metadata key is valid");
    let next_nonce = args
        .deploy_nonce
        .checked_add(1)
        .ok_or_else(|| eyre!("deploy nonce overflow"))?;

    let register_bytes_hash = sign_and_submit(
        &client,
        &authority,
        &private_key,
        [InstructionBox::from(RegisterSmartContractBytes {
            code_hash,
            code: code.clone(),
        })],
    )?;
    let register_manifest_hash = sign_and_submit(
        &client,
        &authority,
        &private_key,
        [InstructionBox::from(RegisterSmartContractCode { manifest })],
    )?;
    let activate_hash = sign_and_submit(
        &client,
        &authority,
        &private_key,
        [
            InstructionBox::from(ActivateContractInstance {
                namespace: args.dataspace.clone(),
                contract_id: contract_address.to_string(),
                code_hash,
            }),
            InstructionBox::from(SetKeyValue::account(
                authority.clone(),
                nonce_key,
                Json::new(next_nonce),
            )),
        ],
    )?;

    let result = norito::json::Value::Object(
        [
            ("ok".to_owned(), norito::json::Value::Bool(true)),
            ("dataspace".to_owned(), args.dataspace.into()),
            (
                "contract_address".to_owned(),
                contract_address.to_string().into(),
            ),
            ("deploy_nonce".to_owned(), args.deploy_nonce.into()),
            ("next_deploy_nonce".to_owned(), next_nonce.into()),
            (
                "code_hash_hex".to_owned(),
                hex::encode(<[u8; 32]>::from(code_hash)).into(),
            ),
            (
                "register_bytes_tx_hash".to_owned(),
                register_bytes_hash.to_string().into(),
            ),
            (
                "register_manifest_tx_hash".to_owned(),
                register_manifest_hash.to_string().into(),
            ),
            (
                "activate_tx_hash".to_owned(),
                activate_hash.to_string().into(),
            ),
        ]
        .into_iter()
        .collect(),
    );
    println!("{}", norito::json::to_json_pretty(&result)?);
    Ok(())
}

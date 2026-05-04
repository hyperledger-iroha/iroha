//! Split contract deploy helper for oversized public deploy envelopes.
#![allow(clippy::too_many_lines)]

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

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
use iroha_version::codec::EncodeVersioned;

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
    #[arg(long)]
    gas_asset_id: Option<String>,
    #[arg(long, default_value_t = false)]
    route_anchor_authority_account: bool,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    emit_only: bool,
}

fn sign_transaction(
    chain: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    metadata: Metadata,
    instructions: impl IntoIterator<Item = InstructionBox>,
) -> SignedTransaction {
    TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions(instructions)
        .with_metadata(metadata)
        .sign(private_key)
}

fn write_tx(out_dir: &Path, stem: &str, tx: &SignedTransaction) -> Result<(PathBuf, usize)> {
    fs::create_dir_all(out_dir)
        .wrap_err_with(|| format!("create output directory {}", out_dir.display()))?;
    let path = out_dir.join(format!("{stem}.norito"));
    let bytes = tx.encode_versioned();
    fs::write(&path, &bytes).wrap_err_with(|| format!("write {}", path.display()))?;
    Ok((path, bytes.len()))
}

fn transaction_metadata(gas_asset_id: Option<&str>) -> Metadata {
    let mut metadata = Metadata::default();
    if let Some(asset_id) = gas_asset_id.filter(|value| !value.trim().is_empty()) {
        let gas_asset_key =
            Name::from_str("gas_asset_id").expect("static metadata key `gas_asset_id`");
        metadata.insert(gas_asset_key, Json::new(asset_id.to_owned()));
    }
    metadata
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
    let tx_metadata = transaction_metadata(args.gas_asset_id.as_deref());
    let route_anchor_instruction = args.route_anchor_authority_account.then(|| {
        InstructionBox::from(SetKeyValue::account(
            authority.clone(),
            nonce_key.clone(),
            Json::new(args.deploy_nonce),
        ))
    });

    let register_bytes_tx = sign_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata.clone(),
        route_anchor_instruction
            .clone()
            .into_iter()
            .chain([InstructionBox::from(RegisterSmartContractBytes {
                code_hash,
                code: code.clone(),
            })]),
    );
    let register_manifest_tx = sign_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata.clone(),
        route_anchor_instruction
            .clone()
            .into_iter()
            .chain([InstructionBox::from(RegisterSmartContractCode { manifest })]),
    );
    let activate_tx = sign_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata,
        [
            InstructionBox::from(ActivateContractInstance {
                contract_address: contract_address.clone(),
                code_hash,
            }),
            InstructionBox::from(SetKeyValue::account(
                authority.clone(),
                nonce_key,
                Json::new(next_nonce),
            )),
        ],
    );

    let register_bytes_hash = register_bytes_tx.hash();
    let register_manifest_hash = register_manifest_tx.hash();
    let activate_hash = activate_tx.hash();

    let written = if let Some(out_dir) = args.out_dir.as_deref() {
        Some(vec![
            (
                "register_bytes",
                write_tx(out_dir, "01-register-bytes", &register_bytes_tx)?,
            ),
            (
                "register_manifest",
                write_tx(out_dir, "02-register-manifest", &register_manifest_tx)?,
            ),
            ("activate", write_tx(out_dir, "03-activate", &activate_tx)?),
        ])
    } else {
        None
    };

    if !args.emit_only {
        client.submit_transaction_blocking(&register_bytes_tx)?;
        client.submit_transaction_blocking(&register_manifest_tx)?;
        client.submit_transaction_blocking(&activate_tx)?;
    }

    let mut fields = std::collections::BTreeMap::from([
        ("ok".to_owned(), norito::json::Value::Bool(true)),
        (
            "submitted".to_owned(),
            norito::json::Value::Bool(!args.emit_only),
        ),
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
    ]);
    if let Some(written) = written {
        let files = written
            .into_iter()
            .map(|(name, (path, size))| {
                norito::json::Value::Object(
                    [
                        ("name".to_owned(), name.into()),
                        ("path".to_owned(), path.display().to_string().into()),
                        ("size".to_owned(), (size as u64).into()),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
            .collect();
        fields.insert("files".to_owned(), norito::json::Value::Array(files));
    }
    let result = norito::json::Value::Object(fields);
    println!("{}", norito::json::to_json_pretty(&result)?);
    Ok(())
}

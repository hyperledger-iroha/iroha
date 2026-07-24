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
            CommitContractDeployment, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
            SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
        },
        metadata::Metadata,
        name::Name,
        prelude::*,
        transaction::{FeePaymentIntent, TransactionBuilder},
    },
};
use iroha_crypto::{Hash, KeyPair, PrivateKey};
use iroha_torii_shared::FeeQuoteResponse;
use iroha_version::codec::EncodeVersioned;

#[cfg(test)]
use iroha::data_model::transaction::Executable;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    authority: String,
    /// File containing one exact private-key literal. Inline key arguments are
    /// intentionally unsupported so process listings cannot expose the signer.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
    #[arg(long)]
    code_file: PathBuf,
    #[arg(long)]
    contract_address: String,
    #[arg(long)]
    contract_alias: String,
    #[arg(long, default_value = "universal")]
    dataspace: String,
    #[arg(long)]
    deploy_nonce: u64,
    #[arg(long, default_value_t = 753)]
    chain_discriminant: u16,
    #[arg(long)]
    lease_expiry_ms: Option<u64>,
    #[arg(long)]
    expected_previous_contract_address: Option<String>,
    /// Canonical JSON file selecting authority or an exact sponsor-program revision.
    #[arg(long, value_name = "PATH")]
    fee_payment_json: PathBuf,
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
) -> Result<SignedTransaction> {
    TransactionBuilder::new(
        chain.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .with_metadata(metadata)
    .try_sign(private_key)
    .wrap_err("failed to sign split contract deploy transaction")
}

fn fee_payment_selection_matches(requested: &FeePaymentIntent, quoted: &FeePaymentIntent) -> bool {
    match (requested, quoted) {
        (FeePaymentIntent::Authority(requested), FeePaymentIntent::Authority(quoted)) => {
            requested.gas_limit == quoted.gas_limit
        }
        (FeePaymentIntent::Sponsor(requested), FeePaymentIntent::Sponsor(quoted)) => {
            requested.program_id == quoted.program_id
                && requested.program_revision == quoted.program_revision
                && requested.gas_limit == quoted.gas_limit
        }
        _ => false,
    }
}

fn quote_and_resign_transaction(
    client: &Client,
    draft: SignedTransaction,
    fee_payment: &FeePaymentIntent,
    private_key: &PrivateKey,
) -> Result<(SignedTransaction, FeeQuoteResponse)> {
    let mut payload = draft.payload().clone();
    payload.fee_payment = fee_payment.clone();
    let quote = client
        .quote_fees(&payload)
        .wrap_err("failed to quote exact split-deploy transaction fees")?;
    if !fee_payment_selection_matches(fee_payment, &quote.intent) {
        return Err(eyre!(
            "fee quote changed the selected payer, sponsor revision, or gas bound"
        ));
    }
    quote
        .intent
        .validate()
        .wrap_err("fee quote returned an invalid payment intent")?;
    payload.fee_payment = quote.intent.clone();
    let transaction = TransactionBuilder::from_payload(payload)
        .wrap_err("quoted split-deploy payload has an invalid fee payment intent")?
        .try_sign(private_key)
        .wrap_err("failed to sign exact quoted split-deploy transaction")?;
    Ok((transaction, quote))
}

fn quote_native_upload_plan(
    client: &Client,
    mut plan: NativeUploadPlan,
    fee_payment: &FeePaymentIntent,
    private_key: &PrivateKey,
) -> Result<(NativeUploadPlan, Vec<FeeQuoteResponse>)> {
    let mut quotes = Vec::with_capacity(plan.pre_stage.len() + 1);
    for (_, _, transaction) in &mut plan.pre_stage {
        let (quoted, quote) =
            quote_and_resign_transaction(client, transaction.clone(), fee_payment, private_key)?;
        *transaction = quoted;
        quotes.push(quote);
    }
    let (quoted, quote) =
        quote_and_resign_transaction(client, plan.finalize.2, fee_payment, private_key)?;
    plan.finalize.2 = quoted;
    quotes.push(quote);
    Ok((plan, quotes))
}

fn write_tx(out_dir: &Path, stem: &str, tx: &SignedTransaction) -> Result<(PathBuf, usize)> {
    fs::create_dir_all(out_dir)
        .wrap_err_with(|| format!("create output directory {}", out_dir.display()))?;
    let path = out_dir.join(format!("{stem}.norito"));
    let bytes = tx.encode_versioned();
    fs::write(&path, &bytes).wrap_err_with(|| format!("write {}", path.display()))?;
    Ok((path, bytes.len()))
}

fn insert_contract_deployment_address_metadata(
    metadata: &mut Metadata,
    contract_address: &iroha::data_model::smart_contract::ContractAddress,
) {
    let address = contract_address.to_string();
    for key in ["gov_contract_address", "contract_address"] {
        metadata.insert(
            Name::from_str(key).expect("static contract deployment metadata key"),
            Json::new(address.clone()),
        );
    }
}

struct NativeUploadPlan {
    chunk_count: u32,
    pre_stage: Vec<(String, String, SignedTransaction)>,
    finalize: (String, String, SignedTransaction),
}

fn native_upload_report(plan: &NativeUploadPlan) -> norito::json::Value {
    let register_bytes_stage_tx_hashes = plan
        .pre_stage
        .iter()
        .map(|(_, _, transaction)| transaction.hash().to_string())
        .collect::<Vec<_>>();

    norito::json!({
        "register_bytes_tx_strategy": ("native_chunks"),
        "register_bytes_chunk_size": (u64::try_from(SMART_CONTRACT_CODE_CHUNK_BYTES)
            .expect("public contract chunk size fits u64")),
        "register_bytes_chunk_count": (plan.chunk_count),
        "register_bytes_stage_tx_hashes": (register_bytes_stage_tx_hashes),
        "register_bytes_tx_hash": (plan.finalize.2.hash().to_string()),
    })
}

fn deployment_transaction_sequence(
    upload_plan: NativeUploadPlan,
    register_manifest_tx: SignedTransaction,
    commit_tx: SignedTransaction,
) -> Vec<(String, String, SignedTransaction)> {
    let NativeUploadPlan {
        mut pre_stage,
        finalize,
        ..
    } = upload_plan;
    pre_stage.push(finalize);
    pre_stage.push((
        "register_manifest".to_owned(),
        "register-manifest".to_owned(),
        register_manifest_tx,
    ));
    pre_stage.push(("commit".to_owned(), "commit".to_owned(), commit_tx));
    pre_stage
}

fn build_native_upload_plan(
    chain: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    metadata: &Metadata,
    code_hash: Hash,
    code: &[u8],
) -> Result<NativeUploadPlan> {
    if code.is_empty() {
        return Err(eyre!("contract artifact must not be empty"));
    }
    let canonical_code_hash = ivm::contract_code_hash(code);
    if code_hash != canonical_code_hash {
        return Err(eyre!(
            "contract code hash does not match the canonical artifact hash"
        ));
    }
    let total_size = u64::try_from(code.len())
        .wrap_err("contract artifact length does not fit the upload descriptor")?;
    let chunk_count_usize = code.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES);
    let chunk_count = u32::try_from(chunk_count_usize)
        .wrap_err("contract upload chunk count does not fit u32")?;
    let mut pre_stage = Vec::with_capacity(chunk_count_usize.saturating_sub(1));

    for (index, chunk) in code.chunks(SMART_CONTRACT_CODE_CHUNK_BYTES).enumerate() {
        let chunk_index =
            u32::try_from(index).wrap_err("contract upload index does not fit u32")?;
        let mut instructions = Vec::with_capacity(2);
        instructions.push(InstructionBox::from(UploadSmartContractCodeChunk {
            code_hash,
            total_size,
            chunk_index,
            chunk_count,
            chunk: chunk.to_vec(),
        }));
        let is_final = index + 1 == chunk_count_usize;
        if is_final {
            instructions.push(InstructionBox::from(FinalizeSmartContractCodeUpload {
                code_hash,
                total_size,
                chunk_count,
            }));
        }
        let transaction = sign_transaction(
            chain,
            authority,
            private_key,
            metadata.clone(),
            instructions,
        )?;
        if is_final {
            return Ok(NativeUploadPlan {
                chunk_count,
                pre_stage,
                finalize: (
                    "register_bytes_finalize".to_owned(),
                    "register-bytes-finalize".to_owned(),
                    transaction,
                ),
            });
        }
        let ordinal = index + 1;
        pre_stage.push((
            format!("register_bytes_chunk_{ordinal:04}_of_{chunk_count_usize:04}"),
            format!("register-bytes-chunk-{ordinal:04}-of-{chunk_count_usize:04}"),
            transaction,
        ));
    }
    Err(eyre!("contract upload plan did not contain a final chunk"))
}

fn read_private_key_file(path: &Path) -> Result<PrivateKey> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect private-key file {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(eyre!(
            "private-key file {} must be a regular non-symlink file",
            path.display()
        ));
    }
    if metadata.len() > 16 * 1024 {
        return Err(eyre!(
            "private-key file {} exceeds the 16384 byte limit",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(eyre!(
                "private-key file {} must not be accessible by group or other users",
                path.display()
            ));
        }
    }
    let raw = fs::read_to_string(path)
        .wrap_err_with(|| format!("read private-key file {}", path.display()))?;
    let private_key = raw.trim_end_matches(['\r', '\n']);
    if private_key.is_empty()
        || private_key.trim() != private_key
        || private_key.chars().any(char::is_control)
    {
        return Err(eyre!(
            "private-key file {} must contain one exact private-key literal",
            path.display()
        ));
    }
    private_key
        .parse()
        .wrap_err_with(|| format!("parse private-key file {}", path.display()))
}

fn read_fee_payment_file(path: &Path) -> Result<FeePaymentIntent> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("read fee-payment file {}", path.display()))?;
    let supplied: norito::json::Value = norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("parse fee-payment file {}", path.display()))?;
    let intent: FeePaymentIntent = norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("parse fee-payment file {}", path.display()))?;
    intent
        .validate()
        .wrap_err("invalid signature-bound fee payment intent")?;
    let canonical =
        norito::json::to_value(&intent).wrap_err("serialize canonical fee payment intent")?;
    if supplied != canonical {
        return Err(eyre!(
            "fee-payment file {} is not the exact canonical intent schema",
            path.display()
        ));
    }
    Ok(intent)
}

#[allow(clippy::too_many_arguments)]
fn build_commit_transaction(
    chain: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    metadata: Metadata,
    expected_deploy_nonce: u64,
    contract_address: iroha::data_model::smart_contract::ContractAddress,
    code_hash: Hash,
    contract_alias: iroha::data_model::smart_contract::ContractAlias,
    lease_expiry_ms: Option<u64>,
    expected_previous_contract_address: Option<iroha::data_model::smart_contract::ContractAddress>,
) -> Result<SignedTransaction> {
    sign_transaction(
        chain,
        authority,
        private_key,
        metadata,
        [InstructionBox::from(CommitContractDeployment {
            expected_deploy_nonce,
            contract_address,
            code_hash,
            contract_alias,
            lease_expiry_ms,
            expected_previous_contract_address,
        })],
    )
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
    let private_key = read_private_key_file(&args.private_key_file)?;
    let fee_payment = read_fee_payment_file(&args.fee_payment_json)?;
    let signer = KeyPair::from(private_key.clone());
    let contract_address: iroha::data_model::smart_contract::ContractAddress = args
        .contract_address
        .parse()
        .wrap_err("failed to parse --contract-address")?;
    let contract_alias: iroha::data_model::smart_contract::ContractAlias = args
        .contract_alias
        .parse()
        .wrap_err("failed to parse --contract-alias")?;
    let expected_previous_contract_address = args
        .expected_previous_contract_address
        .as_deref()
        .map(str::parse)
        .transpose()
        .wrap_err("failed to parse --expected-previous-contract-address")?;

    let code =
        fs::read(&args.code_file).wrap_err_with(|| format!("read {}", args.code_file.display()))?;
    let verified = ivm::verify_contract_artifact(&code)
        .map_err(|err| eyre!("verify contract artifact: {err}"))?;
    let manifest = verified
        .manifest
        .try_signed(&signer)
        .wrap_err("failed to sign contract manifest")?;
    let code_hash = verified.code_hash;
    let next_nonce = args
        .deploy_nonce
        .checked_add(1)
        .ok_or_else(|| eyre!("deploy nonce overflow"))?;
    let mut tx_metadata = Metadata::default();
    insert_contract_deployment_address_metadata(&mut tx_metadata, &contract_address);

    let upload_plan = build_native_upload_plan(
        &client.chain,
        &authority,
        &private_key,
        &tx_metadata,
        code_hash,
        &code,
    )?;
    let (upload_plan, mut fee_quotes) =
        quote_native_upload_plan(&client, upload_plan, &fee_payment, &private_key)?;
    let upload_report = native_upload_report(&upload_plan);
    let register_manifest_tx = sign_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata.clone(),
        [InstructionBox::from(RegisterSmartContractCode { manifest })],
    )?;
    let (register_manifest_tx, register_manifest_quote) =
        quote_and_resign_transaction(&client, register_manifest_tx, &fee_payment, &private_key)?;
    fee_quotes.push(register_manifest_quote);
    let commit_tx = build_commit_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata,
        args.deploy_nonce,
        contract_address.clone(),
        code_hash,
        contract_alias.clone(),
        args.lease_expiry_ms,
        expected_previous_contract_address.clone(),
    )?;
    let (commit_tx, commit_quote) =
        quote_and_resign_transaction(&client, commit_tx, &fee_payment, &private_key)?;
    fee_quotes.push(commit_quote);

    let register_manifest_hash = register_manifest_tx.hash();
    let commit_hash = commit_tx.hash();
    let contract_subject_account = contract_address
        .subject_id()
        .to_i105_for_discriminant(args.chain_discriminant)
        .map_err(|err| eyre!(err.to_string()))
        .wrap_err("failed to encode contract subject for the target chain")?;
    let planned_transactions =
        deployment_transaction_sequence(upload_plan, register_manifest_tx, commit_tx);

    let written = if let Some(out_dir) = args.out_dir.as_deref() {
        Some(
            planned_transactions
                .iter()
                .map(|(name, slug, tx)| Ok((name.clone(), write_tx(out_dir, slug, tx)?)))
                .collect::<Result<Vec<_>>>()?,
        )
    } else {
        None
    };

    if !args.emit_only {
        for (_, _, transaction) in &planned_transactions {
            client.submit_transaction_blocking(transaction)?;
        }
    }

    let mut fields = std::collections::BTreeMap::from([
        ("ok".to_owned(), norito::json::Value::Bool(true)),
        (
            "submitted".to_owned(),
            norito::json::Value::Bool(!args.emit_only),
        ),
        ("chain_id".to_owned(), client.chain.to_string().into()),
        (
            "chain_discriminant".to_owned(),
            u64::from(args.chain_discriminant).into(),
        ),
        ("dataspace".to_owned(), args.dataspace.into()),
        (
            "contract_address".to_owned(),
            contract_address.to_string().into(),
        ),
        (
            "contract_alias".to_owned(),
            contract_alias.to_string().into(),
        ),
        (
            "contract_subject_account".to_owned(),
            contract_subject_account.into(),
        ),
        ("deploy_nonce".to_owned(), args.deploy_nonce.into()),
        ("next_deploy_nonce".to_owned(), next_nonce.into()),
        (
            "code_hash_hex".to_owned(),
            hex::encode(<[u8; 32]>::from(code_hash)).into(),
        ),
        (
            "register_manifest_tx_hash".to_owned(),
            register_manifest_hash.to_string().into(),
        ),
        ("commit_tx_hash".to_owned(), commit_hash.to_string().into()),
        (
            "fee_quotes".to_owned(),
            norito::json::to_value(&fee_quotes).wrap_err("encode split-deploy fee quotes")?,
        ),
    ]);
    fields.insert(
        "expected_previous_contract_address".to_owned(),
        expected_previous_contract_address.map_or(norito::json::Value::Null, |address| {
            address.to_string().into()
        }),
    );
    let norito::json::Value::Object(upload_report) = upload_report else {
        unreachable!("native upload report is always an object");
    };
    fields.extend(upload_report);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_split_contract_deploy_ed25519_key_fixture() -> KeyPair {
        KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519)
            .expect("generate checked split contract deploy fixture key")
    }

    fn private_key_file_fixture(contents: &str) -> Result<tempfile::NamedTempFile> {
        use std::io::Write as _;

        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(contents.as_bytes())?;
        file.flush()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(file.path(), fs::Permissions::from_mode(0o600))?;
        }
        Ok(file)
    }

    #[test]
    fn private_key_file_accepts_one_exact_literal_with_terminal_newline() -> Result<()> {
        let expected = checked_split_contract_deploy_ed25519_key_fixture();
        let exposed = iroha_crypto::ExposedPrivateKey(expected.private_key().clone()).to_string();
        let file = private_key_file_fixture(&format!("{exposed}\n"))?;

        let actual = read_private_key_file(file.path())?;

        assert_eq!(
            KeyPair::from(actual).public_key(),
            expected.public_key(),
            "the file parser must preserve the exact private key"
        );
        Ok(())
    }

    #[test]
    fn private_key_file_rejects_surrounding_whitespace_without_echoing_secret() -> Result<()> {
        let secret = "secret-material-that-must-not-appear-in-errors";
        let file = private_key_file_fixture(&format!(" {secret}\n"))?;

        let error = read_private_key_file(file.path()).expect_err("whitespace must be rejected");
        let message = error.to_string();

        assert!(message.contains("one exact private-key literal"));
        assert!(!message.contains(secret));
        Ok(())
    }

    #[test]
    fn fee_payment_file_accepts_canonical_authority_gas_bound() -> Result<()> {
        let file = private_key_file_fixture(
            r#"{"payer":"authority","value":{"charge_limits":[],"gas_limit":2000000}}"#,
        )?;

        let actual = read_fee_payment_file(file.path())?;

        assert_eq!(
            actual,
            FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(2_000_000))
        );
        Ok(())
    }

    #[test]
    fn fee_payment_file_rejects_unknown_compatibility_fields() -> Result<()> {
        let file = private_key_file_fixture(
            r#"{"payer":"authority","value":{"charge_limits":[],"gas_limit":2000000,"legacy_fee":true}}"#,
        )?;

        let error =
            read_fee_payment_file(file.path()).expect_err("unknown fee fields must be rejected");

        let message = error.to_string();
        assert!(
            message.contains("parse fee-payment file")
                || message.contains("not the exact canonical intent schema")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn private_key_file_rejects_group_readable_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let file = private_key_file_fixture("not-inspected-after-mode-check\n")?;
        fs::set_permissions(file.path(), fs::Permissions::from_mode(0o640))?;

        let error =
            read_private_key_file(file.path()).expect_err("group-readable secrets must fail");

        assert!(error.to_string().contains("group or other users"));
        Ok(())
    }

    #[test]
    fn clap_surface_does_not_accept_inline_private_keys() {
        let parsed = Args::try_parse_from([
            "split-contract-deploy",
            "--config",
            "client.toml",
            "--authority",
            "authority",
            "--private-key",
            "must-not-be-accepted",
            "--code-file",
            "contract.to",
            "--contract-address",
            "contract",
            "--deploy-nonce",
            "1",
            "--fee-payment-json",
            "fee.json",
        ]);

        assert!(
            parsed.is_err(),
            "inline private keys must not be a CLI option"
        );
    }

    #[test]
    fn split_contract_deploy_fixture_uses_checked_ed25519_key_generation() {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let actual = key_pair
            .public_key()
            .try_algorithm()
            .expect("split contract deploy fixture key advertises a valid algorithm");

        assert_eq!(actual, iroha_crypto::Algorithm::Ed25519);
    }

    #[test]
    fn sign_transaction_checked_helper_verifies() -> Result<()> {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());

        let tx = sign_transaction(
            &ChainId::from("split-contract-deploy-sign-test"),
            &authority,
            key_pair.private_key(),
            Metadata::default(),
            Vec::<InstructionBox>::new(),
        )?;

        tx.verify_signature()
            .wrap_err("verify split contract deploy helper signature")?;
        assert_eq!(tx.authority(), &authority);
        Ok(())
    }

    #[test]
    fn commit_transaction_uses_native_nonce_cas_without_generic_metadata_write() -> Result<()> {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            369,
            &authority,
            7,
            DataSpaceId::UNIVERSAL,
        )?;
        let contract_alias: iroha::data_model::smart_contract::ContractAlias =
            "validation_fee_pool::universal".parse()?;
        let code_hash = Hash::new(b"reviewed-contract-artifact");
        let transaction = build_commit_transaction(
            &ChainId::from("split-contract-deploy-native-commit-test"),
            &authority,
            key_pair.private_key(),
            Metadata::default(),
            7,
            contract_address.clone(),
            code_hash,
            contract_alias.clone(),
            None,
            None,
        )?;

        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("native contract commit must use one instruction transaction");
        };
        assert_eq!(instructions.len(), 1);
        let commit = instructions[0]
            .as_any()
            .downcast_ref::<CommitContractDeployment>()
            .expect("deployment must use the native nonce/alias CAS instruction");
        assert_eq!(commit.expected_deploy_nonce, 7);
        assert_eq!(commit.contract_address, contract_address);
        assert_eq!(commit.code_hash, code_hash);
        assert_eq!(commit.contract_alias, contract_alias);
        assert!(commit.expected_previous_contract_address.is_none());
        assert!(
            instructions[0]
                .as_any()
                .downcast_ref::<iroha::data_model::isi::SetKeyValueBox>()
                .is_none(),
            "generic account metadata writes cannot advance the reserved deploy nonce"
        );
        Ok(())
    }

    #[test]
    fn native_upload_plan_rejects_empty_artifact() {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let result = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-empty-upload-test"),
            &authority,
            key_pair.private_key(),
            &Metadata::default(),
            Hash::new(b""),
            &[],
        );
        let error = match result {
            Ok(_) => panic!("an empty artifact cannot form a native upload"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn native_upload_plan_rejects_noncanonical_code_hash() {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let code = [0x01, 0x02, 0x03];
        let result = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-wrong-hash-test"),
            &authority,
            key_pair.private_key(),
            &Metadata::default(),
            Hash::new(b"not-the-canonical-artifact-hash"),
            &code,
        );
        let error = match result {
            Ok(_) => panic!("a mismatched code hash cannot form a native upload"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("canonical artifact hash"));
    }

    #[test]
    fn one_chunk_upload_uploads_and_finalizes_without_reserved_nonce_mutation() -> Result<()> {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let code = vec![0x5a; SMART_CONTRACT_CODE_CHUNK_BYTES];
        let plan = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-native-upload-test"),
            &authority,
            key_pair.private_key(),
            &Metadata::default(),
            ivm::contract_code_hash(&code),
            &code,
        )?;

        assert_eq!(plan.chunk_count, 1);
        assert!(plan.pre_stage.is_empty());
        let Executable::Instructions(instructions) = plan.finalize.2.instructions() else {
            panic!("native upload must use instruction transactions");
        };
        assert_eq!(instructions.len(), 2);
        let upload = instructions[0]
            .as_any()
            .downcast_ref::<UploadSmartContractCodeChunk>()
            .expect("one-chunk transaction uploads code");
        assert_eq!(upload.code_hash, ivm::contract_code_hash(&code));
        assert_eq!(upload.total_size, u64::try_from(code.len())?);
        assert_eq!(upload.chunk_index, 0);
        assert_eq!(upload.chunk_count, 1);
        assert_eq!(upload.chunk, code);
        let finalize = instructions[1]
            .as_any()
            .downcast_ref::<FinalizeSmartContractCodeUpload>()
            .expect("one-chunk transaction finalizes after upload");
        assert_eq!(finalize.code_hash, upload.code_hash);
        assert_eq!(finalize.total_size, upload.total_size);
        assert_eq!(finalize.chunk_count, upload.chunk_count);

        let report = native_upload_report(&plan);
        let fields = report.as_object().expect("upload report is an object");
        assert_eq!(fields.len(), 5);
        assert!(!fields.contains_key("direct_register_bytes_tx_size"));
        assert_eq!(
            fields["register_bytes_tx_strategy"].as_str(),
            Some("native_chunks")
        );
        assert_eq!(fields["register_bytes_chunk_count"].as_u64(), Some(1));
        assert!(
            fields["register_bytes_stage_tx_hashes"]
                .as_array()
                .expect("stage hashes are an array")
                .is_empty()
        );
        let finalize_hash = plan.finalize.2.hash().to_string();
        assert_eq!(
            fields["register_bytes_tx_hash"].as_str(),
            Some(finalize_hash.as_str())
        );
        Ok(())
    }

    #[test]
    fn multi_mib_upload_is_bounded_ordered_and_carries_stable_metadata() -> Result<()> {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let code = (0..(3 * 1024 * 1024 + 17))
            .map(|index| u8::try_from(index % 251).expect("remainder fits u8"))
            .collect::<Vec<_>>();
        let mut metadata = Metadata::default();
        let contract_address = iroha::data_model::smart_contract::ContractAddress::derive(
            0,
            &authority,
            0,
            DataSpaceId::UNIVERSAL,
        )?;
        insert_contract_deployment_address_metadata(&mut metadata, &contract_address);
        let code_hash = ivm::contract_code_hash(&code);
        let plan = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-large-native-upload-test"),
            &authority,
            key_pair.private_key(),
            &metadata,
            code_hash,
            &code,
        )?;

        let expected_count = code.len().div_ceil(SMART_CONTRACT_CODE_CHUNK_BYTES);
        assert_eq!(usize::try_from(plan.chunk_count)?, expected_count);
        assert_eq!(plan.pre_stage.len(), expected_count - 1);
        assert_eq!(
            plan.pre_stage.first().map(|(_, slug, _)| slug.as_str()),
            Some("register-bytes-chunk-0001-of-0049")
        );
        assert_eq!(
            plan.pre_stage.last().map(|(_, slug, _)| slug.as_str()),
            Some("register-bytes-chunk-0048-of-0049")
        );
        assert_eq!(plan.finalize.1, "register-bytes-finalize");

        let transactions = plan
            .pre_stage
            .iter()
            .map(|(_, _, transaction)| transaction)
            .chain(std::iter::once(&plan.finalize.2))
            .collect::<Vec<_>>();
        let address = contract_address.to_string();
        let mut rebuilt = Vec::with_capacity(code.len());
        for (index, transaction) in transactions.iter().enumerate() {
            assert_eq!(transaction.metadata(), &metadata);
            for key in ["gov_contract_address", "contract_address"] {
                let key = Name::from_str(key)?;
                assert_eq!(
                    transaction
                        .metadata()
                        .get(&key)
                        .and_then(|value| value.try_into_any_norito::<String>().ok())
                        .as_deref(),
                    Some(address.as_str())
                );
            }
            assert!(
                transaction.encode_versioned().len()
                    < iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_TX_GOSSIP.get(),
                "chunk transaction {index} exceeded the default gossip frame"
            );
            let Executable::Instructions(instructions) = transaction.instructions() else {
                panic!("native upload must use instruction transactions");
            };
            let uploads = instructions
                .iter()
                .filter_map(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<UploadSmartContractCodeChunk>()
                })
                .collect::<Vec<_>>();
            assert_eq!(uploads.len(), 1);
            let upload = uploads[0];
            assert_eq!(usize::try_from(upload.chunk_index)?, index);
            assert_eq!(upload.chunk_count, plan.chunk_count);
            assert_eq!(upload.total_size, u64::try_from(code.len())?);
            assert_eq!(upload.code_hash, code_hash);
            let expected_chunk_len = code
                .len()
                .saturating_sub(index * SMART_CONTRACT_CODE_CHUNK_BYTES)
                .min(SMART_CONTRACT_CODE_CHUNK_BYTES);
            assert_eq!(upload.chunk.len(), expected_chunk_len);
            assert!(upload.chunk.len() < code.len());
            rebuilt.extend_from_slice(&upload.chunk);
            let reserved_nonce_mutation_count = instructions
                .iter()
                .filter(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<iroha::data_model::isi::SetKeyValueBox>()
                        .is_some()
                })
                .count();
            assert_eq!(reserved_nonce_mutation_count, 0);
            let finalize_count = instructions
                .iter()
                .filter(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<FinalizeSmartContractCodeUpload>()
                        .is_some()
                })
                .count();
            assert_eq!(finalize_count, usize::from(index + 1 == expected_count));
            if index + 1 == expected_count {
                let finalize = instructions
                    .iter()
                    .find_map(|instruction| {
                        instruction
                            .as_any()
                            .downcast_ref::<FinalizeSmartContractCodeUpload>()
                    })
                    .expect("last upload transaction must finalize");
                assert_eq!(finalize.code_hash, upload.code_hash);
                assert_eq!(finalize.total_size, upload.total_size);
                assert_eq!(finalize.chunk_count, upload.chunk_count);
            }
            assert_eq!(
                instructions.len(),
                1 + usize::from(index + 1 == expected_count)
            );
        }
        assert_eq!(rebuilt, code);

        let report = native_upload_report(&plan);
        let fields = report.as_object().expect("upload report is an object");
        assert_eq!(
            fields["register_bytes_chunk_size"].as_u64(),
            Some(u64::try_from(SMART_CONTRACT_CODE_CHUNK_BYTES)?)
        );
        assert_eq!(
            fields["register_bytes_chunk_count"].as_u64(),
            Some(u64::try_from(expected_count)?)
        );
        let stage_hashes = fields["register_bytes_stage_tx_hashes"]
            .as_array()
            .expect("stage hashes are an array");
        assert_eq!(stage_hashes.len(), expected_count - 1);
        for (reported, (_, _, transaction)) in stage_hashes.iter().zip(&plan.pre_stage) {
            let expected = transaction.hash().to_string();
            assert_eq!(reported.as_str(), Some(expected.as_str()));
        }
        Ok(())
    }

    #[test]
    fn emit_sequence_writes_exact_ordered_native_filenames() -> Result<()> {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let chain = ChainId::from("split-contract-deploy-emit-sequence-test");
        let metadata = Metadata::default();
        let code = vec![0x83; 2 * SMART_CONTRACT_CODE_CHUNK_BYTES + 1];
        let upload_plan = build_native_upload_plan(
            &chain,
            &authority,
            key_pair.private_key(),
            &metadata,
            ivm::contract_code_hash(&code),
            &code,
        )?;
        let trailing_transaction = || {
            sign_transaction(
                &chain,
                &authority,
                key_pair.private_key(),
                metadata.clone(),
                Vec::<InstructionBox>::new(),
            )
        };
        let sequence = deployment_transaction_sequence(
            upload_plan,
            trailing_transaction()?,
            trailing_transaction()?,
        );
        let expected_slugs = [
            "register-bytes-chunk-0001-of-0003",
            "register-bytes-chunk-0002-of-0003",
            "register-bytes-finalize",
            "register-manifest",
            "commit",
        ];
        assert_eq!(
            sequence
                .iter()
                .map(|(_, slug, _)| slug.as_str())
                .collect::<Vec<_>>(),
            expected_slugs.to_vec()
        );
        assert!(
            sequence
                .iter()
                .all(|(_, _, transaction)| transaction.metadata() == &metadata)
        );

        let output = tempfile::tempdir()?;
        let written = sequence
            .iter()
            .map(|(_, slug, transaction)| write_tx(output.path(), slug, transaction))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(
            written
                .iter()
                .map(|(path, _)| path.file_name().and_then(std::ffi::OsStr::to_str).unwrap())
                .collect::<Vec<_>>(),
            expected_slugs
                .iter()
                .map(|slug| format!("{slug}.norito"))
                .collect::<Vec<_>>()
        );
        for ((_, _, transaction), (path, byte_len)) in sequence.iter().zip(&written) {
            let encoded = transaction.encode_versioned();
            assert_eq!(*byte_len, encoded.len());
            assert_eq!(fs::read(path)?, encoded);
        }
        Ok(())
    }
}

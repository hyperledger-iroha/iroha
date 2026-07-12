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
            ActivateContractInstance, FinalizeSmartContractCodeUpload, RegisterSmartContractCode,
            SMART_CONTRACT_CODE_CHUNK_BYTES, UploadSmartContractCodeChunk,
        },
        metadata::Metadata,
        name::Name,
        prelude::*,
        smart_contract::CONTRACT_DEPLOY_NONCE_METADATA_KEY,
        transaction::TransactionBuilder,
    },
};
use iroha_crypto::{Hash, KeyPair, PrivateKey};
use iroha_version::codec::EncodeVersioned;

#[cfg(test)]
use iroha::data_model::transaction::Executable;

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
) -> Result<SignedTransaction> {
    TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions(instructions)
        .with_metadata(metadata)
        .try_sign(private_key)
        .wrap_err("failed to sign split contract deploy transaction")
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
    activate_tx: SignedTransaction,
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
    pre_stage.push(("activate".to_owned(), "activate".to_owned(), activate_tx));
    pre_stage
}

fn build_native_upload_plan(
    chain: &ChainId,
    authority: &AccountId,
    private_key: &PrivateKey,
    metadata: &Metadata,
    route_anchor_instruction: Option<InstructionBox>,
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
        let mut instructions = Vec::with_capacity(3);
        if index == 0
            && let Some(anchor) = route_anchor_instruction.clone()
        {
            instructions.push(anchor);
        }
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
    let manifest = verified
        .manifest
        .try_signed(&signer)
        .wrap_err("failed to sign contract manifest")?;
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

    let upload_plan = build_native_upload_plan(
        &client.chain,
        &authority,
        &private_key,
        &tx_metadata,
        route_anchor_instruction,
        code_hash,
        &code,
    )?;
    let upload_report = native_upload_report(&upload_plan);
    let register_manifest_tx = sign_transaction(
        &client.chain,
        &authority,
        &private_key,
        tx_metadata.clone(),
        [InstructionBox::from(RegisterSmartContractCode { manifest })],
    )?;
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
    )?;

    let register_manifest_hash = register_manifest_tx.hash();
    let activate_hash = activate_tx.hash();
    let planned_transactions =
        deployment_transaction_sequence(upload_plan, register_manifest_tx, activate_tx);

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
            "register_manifest_tx_hash".to_owned(),
            register_manifest_hash.to_string().into(),
        ),
        (
            "activate_tx_hash".to_owned(),
            activate_hash.to_string().into(),
        ),
    ]);
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
    fn native_upload_plan_rejects_empty_artifact() {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let result = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-empty-upload-test"),
            &authority,
            key_pair.private_key(),
            &Metadata::default(),
            None,
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
            None,
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
    fn one_chunk_upload_bootstraps_then_uploads_and_finalizes() -> Result<()> {
        let key_pair = checked_split_contract_deploy_ed25519_key_fixture();
        let authority = AccountId::new(key_pair.public_key().clone());
        let code = vec![0x5a; SMART_CONTRACT_CODE_CHUNK_BYTES];
        let anchor = InstructionBox::from(SetKeyValue::account(
            authority.clone(),
            Name::from_str(CONTRACT_DEPLOY_NONCE_METADATA_KEY)?,
            Json::new(0_u64),
        ));
        let plan = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-native-upload-test"),
            &authority,
            key_pair.private_key(),
            &Metadata::default(),
            Some(anchor),
            ivm::contract_code_hash(&code),
            &code,
        )?;

        assert_eq!(plan.chunk_count, 1);
        assert!(plan.pre_stage.is_empty());
        let Executable::Instructions(instructions) = plan.finalize.2.instructions() else {
            panic!("native upload must use instruction transactions");
        };
        assert_eq!(instructions.len(), 3);
        assert!(
            instructions[0]
                .as_any()
                .downcast_ref::<iroha::data_model::isi::SetKeyValueBox>()
                .is_some()
        );
        let upload = instructions[1]
            .as_any()
            .downcast_ref::<UploadSmartContractCodeChunk>()
            .expect("one-chunk transaction uploads code after bootstrap");
        assert_eq!(upload.code_hash, ivm::contract_code_hash(&code));
        assert_eq!(upload.total_size, u64::try_from(code.len())?);
        assert_eq!(upload.chunk_index, 0);
        assert_eq!(upload.chunk_count, 1);
        assert_eq!(upload.chunk, code);
        let finalize = instructions[2]
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
        let metadata = transaction_metadata(Some("fee#split-native"));
        let anchor = InstructionBox::from(SetKeyValue::account(
            authority.clone(),
            Name::from_str(CONTRACT_DEPLOY_NONCE_METADATA_KEY)?,
            Json::new(0_u64),
        ));
        let code_hash = ivm::contract_code_hash(&code);
        let plan = build_native_upload_plan(
            &ChainId::from("split-contract-deploy-large-native-upload-test"),
            &authority,
            key_pair.private_key(),
            &metadata,
            Some(anchor),
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
        let mut rebuilt = Vec::with_capacity(code.len());
        for (index, transaction) in transactions.iter().enumerate() {
            assert_eq!(transaction.metadata(), &metadata);
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
            let bootstrap_count = instructions
                .iter()
                .filter(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<iroha::data_model::isi::SetKeyValueBox>()
                        .is_some()
                })
                .count();
            assert_eq!(bootstrap_count, usize::from(index == 0));
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
                1 + usize::from(index == 0) + usize::from(index + 1 == expected_count)
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
        let metadata = transaction_metadata(Some("fee#emit"));
        let code = vec![0x83; 2 * SMART_CONTRACT_CODE_CHUNK_BYTES + 1];
        let upload_plan = build_native_upload_plan(
            &chain,
            &authority,
            key_pair.private_key(),
            &metadata,
            None,
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
            "activate",
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

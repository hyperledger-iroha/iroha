//! Verify that one canonical signed genesis is the semantic realization of
//! one policy genesis and is signed by the expected root account.

use std::{
    fs::{self, OpenOptions},
    io::Read as _,
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::Parser;
use iroha_core::validate_genesis_block;
use iroha_crypto::PublicKey;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::decode_framed_signed_block,
    isi::{InstructionBox, SetParameter},
    parameter::{Parameter, system::consensus_metadata},
    transaction::Executable,
};
use iroha_genesis::RawGenesisTransaction;

const MAX_GENESIS_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(about = "Verify a PK3 policy-genesis/signed-genesis/root-signer binding")]
struct Args {
    #[arg(long, value_name = "PATH")]
    genesis: PathBuf,
    #[arg(long, value_name = "PATH")]
    signed_genesis: PathBuf,
    #[arg(long, value_name = "CHAIN")]
    chain_id: ChainId,
    #[arg(long, value_name = "PUBLIC_KEY")]
    genesis_public_key: PublicKey,
}

#[derive(norito::JsonSerialize)]
struct Receipt {
    schema: &'static str,
    status: &'static str,
    chain_id: String,
    genesis_public_key: String,
    transaction_count: u64,
    block_hash: String,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(receipt) => match norito::json::to_json(&receipt) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("PK3 genesis binding verification failed: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("PK3 genesis binding verification failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<Receipt, String> {
    let (policy_bytes, policy_metadata) = read_owner_file(&args.genesis, "policy genesis")?;
    let (signed_bytes, _) = read_owner_file(&args.signed_genesis, "signed genesis")?;

    iroha_genesis::init_instruction_registry();
    let manifest = RawGenesisTransaction::from_path(&args.genesis)
        .map_err(|error| format!("policy genesis is invalid: {error}"))?;
    let policy_after = fs::metadata(&args.genesis)
        .map_err(|error| format!("reinspect policy genesis: {error}"))?;
    if !same_file(&policy_metadata, &policy_after)
        || fs::read(&args.genesis).map_err(|error| format!("reread policy genesis: {error}"))?
            != policy_bytes
    {
        return Err("policy genesis changed while it was parsed".to_owned());
    }
    if manifest.chain_id() != &args.chain_id {
        return Err("policy genesis chain differs from --chain-id".to_owned());
    }

    let block = decode_framed_signed_block(&signed_bytes)
        .map_err(|error| format!("signed genesis is not a framed SignedBlock: {error}"))?;
    let canonical = block
        .encode_wire()
        .map_err(|error| format!("re-encode signed genesis: {error}"))?;
    if canonical != signed_bytes {
        return Err("signed genesis is not canonical framed Norito".to_owned());
    }
    let genesis_account = AccountId::new(args.genesis_public_key.clone());
    validate_genesis_block(&block, &genesis_account, &args.chain_id)
        .map_err(|error| format!("root signature or genesis invariants failed: {error}"))?;

    let expected = manifest
        .with_consensus_meta()
        .parse()
        .map_err(|error| format!("expand policy genesis instructions: {error}"))?;
    let actual = block.external_transactions().collect::<Vec<_>>();
    if expected.len() != actual.len() {
        return Err("signed genesis transaction count differs from policy genesis".to_owned());
    }
    for (index, (expected_batch, transaction)) in expected.iter().zip(&actual).enumerate() {
        if transaction.chain() != &args.chain_id || transaction.authority() != &genesis_account {
            return Err(format!(
                "signed genesis transaction {index} has the wrong chain or root authority"
            ));
        }
        let Executable::Instructions(actual_batch) = transaction.instructions() else {
            return Err(format!(
                "signed genesis transaction {index} is not an instruction batch"
            ));
        };
        let expected_semantic = expected_batch
            .iter()
            .filter(|instruction| !is_staged_consensus_commitment(instruction))
            .map(iroha_data_model::Encode::encode)
            .collect::<Vec<_>>();
        let actual_semantic = actual_batch
            .iter()
            .filter(|instruction| !is_staged_consensus_commitment(instruction))
            .map(iroha_data_model::Encode::encode)
            .collect::<Vec<_>>();
        if expected_semantic != actual_semantic {
            return Err(format!(
                "signed genesis transaction {index} differs from policy genesis"
            ));
        }
    }

    Ok(Receipt {
        schema: "pkdeploy.pk3.genesis_binding_verification.v1",
        status: "verified",
        chain_id: args.chain_id.to_string(),
        genesis_public_key: args.genesis_public_key.to_string(),
        transaction_count: u64::try_from(actual.len())
            .map_err(|_| "genesis transaction count does not fit u64".to_owned())?,
        block_hash: block.hash().to_string(),
    })
}

fn is_staged_consensus_commitment(instruction: &InstructionBox) -> bool {
    let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() else {
        return false;
    };
    let Parameter::Custom(custom) = set_parameter.inner() else {
        return false;
    };
    custom.id() == &consensus_metadata::handshake_meta_id()
}

fn read_owner_file(path: &Path, label: &str) -> Result<(Vec<u8>, fs::Metadata), String> {
    reject_symlink_components(path, label)?;
    let lexical =
        fs::symlink_metadata(path).map_err(|error| format!("inspect {label}: {error}"))?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let mut file = options
        .open(path)
        .map_err(|error| format!("open {label} without following links: {error}"))?;
    let before = file
        .metadata()
        .map_err(|error| format!("inspect opened {label}: {error}"))?;
    if !before.is_file()
        || !same_identity(&lexical, &before)
        || before.uid() != rustix::process::geteuid().as_raw()
        || before.mode() & 0o777 != 0o600
        || before.nlink() != 1
        || before.size() == 0
        || before.size() > MAX_GENESIS_BYTES
    {
        return Err(format!(
            "{label} must be an owner-held mode-0600 single-link regular file"
        ));
    }
    let capacity = usize::try_from(before.size()).map_err(|_| format!("{label} is too large"))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("read {label}: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("reinspect {label}: {error}"))?;
    if !same_file(&before, &after) || bytes.len() as u64 != before.size() {
        return Err(format!("{label} changed while it was read"));
    }
    Ok((bytes, before))
}

fn reject_symlink_components(path: &Path, label: &str) -> Result<(), String> {
    let mut current = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("resolve current directory: {error}"))?
            .join(path)
    };
    loop {
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!("{label} path contains a symbolic link"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect {label} path: {error}")),
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }
    Ok(())
}

fn same_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_identity(left, right)
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.size() == right.size()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

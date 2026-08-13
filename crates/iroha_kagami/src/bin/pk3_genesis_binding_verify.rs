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
    isi::{InstructionBox, SetParameter},
    parameter::{Parameter, system::consensus_metadata},
    transaction::Executable,
};
use iroha_genesis::{
    GENESIS_MANIFEST_JSON_MAX_BYTES_V1, RawGenesisTransaction, SIGNED_GENESIS_MAX_BYTES_V1,
    decode_signed_genesis,
};
const MAX_POLICY_GENESIS_BYTES: usize = GENESIS_MANIFEST_JSON_MAX_BYTES_V1;
const MAX_SIGNED_GENESIS_BYTES: usize = SIGNED_GENESIS_MAX_BYTES_V1;
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
    let (policy_bytes, policy_metadata) =
        read_owner_file(&args.genesis, "policy genesis", MAX_POLICY_GENESIS_BYTES)?;
    iroha_genesis::init_instruction_registry();
    let manifest = RawGenesisTransaction::from_json_slice(&policy_bytes)
        .map_err(|error| format!("policy genesis is invalid: {error}"))?;
    let (policy_after_bytes, policy_after) =
        read_owner_file(&args.genesis, "policy genesis", MAX_POLICY_GENESIS_BYTES)?;
    if !same_file(&policy_metadata, &policy_after) || policy_after_bytes != policy_bytes {
        return Err("policy genesis changed while it was parsed".to_owned());
    }
    drop(policy_bytes);
    drop(policy_after_bytes);
    if manifest.chain_id() != &args.chain_id {
        return Err("policy genesis chain differs from --chain-id".to_owned());
    }
    let (signed_bytes, _) = read_owner_file(
        &args.signed_genesis,
        "signed genesis",
        MAX_SIGNED_GENESIS_BYTES,
    )?;
    let block = decode_signed_genesis(&signed_bytes)
        .map_err(|error| format!("signed genesis is not a framed SignedBlock: {error}"))?;
    let canonical = block
        .encode_wire()
        .map_err(|error| format!("re-encode signed genesis: {error}"))?;
    if canonical != signed_bytes {
        return Err("signed genesis is not canonical framed Norito".to_owned());
    }
    drop(canonical);
    drop(signed_bytes);
    let genesis_account = AccountId::new(args.genesis_public_key.clone());
    validate_genesis_block(&block, &genesis_account)
        .map_err(|error| format!("root signature or genesis invariants failed: {error}"))?;
    let expected = manifest
        .with_consensus_meta()
        .parse()
        .map_err(|error| format!("expand policy genesis instructions: {error}"))?;
    let actual_len = block.external_transactions().len();
    if expected.len() != actual_len {
        return Err("signed genesis transaction count differs from policy genesis".to_owned());
    }
    for (index, (expected_batch, transaction)) in expected
        .iter()
        .zip(block.external_transactions())
        .enumerate()
    {
        if transaction.domain() != &iroha_data_model::transaction::TransactionDomain::Genesis
            || transaction.authority() != &genesis_account
        {
            return Err(format!(
                "signed genesis transaction {index} has the wrong genesis domain or root authority"
            ));
        }
        let Executable::Instructions(actual_batch) = transaction.instructions() else {
            return Err(format!(
                "signed genesis transaction {index} is not an instruction batch"
            ));
        };
        let mut expected_semantic = expected_batch
            .iter()
            .filter(|instruction| !is_staged_consensus_commitment(instruction));
        let mut actual_semantic = actual_batch
            .iter()
            .filter(|instruction| !is_staged_consensus_commitment(instruction));
        loop {
            match (expected_semantic.next(), actual_semantic.next()) {
                (Some(expected), Some(actual))
                    if iroha_data_model::Encode::encode(expected)
                        == iroha_data_model::Encode::encode(actual) => {}
                (None, None) => break,
                _ => {
                    return Err(format!(
                        "signed genesis transaction {index} differs from policy genesis"
                    ));
                }
            }
        }
    }
    Ok(Receipt {
        schema: "pkdeploy.pk3.genesis_binding_verification.v1",
        status: "verified",
        chain_id: args.chain_id.to_string(),
        genesis_public_key: args.genesis_public_key.to_string(),
        transaction_count: u64::try_from(actual_len)
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
fn read_owner_file(
    path: &Path,
    label: &str,
    max_bytes: usize,
) -> Result<(Vec<u8>, fs::Metadata), String> {
    let max_bytes_u64 =
        u64::try_from(max_bytes).map_err(|_| format!("{label} limit does not fit u64"))?;
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
        || before.size() > max_bytes_u64
    {
        return Err(format!(
            "{label} must be an owner-held mode-0600 single-link regular file"
        ));
    }
    let capacity = usize::try_from(before.size()).map_err(|_| format!("{label} is too large"))?;
    let mut bytes = Vec::with_capacity(capacity.saturating_add(1));
    (&mut file)
        .take(before.size().saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label}: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("reinspect {label}: {error}"))?;
    if !same_file(&before, &after)
        || u64::try_from(bytes.len()).ok() != Some(before.size())
        || bytes.len() > max_bytes
    {
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
#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use super::*;
    #[test]
    fn owner_file_reader_rejects_first_byte_over_limit() {
        let directory = tempfile::tempdir().expect("create owner-file test directory");
        let root = directory
            .path()
            .canonicalize()
            .expect("canonical owner-file test directory");
        let path = root.join("genesis.nrt");
        fs::write(&path, [0xA5; 33]).expect("write bounded owner file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("set owner-only test permissions");
        assert_eq!(
            read_owner_file(&path, "test genesis", 32)
                .expect_err("oversized owner file must fail before reading")
                .as_str(),
            "test genesis must be an owner-held mode-0600 single-link regular file"
        );
    }
}

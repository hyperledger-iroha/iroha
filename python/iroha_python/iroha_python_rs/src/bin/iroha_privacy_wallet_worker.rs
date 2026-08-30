#![deny(unsafe_code)]
use iroha_python_rs::{
    privacy_wallet_bundle::encode_privacy_wallet_execution_bundle_v1,
    privacy_wallet_worker::run_pipe_session,
};
use rand_core_06::{OsRng, RngCore as _};
use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};
use zeroize::{Zeroize, Zeroizing};

const MAX_PUBLIC_ACTION_BYTES: u64 = 512 * 1_024;
const MAX_PROTOCOL_WITNESS_BYTES: u64 = 8 * 1_024 * 1_024;

#[cfg(all(unix, not(target_os = "haiku")))]
fn harden_process() -> Result<(), ()> {
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|_| ())?;
    #[cfg(target_os = "linux")]
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .map_err(|_| ())?;
    Ok(())
}
#[cfg(any(not(unix), target_os = "haiku"))]
fn harden_process() -> Result<(), ()> {
    Err(())
}

struct BundleWriterArgs {
    wallet_id: String,
    authority: String,
    protocol_id: String,
    operation_schema: String,
    public_action_path: PathBuf,
    signer_seed_path: PathBuf,
    protocol_witness_path: PathBuf,
    output_path: PathBuf,
}

fn take_required_text(value: Option<OsString>) -> Result<String, &'static str> {
    value
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .ok_or("bundle writer arguments are invalid")
}

fn take_required_path(value: Option<OsString>) -> Result<PathBuf, &'static str> {
    let path = PathBuf::from(value.ok_or("bundle writer arguments are invalid")?);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err("bundle writer paths must be absolute and normalized");
    }
    Ok(path)
}

fn parse_bundle_writer_args(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<BundleWriterArgs, &'static str> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("write-bundle-v1")) {
        return Err("bundle writer subcommand is invalid");
    }
    let mut wallet_id = None;
    let mut authority = None;
    let mut protocol_id = None;
    let mut operation_schema = None;
    let mut public_action_path = None;
    let mut signer_seed_path = None;
    let mut protocol_witness_path = None;
    let mut output_path = None;
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or("bundle writer arguments are invalid")?;
        let slot = match flag.to_str() {
            Some("--wallet-id") => &mut wallet_id,
            Some("--authority") => &mut authority,
            Some("--protocol-id") => &mut protocol_id,
            Some("--operation-schema") => &mut operation_schema,
            Some("--public-action") => &mut public_action_path,
            Some("--signer-seed") => &mut signer_seed_path,
            Some("--protocol-witness") => &mut protocol_witness_path,
            Some("--output") => &mut output_path,
            _ => return Err("bundle writer arguments are invalid"),
        };
        if slot.replace(value).is_some() {
            return Err("bundle writer arguments are invalid");
        }
    }
    Ok(BundleWriterArgs {
        wallet_id: take_required_text(wallet_id)?,
        authority: take_required_text(authority)?,
        protocol_id: take_required_text(protocol_id)?,
        operation_schema: take_required_text(operation_schema)?,
        public_action_path: take_required_path(public_action_path)?,
        signer_seed_path: take_required_path(signer_seed_path)?,
        protocol_witness_path: take_required_path(protocol_witness_path)?,
        output_path: take_required_path(output_path)?,
    })
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(not(unix))]
fn same_file_snapshot(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn validate_input_metadata(metadata: &fs::Metadata, secret: bool) -> Result<(), &'static str> {
    use std::os::unix::fs::MetadataExt as _;
    let forbidden_mode = if secret { 0o077 } else { 0o022 };
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & forbidden_mode != 0
    {
        return Err("bundle writer input failed owner or permission validation");
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_input_metadata(_metadata: &fs::Metadata, _secret: bool) -> Result<(), &'static str> {
    Err("bundle writer is unavailable on this platform")
}

fn read_stable_input(
    path: &Path,
    maximum_bytes: u64,
    secret: bool,
) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let before = fs::symlink_metadata(path).map_err(|_| "bundle writer input is unavailable")?;
    if before.file_type().is_symlink()
        || !before.is_file()
        || before.len() == 0
        || before.len() > maximum_bytes
    {
        return Err("bundle writer input failed type or size validation");
    }
    validate_input_metadata(&before, secret)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .truncate(false)
        .open(path)
        .map_err(|_| "bundle writer input could not be opened")?;
    let opened = file
        .metadata()
        .map_err(|_| "bundle writer input metadata is unavailable")?;
    if !same_file_snapshot(&before, &opened) {
        return Err("bundle writer input changed before opening");
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(
        usize::try_from(before.len()).map_err(|_| "bundle writer input is too large")?,
    ));
    Read::by_ref(&mut file)
        .take(maximum_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "bundle writer input could not be read")?;
    let after = file
        .metadata()
        .map_err(|_| "bundle writer input metadata is unavailable")?;
    if bytes.is_empty()
        || bytes.len() as u64 > maximum_bytes
        || bytes.len() as u64 != after.len()
        || !same_file_snapshot(&opened, &after)
    {
        return Err("bundle writer input changed while reading");
    }
    Ok(bytes)
}

#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "redox"
))]
fn persist_owner_bundle(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or("bundle output parent is invalid")?;
    let parent_metadata =
        fs::symlink_metadata(parent).map_err(|_| "bundle output parent is unavailable")?;
    if parent_metadata.file_type().is_symlink()
        || !parent_metadata.is_dir()
        || parent_metadata.uid() != rustix::process::geteuid().as_raw()
        || parent_metadata.mode() & 0o077 != 0
        || fs::canonicalize(parent).map_err(|_| "bundle output parent is unavailable")? != parent
    {
        return Err("bundle output parent failed owner-only validation");
    }
    let mut random_suffix = Zeroizing::new([0_u8; 16]);
    OsRng.fill_bytes(&mut random_suffix[..]);
    let temporary_path = parent.join(format!(
        ".iroha-privacy-wallet-bundle-v1.{}.tmp",
        hex::encode(&random_suffix[..])
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .truncate(false)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|_| "bundle output temporary could not be created")?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| "bundle output permissions could not be fixed")?;
    let opened = file
        .metadata()
        .map_err(|_| "bundle output metadata is unavailable")?;
    if !opened.is_file()
        || opened.uid() != rustix::process::geteuid().as_raw()
        || opened.mode() & 0o777 != 0o600
        || opened.nlink() != 1
        || opened.len() != 0
    {
        cleanup_temporary_bundle(&temporary_path, &opened)?;
        return Err("bundle output temporary failed owner-only validation");
    }
    let staged = (|| -> Result<(), &'static str> {
        file.write_all(bytes)
            .map_err(|_| "bundle output temporary could not be written")?;
        file.sync_all()
            .map_err(|_| "bundle output temporary could not be synchronized")?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| "bundle output temporary could not be verified")?;
        let mut readback = Zeroizing::new(Vec::with_capacity(bytes.len()));
        Read::by_ref(&mut file)
            .take((bytes.len() as u64).saturating_add(1))
            .read_to_end(&mut readback)
            .map_err(|_| "bundle output temporary could not be verified")?;
        if readback.as_slice() != bytes {
            return Err("bundle output temporary readback differs");
        }
        Ok(())
    })();
    if let Err(error) = staged {
        drop(file);
        cleanup_temporary_bundle(&temporary_path, &opened)?;
        return Err(error);
    }
    let written = file
        .metadata()
        .map_err(|_| "bundle output metadata is unavailable")?;
    if !same_file_snapshot_except_length(&opened, &written)
        || written.len() != bytes.len() as u64
        || written.mode() & 0o777 != 0o600
        || written.nlink() != 1
    {
        drop(file);
        cleanup_temporary_bundle(&temporary_path, &written)?;
        return Err("bundle output temporary changed while writing");
    }
    if rustix::fs::renameat_with(
        rustix::fs::CWD,
        &temporary_path,
        rustix::fs::CWD,
        path,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .is_err()
    {
        drop(file);
        cleanup_temporary_bundle(&temporary_path, &written)?;
        return Err("bundle output could not be atomically created without replacement");
    }
    let published = fs::symlink_metadata(path)
        .map_err(|_| "published bundle output metadata is unavailable")?;
    if published.file_type().is_symlink()
        || !published.is_file()
        || !same_file_snapshot_except_length(&written, &published)
        || published.len() != bytes.len() as u64
        || published.mode() & 0o777 != 0o600
        || published.nlink() != 1
    {
        return Err("published bundle output changed identity or custody");
    }
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "bundle output directory could not be synchronized")?;
    Ok(())
}

#[cfg(unix)]
fn same_file_snapshot_except_length(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
}

#[cfg(unix)]
fn cleanup_temporary_bundle(path: &Path, expected: &fs::Metadata) -> Result<(), &'static str> {
    let observed = fs::symlink_metadata(path)
        .map_err(|_| "bundle output temporary cleanup identity is unavailable")?;
    if observed.file_type().is_symlink()
        || !observed.is_file()
        || !same_file_snapshot_except_length(expected, &observed)
    {
        return Err("bundle output temporary cleanup identity changed");
    }
    fs::remove_file(path).map_err(|_| "bundle output temporary could not be removed")
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "redox"
)))]
fn persist_owner_bundle(_path: &Path, _bytes: &[u8]) -> Result<(), &'static str> {
    Err("bundle writer is unavailable on this platform")
}

fn write_bundle(arguments: impl IntoIterator<Item = OsString>) -> Result<(), &'static str> {
    let arguments = parse_bundle_writer_args(arguments)?;
    let public_action = read_stable_input(
        &arguments.public_action_path,
        MAX_PUBLIC_ACTION_BYTES,
        false,
    )?;
    let mut signer_seed_bytes = read_stable_input(&arguments.signer_seed_path, 32, true)?;
    if signer_seed_bytes.len() != 32 {
        return Err("bundle writer signer seed must contain exactly 32 raw bytes");
    }
    let mut signer_seed = Zeroizing::new([0_u8; 32]);
    signer_seed.copy_from_slice(&signer_seed_bytes);
    signer_seed_bytes.zeroize();
    let protocol_witness = read_stable_input(
        &arguments.protocol_witness_path,
        MAX_PROTOCOL_WITNESS_BYTES,
        true,
    )?;
    let bundle = encode_privacy_wallet_execution_bundle_v1(
        &arguments.wallet_id,
        &arguments.authority,
        &arguments.protocol_id,
        &arguments.operation_schema,
        &public_action,
        &signer_seed,
        &protocol_witness,
    )
    .map_err(|_| "bundle writer rejected the native action material")?;
    persist_owner_bundle(&arguments.output_path, &bundle)?;
    println!("privacy wallet execution bundle v1 written");
    Ok(())
}

fn run_worker() -> Result<(), i32> {
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = BufWriter::new(io::stdout().lock());
    let mut auth_key = Zeroizing::new([0_u8; 32]);
    if input.read_exact(&mut auth_key[..]).is_err() || auth_key.iter().all(|byte| *byte == 0) {
        eprintln!("privacy wallet worker startup failed: missing authentication key");
        return Err(64);
    }
    if let Err(error) = run_pipe_session(&mut input, &mut output, auth_key) {
        eprintln!("privacy wallet worker terminated: {}", error.message());
        return Err(70);
    }
    Ok(())
}

fn main() {
    if harden_process().is_err() {
        eprintln!("privacy wallet worker startup hardening failed");
        std::process::exit(63);
    }
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if arguments.is_empty() {
        if let Err(code) = run_worker() {
            std::process::exit(code);
        }
    } else if let Err(message) = write_bundle(arguments) {
        eprintln!("{message}");
        std::process::exit(64);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, PrivateKey, PublicKey};
    use iroha_data_model::prelude::AccountId;
    use iroha_python_rs::privacy_wallet_bundle::inspect_privacy_wallet_execution_bundle_v1;
    use std::os::unix::fs::PermissionsExt as _;
    use tempfile::TempDir;

    const PUBLIC_ACTION: &[u8] = br#"{"evaluation_point_hex":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
    const WITNESS: &[u8] = br#"{"polynomials_hex":[["0000000000000000000000000000000000000000000000000000000000000000"],["0000000000000000000000000000000000000000000000000000000000000001"],["0000000000000000000000000000000000000000000000000000000000000002"],["0000000000000000000000000000000000000000000000000000000000000003"]]}"#;

    fn private_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).expect("write private input");
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("secure private input");
    }

    fn arguments(root: &Path, authority: &str) -> Vec<OsString> {
        [
            OsString::from("write-bundle-v1"),
            OsString::from("--wallet-id"),
            OsString::from("wallet-retail-adult-001"),
            OsString::from("--authority"),
            OsString::from(authority),
            OsString::from("--protocol-id"),
            OsString::from("iroha-jindo-polynomial-commitment-v0"),
            OsString::from("--operation-schema"),
            OsString::from("jindo_polynomial_evaluation_v1"),
            OsString::from("--public-action"),
            root.join("public-action.json").into_os_string(),
            OsString::from("--signer-seed"),
            root.join("signer-seed.bin").into_os_string(),
            OsString::from("--protocol-witness"),
            root.join("protocol-witness.json").into_os_string(),
            OsString::from("--output"),
            root.join("execution.ipwb").into_os_string(),
        ]
        .into()
    }

    fn fixture() -> (TempDir, PathBuf, String) {
        let directory = TempDir::new().expect("temporary directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("owner-only directory");
        let root = fs::canonicalize(directory.path()).expect("canonical temporary directory");
        let seed = [7_u8; 32];
        private_file(&root.join("public-action.json"), PUBLIC_ACTION);
        private_file(&root.join("signer-seed.bin"), &seed);
        private_file(&root.join("protocol-witness.json"), WITNESS);
        let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("private key");
        let authority = AccountId::new(PublicKey::from(private_key)).to_string();
        (directory, root, authority)
    }

    #[test]
    fn writer_creates_one_valid_owner_only_bundle_without_replacement() {
        let (_directory, root, authority) = fixture();
        let arguments = arguments(&root, &authority);
        write_bundle(arguments.clone()).expect("write bundle");
        let output = root.join("execution.ipwb");
        let metadata = fs::metadata(&output).expect("bundle metadata");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        let bytes = fs::read(&output).expect("read test bundle");
        let inspected =
            inspect_privacy_wallet_execution_bundle_v1(&bytes).expect("inspect written bundle");
        assert_eq!(inspected.manifest.wallet_id, "wallet-retail-adult-001");
        assert_eq!(inspected.manifest.authority.to_string(), authority);
        assert!(
            write_bundle(arguments).is_err(),
            "must never replace a bundle"
        );
        assert_eq!(fs::read(output).expect("preserved bundle"), bytes);
    }

    #[test]
    fn writer_rejects_non_owner_only_secret_inputs_and_duplicate_flags() {
        let (_directory, root, authority) = fixture();
        fs::set_permissions(
            root.join("protocol-witness.json"),
            fs::Permissions::from_mode(0o640),
        )
        .expect("relax witness mode");
        assert!(write_bundle(arguments(&root, &authority)).is_err());

        let mut duplicate = arguments(&root, &authority);
        duplicate.extend([OsString::from("--wallet-id"), OsString::from("other")]);
        assert!(parse_bundle_writer_args(duplicate).is_err());
    }
}

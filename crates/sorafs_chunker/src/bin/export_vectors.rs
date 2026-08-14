use blake3::Hash;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature};
use norito::json::{self, Map, Value};
use sorafs_chunker::{
    ChunkProfile, Chunker,
    fixtures::{FixtureProfile, FixtureVectors, to_hex},
};
use std::{
    collections::BTreeSet,
    env, fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnerMode {
    Check,
    Write,
}
#[derive(Default)]
struct CliOptions {
    mode: Option<OwnerMode>,
    staging_root: Option<PathBuf>,
    signing_key_hex: Option<String>,
    signing_key_file: Option<PathBuf>,
    signer_hex: Option<String>,
    // Internal override used by focused signature tests and the staged owner.
    signature_out: Option<PathBuf>,
}
enum CliError {
    Help,
    Message(String),
}
const USAGE: &str = "\
Usage: export_vectors (--check|--write) --staging-root <path> [OPTIONS]

Options:
    --check                     Generate and compare without publishing
    --write                     Generate, validate, and atomically publish changed files
    --staging-root <path>       Existing absolute empty private directory outside the repository
    --signing-key <hex>         Ed25519 private key (32- or 64-byte hex) for signing the manifest
    --signing-key-file <path>   Regular private file containing the Ed25519 signing key
    --signer <hex>              Expected public key (32-byte hex). Defaults to the key derived from signing authority
    -h, --help                  Show this help message
";
const GENERATED_PATHS: [&str; 8] = [
    "fixtures/sorafs_chunker/sf1_profile_v1.json",
    "fixtures/sorafs_chunker/sf1_profile_v1.rs",
    "fixtures/sorafs_chunker/sf1_profile_v1.ts",
    "fixtures/sorafs_chunker/sf1_profile_v1.go",
    "fixtures/sorafs_chunker/manifest_blake3.json",
    "fixtures/sorafs_chunker/manifest_signatures.json",
    "fuzz/sorafs_chunker/sf1_profile_v1_input.bin",
    "fuzz/sorafs_chunker/sf1_profile_v1_backpressure.json",
];
const MANIFEST_PROFILE_FILES: [&str; 4] = [
    "sf1_profile_v1.json",
    "sf1_profile_v1.rs",
    "sf1_profile_v1.ts",
    "sf1_profile_v1.go",
];
// The largest SF1 owner output is the fixed 1 MiB fuzz input. Keep reads
// bounded so a replaced path cannot force an unbounded allocation.
const MAX_GENERATED_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SIGNING_KEY_FILE_BYTES: u64 = 130;
const CANONICAL_PROFILE_HANDLE: &str = "sorafs.sf1@1.0.0";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(CliError::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(CliError::Message(err)) => {
            eprintln!("error: {err}");
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };
    let repo_root = repo_root()?;
    let staging_root = bind_staging_root(
        cli.staging_root
            .as_deref()
            .ok_or("--staging-root is required")?,
        &repo_root,
    )?;
    let output_dir = staging_root.join("fixtures").join("sorafs_chunker");
    fs::create_dir_all(&output_dir)?;
    let vectors = FixtureProfile::SF1_V1.generate_vectors();
    write_json(&output_dir, &vectors)?;
    write_rust(&output_dir, &vectors)?;
    write_typescript(&output_dir, &vectors)?;
    write_go(&output_dir, &vectors)?;
    let manifest_digest = write_manifest(&output_dir, &vectors)?;
    let fuzz_digests = write_fuzz_corpora(&staging_root, &vectors)?;
    let live_manifest = repo_root.join("fixtures/sorafs_chunker/manifest_blake3.json");
    let live_signature = repo_root.join("fixtures/sorafs_chunker/manifest_signatures.json");
    let manifest_changed = blake3::hash(&read_regular_file(&live_manifest)?) != manifest_digest;
    let mut staged_cli = cli;
    staged_cli.signature_out = Some(output_dir.join("manifest_signatures.json"));
    if manifest_changed {
        if staged_cli.signing_key_hex.is_none() {
            return Err(
                "manifest digest changed; explicit signing-key authority is required".into(),
            );
        }
    } else {
        fs::write(
            output_dir.join("manifest_signatures.json"),
            read_regular_file(&live_signature)?,
        )?;
    }
    write_manifest_signatures(&output_dir, &vectors, manifest_digest, &staged_cli)?;
    validate_staged_tree(&staging_root)?;
    validate_staged_authority(&staging_root, &vectors, &manifest_digest, &fuzz_digests)?;
    match staged_cli
        .mode
        .ok_or("one of --check or --write is required")?
    {
        OwnerMode::Check => check_staged_tree(&staging_root, &repo_root)?,
        OwnerMode::Write => publish_staged_tree(&staging_root, &repo_root)?,
    }
    println!(
        "{} SF1 fixtures from validated stage {}",
        if staged_cli.mode == Some(OwnerMode::Write) {
            "Published"
        } else {
            "Verified"
        },
        staging_root.display()
    );
    Ok(())
}
fn parse_cli() -> Result<CliOptions, CliError> {
    parse_cli_from(env::args().skip(1))
}
fn parse_cli_from<I>(args: I) -> Result<CliOptions, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut options = CliOptions::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(CliError::Help),
            "--check" => set_mode(&mut options.mode, OwnerMode::Check, "--check")?,
            "--write" => set_mode(&mut options.mode, OwnerMode::Write, "--write")?,
            value if value.starts_with("--staging-root=") => {
                return Err(CliError::Message(
                    "--staging-root requires a separate path argument".to_owned(),
                ));
            }
            "--staging-root" => {
                let raw = take_value(&mut args, "--staging-root")?;
                set_option(
                    &mut options.staging_root,
                    PathBuf::from(raw),
                    "--staging-root",
                )?;
            }
            value if value.starts_with("--signing-key=") => {
                let cleaned = clean_hex(&value["--signing-key=".len()..], "--signing-key")?;
                validate_signing_key_hex(&cleaned)?;
                set_option(&mut options.signing_key_hex, cleaned, "--signing-key")?;
            }
            "--signing-key" => {
                let raw = take_value(&mut args, "--signing-key")?;
                let cleaned = clean_hex(&raw, "--signing-key")?;
                validate_signing_key_hex(&cleaned)?;
                set_option(&mut options.signing_key_hex, cleaned, "--signing-key")?;
            }
            value if value.starts_with("--signing-key-file=") => {
                return Err(CliError::Message(
                    "--signing-key-file requires a separate path argument".to_owned(),
                ));
            }
            "--signing-key-file" => {
                let raw = take_value(&mut args, "--signing-key-file")?;
                set_option(
                    &mut options.signing_key_file,
                    PathBuf::from(raw),
                    "--signing-key-file",
                )?;
            }
            value if value.starts_with("--signer=") => {
                let cleaned = clean_hex(&value["--signer=".len()..], "--signer")?;
                validate_signer_len(&cleaned)?;
                set_option(&mut options.signer_hex, cleaned, "--signer")?;
            }
            "--signer" => {
                let raw = take_value(&mut args, "--signer")?;
                let cleaned = clean_hex(&raw, "--signer")?;
                validate_signer_len(&cleaned)?;
                set_option(&mut options.signer_hex, cleaned, "--signer")?;
            }
            other => return Err(CliError::Message(format!("unknown argument {other}"))),
        }
    }
    if options.mode.is_none() {
        return Err(CliError::Message(
            "one of --check or --write is required".to_owned(),
        ));
    }
    if options.staging_root.is_none() {
        return Err(CliError::Message("--staging-root is required".to_owned()));
    }
    if options.signing_key_hex.is_some() && options.signing_key_file.is_some() {
        return Err(CliError::Message(
            "--signing-key and --signing-key-file are mutually exclusive".to_owned(),
        ));
    }
    if let Some(path) = options.signing_key_file.take() {
        options.signing_key_hex = Some(read_signing_key_file(&path)?);
    }
    if options.signer_hex.is_some() && options.signing_key_hex.is_none() {
        return Err(CliError::Message(
            "--signer requires explicit signing-key authority".to_owned(),
        ));
    }
    Ok(options)
}
fn set_mode(slot: &mut Option<OwnerMode>, mode: OwnerMode, flag: &str) -> Result<(), CliError> {
    if slot.is_some() {
        return Err(CliError::Message(format!(
            "generation mode specified multiple times (at {flag})"
        )));
    }
    *slot = Some(mode);
    Ok(())
}
fn take_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, CliError> {
    args.next()
        .ok_or_else(|| CliError::Message(format!("{flag} requires a value")))
}
fn set_option<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), CliError> {
    if slot.is_some() {
        return Err(CliError::Message(format!(
            "{flag} specified multiple times"
        )));
    }
    *slot = Some(value);
    Ok(())
}
fn clean_hex(value: &str, flag: &str) -> Result<String, CliError> {
    if value.is_empty() {
        return Err(CliError::Message(format!(
            "{flag} requires a non-empty hex value"
        )));
    }
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        return Err(CliError::Message(format!(
            "{flag} must not contain ASCII whitespace"
        )));
    }
    if value.starts_with("0x") || value.starts_with("0X") {
        return Err(CliError::Message(format!(
            "{flag} must be raw lowercase hex without a 0x prefix"
        )));
    }
    if !value.len().is_multiple_of(2) {
        return Err(CliError::Message(format!(
            "{flag} value must contain an even number of hex digits"
        )));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CliError::Message(format!(
            "{flag} value must be lowercase hex-encoded"
        )));
    }
    Ok(value.to_owned())
}
fn validate_signing_key_hex(cleaned: &str) -> Result<(), CliError> {
    if cleaned.len() != 64 && cleaned.len() != 128 {
        return Err(CliError::Message(
            "--signing-key must be a 32- or 64-byte hex string (64 or 128 hex characters)"
                .to_owned(),
        ));
    }
    if cleaned.as_bytes().iter().all(|byte| *byte == b'0') {
        return Err(CliError::Message(
            "--signing-key material must not be all zero".to_owned(),
        ));
    }
    Ok(())
}
fn validate_signer_len(cleaned: &str) -> Result<(), CliError> {
    if cleaned.len() != 64 {
        return Err(CliError::Message(
            "--signer must be a 32-byte hex string (64 hex characters)".to_owned(),
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}
fn read_signing_key_file(path: &Path) -> Result<String, CliError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CliError::Message(
            "--signing-key-file must be an absolute normalized path".to_owned(),
        ));
    }
    for ancestor in path.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(ancestor).map_err(|error| {
            CliError::Message(format!(
                "failed to inspect --signing-key-file ancestry: {error}"
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CliError::Message(
                "--signing-key-file ancestry must contain only real directories".to_owned(),
            ));
        }
    }
    let before = fs::symlink_metadata(path).map_err(|error| {
        CliError::Message(format!("failed to inspect --signing-key-file: {error}"))
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(CliError::Message(
            "--signing-key-file must be a regular non-symbolic file".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if before.nlink() != 1 {
            return Err(CliError::Message(
                "--signing-key-file must have exactly one hard link".to_owned(),
            ));
        }
        if before.permissions().mode() & 0o077 != 0 {
            return Err(CliError::Message(
                "--signing-key-file must not grant group or other permissions".to_owned(),
            ));
        }
    }
    if before.len() > MAX_SIGNING_KEY_FILE_BYTES {
        return Err(CliError::Message(
            "--signing-key-file exceeds the bounded key length".to_owned(),
        ));
    }
    let mut file = File::open(path).map_err(|error| {
        CliError::Message(format!("failed to open --signing-key-file: {error}"))
    })?;
    let opened = file.metadata().map_err(|error| {
        CliError::Message(format!(
            "failed to inspect open --signing-key-file: {error}"
        ))
    })?;
    if !opened.is_file() || !same_file_identity(&before, &opened) {
        return Err(CliError::Message(
            "--signing-key-file changed before it was opened".to_owned(),
        ));
    }
    let mut raw = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(MAX_SIGNING_KEY_FILE_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|error| {
            CliError::Message(format!("failed to read --signing-key-file: {error}"))
        })?;
    if raw.len() as u64 > MAX_SIGNING_KEY_FILE_BYTES {
        return Err(CliError::Message(
            "--signing-key-file exceeds the bounded key length".to_owned(),
        ));
    }
    let opened_after = file.metadata().map_err(|error| {
        CliError::Message(format!(
            "failed to re-inspect open --signing-key-file: {error}"
        ))
    })?;
    let after = fs::symlink_metadata(path).map_err(|error| {
        CliError::Message(format!("failed to re-inspect --signing-key-file: {error}"))
    })?;
    if !same_file_identity(&before, &opened_after)
        || !same_file_identity(&before, &after)
        || before.len() != raw.len() as u64
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || after.file_type().is_symlink()
        || !after.is_file()
    {
        return Err(CliError::Message(
            "--signing-key-file changed while it was read".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if opened_after.nlink() != 1
            || after.nlink() != 1
            || opened_after.permissions().mode() & 0o077 != 0
            || after.permissions().mode() & 0o077 != 0
        {
            return Err(CliError::Message(
                "--signing-key-file authority changed while it was read".to_owned(),
            ));
        }
    }
    let raw = String::from_utf8(raw)
        .map_err(|_| CliError::Message("--signing-key-file must contain UTF-8 hex".to_owned()))?;
    let cleaned = raw
        .strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .unwrap_or(&raw);
    if cleaned.ends_with('\r') || cleaned.ends_with('\n') {
        return Err(CliError::Message(
            "--signing-key-file must contain one key line".to_owned(),
        ));
    }
    let cleaned = clean_hex(cleaned, "--signing-key-file")?;
    validate_signing_key_hex(&cleaned)?;
    Ok(cleaned)
}
fn repo_root() -> Result<PathBuf, std::io::Error> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("..").join("..").canonicalize()
}
fn bind_staging_root(raw: &Path, repo_root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !raw.is_absolute()
        || raw
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err("--staging-root must be an absolute normalized path".into());
    }
    for ancestor in raw.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "--staging-root ancestry must not contain symbolic links: {}",
                ancestor.display()
            )
            .into());
        }
    }
    let metadata = fs::symlink_metadata(raw)?;
    if !metadata.is_dir() {
        return Err("--staging-root must be an existing directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o700 {
            return Err("--staging-root must have exact mode 0700".into());
        }
    }
    let staging_root = raw.canonicalize()?;
    if staging_root.starts_with(repo_root) {
        return Err("--staging-root must be outside the repository".into());
    }
    if fs::read_dir(&staging_root)?.next().is_some() {
        return Err("--staging-root must be empty".into());
    }
    Ok(staging_root)
}
fn collect_staged_files(
    root: &Path,
    current: &Path,
    files: &mut BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "staged output must not be a symbolic link: {}",
                path.display()
            )
            .into());
        }
        if metadata.is_dir() {
            collect_staged_files(root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(format!("staged output must be a regular file: {}", path.display()).into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Err(format!(
                    "staged output must have exactly one hard link: {}",
                    path.display()
                )
                .into());
            }
        }
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .ok_or("staged output path is not valid UTF-8")?
            .replace('\\', "/");
        files.insert(relative);
    }
    Ok(())
}
fn validate_staged_tree(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut actual = BTreeSet::new();
    collect_staged_files(root, root, &mut actual)?;
    let expected = GENERATED_PATHS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).cloned().collect::<Vec<_>>();
        return Err(format!(
            "staged SF1 output inventory mismatch; missing={missing:?}, unexpected={unexpected:?}"
        )
        .into());
    }
    Ok(())
}
fn validate_staged_authority(
    root: &Path,
    vectors: &FixtureVectors,
    expected_manifest_digest: &Hash,
    expected_fuzz_digests: &[Hash; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_dir = root.join("fixtures/sorafs_chunker");
    let manifest_path = fixture_dir.join("manifest_blake3.json");
    let manifest_bytes = read_regular_file(&manifest_path)?;
    let actual_manifest_digest = blake3::hash(&manifest_bytes);
    if actual_manifest_digest != *expected_manifest_digest {
        return Err("staged manifest changed after generation".into());
    }
    let manifest: Value = json::from_slice(&manifest_bytes)
        .map_err(|error| format!("failed to parse staged manifest: {error}"))?;
    let manifest = manifest
        .as_object()
        .ok_or("staged manifest must contain a JSON object")?;
    if manifest.get("profile").and_then(Value::as_str) != Some(CANONICAL_PROFILE_HANDLE) {
        return Err("staged manifest has the wrong canonical profile".into());
    }
    let aliases = manifest
        .get("profile_aliases")
        .and_then(Value::as_array)
        .ok_or("staged manifest is missing profile_aliases")?;
    if aliases.len() != 1
        || aliases.first().and_then(Value::as_str) != Some(CANONICAL_PROFILE_HANDLE)
    {
        return Err("staged manifest has non-canonical profile aliases".into());
    }
    if manifest
        .get("chunk_digest_sha3_256")
        .and_then(Value::as_str)
        != Some(vectors.sha3_digest_hex().as_str())
    {
        return Err("staged manifest has the wrong chunk-plan digest".into());
    }
    let entries = manifest
        .get("files")
        .and_then(Value::as_array)
        .ok_or("staged manifest is missing its files array")?;
    let expected_files = MANIFEST_PROFILE_FILES.into_iter().collect::<BTreeSet<_>>();
    let mut actual_files = BTreeSet::new();
    for entry in entries {
        let entry = entry
            .as_object()
            .ok_or("staged manifest file row must be an object")?;
        let name = entry
            .get("file")
            .and_then(Value::as_str)
            .ok_or("staged manifest file row is missing file")?;
        if !expected_files.contains(name) {
            return Err(format!("staged manifest contains unexpected file {name}").into());
        }
        if !actual_files.insert(name) {
            return Err(format!("staged manifest contains duplicate file {name}").into());
        }
        let declared_size = entry
            .get("size")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("staged manifest file {name} is missing size"))?;
        let declared_digest = entry
            .get("blake3")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("staged manifest file {name} is missing blake3"))?;
        decode_canonical_hex_exact(declared_digest, "manifest file blake3", 32)?;
        let generated = read_regular_file(&fixture_dir.join(name))?;
        if generated.len() as u64 != declared_size {
            return Err(format!("staged manifest size mismatch for {name}").into());
        }
        if to_hex(blake3::hash(&generated).as_bytes()) != declared_digest {
            return Err(format!("staged manifest BLAKE3 mismatch for {name}").into());
        }
    }
    if actual_files != expected_files {
        let missing = expected_files
            .difference(&actual_files)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!("staged manifest is missing files: {missing:?}").into());
    }
    let signature_path = fixture_dir.join("manifest_signatures.json");
    let signatures =
        load_existing_manifest_signatures(&signature_path, vectors, expected_manifest_digest)?
            .ok_or("staged manifest signatures are missing")?;
    ensure_signed(&signatures, expected_manifest_digest)?;
    for (relative, expected) in [
        (
            "fuzz/sorafs_chunker/sf1_profile_v1_input.bin",
            expected_fuzz_digests[0],
        ),
        (
            "fuzz/sorafs_chunker/sf1_profile_v1_backpressure.json",
            expected_fuzz_digests[1],
        ),
    ] {
        let actual = blake3::hash(&read_regular_file(&root.join(relative))?);
        if actual != expected {
            return Err(format!("staged fuzz output changed after generation: {relative}").into());
        }
    }
    Ok(())
}
fn read_regular_file(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!(
            "generated destination must be a regular file: {}",
            path.display()
        )
        .into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.nlink() != 1 {
            return Err(format!(
                "generated destination must have exactly one hard link: {}",
                path.display()
            )
            .into());
        }
    }
    if before.len() > MAX_GENERATED_FILE_BYTES {
        return Err(format!(
            "generated file exceeds the {MAX_GENERATED_FILE_BYTES}-byte bound: {}",
            path.display()
        )
        .into());
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || !same_file_identity(&before, &opened) {
        return Err(format!(
            "generated destination changed before open: {}",
            path.display()
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(MAX_GENERATED_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_GENERATED_FILE_BYTES {
        return Err(format!(
            "generated file exceeds the {MAX_GENERATED_FILE_BYTES}-byte bound: {}",
            path.display()
        )
        .into());
    }
    let opened_after = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    if !same_file_identity(&before, &opened_after)
        || !same_file_identity(&before, &after)
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || after.file_type().is_symlink()
        || !after.is_file()
        || bytes.len() as u64 != after.len()
    {
        return Err(format!(
            "generated destination changed while read: {}",
            path.display()
        )
        .into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened_after.nlink() != 1 || after.nlink() != 1 {
            return Err(format!(
                "generated destination hard-link authority changed while read: {}",
                path.display()
            )
            .into());
        }
    }
    Ok(bytes)
}
fn check_staged_tree(stage: &Path, repo_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut stale = Vec::new();
    for relative in GENERATED_PATHS {
        let staged = read_regular_file(&stage.join(relative))?;
        let live = read_regular_file(&repo_root.join(relative))?;
        if staged != live {
            stale.push(relative);
        }
    }
    if !stale.is_empty() {
        return Err(format!("checked-in SF1 fixtures are stale: {}", stale.join(", ")).into());
    }
    Ok(())
}
struct PreparedPublication {
    target: PathBuf,
    replacement: PathBuf,
    backup: PathBuf,
    original: Vec<u8>,
}
fn create_private_sibling(
    target: &Path,
    label: &str,
    bytes: &[u8],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let parent = target
        .parent()
        .ok_or("generated destination has no parent")?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("generated destination name is not valid UTF-8")?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    for attempt in 0..64 {
        let path = parent.join(format!(
            ".{name}.sf1-owner.{}.{nonce}.{attempt}.{label}",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                let prepare = (|| -> Result<(), std::io::Error> {
                    file.write_all(bytes)?;
                    file.set_permissions(fs::metadata(target)?.permissions())?;
                    file.sync_all()
                })();
                drop(file);
                if let Err(error) = prepare {
                    let cleanup = fs::remove_file(&path);
                    return match cleanup {
                        Ok(()) => Err(error.into()),
                        Err(cleanup_error) => Err(format!(
                            "failed to prepare {}: {error}; failed to remove incomplete private publication file: {cleanup_error}",
                            path.display()
                        )
                        .into()),
                    };
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(format!(
        "failed to allocate private publication file beside {}",
        target.display()
    )
    .into())
}
fn cleanup_publications(publications: &[PreparedPublication], preserve_backups: bool) {
    for publication in publications {
        let mut paths = vec![&publication.replacement];
        if !preserve_backups {
            paths.push(&publication.backup);
        }
        for path in paths {
            if let Err(error) = fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!(
                    "warning: failed to remove private publication file {}: {error}",
                    path.display()
                );
            }
        }
    }
}
fn rollback_publications(
    publications: &[PreparedPublication],
    committed: usize,
    fail_at: Option<usize>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, publication) in publications[..committed].iter().enumerate().rev() {
        if fail_at == Some(index) {
            errors.push(format!(
                "{}: injected SF1 rollback failure; original retained at {}",
                publication.target.display(),
                publication.backup.display()
            ));
            continue;
        }
        if let Err(error) = fs::rename(&publication.backup, &publication.target) {
            errors.push(format!("{}: {error}", publication.target.display()));
        }
    }
    errors
}
fn sync_publication_directories(
    publications: &[PreparedPublication],
) -> Result<(), Box<dyn std::error::Error>> {
    let parents = publications
        .iter()
        .filter_map(|publication| publication.target.parent().map(Path::to_path_buf))
        .collect::<BTreeSet<_>>();
    for parent in parents {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}
fn commit_publications(
    publications: &[PreparedPublication],
    fail_after: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut committed = 0;
    for publication in publications {
        let publish = (|| -> Result<(), Box<dyn std::error::Error>> {
            let current = read_regular_file(&publication.target)?;
            if current != publication.original {
                return Err(format!(
                    "generated destination changed immediately before publication: {}",
                    publication.target.display()
                )
                .into());
            }
            if fail_after == Some(committed) {
                return Err(std::io::Error::other("injected SF1 publication failure").into());
            }
            fs::rename(&publication.replacement, &publication.target)?;
            Ok(())
        })();
        if let Err(error) = publish {
            let rollback_errors = rollback_publications(publications, committed, None);
            let rollback_sync_error = sync_publication_directories(publications)
                .err()
                .map(|sync_error| sync_error.to_string());
            cleanup_publications(publications, !rollback_errors.is_empty());
            return Err(format!(
                "atomic SF1 publication failed at {}: {error}; rollback_errors={rollback_errors:?}; rollback_sync_error={rollback_sync_error:?}",
                publication.target.display()
            )
            .into());
        }
        committed += 1;
    }
    if let Err(error) = sync_publication_directories(publications) {
        let rollback_errors = rollback_publications(publications, committed, None);
        let rollback_sync_error = sync_publication_directories(publications)
            .err()
            .map(|sync_error| sync_error.to_string());
        cleanup_publications(publications, !rollback_errors.is_empty());
        return Err(format!(
            "failed to sync atomic SF1 publication: {error}; rollback_errors={rollback_errors:?}; rollback_sync_error={rollback_sync_error:?}"
        )
        .into());
    }
    cleanup_publications(publications, false);
    Ok(())
}
fn publish_staged_tree(stage: &Path, repo_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut publications = Vec::new();
    for relative in GENERATED_PATHS {
        let target = repo_root.join(relative);
        let original = read_regular_file(&target)?;
        let generated = read_regular_file(&stage.join(relative))?;
        if original == generated {
            continue;
        }
        let replacement = match create_private_sibling(&target, "new", &generated) {
            Ok(path) => path,
            Err(error) => {
                cleanup_publications(&publications, false);
                return Err(error);
            }
        };
        let backup = match create_private_sibling(&target, "backup", &original) {
            Ok(path) => path,
            Err(error) => {
                let _ = fs::remove_file(&replacement);
                cleanup_publications(&publications, false);
                return Err(error);
            }
        };
        publications.push(PreparedPublication {
            target,
            replacement,
            backup,
            original,
        });
    }
    if let Err(error) = sync_publication_directories(&publications) {
        cleanup_publications(&publications, false);
        return Err(format!("failed to sync prepared SF1 publication: {error}").into());
    }
    for publication in &publications {
        let current = match read_regular_file(&publication.target) {
            Ok(bytes) => bytes,
            Err(error) => {
                cleanup_publications(&publications, false);
                return Err(error);
            }
        };
        if current != publication.original {
            cleanup_publications(&publications, false);
            return Err(format!(
                "generated destination changed before publication: {}",
                publication.target.display()
            )
            .into());
        }
    }
    commit_publications(&publications, None)
}
fn write_json(dir: &Path, vectors: &FixtureVectors) -> Result<(), Box<dyn std::error::Error>> {
    let mut prng = Map::new();
    prng.insert(
        "multiplier".to_owned(),
        Value::from(vectors.prng.multiplier),
    );
    prng.insert("increment".to_owned(), Value::from(vectors.prng.increment));
    let chunk_lengths = Value::Array(
        vectors
            .chunk_lengths
            .iter()
            .map(|len| Value::from(*len as u64))
            .collect(),
    );
    let chunk_offsets = Value::Array(
        vectors
            .chunk_offsets
            .iter()
            .map(|offset| Value::from(*offset as u64))
            .collect(),
    );
    let chunk_digests = Value::Array(
        vectors
            .blake3_digest_hexes()
            .into_iter()
            .map(Value::from)
            .collect(),
    );
    let mut root = Map::new();
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert("input_seed".to_owned(), Value::from(vectors.input_seed_hex));
    root.insert(
        "input_length".to_owned(),
        Value::from(vectors.input_length as u64),
    );
    root.insert("prng".to_owned(), Value::Object(prng));
    root.insert(
        "chunk_count".to_owned(),
        Value::from(vectors.chunk_count() as u64),
    );
    root.insert("chunk_lengths".to_owned(), chunk_lengths);
    root.insert("chunk_offsets".to_owned(), chunk_offsets);
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("chunk_digests_blake3".to_owned(), chunk_digests);
    let json_bytes = json::to_vec_pretty(&Value::Object(root))?;
    fs::write(dir.join("sf1_profile_v1.json"), json_bytes)?;
    Ok(())
}
fn write_rust(dir: &Path, vectors: &FixtureVectors) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(dir.join("sf1_profile_v1.rs"))?;
    writeln!(
        file,
        "// @generated by `cargo run -p sorafs_chunker --features dev-tools --bin export_vectors`\n\
         // Canonical fixture constants for the SoraFS chunker."
    )?;
    writeln!(
        file,
        "pub const PROFILE: &str = \"{CANONICAL_PROFILE_HANDLE}\";"
    )?;
    writeln!(
        file,
        "pub const PROFILE_ALIASES: &[&str] = &[\"{CANONICAL_PROFILE_HANDLE}\"];"
    )?;
    writeln!(
        file,
        "pub const INPUT_SEED: &str = \"{}\";",
        vectors.input_seed_hex
    )?;
    writeln!(
        file,
        "pub const INPUT_LENGTH: usize = {};",
        vectors.input_length
    )?;
    writeln!(
        file,
        "pub const PRNG_MULTIPLIER: u64 = {}u64;",
        vectors.prng.multiplier
    )?;
    writeln!(
        file,
        "pub const PRNG_INCREMENT: u64 = {}u64;",
        vectors.prng.increment
    )?;
    writeln!(
        file,
        "pub const CHUNK_COUNT: usize = {};",
        vectors.chunk_count()
    )?;
    write_array(&mut file, "CHUNK_LENGTHS", "usize", &vectors.chunk_lengths)?;
    write_array(&mut file, "CHUNK_OFFSETS", "usize", &vectors.chunk_offsets)?;
    writeln!(
        file,
        "pub const CHUNK_DIGEST_SHA3_256: &str = \"{}\";",
        vectors.sha3_digest_hex()
    )?;
    write_str_array(
        &mut file,
        "CHUNK_DIGESTS_BLAKE3",
        &vectors.blake3_digest_hexes(),
    )?;
    Ok(())
}
fn write_typescript(dir: &Path, vectors: &FixtureVectors) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(dir.join("sf1_profile_v1.ts"))?;
    writeln!(
        file,
        "// @generated by `cargo run -p sorafs_chunker --features dev-tools --bin export_vectors`\n\
         // Canonical fixture constants for the SoraFS chunker.\n"
    )?;
    writeln!(
        file,
        "export interface ChunkerFixture {{\n    profile: string;\n    inputSeed: string;\n    inputLength: number;\n    prngMultiplier: string;\n    prngIncrement: string;\n    chunkCount: number;\n    chunkLengths: readonly number[];\n    chunkOffsets: readonly number[];\n    chunkDigestSha3_256: string;\n    chunkDigestsBlake3: readonly string[];\n}}\n"
    )?;
    writeln!(file, "export const sf1ProfileV1: ChunkerFixture = {{")?;
    writeln!(file, "    profile: \"{CANONICAL_PROFILE_HANDLE}\",")?;
    writeln!(
        file,
        "    profileAliases: [\"{CANONICAL_PROFILE_HANDLE}\"] as const,"
    )?;
    writeln!(file, "    inputSeed: \"{}\",", vectors.input_seed_hex)?;
    writeln!(file, "    inputLength: {},", vectors.input_length)?;
    writeln!(file, "    prngMultiplier: \"{}\",", vectors.prng.multiplier)?;
    writeln!(file, "    prngIncrement: \"{}\",", vectors.prng.increment)?;
    writeln!(file, "    chunkCount: {},", vectors.chunk_count())?;
    write_ts_number_array(&mut file, "chunkLengths", &vectors.chunk_lengths)?;
    write_ts_number_array(&mut file, "chunkOffsets", &vectors.chunk_offsets)?;
    writeln!(
        file,
        "    chunkDigestSha3_256: \"{}\",",
        vectors.sha3_digest_hex()
    )?;
    write_ts_string_array(
        &mut file,
        "chunkDigestsBlake3",
        &vectors.blake3_digest_hexes(),
    )?;
    writeln!(file, "}} as const;")?;
    Ok(())
}
fn write_go(dir: &Path, vectors: &FixtureVectors) -> Result<(), std::io::Error> {
    let mut file = fs::File::create(dir.join("sf1_profile_v1.go"))?;
    writeln!(
        file,
        "// Code generated by `cargo run -p sorafs_chunker --features dev-tools --bin export_vectors`; DO NOT EDIT.\n\
         package sorafsfixtures\n"
    )?;
    writeln!(
        file,
        "type ChunkerFixture struct {{\n    Profile string\n    ProfileAliases []string\n    InputSeed string\n    InputLength int\n    PRNGMultiplier uint64\n    PRNGIncrement uint64\n    ChunkCount int\n    ChunkLengths []int\n    ChunkOffsets []int\n    ChunkDigestSHA3_256 string\n    ChunkDigestsBLAKE3 []string\n}}\n"
    )?;
    writeln!(file, "var SF1ProfileV1 = ChunkerFixture{{")?;
    writeln!(file, "    Profile: \"{CANONICAL_PROFILE_HANDLE}\",")?;
    writeln!(
        file,
        "    ProfileAliases: []string{{\"{CANONICAL_PROFILE_HANDLE}\"}},"
    )?;
    writeln!(file, "    InputSeed: \"{}\",", vectors.input_seed_hex)?;
    writeln!(file, "    InputLength: {},", vectors.input_length)?;
    writeln!(file, "    PRNGMultiplier: {},", vectors.prng.multiplier)?;
    writeln!(file, "    PRNGIncrement: {},", vectors.prng.increment)?;
    writeln!(file, "    ChunkCount: {},", vectors.chunk_count())?;
    write_go_int_slice(&mut file, "ChunkLengths", &vectors.chunk_lengths)?;
    write_go_int_slice(&mut file, "ChunkOffsets", &vectors.chunk_offsets)?;
    writeln!(
        file,
        "    ChunkDigestSHA3_256: \"{}\",",
        vectors.sha3_digest_hex()
    )?;
    write_go_string_slice(
        &mut file,
        "ChunkDigestsBLAKE3",
        &vectors.blake3_digest_hexes(),
    )?;
    writeln!(file, "}}")?;
    Ok(())
}
fn write_manifest(
    dir: &Path,
    vectors: &FixtureVectors,
) -> Result<Hash, Box<dyn std::error::Error>> {
    let mut entries = Vec::with_capacity(MANIFEST_PROFILE_FILES.len());
    for name in MANIFEST_PROFILE_FILES {
        let path = dir.join(name);
        let bytes = read_regular_file(&path)?;
        let digest = to_hex(blake3::hash(&bytes).as_bytes());
        let mut entry = Map::new();
        entry.insert("file".to_owned(), Value::from(name));
        entry.insert("size".to_owned(), Value::from(bytes.len() as u64));
        entry.insert("blake3".to_owned(), Value::from(digest));
        entries.push(Value::Object(entry));
    }
    let mut root = Map::new();
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("files".to_owned(), Value::Array(entries));
    let bytes = json::to_vec_pretty(&Value::Object(root))?;
    let digest = blake3::hash(&bytes);
    fs::write(dir.join("manifest_blake3.json"), bytes)?;
    Ok(digest)
}
fn write_manifest_signatures(
    dir: &Path,
    vectors: &FixtureVectors,
    manifest_digest: Hash,
    cli: &CliOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_path = cli
        .signature_out
        .clone()
        .unwrap_or_else(|| dir.join("manifest_signatures.json"));
    let existing_root = load_existing_manifest_signatures(&out_path, vectors, &manifest_digest)?;
    if cli.signing_key_hex.is_none() {
        match existing_root.as_ref() {
            Some(root) => {
                ensure_signed(root, &manifest_digest)?;
                return Ok(());
            }
            None => {
                return Err(
                    "manifest_signatures.json missing; provide explicit signing-key authority"
                        .into(),
                );
            }
        }
    }
    let signing_key_hex = cli
        .signing_key_hex
        .as_ref()
        .expect("signing key checked above");
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, signing_key_hex)
        .map_err(|err| format!("failed to parse --signing-key: {err}"))?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|err| format!("failed to derive public key from --signing-key: {err}"))?;
    let public_key = key_pair.public_key();
    let (algorithm, public_bytes) = public_key
        .try_to_bytes()
        .map_err(|err| format!("signing public key is malformed: {err}"))?;
    if algorithm != Algorithm::Ed25519 {
        return Err("signing key must use the Ed25519 algorithm".into());
    }
    let derived_signer_hex = to_hex(public_bytes);
    if cli
        .signer_hex
        .as_ref()
        .is_some_and(|expected| expected != &derived_signer_hex)
    {
        return Err(format!(
            "--signer does not match the public key derived from --signing-key (expected {derived_signer_hex})"
        )
        .into());
    }
    let signature = Signature::try_new(key_pair.private_key(), manifest_digest.as_bytes())
        .map_err(|err| format!("failed to sign manifest digest: {err}"))?;
    let signature_hex = to_hex(signature.payload());
    let manifest_digest_hex = to_hex(manifest_digest.as_bytes());
    let mut entry = Map::new();
    entry.insert("algorithm".to_owned(), Value::from("ed25519"));
    entry.insert("signer".to_owned(), Value::from(derived_signer_hex.clone()));
    entry.insert("signature".to_owned(), Value::from(signature_hex));
    entry.insert(
        "signer_multihash".to_owned(),
        Value::from(public_key.to_string()),
    );
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut root = existing_root.unwrap_or_default();
    let mut signatures = match root.remove("signatures") {
        Some(Value::Array(items)) => items,
        Some(_) => {
            return Err("manifest signatures field must be an array"
                .to_owned()
                .into());
        }
        None => Vec::new(),
    };
    let entry_value = Value::Object(entry.clone());
    let mut replaced = false;
    for existing in signatures.iter_mut() {
        if signature_signer(existing)? == derived_signer_hex {
            if *existing != entry_value {
                *existing = entry_value.clone();
            }
            replaced = true;
            break;
        }
    }
    if !replaced {
        signatures.push(entry_value);
    }
    let mut signatures_with_keys = Vec::with_capacity(signatures.len());
    for value in signatures {
        let signer = signature_signer(&value)?.to_owned();
        signatures_with_keys.push((signer, value));
    }
    signatures_with_keys.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    let signatures: Vec<Value> = signatures_with_keys
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    verify_signatures(&signatures, &manifest_digest)?;
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert("manifest".to_owned(), Value::from("manifest_blake3.json"));
    root.insert(
        "manifest_blake3".to_owned(),
        Value::from(manifest_digest_hex),
    );
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("signatures".to_owned(), Value::Array(signatures.clone()));
    ensure_signed(&root, &manifest_digest)?;
    let bytes = json::to_vec_pretty(&Value::Object(root))?;
    fs::write(out_path, bytes)?;
    Ok(())
}
fn signature_signer(value: &Value) -> Result<&str, Box<dyn std::error::Error>> {
    let signer = value
        .as_object()
        .and_then(|map| map.get("signer"))
        .and_then(Value::as_str)
        .ok_or_else(|| "signature entries must include a signer field".to_owned())?;
    Ok(signer)
}
type JsonMap = Map;
fn load_existing_manifest_signatures(
    path: &Path,
    vectors: &FixtureVectors,
    manifest_digest: &Hash,
) -> Result<Option<JsonMap>, Box<dyn std::error::Error>> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let existing = json::from_slice(&read_regular_file(path)?)
        .map_err(|err| format!("failed to parse existing manifest signatures file: {err}"))?;
    let root = match existing {
        Value::Object(map) => map,
        _ => return Err("manifest signatures file must contain a JSON object".into()),
    };
    let profile = root
        .get("profile")
        .and_then(Value::as_str)
        .ok_or_else(|| "manifest signatures missing profile field".to_owned())?;
    if profile != CANONICAL_PROFILE_HANDLE {
        return Err(
            format!(
                "existing manifest signatures profile {profile} mismatches expected canonical handle {CANONICAL_PROFILE_HANDLE}"
            )
            .into(),
        );
    }
    let aliases = root
        .get("profile_aliases")
        .and_then(Value::as_array)
        .ok_or("manifest signatures missing profile_aliases field")?;
    if aliases.len() != 1
        || aliases.first().and_then(Value::as_str) != Some(CANONICAL_PROFILE_HANDLE)
    {
        return Err(
            "manifest signatures profile_aliases must contain only the canonical handle".into(),
        );
    }
    match root.get("manifest").and_then(Value::as_str) {
        Some("manifest_blake3.json") => {}
        Some(other) => {
            return Err(format!(
                "existing manifest signatures references unexpected manifest {other}"
            )
            .into());
        }
        None => return Err("manifest signatures missing manifest field".into()),
    }
    let manifest_digest_hex = to_hex(manifest_digest.as_bytes());
    match root.get("manifest_blake3").and_then(Value::as_str) {
        Some(digest) if digest == manifest_digest_hex => {}
        Some(_) => {
            return Err(
                "existing manifest signatures digest mismatches regenerated manifest"
                    .to_owned()
                    .into(),
            );
        }
        None => return Err("manifest signatures missing manifest_blake3 field".into()),
    }
    match root.get("chunk_digest_sha3_256").and_then(Value::as_str) {
        Some(chunk_digest) if chunk_digest == vectors.sha3_digest_hex() => {}
        Some(_) => {
            return Err(
                "existing manifest signatures chunk digest mismatches regenerated vectors"
                    .to_owned()
                    .into(),
            );
        }
        None => return Err("manifest signatures missing chunk_digest_sha3_256 field".into()),
    }
    Ok(Some(root))
}
fn extract_signatures(map: &JsonMap) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    match map.get("signatures") {
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err("manifest signatures field must be an array".into()),
        None => Err("manifest signatures missing signatures array".into()),
    }
}
fn ensure_signed(map: &JsonMap, manifest_digest: &Hash) -> Result<(), Box<dyn std::error::Error>> {
    let signatures = extract_signatures(map)?;
    if signatures.is_empty() {
        return Err("manifest signatures file contains no council signatures".into());
    }
    verify_signatures(&signatures, manifest_digest)?;
    Ok(())
}
fn verify_signatures(
    signatures: &[Value],
    manifest_digest: &Hash,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut seen_signers = BTreeSet::new();
    for entry in signatures {
        let map = entry
            .as_object()
            .ok_or_else(|| "signature entry must be an object".to_owned())?;
        let algorithm = map
            .get("algorithm")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing algorithm".to_owned())?;
        if algorithm != "ed25519" {
            return Err(format!("unsupported signature algorithm {algorithm}").into());
        }
        let signer_hex = map
            .get("signer")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing signer".to_owned())?;
        if !seen_signers.insert(signer_hex) {
            return Err(format!("duplicate manifest signer {signer_hex}").into());
        }
        let signature_hex = map
            .get("signature")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing signature".to_owned())?;
        let signer_bytes = decode_canonical_hex_exact(signer_hex, "signer", 32)?;
        let signature_bytes = decode_canonical_hex_exact(signature_hex, "signature", 64)?;
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes)
            .map_err(|err| format!("invalid signer public key: {err}"))?;
        let expected_multihash = public_key.to_string();
        let multihash = map
            .get("signer_multihash")
            .and_then(Value::as_str)
            .ok_or("signature entry missing signer_multihash")?;
        if multihash != expected_multihash.as_str() {
            return Err("signer_multihash does not match encoded public key".into());
        }
        let signature = iroha_crypto::ed25519_parse_signature(&signature_bytes)
            .map_err(|err| format!("invalid signature material: {err}"))?;
        signature
            .verify(&public_key, manifest_digest.as_bytes())
            .map_err(|err| format!("signature verification failed: {err}"))?;
    }
    Ok(())
}
fn decode_canonical_hex_exact(
    value: &str,
    field: &str,
    expected_len: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if value.as_bytes().iter().any(u8::is_ascii_whitespace) {
        return Err(format!("{field} hex must not contain ASCII whitespace").into());
    }
    if value.starts_with("0x") || value.starts_with("0X") {
        return Err(format!("{field} hex must not include a 0x prefix").into());
    }
    let expected_digits = expected_len
        .checked_mul(2)
        .ok_or_else(|| format!("{field} expected length overflow"))?;
    if value.len() != expected_digits {
        return Err(
            format!("{field} hex must be {expected_digits} lowercase hex characters").into(),
        );
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} hex must be lowercase hex").into());
    }
    let decoded =
        hex::decode(value).map_err(|err| format!("failed to decode {field} hex: {err}"))?;
    if decoded.iter().all(|byte| *byte == 0) {
        return Err(format!("{field} material must not be all zero").into());
    }
    Ok(decoded)
}
fn write_fuzz_corpora(
    repo_root: &Path,
    vectors: &FixtureVectors,
) -> Result<[Hash; 2], Box<dyn std::error::Error>> {
    let fuzz_dir = repo_root.join("fuzz").join("sorafs_chunker");
    fs::create_dir_all(&fuzz_dir)?;
    // Raw input corpus for fuzzers/back-pressure harnesses.
    let input_digest = blake3::hash(&vectors.input);
    fs::write(fuzz_dir.join("sf1_profile_v1_input.bin"), &vectors.input)?;
    let scenarios = generate_backpressure_scenarios(vectors);
    let mut scenario_array = Vec::with_capacity(scenarios.len());
    for scenario in scenarios {
        let mut map = Map::new();
        map.insert("name".to_owned(), Value::from(scenario.name));
        map.insert(
            "feed_sizes".to_owned(),
            Value::Array(
                scenario
                    .feed_sizes
                    .iter()
                    .map(|&size| Value::from(size as u64))
                    .collect(),
            ),
        );
        map.insert(
            "expected_chunk_lengths".to_owned(),
            Value::Array(
                scenario
                    .expected_chunk_lengths
                    .iter()
                    .map(|&len| Value::from(len as u64))
                    .collect(),
            ),
        );
        map.insert(
            "chunk_count".to_owned(),
            Value::from(scenario.expected_chunk_lengths.len() as u64),
        );
        map.insert(
            "max_feed_size".to_owned(),
            Value::from(scenario.max_feed_size as u64),
        );
        map.insert(
            "min_feed_size".to_owned(),
            Value::from(scenario.min_feed_size as u64),
        );
        scenario_array.push(Value::Object(map));
    }
    let mut root = Map::new();
    let profile_aliases = vec![Value::from(CANONICAL_PROFILE_HANDLE)];
    root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert(
        "input_length".to_owned(),
        Value::from(vectors.input_length as u64),
    );
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(vectors.sha3_digest_hex()),
    );
    root.insert("scenarios".to_owned(), Value::Array(scenario_array));
    let bytes = json::to_vec_pretty(&Value::Object(root))?;
    let backpressure_digest = blake3::hash(&bytes);
    fs::write(fuzz_dir.join("sf1_profile_v1_backpressure.json"), bytes)?;
    Ok([input_digest, backpressure_digest])
}
struct BackpressureScenario {
    name: &'static str,
    feed_sizes: Vec<usize>,
    expected_chunk_lengths: Vec<usize>,
    max_feed_size: usize,
    min_feed_size: usize,
}
fn generate_backpressure_scenarios(vectors: &FixtureVectors) -> Vec<BackpressureScenario> {
    let total = vectors.input.len();
    let profile = vectors.chunk_profile;
    let patterns: [(&str, Vec<usize>); 3] = [
        ("uniform_4k", vec![4 * 1024]),
        (
            "burst_64k",
            vec![64 * 1024, 3 * 1024, 8 * 1024, 2 * 1024, 48 * 1024],
        ),
        (
            "jitter_prime",
            vec![
                1_537, 3_073, 5_003, 7_013, 11_143, 17_147, 19_313, 23_477, 29_597, 31_337,
            ],
        ),
    ];
    patterns
        .iter()
        .map(|(name, pattern)| {
            let feed_sizes = partition_input(total, pattern);
            let expected_chunk_lengths =
                capture_stream_chunks(profile, &vectors.input, &feed_sizes);
            let max_feed_size = feed_sizes.iter().copied().max().unwrap_or(0);
            let min_feed_size = feed_sizes.iter().copied().min().unwrap_or(0);
            BackpressureScenario {
                name,
                feed_sizes,
                expected_chunk_lengths,
                max_feed_size,
                min_feed_size,
            }
        })
        .collect()
}
fn partition_input(total: usize, pattern: &[usize]) -> Vec<usize> {
    assert!(!pattern.is_empty(), "partition pattern may not be empty");
    let mut feed = Vec::with_capacity(total / pattern.iter().sum::<usize>().max(1) + 1);
    let mut remaining = total;
    let mut idx = 0;
    while remaining > 0 {
        let candidate = pattern[idx % pattern.len()];
        let size = candidate.min(remaining);
        if size == 0 {
            break;
        }
        feed.push(size);
        remaining -= size;
        idx += 1;
    }
    if remaining > 0 {
        feed.push(remaining);
    }
    feed
}
fn capture_stream_chunks(profile: ChunkProfile, input: &[u8], feed_sizes: &[usize]) -> Vec<usize> {
    let mut chunker = Chunker::with_profile(profile);
    let mut emitted = Vec::new();
    let mut offset = 0usize;
    for &feed in feed_sizes {
        let end = (offset + feed).min(input.len());
        let slice = &input[offset..end];
        chunker.feed(slice, |chunk| emitted.push(chunk.length));
        offset = end;
    }
    chunker.finish(|chunk| emitted.push(chunk.length));
    // If finish emitted the sentinel zero-length chunk (only when no data),
    // filter it out to keep parity with batch chunking fixtures.
    if input.is_empty() {
        emitted
    } else {
        emitted.retain(|len| *len > 0);
        emitted
    }
}
fn write_array<T: std::fmt::Display>(
    file: &mut fs::File,
    name: &str,
    ty: &str,
    values: &[T],
) -> Result<(), std::io::Error> {
    writeln!(file, "pub const {name}: [{ty}; {}] = [", values.len())?;
    for value in values {
        writeln!(file, "    {value},")?;
    }
    writeln!(file, "];")?;
    Ok(())
}
fn write_str_array(
    file: &mut fs::File,
    name: &str,
    values: &[String],
) -> Result<(), std::io::Error> {
    writeln!(file, "pub const {name}: [&str; {}] = [", values.len())?;
    for value in values {
        writeln!(file, "    \"{value}\",")?;
    }
    writeln!(file, "];")?;
    Ok(())
}
fn write_ts_number_array(
    file: &mut fs::File,
    name: &str,
    values: &[usize],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: [")?;
    for value in values {
        writeln!(file, "        {value},")?;
    }
    writeln!(file, "    ] as const,")?;
    Ok(())
}
fn write_ts_string_array(
    file: &mut fs::File,
    name: &str,
    values: &[String],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: [")?;
    for value in values {
        writeln!(file, "        \"{value}\",")?;
    }
    writeln!(file, "    ] as const,")?;
    Ok(())
}
fn write_go_int_slice(
    file: &mut fs::File,
    name: &str,
    values: &[usize],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: []int{{")?;
    for value in values {
        writeln!(file, "        {value},")?;
    }
    writeln!(file, "    }},")?;
    Ok(())
}
fn write_go_string_slice(
    file: &mut fs::File,
    name: &str,
    values: &[String],
) -> Result<(), std::io::Error> {
    writeln!(file, "    {name}: []string{{")?;
    for value in values {
        writeln!(file, "        \"{value}\",")?;
    }
    writeln!(file, "    }},")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
    use sorafs_chunker::fixtures::{FixtureProfile, FixtureVectors, to_hex};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    const SIGNING_KEY_1: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const SIGNING_KEY_2: &str = "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f";
    fn temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        dir.push(format!(
            "sorafs_chunker_test_{:x}_{}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .expect("make temp dir private");
        }
        dir
    }
    fn prepare_generated_tree(root: &Path, marker: &[u8]) {
        for relative in GENERATED_PATHS {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().expect("generated path parent"))
                .expect("create generated parent");
            let mut bytes = marker.to_vec();
            bytes.extend_from_slice(relative.as_bytes());
            fs::write(path, bytes).expect("write generated file");
        }
    }
    fn prepare_fixture_files(dir: &Path, vectors: &FixtureVectors) {
        write_json(dir, vectors).expect("write json fixture");
        write_rust(dir, vectors).expect("write rust fixture");
        write_typescript(dir, vectors).expect("write ts fixture");
        write_go(dir, vectors).expect("write go fixture");
    }
    fn derive_public_hex(secret_hex: &str) -> String {
        let private_key =
            PrivateKey::from_hex(Algorithm::Ed25519, secret_hex).expect("valid private key");
        let key_pair = KeyPair::from_private_key(private_key).expect("derive public key");
        let (algorithm, public_bytes) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be well-formed");
        assert_eq!(algorithm, Algorithm::Ed25519);
        to_hex(public_bytes)
    }
    fn expect_cli_message(result: Result<CliOptions, CliError>) -> String {
        match result {
            Err(CliError::Message(message)) => message,
            Err(CliError::Help) => panic!("expected CLI validation error, got help"),
            Ok(_) => panic!("expected CLI validation error, got parsed options"),
        }
    }
    fn read_signers(path: &Path) -> Vec<String> {
        let bytes = fs::read(path).expect("read manifest signatures");
        let value: Value = json::from_slice(&bytes).expect("parse manifest signatures json");
        let signatures = value
            .get("signatures")
            .and_then(Value::as_array)
            .expect("signatures array");
        signatures
            .iter()
            .map(|entry| {
                entry
                    .get("signer")
                    .and_then(Value::as_str)
                    .expect("signer field")
                    .to_owned()
            })
            .collect()
    }
    #[test]
    fn parse_cli_rejects_noncanonical_signing_material() {
        for (value, expected) in [
            ("", "non-empty"),
            (
                " 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
                "whitespace",
            ),
            (
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f ",
                "whitespace",
            ),
            (
                "0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
                "0x prefix",
            ),
            (
                "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F",
                "lowercase",
            ),
            ("00", "32- or 64-byte"),
            (
                "0000000000000000000000000000000000000000000000000000000000000000",
                "all zero",
            ),
        ] {
            let err = expect_cli_message(parse_cli_from([format!("--signing-key={value}")]));
            assert!(
                err.contains(expected),
                "unexpected error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn parse_cli_accepts_canonical_signing_material_and_signer() {
        let signer = derive_public_hex(SIGNING_KEY_1);
        let options = parse_cli_from([
            "--write".to_owned(),
            "--staging-root".to_owned(),
            "/private/tmp/sf1-owner-test".to_owned(),
            format!("--signing-key={SIGNING_KEY_1}"),
            format!("--signer={signer}"),
        ])
        .unwrap_or_else(|_| panic!("canonical CLI inputs must parse"));
        assert_eq!(options.signing_key_hex.as_deref(), Some(SIGNING_KEY_1));
        assert_eq!(options.signer_hex.as_deref(), Some(signer.as_str()));
        assert_eq!(options.mode, Some(OwnerMode::Write));
        assert_eq!(
            options.staging_root.as_deref(),
            Some(Path::new("/private/tmp/sf1-owner-test"))
        );
        assert!(options.signature_out.is_none());
    }
    #[test]
    fn owner_helpers_bind_stage_and_read_private_signing_key() {
        let stage = temp_dir().canonicalize().expect("canonical temp stage");
        let repository = repo_root().expect("repository root");
        let bound = bind_staging_root(&stage, &repository).expect("bind private external stage");
        assert_eq!(bound, stage.canonicalize().expect("canonical stage"));
        let key_path = stage.join("signing-key");
        fs::write(&key_path, format!("{SIGNING_KEY_1}\n")).expect("write signing key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
                .expect("make signing key private");
        }
        assert_eq!(
            read_signing_key_file(&key_path).expect("read private signing key"),
            SIGNING_KEY_1
        );
        fs::remove_dir_all(&stage).expect("cleanup temp dir");
    }
    #[test]
    fn staged_inventory_check_and_publication_are_exact() {
        let stage = temp_dir();
        let repository = temp_dir();
        prepare_generated_tree(&stage, b"new:");
        prepare_generated_tree(&repository, b"old:");
        validate_staged_tree(&stage).expect("validate complete stage");
        let stale = check_staged_tree(&stage, &repository)
            .expect_err("different repository bytes must be stale");
        assert!(
            stale
                .to_string()
                .contains("checked-in SF1 fixtures are stale")
        );
        publish_staged_tree(&stage, &repository).expect("publish complete stage");
        check_staged_tree(&stage, &repository).expect("published bytes must be exact");
        for relative in GENERATED_PATHS {
            assert_eq!(
                read_regular_file(&repository.join(relative)).expect("read published file"),
                read_regular_file(&stage.join(relative)).expect("read staged file")
            );
        }
        fs::remove_dir_all(&stage).expect("cleanup stage");
        fs::remove_dir_all(&repository).expect("cleanup repository");
    }
    #[test]
    fn publication_failure_rolls_back_every_committed_target() {
        let dir = temp_dir();
        let mut publications = Vec::new();
        for name in ["first", "second"] {
            let target = dir.join(name);
            fs::write(&target, b"old").expect("write target");
            let replacement =
                create_private_sibling(&target, "new", b"new").expect("create replacement");
            let backup = create_private_sibling(&target, "backup", b"old").expect("create backup");
            publications.push(PreparedPublication {
                target,
                replacement,
                backup,
                original: b"old".to_vec(),
            });
        }
        let error = commit_publications(&publications, Some(1))
            .expect_err("injected second publication must fail");
        assert!(
            error
                .to_string()
                .contains("injected SF1 publication failure")
        );
        for publication in &publications {
            assert_eq!(
                fs::read(&publication.target).expect("read restored target"),
                b"old"
            );
            assert!(!publication.replacement.exists());
            assert!(!publication.backup.exists());
        }
        let retained_target = dir.join("retained");
        fs::write(&retained_target, b"new").expect("write retained target");
        let retained = vec![PreparedPublication {
            replacement: create_private_sibling(&retained_target, "new", b"newer")
                .expect("create retained replacement"),
            backup: create_private_sibling(&retained_target, "backup", b"old")
                .expect("create retained backup"),
            target: retained_target,
            original: b"old".to_vec(),
        }];
        let rollback_errors = rollback_publications(&retained, 1, Some(0));
        assert_eq!(rollback_errors.len(), 1);
        cleanup_publications(&retained, true);
        assert_eq!(
            fs::read(&retained[0].backup).expect("read retained recovery copy"),
            b"old"
        );
        cleanup_publications(&retained, false);
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn parse_cli_rejects_noncanonical_signer_material() {
        for (value, expected) in [
            ("", "non-empty"),
            (
                " 03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8",
                "whitespace",
            ),
            (
                "0x03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8",
                "0x prefix",
            ),
            (
                "03A107BFF3CE10BE1D70DD18E74BC09967E4D6309BA50D5F1DDC8664125531B8",
                "lowercase",
            ),
            ("03a107", "32-byte"),
        ] {
            let err = expect_cli_message(parse_cli_from([
                format!("--signing-key={SIGNING_KEY_1}"),
                format!("--signer={value}"),
            ]));
            assert!(
                err.contains(expected),
                "unexpected signer error for {value:?}: {err}"
            );
        }
    }
    #[test]
    fn manifest_signatures_merge_without_duplicates() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let cli = CliOptions {
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            signature_out: Some(dir.join("signatures.json")),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect("initial signature write");
        let out_path = cli.signature_out.as_ref().expect("signature output path");
        let first_signer = derive_public_hex(cli.signing_key_hex.as_deref().unwrap());
        assert_eq!(read_signers(out_path), vec![first_signer.clone()]);
        // Re-sign with the same key; should not create duplicates.
        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect("idempotent signature write");
        assert_eq!(read_signers(out_path), vec![first_signer.clone()]);
        let cli_second = CliOptions {
            signing_key_hex: Some(SIGNING_KEY_2.to_owned()),
            signature_out: Some(out_path.clone()),
            ..CliOptions::default()
        };
        let second_signer = derive_public_hex(cli_second.signing_key_hex.as_deref().unwrap());
        write_manifest_signatures(&dir, &vectors, manifest_digest, &cli_second)
            .expect("append second signature");
        let mut expected = vec![first_signer, second_signer];
        expected.sort();
        assert_eq!(read_signers(out_path), expected);
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn unsigned_regeneration_without_signatures_is_rejected() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let cli = CliOptions {
            signature_out: Some(dir.join("manifest_signatures.json")),
            ..CliOptions::default()
        };
        let err = write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect_err("missing signatures must fail");
        assert!(
            err.to_string().contains("manifest_signatures.json missing"),
            "unexpected error: {err}"
        );
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn existing_manifest_signatures_reject_empty_signature_set() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let signature_path = dir.join("manifest_signatures.json");
        let mut root = Map::new();
        root.insert("profile".to_owned(), Value::from(CANONICAL_PROFILE_HANDLE));
        root.insert(
            "profile_aliases".to_owned(),
            Value::Array(vec![Value::from(CANONICAL_PROFILE_HANDLE)]),
        );
        root.insert("manifest".to_owned(), Value::from("manifest_blake3.json"));
        root.insert(
            "manifest_blake3".to_owned(),
            Value::from(to_hex(manifest_digest.as_bytes())),
        );
        root.insert(
            "chunk_digest_sha3_256".to_owned(),
            Value::from(vectors.sha3_digest_hex()),
        );
        root.insert("signatures".to_owned(), Value::Array(Vec::new()));
        let bytes = json::to_vec_pretty(&Value::Object(root)).expect("serialize signatures");
        fs::write(&signature_path, bytes).expect("write unsigned signature file");
        let cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            ..CliOptions::default()
        };
        let err = write_manifest_signatures(&dir, &vectors, manifest_digest, &cli)
            .expect_err("empty signature set must fail");
        assert!(
            err.to_string().contains("contains no council signatures"),
            "unexpected error: {err}"
        );
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn existing_signed_manifest_passes_verification() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let signature_path = dir.join("manifest_signatures.json");
        let signer_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &signer_cli)
            .expect("signing should succeed");
        assert!(signature_path.exists(), "signature file should be created");
        let verify_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &verify_cli)
            .expect("verification should succeed with existing signature");
        // Ensure additional signer can still be added afterwards.
        let second_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_2.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &second_cli)
            .expect("appending second signature must succeed");
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn existing_manifest_signatures_reject_noncanonical_hex_fields() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let signature_path = dir.join("manifest_signatures.json");
        let signer_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &signer_cli)
            .expect("signing should succeed");
        let baseline: Value =
            json::from_slice(&fs::read(&signature_path).expect("read signatures"))
                .expect("signature json parses");
        for (field, value, expected) in [
            (
                "signer",
                format!("0x{}", derive_public_hex(SIGNING_KEY_1)),
                "0x prefix",
            ),
            (
                "signer",
                derive_public_hex(SIGNING_KEY_1).to_ascii_uppercase(),
                "lowercase",
            ),
            ("signer", "00".repeat(32), "all zero"),
            ("signature", "00".repeat(64), "all zero"),
            ("signature", "ab".repeat(63), "128 lowercase hex characters"),
        ] {
            let mut tampered = baseline.clone();
            let first = tampered
                .get_mut("signatures")
                .and_then(Value::as_array_mut)
                .and_then(|signatures| signatures.first_mut())
                .and_then(Value::as_object_mut)
                .expect("signature entry");
            first.insert(field.to_owned(), Value::from(value));
            let bytes = json::to_vec_pretty(&tampered).expect("serialize signature json");
            fs::write(&signature_path, bytes).expect("write tampered signatures");
            let verify_cli = CliOptions {
                signature_out: Some(signature_path.clone()),
                ..CliOptions::default()
            };
            let err = write_manifest_signatures(&dir, &vectors, manifest_digest, &verify_cli)
                .expect_err("noncanonical signature field must fail verification");
            assert!(
                err.to_string().contains(expected),
                "unexpected error for {field}: {err}"
            );
        }
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn existing_manifest_signatures_reject_all_zero_signature_material() {
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let signature_path = dir.join("manifest_signatures.json");
        let signer_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &signer_cli)
            .expect("signing should succeed");
        let mut signature_json: Value =
            json::from_slice(&fs::read(&signature_path).expect("read signatures"))
                .expect("signature json parses");
        let signatures = signature_json
            .get_mut("signatures")
            .and_then(Value::as_array_mut)
            .expect("signatures array");
        let first = signatures
            .first_mut()
            .and_then(Value::as_object_mut)
            .expect("signature entry");
        first.insert("signature".to_owned(), Value::from("00".repeat(64)));
        let bytes = json::to_vec_pretty(&signature_json).expect("serialize signature json");
        fs::write(&signature_path, bytes).expect("write tampered signatures");
        let verify_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            ..CliOptions::default()
        };
        let err = write_manifest_signatures(&dir, &vectors, manifest_digest, &verify_cli)
            .expect_err("all-zero signature material must fail verification");
        assert!(
            err.to_string().contains("all zero"),
            "unexpected error: {err}"
        );
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
    #[test]
    fn existing_manifest_signatures_reject_malformed_signature_r() {
        const SMALL_ORDER_R: [u8; 32] = [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        const NONCANONICAL_R: [u8; 32] = [
            0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0x7f,
        ];
        let dir = temp_dir();
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        prepare_fixture_files(&dir, &vectors);
        let manifest_digest = write_manifest(&dir, &vectors).expect("write manifest");
        let signature_path = dir.join("manifest_signatures.json");
        let signer_cli = CliOptions {
            signature_out: Some(signature_path.clone()),
            signing_key_hex: Some(SIGNING_KEY_1.to_owned()),
            ..CliOptions::default()
        };
        write_manifest_signatures(&dir, &vectors, manifest_digest, &signer_cli)
            .expect("signing should succeed");
        let signature_json: Value =
            json::from_slice(&fs::read(&signature_path).expect("read signatures"))
                .expect("signature json parses");
        let original_signature = signature_json
            .get("signatures")
            .and_then(Value::as_array)
            .and_then(|signatures| signatures.first())
            .and_then(Value::as_object)
            .and_then(|entry| entry.get("signature"))
            .and_then(Value::as_str)
            .expect("signature field");
        let signature_bytes =
            hex::decode(original_signature).expect("generated signature hex decodes");
        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_R),
            ("noncanonical", NONCANONICAL_R),
        ] {
            let mut tampered_json = signature_json.clone();
            let signatures = tampered_json
                .get_mut("signatures")
                .and_then(Value::as_array_mut)
                .expect("signatures array");
            let first = signatures
                .first_mut()
                .and_then(Value::as_object_mut)
                .expect("signature entry");
            let mut tampered_signature = signature_bytes.clone();
            tampered_signature[..32].copy_from_slice(&replacement_r);
            first.insert(
                "signature".to_owned(),
                Value::from(hex::encode(tampered_signature)),
            );
            let bytes = json::to_vec_pretty(&tampered_json).expect("serialize signature json");
            fs::write(&signature_path, bytes).expect("write tampered signatures");
            let verify_cli = CliOptions {
                signature_out: Some(signature_path.clone()),
                ..CliOptions::default()
            };
            let err = write_manifest_signatures(&dir, &vectors, manifest_digest, &verify_cli)
                .expect_err("malformed signature R must fail verification");
            assert!(
                err.to_string().contains("invalid signature material"),
                "{label} signature R produced unexpected error: {err}"
            );
        }
        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
}

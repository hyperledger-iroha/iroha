//! Offline minting, inspection, and revocation tooling for SoraNet admission tokens.

use std::{
    fs::{self, File, Metadata as FsMetadata, OpenOptions},
    io::{self, Read as _},
    path::{Path, PathBuf},
    process,
    time::{Duration, SystemTime},
};

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use norito::json::{self, Value};
use rand::{RngCore, SeedableRng, rng, rngs::StdRng};
use soranet_pq::MlDsaSuite;
use soranet_relay::token_tool::{
    MintRequest, REVOCATION_LIST_MAX_ENTRIES_V1, RevocationList, TokenBundle, decode_token_string,
    encode_token_base64, encode_token_hex, inspect_token, mint_token, parse_hex_array,
    parse_hex_bytes, parse_rfc3339, read_revocation_file,
};

const DEFAULT_TTL_SECS: u64 = 900;
// AdmissionToken v1 encodes a 4-byte magic, fixed body fields, and a u16
// signature length before the signature. This admits every structurally
// representable v1 token while rejecting file-backed data above 65,671 bytes.
const ADMISSION_TOKEN_V1_FIXED_FRAME_BYTES: usize = 4 + 1 + 1 + 8 + 8 + 32 + 32 + 16 + 32 + 2;
const ADMISSION_TOKEN_V1_MAX_FILE_BYTES: usize =
    ADMISSION_TOKEN_V1_FIXED_FRAME_BYTES + u16::MAX as usize;
// ML-DSA-87 is the largest suite supported by this CLI. Keep these literals
// source-coupled to `MlDsaSuite` with the boundary test below so backend width
// changes cannot silently widen file-backed inputs.
const ML_DSA_MAX_PUBLIC_KEY_BYTES_V1: usize = 2_592;
const ML_DSA_MAX_SECRET_KEY_BYTES_V1: usize = 4_896;
const ML_DSA_MAX_PUBLIC_KEY_HEX_BYTES_V1: usize = ML_DSA_MAX_PUBLIC_KEY_BYTES_V1 * 2;
const ML_DSA_MAX_SECRET_KEY_HEX_BYTES_V1: usize = ML_DSA_MAX_SECRET_KEY_BYTES_V1 * 2;
// Key files are single-value operator artifacts. This corridor preserves trim
// semantics for CRLF and modest editor padding without permitting whitespace
// to amplify the exact ML-DSA payload ceiling.
const ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1: usize = 256;
const REVOCATION_LIST_CANONICAL_ENTRY_BYTES: usize = 70;
const REVOCATION_LIST_MAX_CANONICAL_BYTES_V1: usize =
    REVOCATION_LIST_MAX_ENTRIES_V1 * REVOCATION_LIST_CANONICAL_ENTRY_BYTES + 3;

#[cfg(any(target_os = "macos", target_os = "ios"))]
const TOKEN_FILE_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const TOKEN_FILE_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const TOKEN_FILE_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("SoraNet admission-token file loading requires a defined no-follow flag");

#[cfg(unix)]
type TokenFileIdentity = (u64, u64);
#[cfg(windows)]
type TokenFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type TokenFileIdentity = ();

#[cfg(unix)]
fn token_file_identity(metadata: &FsMetadata) -> TokenFileIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn token_file_identity(metadata: &FsMetadata) -> TokenFileIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn token_file_identity(_metadata: &FsMetadata) -> TokenFileIdentity {}

#[cfg(unix)]
const fn token_file_identity_available(_identity: TokenFileIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn token_file_identity_available(identity: TokenFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn token_file_identity_available(_identity: TokenFileIdentity) -> bool {
    false
}

#[cfg(windows)]
fn token_file_is_reparse_point(metadata: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn token_file_is_reparse_point(_metadata: &FsMetadata) -> bool {
    false
}

fn validate_token_file_metadata(metadata: &FsMetadata, artifact: &str) -> io::Result<()> {
    if metadata.file_type().is_symlink()
        || token_file_is_reparse_point(metadata)
        || !metadata.file_type().is_file()
        || !token_file_identity_available(token_file_identity(metadata))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{artifact} must be a direct regular file with a stable identity"),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn open_token_file_direct(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = OpenOptions::new();
    options.read(true).custom_flags(TOKEN_FILE_O_NOFOLLOW_FLAG);
    options.open(path)
}

#[cfg(windows)]
fn open_token_file_direct(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_token_file_direct(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "stable direct-file opens are unavailable on this platform",
    ))
}

#[cfg(unix)]
fn token_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    token_file_identity(left) == token_file_identity(right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
}

#[cfg(windows)]
fn token_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    token_file_identity_available(token_file_identity(left))
        && token_file_identity(left) == token_file_identity(right)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}

#[cfg(not(any(unix, windows)))]
fn token_file_metadata_unchanged(_left: &FsMetadata, _right: &FsMetadata) -> bool {
    false
}

#[cfg(test)]
static TOKEN_FILE_READ_REPLACEMENT: std::sync::Mutex<Option<(PathBuf, PathBuf)>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn replace_token_file_for_test(path: &Path) -> io::Result<()> {
    let replacement = {
        let mut hook = TOKEN_FILE_READ_REPLACEMENT
            .lock()
            .expect("token file race hook lock");
        if hook.as_ref().is_some_and(|(expected, _)| expected == path) {
            hook.take().map(|(_, replacement)| replacement)
        } else {
            None
        }
    };
    if let Some(replacement) = replacement {
        fs::rename(replacement, path)?;
    }
    Ok(())
}

/// Read one immutable token snapshot at the complete v1 framing ceiling.
fn read_admission_token_file(path: &Path) -> io::Result<Vec<u8>> {
    read_bounded_direct_file(path, ADMISSION_TOKEN_V1_MAX_FILE_BYTES, "admission token")
}

fn read_bounded_direct_file(path: &Path, maximum: usize, artifact: &str) -> io::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    validate_token_file_metadata(&before, artifact)?;
    let maximum_u64 = u64::try_from(maximum).unwrap_or(u64::MAX);
    if before.len() > maximum_u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{artifact} is {} bytes; first-release limit is {maximum} bytes",
                before.len()
            ),
        ));
    }

    #[cfg(test)]
    replace_token_file_for_test(path)?;

    let mut file = open_token_file_direct(path)?;
    let opened = file.metadata()?;
    validate_token_file_metadata(&opened, artifact)?;
    if !token_file_metadata_unchanged(&before, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{artifact} changed between inspection and open"),
        ));
    }
    let expected_len = usize::try_from(opened.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{artifact} length is not representable on this host"),
        )
    })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{artifact} changed length while being read"),
            )
        } else {
            error
        }
    })?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{artifact} grew while being read or exceeds its {maximum}-byte limit"),
        ));
    }

    let after_file = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    validate_token_file_metadata(&after_file, artifact)?;
    validate_token_file_metadata(&after_path, artifact)?;
    if !token_file_metadata_unchanged(&opened, &after_file)
        || !token_file_metadata_unchanged(&opened, &after_path)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{artifact} changed while being read"),
        ));
    }
    Ok(bytes)
}

#[derive(Parser, Debug)]
#[command(
    name = "soranet-admission-token",
    version,
    about = "Mint, inspect, and manage SoraNet admission tokens"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Mint a new admission token signed with an ML-DSA issuer key.
    Mint(MintArgs),
    /// Decode an admission token and print its metadata.
    Inspect(InspectArgs),
    /// Append a token identifier to a Norito JSON revocation list.
    Revoke(RevokeArgs),
    /// Print all token identifiers present in a revocation list.
    ListRevocations(ListArgs),
}

#[derive(Args, Debug)]
struct MintArgs {
    #[arg(long, conflicts_with = "issuer_public_file")]
    issuer_public_hex: Option<String>,
    #[arg(long)]
    issuer_public_file: Option<PathBuf>,
    #[arg(long, conflicts_with = "issuer_secret_file")]
    issuer_secret_hex: Option<String>,
    #[arg(long)]
    issuer_secret_file: Option<PathBuf>,
    #[arg(long)]
    relay_id_hex: String,
    #[arg(long)]
    transcript_hash_hex: String,
    #[arg(long)]
    issued_at: Option<String>,
    #[arg(long, conflicts_with = "ttl_secs")]
    expires_at: Option<String>,
    #[arg(long, conflicts_with = "expires_at")]
    ttl_secs: Option<u64>,
    #[arg(long, default_value_t = 0)]
    flags: u8,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = SuiteArg::MlDsa44)]
    suite: SuiteArg,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("inspect_source")
        .required(true)
        .args(&["token", "input"])
))]
struct InspectArgs {
    #[arg(long)]
    token: Option<String>,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("revoke_source")
        .required(true)
        .args(&["token", "token_file", "token_id_hex"])
))]
struct RevokeArgs {
    #[arg(long)]
    list: PathBuf,
    #[arg(long)]
    token: Option<String>,
    #[arg(long)]
    token_file: Option<PathBuf>,
    #[arg(long)]
    token_id_hex: Option<String>,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct ListArgs {
    #[arg(long)]
    list: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Base64,
    Hex,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SuiteArg {
    MlDsa44,
    MlDsa65,
    MlDsa87,
}

impl From<SuiteArg> for MlDsaSuite {
    fn from(value: SuiteArg) -> Self {
        match value {
            SuiteArg::MlDsa44 => MlDsaSuite::MlDsa44,
            SuiteArg::MlDsa65 => MlDsaSuite::MlDsa65,
            SuiteArg::MlDsa87 => MlDsaSuite::MlDsa87,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("soranet-admission-token: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Mint(args) => command_mint(args),
        Command::Inspect(args) => command_inspect(args),
        Command::Revoke(args) => command_revoke(args),
        Command::ListRevocations(args) => command_list(args),
    }
}

fn command_mint(args: MintArgs) -> Result<(), String> {
    let issuer_public = load_hex_source(
        args.issuer_public_hex,
        args.issuer_public_file.as_deref(),
        "issuer_public_key",
    )?;
    let issuer_secret = load_hex_source(
        args.issuer_secret_hex,
        args.issuer_secret_file.as_deref(),
        "issuer_secret_key",
    )?;
    let relay_id =
        parse_hex_array::<32>(&args.relay_id_hex, "relay_id_hex").map_err(|err| err.to_string())?;
    let transcript_hash = parse_hex_array::<32>(&args.transcript_hash_hex, "transcript_hash_hex")
        .map_err(|err| err.to_string())?;

    let issued_at = match args.issued_at {
        Some(ref value) => parse_rfc3339(value, "issued_at").map_err(|err| err.to_string())?,
        None => SystemTime::now(),
    };

    let expires_at = if let Some(ref value) = args.expires_at {
        parse_rfc3339(value, "expires_at").map_err(|err| err.to_string())?
    } else {
        let ttl = args.ttl_secs.unwrap_or(DEFAULT_TTL_SECS);
        issued_at
            .checked_add(Duration::from_secs(ttl))
            .ok_or_else(|| "expiry timestamp overflowed SystemTime".to_owned())?
    };

    let suite: MlDsaSuite = args.suite.into();
    let request = MintRequest {
        suite,
        issuer_public_key: &issuer_public,
        issuer_secret_key: &issuer_secret,
        relay_id,
        transcript_hash,
        issued_at,
        expires_at,
        flags: args.flags,
    };

    let mut seed = [0u8; 32];
    rng().fill_bytes(&mut seed);
    let mut rng = StdRng::from_seed(seed);
    let bundle = mint_token(&request, &mut rng).map_err(|err| err.to_string())?;
    let output = render_bundle(&bundle, args.format)?;
    write_output(args.output.as_deref(), &output)
}

fn command_inspect(args: InspectArgs) -> Result<(), String> {
    let bytes = if let Some(token_str) = args.token {
        decode_token_string(&token_str).map_err(|err| err.to_string())?
    } else if let Some(path) = args.input {
        read_admission_token_file(&path).map_err(|err| err.to_string())?
    } else {
        unreachable!("clap enforces inspect source group");
    };
    let bundle = inspect_token(&bytes).map_err(|err| err.to_string())?;
    let output = render_bundle(&bundle, args.format)?;
    write_output(None, &output)
}

fn command_revoke(args: RevokeArgs) -> Result<(), String> {
    let token_id = if let Some(hex) = args.token_id_hex {
        parse_hex_array::<32>(&hex, "token_id_hex").map_err(|err| err.to_string())?
    } else {
        let bytes = if let Some(token_str) = args.token {
            decode_token_string(&token_str).map_err(|err| err.to_string())?
        } else if let Some(path) = args.token_file {
            read_admission_token_file(&path).map_err(|err| err.to_string())?
        } else {
            unreachable!("clap enforces revoke source group");
        };
        let bundle = inspect_token(&bytes).map_err(|err| err.to_string())?;
        bundle.metadata.token_id
    };

    let mut list = RevocationList::load_or_default(&args.list)
        .map_err(|err| format!("failed to load {}: {err}", args.list.display()))?;
    let inserted = list.insert(token_id);

    if !args.dry_run && inserted {
        list.write(&args.list)
            .map_err(|err| format!("failed to write {}: {err}", args.list.display()))?;
    }

    let mut payload = json::Map::new();
    payload.insert("token_id_hex".into(), Value::from(hex::encode(token_id)));
    payload.insert("inserted".into(), Value::from(inserted));
    payload.insert("dry_run".into(), Value::from(args.dry_run));
    payload.insert(
        "revocation_list_path".into(),
        Value::from(args.list.display().to_string()),
    );
    let mut text = json::to_string_pretty(&Value::Object(payload))
        .map_err(|err| format!("failed to serialise revoke output: {err}"))?;
    text.push('\n');
    write_output(None, &text)
}

fn command_list(args: ListArgs) -> Result<(), String> {
    let ids = read_revocation_file(&args.list)
        .map_err(|err| format!("failed to load {}: {err}", args.list.display()))?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    write_revocation_list_json(&mut output, &ids)
        .map_err(|err| format!("failed to write revocation list: {err}"))
}

fn write_revocation_list_json<W: io::Write>(writer: &mut W, ids: &[[u8; 32]]) -> io::Result<()> {
    if ids.len() > REVOCATION_LIST_MAX_ENTRIES_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "revocation list contains {} entries; first-release limit is {REVOCATION_LIST_MAX_ENTRIES_V1}",
                ids.len()
            ),
        ));
    }
    let expected_len = if ids.is_empty() {
        3
    } else {
        ids.len()
            .checked_mul(REVOCATION_LIST_CANONICAL_ENTRY_BYTES)
            .and_then(|length| length.checked_add(3))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "revocation-list output length overflowed",
                )
            })?
    };
    if expected_len > REVOCATION_LIST_MAX_CANONICAL_BYTES_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "revocation-list output exceeds its first-release byte limit",
        ));
    }
    if ids.is_empty() {
        writer.write_all(b"[]\n")?;
        return writer.flush();
    }

    // This fixed hexadecimal schema matches Norito's pretty JSON layout while
    // retaining only one 64-byte stack encoding at a time.
    writer.write_all(b"[")?;
    for (index, id) in ids.iter().enumerate() {
        if index != 0 {
            writer.write_all(b",")?;
        }
        writer.write_all(b"\n  \"")?;
        writer.write_all(&token_id_hex_bytes(id))?;
        writer.write_all(b"\"")?;
    }
    writer.write_all(b"\n]\n")?;
    writer.flush()
}

fn token_id_hex_bytes(id: &[u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = [0_u8; 64];
    for (index, byte) in id.iter().copied().enumerate() {
        encoded[index * 2] = HEX[usize::from(byte >> 4)];
        encoded[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    encoded
}

fn render_bundle(bundle: &TokenBundle, format: OutputFormat) -> Result<String, String> {
    match format {
        OutputFormat::Json => {
            let mut text = json::to_string_pretty(&bundle.to_json())
                .map_err(|err| format!("failed to serialise JSON: {err}"))?;
            text.push('\n');
            Ok(text)
        }
        OutputFormat::Base64 => {
            let mut text = encode_token_base64(&bundle.token);
            text.push('\n');
            Ok(text)
        }
        OutputFormat::Hex => {
            let mut text = encode_token_hex(&bundle.token);
            text.push('\n');
            Ok(text)
        }
    }
}

fn write_output(path: Option<&Path>, data: &str) -> Result<(), String> {
    if let Some(path) = path {
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            fs::create_dir_all(dir)
                .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        }
        fs::write(path, data).map_err(|err| format!("failed to write {}: {err}", path.display()))
    } else {
        print!("{data}");
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct KeyFileLimits {
    artifact: &'static str,
    maximum_hex_bytes: usize,
}

fn key_file_limits(field: &'static str) -> Result<KeyFileLimits, String> {
    match field {
        "issuer_public_key" => Ok(KeyFileLimits {
            artifact: "issuer public-key file",
            maximum_hex_bytes: ML_DSA_MAX_PUBLIC_KEY_HEX_BYTES_V1,
        }),
        "issuer_secret_key" => Ok(KeyFileLimits {
            artifact: "issuer secret-key file",
            maximum_hex_bytes: ML_DSA_MAX_SECRET_KEY_HEX_BYTES_V1,
        }),
        _ => Err(format!(
            "no bounded file profile is defined for hexadecimal field {field}"
        )),
    }
}

fn load_hex_key_file(
    path: &Path,
    field: &'static str,
    limits: KeyFileLimits,
) -> Result<Vec<u8>, String> {
    let maximum_raw_bytes = limits
        .maximum_hex_bytes
        .checked_add(ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1)
        .ok_or_else(|| format!("{} byte limit overflowed", limits.artifact))?;
    let bytes = read_bounded_direct_file(path, maximum_raw_bytes, limits.artifact)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        format!(
            "{} must contain UTF-8 hexadecimal text: {err}",
            limits.artifact
        )
    })?;
    let trimmed = text.trim();
    let surrounding_whitespace_bytes = text.len().saturating_sub(trimmed.len());
    if surrounding_whitespace_bytes > ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1 {
        return Err(format!(
            "{} contains {surrounding_whitespace_bytes} surrounding whitespace bytes; first-release limit is {ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1}",
            limits.artifact
        ));
    }
    if trimmed.len() > limits.maximum_hex_bytes {
        return Err(format!(
            "{} contains {} hexadecimal bytes; supported ML-DSA limit is {}",
            limits.artifact,
            trimmed.len(),
            limits.maximum_hex_bytes
        ));
    }
    parse_hex_bytes(trimmed, field).map_err(|err| err.to_string())
}

fn load_hex_source(
    inline: Option<String>,
    path: Option<&Path>,
    field: &'static str,
) -> Result<Vec<u8>, String> {
    match (inline, path) {
        (Some(value), None) => parse_hex_bytes(value.trim(), field).map_err(|err| err.to_string()),
        (None, Some(path)) => {
            let limits = key_file_limits(field)?;
            load_hex_key_file(path, field, limits)
        }
        (Some(_), Some(_)) => Err(format!(
            "--{field}-hex and --{field}-file are mutually exclusive"
        )),
        (None, None) => Err(format!("--{field}-hex or --{field}-file must be provided")),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[derive(Default)]
    struct CountingWriter {
        bytes: usize,
    }

    impl io::Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(buffer.len())
                .ok_or_else(|| io::Error::other("test byte count overflowed"))?;
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn key_file_limits_match_largest_supported_mldsa_suite() {
        let suites = [
            MlDsaSuite::MlDsa44,
            MlDsaSuite::MlDsa65,
            MlDsaSuite::MlDsa87,
        ];
        assert_eq!(
            suites.iter().copied().map(MlDsaSuite::public_key_len).max(),
            Some(ML_DSA_MAX_PUBLIC_KEY_BYTES_V1)
        );
        assert_eq!(
            suites.iter().copied().map(MlDsaSuite::secret_key_len).max(),
            Some(ML_DSA_MAX_SECRET_KEY_BYTES_V1)
        );
    }

    #[test]
    fn key_files_accept_exact_payload_and_whitespace_limits() {
        let dir = tempdir().expect("tmp");
        let public_path = dir.path().join("issuer-public.hex");
        let mut public_text = "ab".repeat(ML_DSA_MAX_PUBLIC_KEY_BYTES_V1);
        public_text.push_str(&" ".repeat(ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1));
        assert_eq!(
            public_text.len(),
            ML_DSA_MAX_PUBLIC_KEY_HEX_BYTES_V1
                + ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1
        );
        fs::write(&public_path, public_text).expect("write exact public key file");
        assert_eq!(
            load_hex_source(None, Some(&public_path), "issuer_public_key")
                .expect("exact public key file must load"),
            vec![0xab; ML_DSA_MAX_PUBLIC_KEY_BYTES_V1]
        );

        let secret_path = dir.path().join("issuer-secret.hex");
        fs::write(&secret_path, "cd".repeat(ML_DSA_MAX_SECRET_KEY_BYTES_V1))
            .expect("write exact secret key file");
        assert_eq!(
            load_hex_source(None, Some(&secret_path), "issuer_secret_key")
                .expect("exact secret key file must load"),
            vec![0xcd; ML_DSA_MAX_SECRET_KEY_BYTES_V1]
        );
    }

    #[test]
    fn key_files_reject_payload_and_whitespace_plus_one() {
        let dir = tempdir().expect("tmp");
        let oversized_payload = dir.path().join("oversized-payload.hex");
        fs::write(
            &oversized_payload,
            "ab".repeat(ML_DSA_MAX_PUBLIC_KEY_BYTES_V1 + 1),
        )
        .expect("write oversized payload");
        let error = load_hex_source(None, Some(&oversized_payload), "issuer_public_key")
            .expect_err("decoded key limit + 1 must fail");
        assert!(error.contains("supported ML-DSA limit"), "{error}");

        let oversized_whitespace = dir.path().join("oversized-whitespace.hex");
        let mut text = "cd".repeat(ML_DSA_MAX_SECRET_KEY_BYTES_V1);
        text.push_str(&" ".repeat(ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1 + 1));
        fs::write(&oversized_whitespace, text).expect("write oversized whitespace");
        let error = load_hex_source(None, Some(&oversized_whitespace), "issuer_secret_key")
            .expect_err("raw key file limit + 1 must fail");
        assert!(error.contains("first-release limit"), "{error}");

        let excessive_padding = dir.path().join("excessive-padding.hex");
        let mut text = "ef".repeat(MlDsaSuite::MlDsa44.public_key_len());
        text.push_str(&" ".repeat(ML_DSA_KEY_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1 + 1));
        fs::write(&excessive_padding, text).expect("write excessive padding");
        let error = load_hex_source(None, Some(&excessive_padding), "issuer_public_key")
            .expect_err("surrounding whitespace limit + 1 must fail");
        assert!(error.contains("surrounding whitespace bytes"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn key_file_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tmp");
        let target = dir.path().join("issuer-public.hex");
        let link = dir.path().join("issuer-public-link.hex");
        fs::write(&target, "ab".repeat(ML_DSA_MAX_PUBLIC_KEY_BYTES_V1))
            .expect("write public key target");
        symlink(&target, &link).expect("create key symlink");

        let error = load_hex_source(None, Some(&link), "issuer_public_key")
            .expect_err("key symlink must fail");
        assert!(error.contains("direct regular file"), "{error}");
    }

    #[test]
    fn inline_hex_source_preserves_prior_argument_length_handling() {
        let inline = "ab".repeat(ML_DSA_MAX_SECRET_KEY_BYTES_V1 + 1);
        assert_eq!(
            load_hex_source(Some(inline), None, "issuer_secret_key")
                .expect("inline parsing remains delegated to suite validation")
                .len(),
            ML_DSA_MAX_SECRET_KEY_BYTES_V1 + 1
        );
    }

    #[test]
    fn token_file_limit_accepts_exact_and_rejects_plus_one() {
        assert_eq!(ADMISSION_TOKEN_V1_MAX_FILE_BYTES, 65_671);
        let dir = tempdir().expect("tmp");
        let exact = dir.path().join("exact.token");
        let exact_file = File::create(&exact).expect("create exact token file");
        exact_file
            .set_len(
                u64::try_from(ADMISSION_TOKEN_V1_MAX_FILE_BYTES)
                    .expect("fixed token limit fits u64"),
            )
            .expect("size exact token file");
        assert_eq!(
            read_admission_token_file(&exact)
                .expect("exact token file limit must load")
                .len(),
            ADMISSION_TOKEN_V1_MAX_FILE_BYTES
        );

        let plus_one = dir.path().join("plus-one.token");
        let oversized_file = File::create(&plus_one).expect("create oversized token file");
        oversized_file
            .set_len(
                u64::try_from(ADMISSION_TOKEN_V1_MAX_FILE_BYTES + 1)
                    .expect("fixed token limit + 1 fits u64"),
            )
            .expect("size oversized token file");
        let error = read_admission_token_file(&plus_one).expect_err("limit + 1 must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn token_file_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("tmp");
        let target = dir.path().join("target.token");
        let link = dir.path().join("link.token");
        fs::write(&target, b"token").expect("write target");
        symlink(&target, &link).expect("create symlink");

        let error = read_admission_token_file(&link).expect_err("symlink must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn token_file_reader_rejects_path_replacement_race() {
        let dir = tempdir().expect("tmp");
        let configured = dir.path().join("configured.token");
        let replacement = dir.path().join("replacement.token");
        fs::write(&configured, b"token").expect("write configured token");
        fs::write(&replacement, b"token").expect("write replacement token");
        *TOKEN_FILE_READ_REPLACEMENT.lock().expect("race hook lock") =
            Some((configured.clone(), replacement));

        let error = read_admission_token_file(&configured).expect_err("path replacement must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn revocation_output_matches_norito_pretty_layout() {
        let ids = [[0x11; 32], [0x22; 32]];
        let mut output = Vec::new();
        write_revocation_list_json(&mut output, &ids).expect("write revocation output");
        assert_eq!(
            output,
            format!(
                "[\n  \"{}\",\n  \"{}\"\n]\n",
                hex::encode(ids[0]),
                hex::encode(ids[1])
            )
            .into_bytes()
        );
    }

    #[test]
    fn revocation_output_accepts_exact_count_and_rejects_plus_one() {
        let exact = vec![[0x33; 32]; REVOCATION_LIST_MAX_ENTRIES_V1];
        let mut exact_writer = CountingWriter::default();
        write_revocation_list_json(&mut exact_writer, &exact)
            .expect("exact entry count must stream");
        assert_eq!(exact_writer.bytes, REVOCATION_LIST_MAX_CANONICAL_BYTES_V1);

        let plus_one = vec![[0x44; 32]; REVOCATION_LIST_MAX_ENTRIES_V1 + 1];
        let mut rejected_writer = CountingWriter::default();
        let error = write_revocation_list_json(&mut rejected_writer, &plus_one)
            .expect_err("entry count + 1 must fail before output");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(rejected_writer.bytes, 0);
    }
}

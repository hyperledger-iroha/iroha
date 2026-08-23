//! Command-line harness for SoraNet handshake fixtures and diagnostics.

use clap::{Parser, Subcommand};
use soranet_handshake_harness::{
    CapabilitySummary, CapabilityTlv, HandshakeSuite, HarnessError, HexInput,
    SaltAnnouncementParams, SimulationParams, TelemetryReport, TranscriptInputs, decode_hex,
    decode_salt_hex, diff_capabilities, format_capabilities,
    generate_capability_fixtures as harness_generate, parse_capabilities, salt_announcement_json,
    simulate_handshake, simulation_report_json, soranet_telemetry_json,
    verify_fixtures as harness_verify, verify_salt_vector,
};
use soranet_pq::{
    MlKemSuite, SuiteParseError, validate_mlkem_ciphertext, validate_mlkem_public_key,
    validate_mlkem_secret_key,
};
#[cfg(unix)]
use std::fs::{File, Metadata as FsMetadata, OpenOptions};
use std::{
    env, fmt, fs,
    io::{self, Read as _},
    path::{Path, PathBuf},
};

const SECRET_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1: usize = 256;
const STATIC_SECRET_KEY_BYTES: usize = 32;

struct SecretBytes(Vec<u8>);

impl fmt::Debug for SecretBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted secret bytes>")
    }
}

impl SecretBytes {
    fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        // Best-effort wipe without adding dependency metadata to this
        // developer-only harness. Cryptographic backends may own additional
        // internal copies while an operation is in progress.
        self.0.fill(0);
        std::hint::black_box(&self.0);
    }
}

struct StaticSecretKey([u8; STATIC_SECRET_KEY_BYTES]);

impl StaticSecretKey {
    fn as_array(&self) -> &[u8; STATIC_SECRET_KEY_BYTES] {
        &self.0
    }
}

impl Drop for StaticSecretKey {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&self.0);
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
const SECRET_FILE_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SECRET_FILE_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const SECRET_FILE_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
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
compile_error!("SoraNet handshake-harness secret loading requires a defined no-follow flag");

#[cfg(unix)]
type SecretFileIdentity = (u64, u64);

#[cfg(unix)]
fn secret_file_identity(metadata: &FsMetadata) -> SecretFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}

#[cfg(unix)]
fn secret_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    secret_file_identity(left) == secret_file_identity(right)
        && left.len() == right.len()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(unix)]
fn current_process_owner_uid() -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt as _;
    Ok(tempfile::tempfile()?.metadata()?.uid())
}

#[cfg(unix)]
fn validate_private_secret_file(metadata: &FsMetadata, label: &str, owner: u32) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.uid() != owner
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{label} must be a direct regular file owned by the current user, owner-private, and have exactly one link"
            ),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn open_private_secret_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(SECRET_FILE_O_NOFOLLOW_FLAG);
    options.open(path)
}

#[cfg(unix)]
fn read_private_secret_file(
    path: &Path,
    maximum_raw_bytes: usize,
    label: &str,
) -> Result<SecretBytes, HarnessError> {
    let owner = current_process_owner_uid()?;
    let before = fs::symlink_metadata(path)?;
    validate_private_secret_file(&before, label, owner)?;
    if before.len() > u64::try_from(maximum_raw_bytes).unwrap_or(u64::MAX) {
        return Err(HarnessError::Validation(format!(
            "{label} exceeds its {maximum_raw_bytes}-byte first-release input limit"
        )));
    }
    let mut file = open_private_secret_file(path)?;
    let opened = file.metadata()?;
    validate_private_secret_file(&opened, label, owner)?;
    if !secret_file_metadata_unchanged(&before, &opened) {
        return Err(HarnessError::Validation(format!(
            "{label} changed between inspection and open"
        )));
    }
    let expected_len = usize::try_from(opened.len()).map_err(|_| {
        HarnessError::Validation(format!("{label} length is not representable on this host"))
    })?;
    let mut bytes = SecretBytes::new(Vec::new());
    bytes
        .0
        .try_reserve_exact(expected_len)
        .map_err(|_| HarnessError::Validation(format!("failed to reserve bounded {label}")))?;
    bytes.0.resize(expected_len, 0);
    file.read_exact(&mut bytes.0).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            HarnessError::Validation(format!("{label} changed length while being read"))
        } else {
            HarnessError::Io(error)
        }
    })?;
    let mut growth_probe = [0_u8; 1];
    let grew = file.read(&mut growth_probe)? != 0;
    growth_probe.fill(0);
    std::hint::black_box(&growth_probe);
    if grew {
        return Err(HarnessError::Validation(format!(
            "{label} grew while being read or exceeds its {maximum_raw_bytes}-byte limit"
        )));
    }
    let after_file = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    validate_private_secret_file(&after_file, label, owner)?;
    validate_private_secret_file(&after_path, label, owner)?;
    if !secret_file_metadata_unchanged(&opened, &after_file)
        || !secret_file_metadata_unchanged(&opened, &after_path)
    {
        return Err(HarnessError::Validation(format!(
            "{label} changed while being read"
        )));
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_private_secret_file(
    _path: &Path,
    _maximum_raw_bytes: usize,
    label: &str,
) -> Result<SecretBytes, HarnessError> {
    Err(HarnessError::Validation(format!(
        "private {label} file custody checks are unavailable on this platform; use standard input"
    )))
}

fn read_bounded_secret_input(
    mut reader: impl io::Read,
    maximum_raw_bytes: usize,
    label: &str,
) -> Result<SecretBytes, HarnessError> {
    let read_limit = maximum_raw_bytes
        .checked_add(1)
        .ok_or_else(|| HarnessError::Validation(format!("{label} input limit overflowed")))?;
    let mut bytes = SecretBytes::new(Vec::new());
    bytes
        .0
        .try_reserve_exact(read_limit)
        .map_err(|_| HarnessError::Validation(format!("failed to reserve bounded {label}")))?;
    let mut bounded = reader.by_ref().take(u64::try_from(read_limit).map_err(|_| {
        HarnessError::Validation(format!("{label} input limit is not representable"))
    })?);
    bounded.read_to_end(&mut bytes.0)?;
    if bytes.0.len() > maximum_raw_bytes {
        return Err(HarnessError::Validation(format!(
            "{label} exceeds its {maximum_raw_bytes}-byte first-release input limit"
        )));
    }
    Ok(bytes)
}

fn decode_secret_hex_source(
    path: &Path,
    maximum_decoded_bytes: usize,
    label: &str,
) -> Result<SecretBytes, HarnessError> {
    let maximum_raw_bytes = maximum_decoded_bytes
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(SECRET_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1))
        .ok_or_else(|| HarnessError::Validation(format!("{label} input limit overflowed")))?;
    let raw = if path == Path::new("-") {
        let stdin = io::stdin();
        read_bounded_secret_input(stdin.lock(), maximum_raw_bytes, label)?
    } else {
        read_private_secret_file(path, maximum_raw_bytes, label)?
    };
    decode_secret_hex_bytes(raw.as_slice(), maximum_decoded_bytes, label)
}

fn decode_secret_hex_bytes(
    raw: &[u8],
    maximum_decoded_bytes: usize,
    label: &str,
) -> Result<SecretBytes, HarnessError> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| HarnessError::Validation(format!("{label} must be UTF-8 hexadecimal text")))?;
    let trimmed = text.trim();
    let surrounding_whitespace_bytes = text.len().saturating_sub(trimmed.len());
    if surrounding_whitespace_bytes > SECRET_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1 {
        return Err(HarnessError::Validation(format!(
            "{label} exceeds the {}-byte surrounding-whitespace limit",
            SECRET_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1
        )));
    }
    let decoded =
        SecretBytes::new(decode_hex(trimmed).map_err(|error| {
            HarnessError::Validation(format!("failed to decode {label}: {error}"))
        })?);
    if decoded.0.len() > maximum_decoded_bytes {
        return Err(HarnessError::Validation(format!(
            "{label} decodes to {} bytes; first-release limit is {maximum_decoded_bytes}",
            decoded.0.len()
        )));
    }
    Ok(decoded)
}

fn load_static_secret(path: &Path, label: &str) -> Result<StaticSecretKey, HarnessError> {
    let decoded = decode_secret_hex_source(path, STATIC_SECRET_KEY_BYTES, label)?;
    if decoded.0.len() != STATIC_SECRET_KEY_BYTES {
        return Err(HarnessError::Validation(format!(
            "{label} must decode to {STATIC_SECRET_KEY_BYTES} bytes, got {}",
            decoded.0.len()
        )));
    }
    if decoded
        .0
        .first()
        .is_some_and(|first| decoded.0.iter().all(|byte| byte == first))
    {
        return Err(HarnessError::Validation(format!(
            "{label} must not be an all-zero or repeated-byte degenerate key"
        )));
    }
    let mut key = [0_u8; STATIC_SECRET_KEY_BYTES];
    key.copy_from_slice(decoded.as_slice());
    Ok(StaticSecretKey(key))
}

fn validate_simulation_secret_sources(client: &Path, relay: &Path) -> Result<(), HarnessError> {
    if client == Path::new("-") && relay == Path::new("-") {
        return Err(HarnessError::Validation(
            "client and relay static secrets cannot both read from standard input".into(),
        ));
    }
    Ok(())
}
/// Command-line interface for the (still evolving) SoraNet handshake harness.
#[derive(Parser, Debug)]
#[command(author, version, about = "SoraNet handshake harness", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspect capability TLVs and compute the transcript hash.
    Inspect {
        /// Client capability vector (hex)
        #[arg(long)]
        client_hex: String,
        /// Relay capability vector (hex)
        #[arg(long)]
        relay_hex: String,
        /// Descriptor commitment (hex)
        #[arg(long)]
        descriptor_commit_hex: String,
        /// Client nonce (hex)
        #[arg(long)]
        client_nonce_hex: String,
        /// Relay nonce (hex)
        #[arg(long)]
        relay_nonce_hex: String,
        /// KEM identifier (decimal)
        #[arg(long)]
        kem_id: u8,
        /// ML-KEM suite label (e.g., mlkem768); overrides `--kem-id` when provided.
        #[arg(long = "kem-suite")]
        kem_suite: Option<String>,
        /// Signature identifier (decimal)
        #[arg(long)]
        sig_id: u8,
        /// Optional resume hash (hex)
        #[arg(long)]
        resume_hash_hex: Option<HexInput>,
    },
    /// Emit a JSON summary of a capability vector.
    Summary {
        /// Capability vector (hex)
        #[arg(long)]
        vector_hex: String,
    },
    /// Render a SaltAnnouncementV1 payload as JSON.
    Salt {
        #[arg(long)]
        epoch_id: u32,
        #[arg(long)]
        valid_after: String,
        #[arg(long)]
        valid_until: String,
        #[arg(long)]
        salt_hex: String,
        #[arg(long)]
        previous_epoch: Option<u32>,
        #[arg(long, default_value_t = false)]
        emergency: bool,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Verify a SaltAnnouncementV1 fixture on disk.
    SaltVerify {
        /// Path to the salt fixture (JSON).
        vector: PathBuf,
    },
    /// Render a SoraNetTelemetryV1 payload as JSON.
    Telemetry {
        #[arg(long)]
        epoch: u32,
        #[arg(long)]
        downgrade_attempts: u32,
        #[arg(long)]
        pq_disabled_sessions: u32,
        #[arg(long)]
        cover_ratio: f32,
        #[arg(long)]
        lagging_clients: u32,
        #[arg(long)]
        max_latency_ms: u32,
        #[arg(long)]
        incident_reference: Option<String>,
        #[arg(long)]
        signature: Option<String>,
        #[arg(long)]
        witness_signature: Option<String>,
        /// Owner-private relay static-secret hex file. Use `-` for standard input.
        #[arg(long)]
        relay_static_sk_file: Option<PathBuf>,
    },
    /// Generate or verify the reference handshake fixtures.
    Fixtures {
        /// Output directory for capability fixtures (defaults to fixtures/soranet_handshake/capabilities)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Verify existing fixtures instead of regenerating them.
        #[arg(long, default_value_t = false)]
        verify: bool,
    },
    /// Validate ML-KEM key material against the configured suite.
    KemValidate {
        /// ML-KEM identifier (decimal). Required unless `--kem-suite` is supplied.
        #[arg(long)]
        kem_id: Option<u8>,
        /// ML-KEM suite label (e.g., mlkem768); overrides `--kem-id` when provided.
        #[arg(long = "kem-suite")]
        kem_suite: Option<String>,
        /// Public key bytes to validate (hex).
        #[arg(long)]
        public_hex: Option<String>,
        /// Owner-private secret-key hex file. Use `-` for standard input.
        #[arg(long)]
        secret_file: Option<PathBuf>,
        /// Ciphertext bytes to validate (hex).
        #[arg(long)]
        ciphertext_hex: Option<String>,
    },
    /// Run the Noise XX handshake simulation pipeline (work in progress).
    Simulate {
        /// Client capability vector (hex)
        #[arg(long)]
        client_hex: String,
        /// Relay capability vector (hex)
        #[arg(long)]
        relay_hex: String,
        /// Owner-private client static-secret hex file. Use `-` for standard input.
        #[arg(long)]
        client_static_sk_file: PathBuf,
        /// Owner-private relay static-secret hex file. Use `-` for standard input.
        #[arg(long)]
        relay_static_sk_file: PathBuf,
        /// Optional resume hash (hex)
        #[arg(long)]
        resume_hash_hex: Option<HexInput>,
        /// Descriptor commitment (hex)
        #[arg(long)]
        descriptor_commit_hex: String,
        /// Client nonce (hex)
        #[arg(long)]
        client_nonce_hex: String,
        /// Relay nonce (hex)
        #[arg(long)]
        relay_nonce_hex: String,
        /// KEM identifier (decimal)
        #[arg(long)]
        kem_id: u8,
        /// ML-KEM suite label (e.g., mlkem768); overrides `--kem-id` when provided.
        #[arg(long = "kem-suite")]
        kem_suite: Option<String>,
        /// Signature identifier (decimal)
        #[arg(long)]
        sig_id: u8,
        /// Optional path to write a JSON report (use '-' for stdout)
        #[arg(long)]
        json_out: Option<PathBuf>,
        /// Optional directory to dump binary handshake frames.
        #[arg(long)]
        frames_out: Option<PathBuf>,
        /// Optional path to write the first telemetry payload JSON.
        #[arg(long)]
        telemetry_out: Option<PathBuf>,
        /// Print placeholder handshake steps to stdout.
        #[arg(long, default_value_t = false)]
        show_steps: bool,
        /// Only report warnings/capabilities for the specified types (hex like 0x0101 or decimal).
        #[arg(long = "only-capability")]
        only_capabilities: Vec<String>,
    },
}
fn main() -> Result<(), HarnessError> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Inspect {
            client_hex,
            relay_hex,
            descriptor_commit_hex,
            client_nonce_hex,
            relay_nonce_hex,
            kem_id,
            kem_suite,
            sig_id,
            resume_hash_hex,
        } => {
            let client_bytes = decode_hex(&client_hex)?;
            let relay_bytes = decode_hex(&relay_hex)?;
            let desc_commit = decode_hex(&descriptor_commit_hex)?;
            let client_nonce = decode_hex(&client_nonce_hex)?;
            let relay_nonce = decode_hex(&relay_nonce_hex)?;
            let resume_hash = resume_hash_hex.map(|h| h.0);
            let kem_id = resolve_kem_id(kem_id, kem_suite.as_deref())?;
            let client_caps = parse_capabilities(&client_bytes)?;
            let relay_caps = parse_capabilities(&relay_bytes)?;
            println!(
                "Client capabilities:\n{}",
                format_capabilities(&client_caps)
            );
            println!("Relay capabilities:\n{}", format_capabilities(&relay_caps));
            let handshake_suite = negotiate_handshake_suite(&client_caps, &relay_caps)?;
            let transcript = TranscriptInputs {
                descriptor_commit: &desc_commit,
                client_nonce: &client_nonce,
                relay_nonce: &relay_nonce,
                capability_bytes: &client_bytes, // include client TLVs per draft; harness will evolve
                kem_id,
                sig_id,
                handshake_suite,
                resume_hash: resume_hash.as_deref(),
            };
            let hash = transcript.compute_hash()?;
            println!("Transcript hash: 0x{}", hex::encode(hash));
            println!("Transcript handshake suite: {handshake_suite}");
            println!(
                "Selected ML-KEM suite: {}",
                mlkem_suite_from_id(kem_id)
                    .map(|suite| suite.to_string())
                    .unwrap_or_else(|| format!("unknown({kem_id})"))
            );
            let warnings = diff_capabilities(&client_caps, &relay_caps);
            if warnings.is_empty() {
                println!("All required capabilities satisfied.");
            } else {
                for warning in warnings {
                    println!("warning: {warning:?}");
                }
            }
        }
        Commands::Summary { vector_hex } => {
            let bytes = decode_hex(&vector_hex)?;
            let caps = parse_capabilities(&bytes)?;
            let summary = CapabilitySummary { tlvs: &caps };
            println!(
                "{}",
                summary.to_pretty_json().unwrap_or_else(|_| "{}".into())
            );
        }
        Commands::Salt {
            epoch_id,
            valid_after,
            valid_until,
            salt_hex,
            previous_epoch,
            emergency,
            notes,
        } => {
            let salt = decode_salt_hex(&salt_hex)?;
            let json = salt_announcement_json(&SaltAnnouncementParams {
                epoch_id,
                previous_epoch,
                valid_after: &valid_after,
                valid_until: &valid_until,
                blinded_cid_salt: &salt,
                emergency_rotation: emergency,
                notes: notes.as_deref(),
                signature: None,
            })?;
            println!("{json}");
        }
        Commands::SaltVerify { vector } => {
            let validation = verify_salt_vector(&vector)?;
            println!(
                "salt vector {} OK (epoch {}, signature: {})",
                vector.display(),
                validation.epoch_id,
                if validation.has_signature {
                    "present"
                } else {
                    "missing"
                }
            );
        }
        Commands::Telemetry {
            epoch,
            downgrade_attempts,
            pq_disabled_sessions,
            cover_ratio,
            lagging_clients,
            max_latency_ms,
            incident_reference,
            signature,
            witness_signature,
            relay_static_sk_file,
        } => {
            let signing_key = relay_static_sk_file
                .as_deref()
                .map(|path| load_static_secret(path, "relay static secret"))
                .transpose()?;
            let signature_ref = if signing_key.is_some() {
                None
            } else {
                signature.as_deref()
            };
            let witness_signature_ref = if signing_key.is_some() {
                None
            } else {
                witness_signature.as_deref()
            };
            let json = soranet_telemetry_json(
                &TelemetryReport {
                    epoch,
                    downgrade_attempts,
                    pq_disabled_sessions,
                    cover_ratio,
                    lagging_clients,
                    max_latency_ms,
                    incident_reference: incident_reference.as_deref(),
                    signature: signature_ref,
                    witness_signature: witness_signature_ref,
                },
                signing_key.as_ref().map(StaticSecretKey::as_array),
            )?;
            println!("{json}");
        }
        Commands::Fixtures { out, verify } => {
            let default = env::current_dir()?.join("fixtures/soranet_handshake/capabilities");
            let target = out.unwrap_or(default);
            if verify {
                harness_verify(&target)?;
            } else {
                harness_generate(&target)?;
            }
        }
        Commands::KemValidate {
            kem_id,
            kem_suite,
            public_hex,
            secret_file,
            ciphertext_hex,
        } => {
            let (resolved_id, suite) = resolve_kem_suite(kem_id, kem_suite.as_deref())?;
            let public = decode_optional_hex("public key", public_hex)?;
            let secret = secret_file
                .as_deref()
                .map(|path| {
                    decode_secret_hex_source(path, suite.secret_key_len(), "ML-KEM secret key")
                })
                .transpose()?;
            let ciphertext = decode_optional_hex("ciphertext", ciphertext_hex)?;
            let results = run_kem_validation(
                suite,
                public.as_deref(),
                secret.as_ref().map(SecretBytes::as_slice),
                ciphertext.as_deref(),
            )?;
            println!("ML-KEM suite {suite} (id {resolved_id}) validation succeeded.");
            for line in results {
                println!("  - {line}");
            }
        }
        Commands::Simulate {
            client_hex,
            relay_hex,
            client_static_sk_file,
            relay_static_sk_file,
            resume_hash_hex,
            descriptor_commit_hex,
            client_nonce_hex,
            relay_nonce_hex,
            kem_id,
            kem_suite,
            sig_id,
            json_out,
            frames_out,
            telemetry_out,
            show_steps,
            only_capabilities,
        } => {
            let client_caps = decode_hex(&client_hex)?;
            let relay_caps = decode_hex(&relay_hex)?;
            validate_simulation_secret_sources(&client_static_sk_file, &relay_static_sk_file)?;
            let client_sk = load_static_secret(&client_static_sk_file, "client static secret")?;
            let relay_sk = load_static_secret(&relay_static_sk_file, "relay static secret")?;
            let resume_hash = resume_hash_hex.map(|h| h.0);
            let descriptor_commit = decode_hex(&descriptor_commit_hex)?;
            let client_nonce = decode_hex(&client_nonce_hex)?;
            let relay_nonce = decode_hex(&relay_nonce_hex)?;
            let kem_id = resolve_kem_id(kem_id, kem_suite.as_deref())?;
            let capability_filter = parse_capability_filters(&only_capabilities)?;
            let capability_filter_vec = capability_filter
                .as_ref()
                .map(|set| set.iter().copied().collect::<Vec<_>>());
            let result = simulate_handshake(&SimulationParams {
                client_capabilities: &client_caps,
                relay_capabilities: &relay_caps,
                client_static_sk: client_sk.as_array(),
                relay_static_sk: relay_sk.as_array(),
                resume_hash: resume_hash.as_deref(),
                descriptor_commit: &descriptor_commit,
                client_nonce: &client_nonce,
                relay_nonce: &relay_nonce,
                kem_id,
                sig_id,
            })?;
            println!("Transcript hash: 0x{}", hex::encode(result.transcript_hash));
            println!("Negotiated handshake suite: {}", result.handshake_suite);
            println!(
                "Configured ML-KEM suite: {}",
                mlkem_suite_from_id(kem_id)
                    .map(|suite| suite.to_string())
                    .unwrap_or_else(|| format!("unknown({kem_id})"))
            );
            let filtered_warnings: Vec<_> = result
                .warnings
                .iter()
                .filter(|warning| {
                    capability_filter
                        .as_ref()
                        .map(|set| set.contains(&warning.capability_type))
                        .unwrap_or(true)
                })
                .collect();
            if filtered_warnings.is_empty() {
                if capability_filter.is_some() && !result.warnings.is_empty() {
                    println!("No warnings matched the provided capability filter.");
                } else if result.warnings.is_empty() {
                    println!("No warnings generated during simulation.");
                }
            } else {
                for warning in filtered_warnings {
                    println!("warning: {}", warning.message);
                }
            }
            if result.telemetry_payloads.is_empty() {
                println!("No telemetry payloads generated.");
            } else {
                println!(
                    "Generated {} telemetry payload(s).",
                    result.telemetry_payloads.len()
                );
            }
            if show_steps {
                println!("Handshake steps:");
                for step in &result.handshake_steps {
                    println!(
                        "  - {role}::{action}: 0x{msg}",
                        role = step.role,
                        action = step.action,
                        msg = step.message_hex
                    );
                    println!("    note: {}", step.note);
                }
            }
            if let Some(dir) = frames_out {
                fs::create_dir_all(&dir)?;
                for step in &result.handshake_steps {
                    let filename = format!(
                        "{}_{}.bin",
                        step.role.to_lowercase(),
                        step.action.to_lowercase()
                    );
                    let bytes = hex::decode(&step.message_hex).map_err(|err| {
                        HarnessError::Validation(format!(
                            "failed to decode {}/{} frame: {err}",
                            step.role, step.action
                        ))
                    })?;
                    fs::write(dir.join(filename), bytes)?;
                }
                println!("wrote handshake frames to {}", dir.display());
            }
            if let Some(path) = json_out {
                let json = simulation_report_json(&result, capability_filter_vec.as_deref())?;
                if path == std::path::Path::new("-") {
                    println!("{json}");
                } else {
                    fs::write(&path, json + "\n")?;
                    println!("wrote {}", path.display());
                }
            }
            if let Some(path) = telemetry_out {
                if result.telemetry_payloads.is_empty() {
                    println!("No telemetry payloads available to write.");
                } else {
                    let mut payload = result.telemetry_payloads[0].clone();
                    if !payload.ends_with(b"\n") {
                        payload.push(b'\n');
                    }
                    fs::write(&path, payload)?;
                    println!("wrote telemetry {}", path.display());
                }
            }
        }
    }
    Ok(())
}
fn parse_capability_filters(
    values: &[String],
) -> Result<Option<std::collections::BTreeSet<u16>>, HarnessError> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut out = std::collections::BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        let ty = if let Some(rest) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            u16::from_str_radix(rest, 16)
        } else {
            trimmed.parse::<u16>()
        }
        .map_err(|_| HarnessError::CapabilityType(trimmed.to_string()))?;
        out.insert(ty);
    }
    Ok(Some(out))
}
const CAPABILITY_SUITE_LIST: u16 = 0x0104;
fn suite_list_from_caps(
    caps: &[CapabilityTlv],
) -> Result<Option<Vec<HandshakeSuite>>, HarnessError> {
    let cap = caps.iter().find(|cap| cap.ty == CAPABILITY_SUITE_LIST);
    let Some(cap) = cap else {
        return Ok(None);
    };
    if cap.value.is_empty() {
        return Err(HarnessError::Validation(
            "suite_list capability must contain at least one identifier".into(),
        ));
    }
    let mut suites = Vec::with_capacity(cap.value.len());
    let mut ignored = Vec::new();
    let mut pre_release = Vec::new();
    for &raw in &cap.value {
        match HandshakeSuite::try_from(raw) {
            Ok(suite) => {
                if !suites.contains(&suite) {
                    suites.push(suite);
                }
            }
            Err(_) if matches!(raw, 0x02 | 0x03) => pre_release.push(raw),
            Err(_) => ignored.push(raw),
        }
    }
    if !pre_release.is_empty() {
        let rejected = pre_release
            .iter()
            .map(|id| format!("{id:#04x}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(HarnessError::Validation(format!(
            "pre-release handshake suite identifiers are not accepted: {rejected}"
        )));
    }
    if suites.is_empty() {
        let unsupported = ignored
            .iter()
            .map(|id| format!("{id:#04x}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(HarnessError::Validation(format!(
            "suite_list capability must include at least one supported identifier; got {unsupported}"
        )));
    }
    Ok(Some(suites))
}
fn describe_suites(suites: &[HandshakeSuite]) -> String {
    suites
        .iter()
        .map(|suite| suite.label())
        .collect::<Vec<_>>()
        .join(", ")
}
fn negotiate_handshake_suite(
    client_caps: &[CapabilityTlv],
    relay_caps: &[CapabilityTlv],
) -> Result<HandshakeSuite, HarnessError> {
    let client_list = suite_list_from_caps(client_caps)?;
    let relay_list = suite_list_from_caps(relay_caps)?;
    match (client_list, relay_list) {
        (Some(client), Some(relay)) => {
            for suite in &client {
                if relay.contains(suite) {
                    return Ok(*suite);
                }
            }
            Err(HarnessError::Validation(format!(
                "no overlapping handshake suite between client ({}) and relay ({})",
                describe_suites(&client),
                describe_suites(&relay)
            )))
        }
        (Some(client), None) => Err(HarnessError::Validation(format!(
            "relay omitted suite_list capability; client advertised {}",
            describe_suites(&client)
        ))),
        (None, Some(relay)) => Err(HarnessError::Validation(format!(
            "client omitted suite_list capability; relay advertised {}",
            describe_suites(&relay)
        ))),
        (None, None) => Err(HarnessError::Validation(
            "suite_list capability is required for handshake negotiation".into(),
        )),
    }
}
fn resolve_kem_id(base: u8, suite_label: Option<&str>) -> Result<u8, HarnessError> {
    if let Some(label) = suite_label {
        let suite = parse_kem_suite(label)?;
        let derived = suite_to_kem_id(suite);
        if derived != base {
            return Err(HarnessError::Validation(format!(
                "--kem-suite {label} maps to id {derived}, but --kem-id {base} was supplied"
            )));
        }
        Ok(derived)
    } else {
        Ok(base)
    }
}
fn resolve_kem_suite(
    kem_id: Option<u8>,
    suite_label: Option<&str>,
) -> Result<(u8, MlKemSuite), HarnessError> {
    match (kem_id, suite_label) {
        (Some(id), Some(label)) => {
            let suite = parse_kem_suite(label)?;
            let derived = suite_to_kem_id(suite);
            if derived != id {
                return Err(HarnessError::Validation(format!(
                    "--kem-suite {label} maps to id {derived}, but --kem-id {id} was supplied"
                )));
            }
            Ok((derived, suite))
        }
        (Some(id), None) => {
            let suite = mlkem_suite_from_id(id).ok_or_else(|| {
                HarnessError::Validation(format!("unsupported ML-KEM identifier {id}"))
            })?;
            Ok((id, suite))
        }
        (None, Some(label)) => {
            let suite = parse_kem_suite(label)?;
            let id = suite_to_kem_id(suite);
            Ok((id, suite))
        }
        (None, None) => Err(HarnessError::Validation(
            "specify either --kem-id or --kem-suite".to_string(),
        )),
    }
}
fn parse_kem_suite(label: &str) -> Result<MlKemSuite, HarnessError> {
    label
        .parse::<MlKemSuite>()
        .map_err(|SuiteParseError(value)| {
            HarnessError::Validation(format!("unsupported ML-KEM suite '{value}'"))
        })
}
fn suite_to_kem_id(suite: MlKemSuite) -> u8 {
    match suite {
        MlKemSuite::MlKem512 => 0,
        MlKemSuite::MlKem768 => 1,
        MlKemSuite::MlKem1024 => 2,
    }
}
fn mlkem_suite_from_id(id: u8) -> Option<MlKemSuite> {
    match id {
        0 => Some(MlKemSuite::MlKem512),
        1 => Some(MlKemSuite::MlKem768),
        2 => Some(MlKemSuite::MlKem1024),
        _ => None,
    }
}
fn decode_optional_hex(
    label: &str,
    input: Option<String>,
) -> Result<Option<Vec<u8>>, HarnessError> {
    match input {
        Some(value) => match decode_hex(&value) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) => Err(HarnessError::Validation(format!(
                "failed to decode {label}: {err}"
            ))),
        },
        None => Ok(None),
    }
}
fn run_kem_validation(
    suite: MlKemSuite,
    public: Option<&[u8]>,
    secret: Option<&[u8]>,
    ciphertext: Option<&[u8]>,
) -> Result<Vec<String>, HarnessError> {
    if public.is_none() && secret.is_none() && ciphertext.is_none() {
        return Err(HarnessError::Validation(
            "provide at least one of --public-hex, --secret-file, or --ciphertext-hex".into(),
        ));
    }
    let mut status = Vec::new();
    if let Some(bytes) = public {
        validate_mlkem_public_key(suite, bytes)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        status.push(format!("public key valid ({} bytes)", bytes.len()));
    }
    if let Some(bytes) = secret {
        validate_mlkem_secret_key(suite, bytes)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        status.push(format!("secret key valid ({} bytes)", bytes.len()));
    }
    if let Some(bytes) = ciphertext {
        validate_mlkem_ciphertext(suite, bytes)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        status.push(format!("ciphertext valid ({} bytes)", bytes.len()));
    }
    Ok(status)
}
#[cfg(test)]
mod tests {
    use super::*;
    use soranet_pq::{encapsulate_mlkem_from_os, generate_mlkem_keypair_from_os};

    #[test]
    fn cli_removes_secret_bearing_argv_options() {
        for argv in [
            vec![
                "soranet-handshake-harness",
                "telemetry",
                "--relay-static-sk-hex",
                "00",
            ],
            vec![
                "soranet-handshake-harness",
                "kem-validate",
                "--secret-hex",
                "00",
            ],
            vec![
                "soranet-handshake-harness",
                "simulate",
                "--client-static-sk-hex",
                "00",
            ],
            vec![
                "soranet-handshake-harness",
                "simulate",
                "--relay-static-sk-hex",
                "00",
            ],
        ] {
            let error = Cli::try_parse_from(argv)
                .expect_err("secret bytes must not be accepted as process arguments");
            assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        }
    }

    #[test]
    fn cli_accepts_secret_file_or_standard_input_path() {
        let cli = Cli::try_parse_from([
            "soranet-handshake-harness",
            "kem-validate",
            "--kem-id",
            "1",
            "--secret-file",
            "-",
        ])
        .expect("secret-file standard-input marker must parse");
        assert!(matches!(
            cli.command,
            Commands::KemValidate {
                secret_file: Some(path),
                ..
            } if path == Path::new("-")
        ));
    }

    #[test]
    fn simulation_accepts_at_most_one_secret_from_standard_input() {
        validate_simulation_secret_sources(Path::new("-"), Path::new("relay.hex"))
            .expect("one standard-input secret is unambiguous");
        let error = validate_simulation_secret_sources(Path::new("-"), Path::new("-"))
            .expect_err("one stream cannot frame two independent secret values");
        assert!(error.to_string().contains("cannot both read"));
    }

    #[test]
    fn bounded_secret_input_accepts_exact_and_rejects_plus_one() {
        let maximum = 2 + SECRET_FILE_MAX_SURROUNDING_WHITESPACE_BYTES_V1;
        let raw = read_bounded_secret_input(io::Cursor::new(b"ab\n"), maximum, "test secret")
            .expect("bounded input");
        let secret = decode_secret_hex_bytes(raw.as_slice(), 1, "test secret")
            .expect("hex secret must decode");
        assert_eq!(secret.as_slice(), [0xab]);

        let error = read_bounded_secret_input(
            io::Cursor::new(vec![b' '; maximum + 1]),
            maximum,
            "test secret",
        )
        .expect_err("input above the corridor must fail");
        assert!(error.to_string().contains("first-release input limit"));
    }

    #[cfg(unix)]
    #[test]
    fn static_secret_loader_rejects_degenerate_keys() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("temporary directory");
        for (name, byte) in [("zero.hex", "00"), ("repeated.hex", "a5")] {
            let path = dir.path().join(name);
            fs::write(&path, byte.repeat(STATIC_SECRET_KEY_BYTES)).expect("write static secret");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("make secret private");
            let error = match load_static_secret(&path, "test static secret") {
                Ok(_) => panic!("degenerate static secrets must fail closed"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("degenerate key"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn secret_file_requires_private_mode_direct_path_and_single_link() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("secret.hex");
        fs::write(&path, b"ab").expect("write secret fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("make secret fixture non-private");
        let error = decode_secret_hex_source(&path, 1, "test secret")
            .expect_err("group/world-readable secret must fail");
        assert!(error.to_string().contains("owner-private"));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("make secret fixture private");
        let secret = decode_secret_hex_source(&path, 1, "test secret")
            .expect("private direct file must load");
        assert_eq!(secret.as_slice(), [0xab]);

        let second_link = dir.path().join("secret-copy.hex");
        fs::hard_link(&path, &second_link).expect("create hard link");
        let error = decode_secret_hex_source(&path, 1, "test secret")
            .expect_err("multiply linked secret must fail");
        assert!(error.to_string().contains("exactly one link"));
        fs::remove_file(&second_link).expect("remove hard link");

        let link = dir.path().join("secret-link.hex");
        symlink(&path, &link).expect("create symbolic link");
        let error = decode_secret_hex_source(&link, 1, "test secret")
            .expect_err("symbolic-link secret must fail");
        assert!(error.to_string().contains("direct regular file"));
    }

    #[test]
    fn run_kem_validation_accepts_valid_materials() {
        let suite = MlKemSuite::MlKem768;
        let keys = generate_mlkem_keypair_from_os(suite).expect("ML-KEM keypair");
        let (_, ciphertext) = encapsulate_mlkem_from_os(suite, keys.public_key()).unwrap();
        let messages = run_kem_validation(
            suite,
            Some(keys.public_key()),
            Some(keys.secret_key()),
            Some(ciphertext.as_bytes()),
        )
        .expect("validation should pass");
        assert_eq!(messages.len(), 3);
        assert!(messages.iter().all(|msg| msg.contains("valid")));
    }
    #[test]
    fn run_kem_validation_rejects_invalid_public_key() {
        let err =
            run_kem_validation(MlKemSuite::MlKem512, Some(&[0u8; 8]), None, None).unwrap_err();
        assert!(matches!(err, HarnessError::Kem(_)));
    }
    #[test]
    fn run_kem_validation_requires_material() {
        let err = run_kem_validation(MlKemSuite::MlKem512, None, None, None).unwrap_err();
        assert!(matches!(err, HarnessError::Validation(message) if message.contains("provide")));
    }
    #[test]
    fn resolve_kem_suite_from_label_only() {
        let (id, suite) = resolve_kem_suite(None, Some("mlkem1024")).expect("suite should resolve");
        assert_eq!(id, 2);
        assert_eq!(suite, MlKemSuite::MlKem1024);
    }
    #[test]
    fn resolve_kem_suite_rejects_mismatch() {
        let err = resolve_kem_suite(Some(0), Some("mlkem768")).unwrap_err();
        assert!(matches!(err, HarnessError::Validation(message) if message.contains("maps to id")));
    }
    #[test]
    fn suite_list_from_caps_dedupes_entries() {
        let caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk2Hybrid),
                0x7D,
                u8::from(HandshakeSuite::Nk2Hybrid),
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
                0x7E,
            ],
            required: false,
        }];
        let suites = suite_list_from_caps(&caps)
            .expect("suite list")
            .expect("suite list present");
        assert_eq!(
            suites,
            vec![
                HandshakeSuite::Nk2Hybrid,
                HandshakeSuite::Nk3PqForwardSecure
            ]
        );
    }
    #[test]
    fn suite_list_from_caps_rejects_only_pre_release_ids() {
        let caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![0x02, 0x03],
            required: true,
        }];
        let err = suite_list_from_caps(&caps)
            .expect_err("pre-release-only suite list must not negotiate");
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("pre-release handshake suite identifiers"));
                assert!(message.contains("0x02"));
                assert!(message.contains("0x03"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }
    #[test]
    fn negotiate_handshake_suite_prefers_client_order() {
        let client_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
                u8::from(HandshakeSuite::Nk2Hybrid),
            ],
            required: true,
        }];
        let relay_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk2Hybrid),
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
            ],
            required: true,
        }];
        let selected =
            negotiate_handshake_suite(&client_caps, &relay_caps).expect("suite negotiated");
        assert_eq!(selected, HandshakeSuite::Nk3PqForwardSecure);
    }
}

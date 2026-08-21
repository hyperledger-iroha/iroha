//! Native pre-exec authentication for Kagemusha's Python release entrypoints.
//!
//! Python cannot authenticate the interpreter and standard library that have
//! already initialized it.  These launchers therefore close that trust boundary
//! in the dependency-free native controller before either the sealed builder or
//! the production readiness gate enters Python.

use super::{ControllerError, Result, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs,
    path::{Component, Path, PathBuf},
};

const RUNTIME_PARENT: &str = "/private/var/db/iroha-kagemusha-python-runtime-v1";
const REPORT_PARENT: &str = "/private/var/db/iroha-kagemusha-build-reports-v1";
const READINESS_STAGING_PARENT: &str = "/private/var/db/iroha-kagemusha-readiness-v1";
const PYTHON_RELATIVE: &str = "bin/python3";
const BUILDER_RELATIVE: &str = "scripts/build_kagemusha_v4_candidate_bundle.py";
const READINESS_CONTRACT: &str = "iroha.kagemusha.native-python-readiness-launch.v1";
const BUILDER_CONTRACT: &str = "iroha.kagemusha.native-sealed-builder-launch.v1";
const BUILDER_ARGUMENT_CONTRACT: &str = "iroha.kagemusha.sealed-builder-exact-arguments.v1";
const BUILDER_ENVIRONMENT_CONTRACT: &str = "iroha.kagemusha.sealed-builder-exact-environment.v1";
const BUILDER_REPORT_SCHEMA: &str =
    "iroha.kagemusha.native_sealed_candidate_double_build_report.v2";
const BUILDER_INNER_REPORT_SCHEMA: &str = "iroha.kagemusha.sealed_candidate_double_build_report.v1";
const REPORT_PUBLICATION_CONTRACT: &str = "iroha.kagemusha.native-no-replace-report-publication.v1";
const RUNTIME_DEPENDENCY_CONTRACT: &str = "iroha.kagemusha.symlink-free-macho-runtime-closure.v1";
const OS_TCB_CONTRACT: &str = "iroha.kagemusha.macos-os-library-tcb.v1";
const MAX_RUNTIME_RECORDS: usize = 250_000;
const MAX_RUNTIME_FILE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_RUNTIME_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_MACHO_IMAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MACHO_IMAGES: usize = 4096;
const MAX_MACHO_SLICES: usize = 32;
const MAX_LOAD_COMMANDS: usize = 8192;
const MAX_BUILDER_REPORT_BYTES: u64 = 384 * 1024;
const MAX_BUILDER_STDERR_BYTES: u64 = 8 * 1024 * 1024;
const MAX_READINESS_STDOUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_READINESS_STDERR_BYTES: u64 = 16 * 1024 * 1024;
const BUILDER_WALL_SECONDS: u64 = 3900;
const READINESS_WALL_SECONDS: u64 = 10_200;

const OS_LIBRARY_ROOTS: [&str; 3] = [
    "/usr/lib",
    "/System/Library",
    "/System/Volumes/Preboot/Cryptexes/OS/System/Library",
];

const BUILDER_ARGUMENT_NAMES: [&str; 17] = [
    "--root",
    "--cargo",
    "--cargo-sha256",
    "--rustc",
    "--rustc-sha256",
    "--cargo-home",
    "--runtime-uid",
    "--runtime-gid",
    "--target-dir",
    "--reviewed-source-closure",
    "--reviewed-source-closure-sha256",
    "--authenticated-source-seal-projection",
    "--authenticated-source-seal-projection-sha256",
    "--raw-unit-graph",
    "--raw-unit-graph-sha256",
    "--normalized-unit-graph",
    "--normalized-unit-graph-sha256",
];

#[cfg(any(target_os = "macos", test))]
const READINESS_BASE_ENVIRONMENT: &[(&str, &str)] = &[
    ("LANG", "C"),
    ("LC_ALL", "C"),
    ("PATH", "/usr/bin:/bin"),
    ("TMPDIR", "/private/var/tmp"),
];

// This is deliberately the complete caller-visible promotion configuration.
// The native controller rejects additions as well as omissions before any
// interpreter or shell is started. Values used only to select the authenticated
// launch are removed again before the gate child is executed.
#[cfg(any(target_os = "macos", test))]
const READINESS_EXTERNAL_ENVIRONMENT_NAMES: &[&str] = &[
    "KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH",
    "KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256",
    "KAGEMUSHA_PRODUCTION_READINESS_PYTHON",
    "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256",
    "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT",
    "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256",
    "KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD",
    "KAGEMUSHA_V4_RELEASE_POLICY_PATH",
    "KAGEMUSHA_V4_ARTIFACT_ROOT",
    "KAGEMUSHA_V4_KAGAMI_BIN",
    "KAGEMUSHA_V4_KAGAMI_SHA256",
    "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN",
    "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256",
    "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE",
    "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256",
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH",
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256",
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH",
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256",
    "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION",
    "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH_SHA256",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH",
    "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH_SHA256",
    "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_PATH",
    "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_SHA256",
    "KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT",
    "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID",
    "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY",
    "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY",
    "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID",
    "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY",
];

#[cfg(target_os = "macos")]
const READINESS_CALLER_ONLY_ENVIRONMENT_NAMES: &[&str] = &[
    "KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH",
    "KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD",
];

#[derive(Debug)]
struct CommonLaunch {
    runtime_root: PathBuf,
    runtime_tree_sha256: [u8; 32],
    python_sha256: [u8; 32],
    expected_macos_build: String,
}

#[derive(Debug)]
struct ReadinessLaunch {
    common: CommonLaunch,
    gate_source: PathBuf,
    gate_snapshot: PathBuf,
    gate_sha256: [u8; 32],
}

#[derive(Debug)]
struct BuilderLaunch {
    common: CommonLaunch,
    builder: PathBuf,
    builder_sha256: [u8; 32],
    report_output: PathBuf,
    builder_arguments: Vec<OsString>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MachoImage {
    dependencies: Vec<String>,
    dylib_id: Option<String>,
    executable: bool,
    rpaths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MachoSlice {
    dependencies: Vec<String>,
    dylib_id: Option<String>,
    executable: bool,
    rpaths: Vec<String>,
}

pub(super) fn launch_readiness(arguments: &[OsString]) -> Result<u8> {
    #[cfg(target_os = "macos")]
    {
        return launch_readiness_macos(parse_readiness(arguments)?);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = arguments;
        Err(ControllerError::policy(
            "Kagemusha native Python launch is available only on a qualified macOS host",
        ))
    }
}

pub(super) fn launch_sealed_builder(arguments: &[OsString]) -> Result<u8> {
    #[cfg(target_os = "macos")]
    {
        return launch_builder_macos(parse_builder(arguments)?);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = arguments;
        Err(ControllerError::policy(
            "Kagemusha native Python launch is available only on a qualified macOS host",
        ))
    }
}

fn parse_readiness(arguments: &[OsString]) -> Result<ReadinessLaunch> {
    let (values, trailing) = parse_options(arguments)?;
    if !trailing.is_empty() {
        return Err(ControllerError::policy(
            "readiness launcher accepts no trailing command",
        ));
    }
    require_exact_options(
        &values,
        &[
            "--expected-macos-build",
            "--gate-sha256",
            "--gate-snapshot",
            "--gate-source",
            "--python-runtime-root",
            "--python-runtime-tree-sha256",
            "--python-sha256",
        ],
        "readiness launcher",
    )?;
    Ok(ReadinessLaunch {
        common: parse_common(&values)?,
        gate_source: normalized_absolute_path(required(&values, "--gate-source")?)?,
        gate_snapshot: normalized_absolute_path(required(&values, "--gate-snapshot")?)?,
        gate_sha256: parse_sha256(required(&values, "--gate-sha256")?, "gate SHA-256")?,
    })
}

fn parse_builder(arguments: &[OsString]) -> Result<BuilderLaunch> {
    let (values, trailing) = parse_options(arguments)?;
    require_exact_options(
        &values,
        &[
            "--builder",
            "--builder-sha256",
            "--expected-macos-build",
            "--python-runtime-root",
            "--python-runtime-tree-sha256",
            "--python-sha256",
            "--report-output",
        ],
        "sealed-builder launcher",
    )?;
    validate_builder_arguments(&trailing)?;
    Ok(BuilderLaunch {
        common: parse_common(&values)?,
        builder: normalized_absolute_path(required(&values, "--builder")?)?,
        builder_sha256: parse_sha256(required(&values, "--builder-sha256")?, "builder SHA-256")?,
        report_output: normalized_absolute_path(required(&values, "--report-output")?)?,
        builder_arguments: trailing,
    })
}

fn parse_common(values: &BTreeMap<String, String>) -> Result<CommonLaunch> {
    let runtime_root = normalized_absolute_path(required(values, "--python-runtime-root")?)?;
    if runtime_root.parent() != Some(Path::new(RUNTIME_PARENT)) {
        return Err(ControllerError::policy(
            "Python runtime is outside its fixed sealed parent",
        ));
    }
    let expected_macos_build = required(values, "--expected-macos-build")?.to_owned();
    if expected_macos_build.is_empty()
        || expected_macos_build.len() > 64
        || !expected_macos_build
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ControllerError::policy(
            "expected macOS build is not a portable identifier",
        ));
    }
    Ok(CommonLaunch {
        runtime_root,
        runtime_tree_sha256: parse_sha256(
            required(values, "--python-runtime-tree-sha256")?,
            "Python runtime-tree SHA-256",
        )?,
        python_sha256: parse_sha256(
            required(values, "--python-sha256")?,
            "Python interpreter SHA-256",
        )?,
        expected_macos_build,
    })
}

fn parse_options(arguments: &[OsString]) -> Result<(BTreeMap<String, String>, Vec<OsString>)> {
    if arguments.len() > 128 {
        return Err(ControllerError::policy(
            "native Python launcher has too many arguments",
        ));
    }
    let separator = arguments
        .iter()
        .position(|argument| argument == "--")
        .unwrap_or(arguments.len());
    if separator % 2 != 0 {
        return Err(ControllerError::policy(
            "native Python launcher options are not name/value pairs",
        ));
    }
    let mut values = BTreeMap::new();
    for pair in arguments[..separator].chunks_exact(2) {
        let name = pair[0]
            .to_str()
            .ok_or_else(|| ControllerError::policy("launcher option name is not UTF-8"))?;
        let value = pair[1]
            .to_str()
            .ok_or_else(|| ControllerError::policy("launcher option value is not UTF-8"))?;
        if !name.starts_with("--") || value.as_bytes().contains(&0) {
            return Err(ControllerError::policy(
                "native Python launcher option is malformed",
            ));
        }
        if values.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(ControllerError::policy(format!(
                "duplicate native Python launcher option {name}"
            )));
        }
    }
    let trailing = if separator == arguments.len() {
        Vec::new()
    } else {
        arguments[separator + 1..].to_vec()
    };
    Ok((values, trailing))
}

fn require_exact_options(
    values: &BTreeMap<String, String>,
    expected: &[&str],
    label: &str,
) -> Result<()> {
    let actual = values.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(ControllerError::policy(format!(
            "{label} options are not exact"
        )));
    }
    Ok(())
}

fn required<'a>(values: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str> {
    values
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| ControllerError::policy(format!("missing launcher option {name}")))
}

fn normalized_absolute_path(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || value.len() > 4096
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(ControllerError::policy(
            "launcher path is not one normalized absolute path",
        ));
    }
    Ok(path)
}

fn parse_sha256(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64 || value == "0".repeat(64) || !value.bytes().all(is_lower_hex) {
        return Err(ControllerError::policy(format!("{label} is malformed")));
    }
    let mut digest = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(digest)
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    result
}

fn validate_builder_arguments(arguments: &[OsString]) -> Result<()> {
    if arguments.len() != BUILDER_ARGUMENT_NAMES.len() * 2 {
        return Err(ControllerError::policy(
            "sealed-builder arguments are not the exact reviewed set",
        ));
    }
    let mut values = BTreeMap::new();
    for (index, name) in BUILDER_ARGUMENT_NAMES.iter().enumerate() {
        if arguments[index * 2] != *name {
            return Err(ControllerError::policy(
                "sealed-builder argument order differs from its reviewed contract",
            ));
        }
        let value = arguments[index * 2 + 1]
            .to_str()
            .ok_or_else(|| ControllerError::policy("sealed-builder argument is not UTF-8"))?;
        if value.as_bytes().contains(&0) {
            return Err(ControllerError::policy(
                "sealed-builder argument contains NUL",
            ));
        }
        values.insert(*name, value);
    }
    for name in [
        "--root",
        "--cargo",
        "--rustc",
        "--cargo-home",
        "--target-dir",
        "--reviewed-source-closure",
        "--authenticated-source-seal-projection",
        "--raw-unit-graph",
        "--normalized-unit-graph",
    ] {
        normalized_absolute_path(values[name])?;
    }
    for name in [
        "--cargo-sha256",
        "--rustc-sha256",
        "--reviewed-source-closure-sha256",
        "--authenticated-source-seal-projection-sha256",
        "--raw-unit-graph-sha256",
        "--normalized-unit-graph-sha256",
    ] {
        parse_sha256(values[name], name)?;
    }
    for name in ["--runtime-uid", "--runtime-gid"] {
        let value = values[name];
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || value
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0)
                .is_none()
        {
            return Err(ControllerError::policy(format!(
                "sealed-builder {name} is invalid"
            )));
        }
    }
    Ok(())
}

fn digest_strings(contract: &str, values: impl IntoIterator<Item = impl AsRef<[u8]>>) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(contract.as_bytes());
    hash.update(&[0]);
    for value in values {
        hash.update(value.as_ref());
        hash.update(&[0]);
    }
    hash.finish()
}

fn builder_argument_digest(arguments: &[OsString]) -> [u8; 32] {
    digest_strings(
        BUILDER_ARGUMENT_CONTRACT,
        arguments.iter().map(|value| value.as_encoded_bytes()),
    )
}

fn builder_environment() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("HOME", "/var/empty"),
        ("LANG", "C"),
        ("LC_ALL", "C"),
        ("PATH", "/usr/bin:/bin"),
        ("PYTHONDONTWRITEBYTECODE", "1"),
        ("TMPDIR", "/private/var/tmp"),
        ("TZ", "UTC"),
    ])
}

fn builder_environment_digest() -> [u8; 32] {
    digest_strings(
        BUILDER_ENVIRONMENT_CONTRACT,
        builder_environment()
            .into_iter()
            .flat_map(|(name, value)| [name.as_bytes(), value.as_bytes()]),
    )
}

#[cfg(any(target_os = "macos", test))]
fn validate_exact_environment(
    entries: Vec<(OsString, OsString)>,
    external_names: &[&str],
    label: &str,
) -> Result<BTreeMap<OsString, OsString>> {
    let expected = READINESS_BASE_ENVIRONMENT
        .iter()
        .map(|(name, _)| OsString::from(*name))
        .chain(external_names.iter().map(|name| OsString::from(*name)))
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeMap::new();
    for (name, value) in entries {
        let display_name = name
            .to_str()
            .ok_or_else(|| ControllerError::policy(format!("{label} name is not UTF-8")))?
            .to_owned();
        if value.to_str().is_none() || value.as_encoded_bytes().contains(&0) {
            return Err(ControllerError::policy(format!(
                "{label} value for {display_name} is not canonical UTF-8"
            )));
        }
        if actual.insert(name, value).is_some() {
            return Err(ControllerError::policy(format!(
                "{label} contains a duplicate variable {display_name}"
            )));
        }
    }
    let actual_names = actual.keys().cloned().collect::<BTreeSet<_>>();
    if actual_names != expected {
        return Err(ControllerError::policy(format!(
            "{label} variable inventory is not exact"
        )));
    }
    for &(name, value) in READINESS_BASE_ENVIRONMENT {
        if actual.get(OsStr::new(name)).map(OsString::as_os_str) != Some(OsStr::new(value)) {
            return Err(ControllerError::policy(format!(
                "{label} {name} is not exact"
            )));
        }
    }
    Ok(actual)
}

#[cfg(target_os = "macos")]
fn require_environment_value(
    environment: &BTreeMap<OsString, OsString>,
    name: &str,
    expected: &OsStr,
    label: &str,
) -> Result<()> {
    if environment.get(OsStr::new(name)).map(OsString::as_os_str) != Some(expected) {
        return Err(ControllerError::policy(format!(
            "{label} {name} differs from the authenticated launch argument"
        )));
    }
    Ok(())
}

fn macho(bytes: &[u8]) -> Result<Option<MachoImage>> {
    if bytes.len() < 4 {
        return Ok(None);
    }
    let magic: [u8; 4] = bytes[..4].try_into().expect("four-byte prefix");
    let slices = match magic {
        [0xfe, 0xed, 0xfa, 0xce] => vec![parse_macho_slice(bytes, false, false)?],
        [0xce, 0xfa, 0xed, 0xfe] => vec![parse_macho_slice(bytes, true, false)?],
        [0xfe, 0xed, 0xfa, 0xcf] => vec![parse_macho_slice(bytes, false, true)?],
        [0xcf, 0xfa, 0xed, 0xfe] => vec![parse_macho_slice(bytes, true, true)?],
        [0xca, 0xfe, 0xba, 0xbe] => parse_fat_slices(bytes, false, false)?,
        [0xbe, 0xba, 0xfe, 0xca] => parse_fat_slices(bytes, true, false)?,
        [0xca, 0xfe, 0xba, 0xbf] => parse_fat_slices(bytes, false, true)?,
        [0xbf, 0xba, 0xfe, 0xca] => parse_fat_slices(bytes, true, true)?,
        _ => return Ok(None),
    };
    let first = slices
        .first()
        .ok_or_else(|| ControllerError::policy("Mach-O contains no architecture slices"))?;
    if slices.iter().any(|slice| slice != first) {
        return Err(ControllerError::policy(
            "Mach-O architecture slices disagree on loader semantics",
        ));
    }
    Ok(Some(MachoImage {
        dependencies: first.dependencies.clone(),
        dylib_id: first.dylib_id.clone(),
        executable: first.executable,
        rpaths: first.rpaths.clone(),
    }))
}

fn parse_fat_slices(bytes: &[u8], little: bool, wide: bool) -> Result<Vec<MachoSlice>> {
    let count = usize::try_from(read_u32(bytes, 4, little)?)
        .map_err(|_| ControllerError::policy("fat Mach-O slice count is invalid"))?;
    if count == 0 || count > MAX_MACHO_SLICES {
        return Err(ControllerError::policy(
            "fat Mach-O slice count is outside its bound",
        ));
    }
    let entry_size = if wide { 32usize } else { 20usize };
    let table_end = 8usize
        .checked_add(
            count
                .checked_mul(entry_size)
                .ok_or_else(|| ControllerError::policy("fat Mach-O table overflows"))?,
        )
        .ok_or_else(|| ControllerError::policy("fat Mach-O table overflows"))?;
    if table_end > bytes.len() {
        return Err(ControllerError::policy("fat Mach-O table is truncated"));
    }
    let mut ranges = Vec::with_capacity(count);
    let mut slices = Vec::with_capacity(count);
    for index in 0..count {
        let base = 8 + index * entry_size;
        let (offset, size) = if wide {
            (
                read_u64(bytes, base + 8, little)?,
                read_u64(bytes, base + 16, little)?,
            )
        } else {
            (
                u64::from(read_u32(bytes, base + 8, little)?),
                u64::from(read_u32(bytes, base + 12, little)?),
            )
        };
        let start = usize::try_from(offset)
            .map_err(|_| ControllerError::policy("fat Mach-O slice offset is invalid"))?;
        let size = usize::try_from(size)
            .map_err(|_| ControllerError::policy("fat Mach-O slice size is invalid"))?;
        let end = start
            .checked_add(size)
            .ok_or_else(|| ControllerError::policy("fat Mach-O slice overflows"))?;
        if size < 4 || start < table_end || end > bytes.len() {
            return Err(ControllerError::policy("fat Mach-O slice is out of bounds"));
        }
        if ranges
            .iter()
            .any(|(other_start, other_end)| start < *other_end && *other_start < end)
        {
            return Err(ControllerError::policy("fat Mach-O slices overlap"));
        }
        ranges.push((start, end));
        let slice = &bytes[start..end];
        let magic: [u8; 4] = slice[..4].try_into().expect("validated slice prefix");
        let parsed = match magic {
            [0xfe, 0xed, 0xfa, 0xce] => parse_macho_slice(slice, false, false)?,
            [0xce, 0xfa, 0xed, 0xfe] => parse_macho_slice(slice, true, false)?,
            [0xfe, 0xed, 0xfa, 0xcf] => parse_macho_slice(slice, false, true)?,
            [0xcf, 0xfa, 0xed, 0xfe] => parse_macho_slice(slice, true, true)?,
            _ => {
                return Err(ControllerError::policy(
                    "fat Mach-O member is not a thin Mach-O image",
                ));
            }
        };
        slices.push(parsed);
    }
    Ok(slices)
}

fn parse_macho_slice(bytes: &[u8], little: bool, wide: bool) -> Result<MachoSlice> {
    let header_size = if wide { 32usize } else { 28usize };
    if bytes.len() < header_size {
        return Err(ControllerError::policy("Mach-O header is truncated"));
    }
    let file_type = read_u32(bytes, 12, little)?;
    let command_count = usize::try_from(read_u32(bytes, 16, little)?)
        .map_err(|_| ControllerError::policy("Mach-O command count is invalid"))?;
    let command_bytes = usize::try_from(read_u32(bytes, 20, little)?)
        .map_err(|_| ControllerError::policy("Mach-O command size is invalid"))?;
    if command_count > MAX_LOAD_COMMANDS {
        return Err(ControllerError::policy(
            "Mach-O command count exceeds its bound",
        ));
    }
    let commands_end = header_size
        .checked_add(command_bytes)
        .ok_or_else(|| ControllerError::policy("Mach-O command table overflows"))?;
    if commands_end > bytes.len() {
        return Err(ControllerError::policy("Mach-O command table is truncated"));
    }
    let mut dependencies = Vec::new();
    let mut dylib_id = None;
    let mut rpaths = Vec::new();
    let mut cursor = header_size;
    for _ in 0..command_count {
        if cursor + 8 > commands_end {
            return Err(ControllerError::policy("Mach-O load command is truncated"));
        }
        let command = read_u32(bytes, cursor, little)?;
        let size = usize::try_from(read_u32(bytes, cursor + 4, little)?)
            .map_err(|_| ControllerError::policy("Mach-O load command size is invalid"))?;
        let end = cursor
            .checked_add(size)
            .ok_or_else(|| ControllerError::policy("Mach-O load command overflows"))?;
        if size < 8 || size % 4 != 0 || end > commands_end {
            return Err(ControllerError::policy(
                "Mach-O load command has invalid bounds",
            ));
        }
        let base_command = command & !0x8000_0000;
        match base_command {
            0x0c | 0x18 | 0x1f | 0x20 | 0x23 => {
                let dependency = load_command_string(bytes, cursor, end, little)?;
                dependencies.push(dependency);
            }
            0x0d => {
                let value = load_command_string(bytes, cursor, end, little)?;
                if dylib_id.replace(value).is_some() {
                    return Err(ControllerError::policy(
                        "Mach-O contains multiple dylib identities",
                    ));
                }
            }
            0x0e => dependencies.push(load_command_string(bytes, cursor, end, little)?),
            0x1c => rpaths.push(load_command_string(bytes, cursor, end, little)?),
            0x27 => {
                return Err(ControllerError::policy(
                    "Mach-O embeds a forbidden dyld environment override",
                ));
            }
            _ => {}
        }
        cursor = end;
    }
    if cursor != commands_end {
        return Err(ControllerError::policy(
            "Mach-O command accounting is not exact",
        ));
    }
    if dependencies.iter().collect::<BTreeSet<_>>().len() != dependencies.len()
        || rpaths.iter().collect::<BTreeSet<_>>().len() != rpaths.len()
    {
        return Err(ControllerError::policy(
            "Mach-O repeats a dependency or runtime search path",
        ));
    }
    Ok(MachoSlice {
        dependencies,
        dylib_id,
        executable: file_type == 2,
        rpaths,
    })
}

fn load_command_string(bytes: &[u8], start: usize, end: usize, little: bool) -> Result<String> {
    if start + 12 > end {
        return Err(ControllerError::policy(
            "Mach-O string load command is truncated",
        ));
    }
    let relative = usize::try_from(read_u32(bytes, start + 8, little)?)
        .map_err(|_| ControllerError::policy("Mach-O string offset is invalid"))?;
    let string_start = start
        .checked_add(relative)
        .ok_or_else(|| ControllerError::policy("Mach-O string offset overflows"))?;
    if relative < 12 || string_start >= end {
        return Err(ControllerError::policy(
            "Mach-O string offset is outside its load command",
        ));
    }
    let tail = &bytes[string_start..end];
    let nul = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| ControllerError::policy("Mach-O string is not NUL terminated"))?;
    if nul == 0 || tail[nul + 1..].iter().any(|byte| *byte != 0) {
        return Err(ControllerError::policy(
            "Mach-O string padding is not canonical",
        ));
    }
    let value = std::str::from_utf8(&tail[..nul])
        .map_err(|_| ControllerError::policy("Mach-O loader path is not UTF-8"))?;
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(ControllerError::policy("Mach-O loader path is unsafe"));
    }
    Ok(value.to_owned())
}

fn read_u32(bytes: &[u8], offset: usize, little: bool) -> Result<u32> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ControllerError::policy("Mach-O integer is truncated"))?
        .try_into()
        .expect("four-byte range");
    Ok(if little {
        u32::from_le_bytes(raw)
    } else {
        u32::from_be_bytes(raw)
    })
}

fn read_u64(bytes: &[u8], offset: usize, little: bool) -> Result<u64> {
    let raw: [u8; 8] = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| ControllerError::policy("Mach-O integer is truncated"))?
        .try_into()
        .expect("eight-byte range");
    Ok(if little {
        u64::from_le_bytes(raw)
    } else {
        u64::from_be_bytes(raw)
    })
}

fn validate_macho_closure(
    runtime_root: &Path,
    images: &BTreeMap<String, MachoImage>,
) -> Result<()> {
    if images.is_empty() || images.len() > MAX_MACHO_IMAGES {
        return Err(ControllerError::policy(
            "Python runtime Mach-O inventory is outside its bound",
        ));
    }
    let interpreter = images
        .get(PYTHON_RELATIVE)
        .ok_or_else(|| ControllerError::policy("Python interpreter is not Mach-O"))?;
    if !interpreter.executable {
        return Err(ControllerError::policy(
            "Python interpreter Mach-O is not executable",
        ));
    }
    for (relative, image) in images {
        let loader = runtime_root.join(relative);
        for dependency in &image.dependencies {
            resolve_dependency(runtime_root, &loader, image, dependency, images)?;
        }
    }
    Ok(())
}

fn resolve_dependency(
    runtime_root: &Path,
    loader: &Path,
    image: &MachoImage,
    dependency: &str,
    images: &BTreeMap<String, MachoImage>,
) -> Result<()> {
    if let Some(suffix) = dependency.strip_prefix("@loader_path") {
        let target = lexical_join(
            loader
                .parent()
                .ok_or_else(|| ControllerError::policy("Mach-O loader has no parent"))?,
            loader_token_suffix(suffix)?,
        )?;
        return require_dependency_target(runtime_root, &target, images);
    }
    if let Some(suffix) = dependency.strip_prefix("@executable_path") {
        let target = lexical_join(&runtime_root.join("bin"), loader_token_suffix(suffix)?)?;
        return require_dependency_target(runtime_root, &target, images);
    }
    if let Some(suffix) = dependency.strip_prefix("@rpath") {
        let mut candidates = BTreeSet::new();
        for rpath in &image.rpaths {
            let base = expand_rpath(runtime_root, loader, rpath)?;
            let target = lexical_join(&base, loader_token_suffix(suffix)?)?;
            if os_tcb_path(&target) {
                candidates.insert(format!("os:{}", target.display()));
            } else if let Ok(relative) = target.strip_prefix(runtime_root) {
                let relative = relative_path(relative)?;
                if images.contains_key(&relative) {
                    candidates.insert(format!("runtime:{relative}"));
                }
            }
        }
        if candidates.len() != 1 {
            return Err(ControllerError::policy(format!(
                "Mach-O @rpath dependency does not resolve exactly once: {dependency}"
            )));
        }
        return Ok(());
    }
    if dependency.starts_with('@') || !dependency.starts_with('/') {
        return Err(ControllerError::policy(format!(
            "Mach-O dependency uses an unsupported loader form: {dependency}"
        )));
    }
    require_dependency_target(runtime_root, Path::new(dependency), images)
}

fn expand_rpath(runtime_root: &Path, loader: &Path, rpath: &str) -> Result<PathBuf> {
    if let Some(suffix) = rpath.strip_prefix("@loader_path") {
        return lexical_join(
            loader
                .parent()
                .ok_or_else(|| ControllerError::policy("Mach-O loader has no parent"))?,
            loader_token_suffix(suffix)?,
        );
    }
    if let Some(suffix) = rpath.strip_prefix("@executable_path") {
        return lexical_join(&runtime_root.join("bin"), loader_token_suffix(suffix)?);
    }
    if rpath.starts_with('/') {
        return lexical_join(Path::new("/"), rpath);
    }
    Err(ControllerError::policy(format!(
        "Mach-O runtime search path is unsupported: {rpath}"
    )))
}

fn loader_token_suffix(suffix: &str) -> Result<&str> {
    if suffix.is_empty() {
        return Ok(suffix);
    }
    let relative = suffix.strip_prefix('/').ok_or_else(|| {
        ControllerError::policy("Mach-O loader token is not followed by a path separator")
    })?;
    if relative.starts_with('/') {
        return Err(ControllerError::policy(
            "Mach-O loader token suffix is not one relative path",
        ));
    }
    Ok(relative)
}

fn lexical_join(base: &Path, suffix: &str) -> Result<PathBuf> {
    let mut output = if suffix.starts_with('/') {
        PathBuf::from("/")
    } else {
        base.to_path_buf()
    };
    for component in Path::new(suffix).components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) => output.push(value),
            Component::ParentDir => {
                if output == Path::new("/") || !output.pop() {
                    return Err(ControllerError::policy(
                        "Mach-O loader path escapes the filesystem root",
                    ));
                }
            }
            Component::Prefix(_) => {
                return Err(ControllerError::policy(
                    "Mach-O loader path has an unsupported prefix",
                ));
            }
        }
    }
    if !output.is_absolute() {
        return Err(ControllerError::policy(
            "Mach-O loader path did not resolve absolutely",
        ));
    }
    Ok(output)
}

fn require_dependency_target(
    runtime_root: &Path,
    target: &Path,
    images: &BTreeMap<String, MachoImage>,
) -> Result<()> {
    if os_tcb_path(target) {
        return Ok(());
    }
    let relative = target.strip_prefix(runtime_root).map_err(|_| {
        ControllerError::policy(format!(
            "non-OS Mach-O dependency escapes the sealed runtime: {}",
            target.display()
        ))
    })?;
    let relative = relative_path(relative)?;
    if !images.contains_key(&relative) {
        return Err(ControllerError::policy(format!(
            "Mach-O dependency target is absent from the sealed runtime: {relative}"
        )));
    }
    Ok(())
}

fn relative_path(path: &Path) -> Result<String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ControllerError::policy(
            "runtime-relative Mach-O path is malformed",
        ));
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ControllerError::policy("runtime-relative Mach-O path is not UTF-8"))
}

fn os_tcb_path(path: &Path) -> bool {
    OS_LIBRARY_ROOTS.iter().any(|root| {
        let root = Path::new(root);
        path == root || path.starts_with(root)
    })
}

fn os_tcb_digest(build: &str) -> [u8; 32] {
    digest_strings(
        OS_TCB_CONTRACT,
        std::iter::once(build.as_bytes())
            .chain(OS_LIBRARY_ROOTS.iter().map(|root| root.as_bytes()))
            .chain(std::iter::once(b"dyld-shared-cache".as_slice())),
    )
}

#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "audited Darwin sysctl, ACL, descriptor, waitid, and pre-exec boundaries"
)]
mod macos {
    use super::*;
    use crate::{
        MacosJob, Watchdog, bounded_reader, effective_gid, effective_uid,
        ensure_empty_process_group, prepare_isolated_child, send_job_signal,
        validate_no_inherited_fds, validate_trusted_path_acl,
    };
    use std::{
        fs::{File, Metadata, OpenOptions},
        io::{self, Read, Seek, SeekFrom, Write},
        os::{
            fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
            unix::{
                fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
                process::CommandExt,
            },
        },
        process::{Command, ExitStatus, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    const O_NOFOLLOW: i32 = 0x0000_0100;
    const O_CLOEXEC: i32 = 0x0100_0000;
    const F_DUPFD_CLOEXEC: i32 = 67;
    const F_SETFD: i32 = 2;
    const FD_CLOEXEC: i32 = 1;
    const ACL_TYPE_EXTENDED: i32 = 0x0000_0100;
    const ACL_FIRST_ENTRY: i32 = 0;

    #[derive(Debug)]
    pub(super) struct PinnedFile {
        pub(super) path: PathBuf,
        pub(super) file: File,
        pub(super) stable: StableMetadata,
        pub(super) sha256: [u8; 32],
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct StableMetadata {
        device: u64,
        inode: u64,
        mode: u32,
        uid: u32,
        gid: u32,
        links: u64,
        size: u64,
        modified_seconds: i64,
        modified_nanoseconds: i64,
    }

    pub(super) struct RuntimeClosure {
        pub(super) digest: [u8; 32],
        pub(super) images: BTreeMap<String, MachoImage>,
        pub(super) held_images: Vec<PinnedFile>,
        pub(super) python: PinnedFile,
    }

    pub(super) struct Captured {
        pub(super) status: ExitStatus,
        pub(super) stdout: Vec<u8>,
        pub(super) stderr: Vec<u8>,
    }

    unsafe extern "C" {
        fn acl_get_fd_np(descriptor: i32, acl_type: i32) -> *mut std::ffi::c_void;
        fn acl_get_entry(
            acl: *mut std::ffi::c_void,
            entry_id: i32,
            entry: *mut *mut std::ffi::c_void,
        ) -> i32;
        fn acl_free(object: *mut std::ffi::c_void) -> i32;
        fn flistxattr(descriptor: i32, list: *mut i8, size: usize, options: i32) -> isize;
        fn fcntl(descriptor: i32, command: i32, ...) -> i32;
        fn dup2(source: i32, target: i32) -> i32;
        fn pipe(descriptors: *mut i32) -> i32;
        fn sysctlbyname(
            name: *const i8,
            old: *mut std::ffi::c_void,
            old_length: *mut usize,
            new: *mut std::ffi::c_void,
            new_length: usize,
        ) -> i32;
        fn waitid(id_type: i32, id: u32, information: *mut std::ffi::c_void, options: i32) -> i32;
    }

    pub(super) fn validate_root_launch_identity() -> Result<()> {
        if effective_uid() != 0 || effective_gid() != 0 {
            return Err(ControllerError::policy(
                "Kagemusha native Python launcher must run as root:wheel",
            ));
        }
        validate_no_inherited_fds()?;
        Ok(())
    }

    pub(super) fn observed_macos_build() -> Result<String> {
        let name = b"kern.osversion\0";
        let mut length = 0usize;
        if unsafe {
            sysctlbyname(
                name.as_ptr().cast(),
                std::ptr::null_mut(),
                &raw mut length,
                std::ptr::null_mut(),
                0,
            )
        } != 0
            || length < 2
            || length > 128
        {
            return Err(ControllerError::policy(
                "macOS build identity is unavailable",
            ));
        }
        let mut bytes = vec![0u8; length];
        if unsafe {
            sysctlbyname(
                name.as_ptr().cast(),
                bytes.as_mut_ptr().cast(),
                &raw mut length,
                std::ptr::null_mut(),
                0,
            )
        } != 0
            || length != bytes.len()
            || bytes.last() != Some(&0)
        {
            return Err(ControllerError::policy(
                "macOS build identity changed during inspection",
            ));
        }
        bytes.pop();
        let build = String::from_utf8(bytes)
            .map_err(|_| ControllerError::policy("macOS build identity is not UTF-8"))?;
        if build.is_empty()
            || build.len() > 64
            || !build
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(ControllerError::policy("macOS build identity is malformed"));
        }
        Ok(build)
    }

    pub(super) fn require_macos_tcb(expected: &str) -> Result<[u8; 32]> {
        if observed_macos_build()? != expected {
            return Err(ControllerError::policy(
                "macOS build differs from the native-launch TCB pin",
            ));
        }
        for root in OS_LIBRARY_ROOTS {
            require_root_custody(Path::new(root), true)?;
        }
        require_root_custody(Path::new("/bin/bash"), false)?;
        Ok(os_tcb_digest(expected))
    }

    pub(super) fn require_root_custody(path: &Path, directory: bool) -> Result<()> {
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(ControllerError::policy(
                "sealed path is not one normalized absolute path",
            ));
        }
        let mut current = PathBuf::from("/");
        for component in path.components().skip(1) {
            let Component::Normal(name) = component else {
                return Err(ControllerError::policy("sealed path component is invalid"));
            };
            current.push(name);
            let metadata = fs::symlink_metadata(&current).map_err(|_| {
                ControllerError::policy(format!(
                    "sealed path metadata is unavailable: {}",
                    current.display()
                ))
            })?;
            let final_component = current == path;
            if metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o022 != 0
                || (final_component && directory != metadata.is_dir())
                || (!final_component && !metadata.is_dir())
            {
                return Err(ControllerError::policy(format!(
                    "sealed path leaves root-owned, non-writable custody: {}",
                    current.display()
                )));
            }
            validate_trusted_path_acl(&current)?;
            let file = open_nofollow(&current, metadata.is_file())?;
            require_no_xattrs(&file, &current)?;
        }
        Ok(())
    }

    fn require_no_xattrs(file: &File, path: &Path) -> Result<()> {
        let count = unsafe { flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0, 0) };
        if count < 0 {
            return Err(ControllerError::policy(format!(
                "sealed path xattrs are unavailable: {}",
                path.display()
            )));
        }
        if count != 0 {
            return Err(ControllerError::policy(format!(
                "sealed path has unbound extended attributes: {}",
                path.display()
            )));
        }
        let acl = unsafe { acl_get_fd_np(file.as_raw_fd(), ACL_TYPE_EXTENDED) };
        if acl.is_null() {
            return if io::Error::last_os_error().raw_os_error() == Some(2) {
                Ok(())
            } else {
                Err(ControllerError::policy(
                    "sealed path extended ACL is unavailable",
                ))
            };
        }
        let mut entry = std::ptr::null_mut();
        let status = unsafe { acl_get_entry(acl, ACL_FIRST_ENTRY, &raw mut entry) };
        let freed = unsafe { acl_free(acl) };
        if status < 0 || freed != 0 {
            return Err(ControllerError::policy(
                "sealed path extended ACL inspection failed",
            ));
        }
        if status == 0 {
            return Err(ControllerError::policy("sealed path has an extended ACL"));
        }
        Ok(())
    }

    fn open_nofollow(path: &Path, regular: bool) -> Result<File> {
        let mut options = OpenOptions::new();
        options.read(true).custom_flags(O_NOFOLLOW | O_CLOEXEC);
        let file = options.open(path).map_err(|_| {
            ControllerError::policy(format!("sealed path cannot be opened: {}", path.display()))
        })?;
        let metadata = file
            .metadata()
            .map_err(|_| ControllerError::policy("sealed descriptor metadata is unavailable"))?;
        if regular != metadata.is_file() && (regular || !metadata.is_dir()) {
            return Err(ControllerError::policy(
                "sealed descriptor has the wrong filesystem type",
            ));
        }
        Ok(file)
    }

    fn stable(metadata: &Metadata) -> StableMetadata {
        use std::os::unix::fs::MetadataExt;
        StableMetadata {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            links: metadata.nlink(),
            size: metadata.size(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }

    fn hash_open_file(file: &mut File) -> Result<[u8; 32]> {
        let before = file
            .metadata()
            .map_err(|_| ControllerError::policy("sealed file metadata is unavailable"))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("sealed file seek failed"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|_| ControllerError::policy("sealed file read failed"))?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ControllerError::policy("sealed file rewind failed"))?;
        let after = file
            .metadata()
            .map_err(|_| ControllerError::policy("sealed file metadata is unavailable"))?;
        if stable(&before) != stable(&after) {
            return Err(ControllerError::policy(
                "sealed file changed while it was hashed",
            ));
        }
        Ok(hash.finish())
    }

    pub(super) fn pin_regular(path: &Path, expected: [u8; 32]) -> Result<PinnedFile> {
        require_root_custody(path, false)?;
        let mut file = open_nofollow(path, true)?;
        let metadata = file
            .metadata()
            .map_err(|_| ControllerError::policy("pinned file metadata is unavailable"))?;
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != 0
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o022 != 0
            || metadata.size() == 0
            || metadata.size() > MAX_RUNTIME_FILE_BYTES
        {
            return Err(ControllerError::policy(
                "pinned file ownership, mode, links, or size is unsafe",
            ));
        }
        require_no_xattrs(&file, path)?;
        let sha256 = hash_open_file(&mut file)?;
        if sha256 != expected {
            return Err(ControllerError::policy(format!(
                "pinned file differs from its expected SHA-256: {}",
                path.display()
            )));
        }
        Ok(PinnedFile {
            path: path.to_path_buf(),
            stable: stable(&metadata),
            file,
            sha256,
        })
    }

    pub(super) fn validate_pinned(file: &mut PinnedFile) -> Result<()> {
        let descriptor_metadata = file
            .file
            .metadata()
            .map_err(|_| ControllerError::policy("pinned descriptor metadata is unavailable"))?;
        let path_metadata = fs::symlink_metadata(&file.path)
            .map_err(|_| ControllerError::policy("pinned path metadata is unavailable"))?;
        if stable(&descriptor_metadata) != file.stable || stable(&path_metadata) != file.stable {
            return Err(ControllerError::policy(format!(
                "pinned path was substituted: {}",
                file.path.display()
            )));
        }
        if hash_open_file(&mut file.file)? != file.sha256 {
            return Err(ControllerError::policy(format!(
                "pinned file bytes changed: {}",
                file.path.display()
            )));
        }
        Ok(())
    }

    pub(super) fn authenticate_runtime(
        root: &Path,
        expected_tree: [u8; 32],
        expected_python: [u8; 32],
    ) -> Result<RuntimeClosure> {
        require_root_custody(root, true)?;
        let root_metadata = fs::symlink_metadata(root)
            .map_err(|_| ControllerError::policy("Python runtime root is unavailable"))?;
        use std::os::unix::fs::MetadataExt;
        let root_device = root_metadata.dev();
        let mut paths = vec![root.to_path_buf()];
        let mut index = 0usize;
        while index < paths.len() {
            let parent = paths[index].clone();
            index += 1;
            let metadata = fs::symlink_metadata(&parent)
                .map_err(|_| ControllerError::policy("Python runtime traversal changed"))?;
            if !metadata.is_dir() {
                continue;
            }
            let mut children = fs::read_dir(&parent)
                .map_err(|_| ControllerError::policy("Python runtime directory is unreadable"))?
                .map(|entry| {
                    entry
                        .map_err(|_| ControllerError::policy("Python runtime entry is unreadable"))
                        .and_then(|entry| {
                            entry.file_name().into_string().map_err(|_| {
                                ControllerError::policy("Python runtime path is not UTF-8")
                            })?;
                            Ok(entry.path())
                        })
                })
                .collect::<Result<Vec<_>>>()?;
            children.sort();
            for child in children.into_iter().rev() {
                paths.insert(index, child);
            }
            if paths.len() > MAX_RUNTIME_RECORDS {
                return Err(ControllerError::policy(
                    "Python runtime has too many records",
                ));
            }
        }
        paths.sort_by(|left, right| {
            let left = left.strip_prefix(root).unwrap_or(left);
            let right = right.strip_prefix(root).unwrap_or(right);
            left.cmp(right)
        });
        if paths.first() != Some(&root.to_path_buf()) {
            return Err(ControllerError::policy(
                "Python runtime traversal did not begin at its root",
            ));
        }
        let mut tree_hash = Sha256::new();
        let mut total_bytes = 0u64;
        let mut images = BTreeMap::new();
        let mut held_images = Vec::new();
        let mut python = None;
        for path in paths {
            let relative = if path == root {
                ".".to_owned()
            } else {
                relative_path(path.strip_prefix(root).map_err(|_| {
                    ControllerError::policy("Python runtime traversal escaped its root")
                })?)?
            };
            if relative.len() > 4096 {
                return Err(ControllerError::policy(
                    "Python runtime contains an oversized path",
                ));
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| ControllerError::policy("Python runtime metadata is unavailable"))?;
            if metadata.dev() != root_device
                || metadata.uid() != 0
                || metadata.permissions().mode() & 0o022 != 0
                || metadata.file_type().is_symlink()
            {
                return Err(ControllerError::policy(format!(
                    "Python runtime member has unsafe custody: {}",
                    path.display()
                )));
            }
            validate_trusted_path_acl(&path)?;
            let mode = format!("{:o}", metadata.permissions().mode() & 0o7777);
            let (kind, content_size, content_sha256) = if metadata.is_dir() {
                let file = open_nofollow(&path, false)?;
                require_no_xattrs(&file, &path)?;
                ("directory", 0u64, "-".to_owned())
            } else if metadata.is_file() {
                if metadata.nlink() != 1 || metadata.size() > MAX_RUNTIME_FILE_BYTES {
                    return Err(ControllerError::policy(
                        "Python runtime regular file has unsafe links or size",
                    ));
                }
                total_bytes = total_bytes.checked_add(metadata.size()).ok_or_else(|| {
                    ControllerError::policy("Python runtime byte count overflows")
                })?;
                if total_bytes > MAX_RUNTIME_TOTAL_BYTES {
                    return Err(ControllerError::policy(
                        "Python runtime exceeds its total-byte bound",
                    ));
                }
                let mut file = open_nofollow(&path, true)?;
                require_no_xattrs(&file, &path)?;
                let stable = stable(&metadata);
                let digest = hash_open_file(&mut file)?;
                let mut prefix = [0u8; 4];
                let count = file
                    .read(&mut prefix)
                    .map_err(|_| ControllerError::policy("Python runtime file is unreadable"))?;
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| ControllerError::policy("Python runtime file rewind failed"))?;
                let is_macho = count == 4
                    && matches!(
                        prefix,
                        [0xfe, 0xed, 0xfa, 0xce]
                            | [0xce, 0xfa, 0xed, 0xfe]
                            | [0xfe, 0xed, 0xfa, 0xcf]
                            | [0xcf, 0xfa, 0xed, 0xfe]
                            | [0xca, 0xfe, 0xba, 0xbe]
                            | [0xbe, 0xba, 0xfe, 0xca]
                            | [0xca, 0xfe, 0xba, 0xbf]
                            | [0xbf, 0xba, 0xfe, 0xca]
                    );
                let pinned = PinnedFile {
                    path: path.clone(),
                    file,
                    stable,
                    sha256: digest,
                };
                if is_macho {
                    if metadata.size() > MAX_MACHO_IMAGE_BYTES {
                        return Err(ControllerError::policy(
                            "Python runtime Mach-O image exceeds its bound",
                        ));
                    }
                    let mut bytes =
                        Vec::with_capacity(usize::try_from(metadata.size()).map_err(|_| {
                            ControllerError::policy("Mach-O image size is invalid")
                        })?);
                    let mut pinned = pinned;
                    pinned
                        .file
                        .read_to_end(&mut bytes)
                        .map_err(|_| ControllerError::policy("Mach-O image is unreadable"))?;
                    pinned
                        .file
                        .seek(SeekFrom::Start(0))
                        .map_err(|_| ControllerError::policy("Mach-O image rewind failed"))?;
                    let image = macho(&bytes)?.ok_or_else(|| {
                        ControllerError::policy("Mach-O magic was not parsed as Mach-O")
                    })?;
                    if images.insert(relative.clone(), image).is_some() {
                        return Err(ControllerError::policy(
                            "Python runtime Mach-O path repeats",
                        ));
                    }
                    held_images.push(pinned);
                } else if relative == PYTHON_RELATIVE {
                    return Err(ControllerError::policy(
                        "Python interpreter lacks Mach-O magic",
                    ));
                }
                if relative == PYTHON_RELATIVE {
                    let duplicate = pin_regular(&path, expected_python)?;
                    python = Some(duplicate);
                }
                ("file", metadata.size(), hex(&digest))
            } else {
                return Err(ControllerError::policy(
                    "Python runtime contains a special filesystem member",
                ));
            };
            for field in [
                kind.as_bytes(),
                relative.as_bytes(),
                mode.as_bytes(),
                b"0",
                content_size.to_string().as_bytes(),
                content_sha256.as_bytes(),
            ] {
                tree_hash.update(field);
                tree_hash.update(&[0]);
            }
        }
        let digest = tree_hash.finish();
        if digest != expected_tree {
            return Err(ControllerError::policy(
                "Python runtime tree differs from its expected SHA-256",
            ));
        }
        validate_macho_closure(root, &images)?;
        let python = python
            .ok_or_else(|| ControllerError::policy("Python interpreter is absent from runtime"))?;
        if python.stable.mode & 0o111 == 0 {
            return Err(ControllerError::policy(
                "Python interpreter is not executable",
            ));
        }
        Ok(RuntimeClosure {
            digest,
            images,
            held_images,
            python,
        })
    }

    pub(super) fn validate_runtime(
        closure: &mut RuntimeClosure,
        root: &Path,
        expected_tree: [u8; 32],
        expected_python: [u8; 32],
    ) -> Result<()> {
        validate_pinned(&mut closure.python)?;
        for image in &mut closure.held_images {
            validate_pinned(image)?;
        }
        let mut fresh = authenticate_runtime(root, expected_tree, expected_python)?;
        if fresh.digest != closure.digest || fresh.images != closure.images {
            return Err(ControllerError::policy(
                "Python runtime closure changed during native execution",
            ));
        }
        validate_pinned(&mut fresh.python)?;
        Ok(())
    }

    fn duplicate_high(file: &File, minimum: i32) -> Result<OwnedFd> {
        let descriptor = unsafe { fcntl(file.as_raw_fd(), F_DUPFD_CLOEXEC, minimum) };
        if descriptor < 0 {
            return Err(ControllerError::policy(
                "could not duplicate a pinned launch descriptor",
            ));
        }
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }

    fn pipe_with_receipt(receipt: &[u8]) -> Result<OwnedFd> {
        if receipt.len() > 16 * 1024 {
            return Err(ControllerError::policy(
                "native launch receipt exceeds its bound",
            ));
        }
        let mut descriptors = [-1i32; 2];
        if unsafe { pipe(descriptors.as_mut_ptr()) } != 0 {
            return Err(ControllerError::policy(
                "could not create native launch receipt pipe",
            ));
        }
        let read = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
        let mut write = unsafe { File::from_raw_fd(descriptors[1]) };
        for descriptor in [read.as_raw_fd(), write.as_raw_fd()] {
            if unsafe { fcntl(descriptor, F_SETFD, FD_CLOEXEC) } != 0 {
                return Err(ControllerError::policy(
                    "could not seal native launch receipt descriptors",
                ));
            }
        }
        write
            .write_all(receipt)
            .map_err(|_| ControllerError::policy("could not write native launch receipt"))?;
        drop(write);
        Ok(read)
    }

    fn configure_child(command: &mut Command, mappings: Vec<(RawFd, RawFd)>) {
        unsafe {
            command.pre_exec(move || {
                prepare_isolated_child(0)?;
                for (source, target) in &mappings {
                    if dup2(*source, *target) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
    }

    pub(super) fn run_captured(
        mut command: Command,
        mappings: Vec<(RawFd, RawFd)>,
        wall_seconds: u64,
        stdout_limit: u64,
        stderr_limit: u64,
    ) -> Result<Captured> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_child(&mut command, mappings);
        let child = command
            .spawn()
            .map_err(|_| ControllerError::policy("native Python child could not be executed"))?;
        let process_group = child.id() as i32;
        let watchdog = Watchdog::start(process_group)?;
        let mut job = MacosJob::new(child, process_group, watchdog);
        let stdout = job
            .child
            .stdout
            .take()
            .ok_or_else(|| ControllerError::policy("native Python stdout pipe is absent"))?;
        let stderr = job
            .child
            .stderr
            .take()
            .ok_or_else(|| ControllerError::policy("native Python stderr pipe is absent"))?;
        let combined = Arc::new(AtomicU64::new(0));
        let overflow = Arc::new(AtomicBool::new(false));
        let stdout_reader = bounded_reader(
            stdout,
            stdout_limit,
            Some(stdout_limit.saturating_add(stderr_limit)),
            Arc::clone(&combined),
            Arc::clone(&overflow),
        );
        let stderr_reader = bounded_reader(
            stderr,
            stderr_limit,
            Some(stdout_limit.saturating_add(stderr_limit)),
            Arc::clone(&combined),
            Arc::clone(&overflow),
        );
        let started = Instant::now();
        let mut failure = None;
        loop {
            if overflow.load(Ordering::Acquire) {
                failure = Some(ControllerError::limit(
                    "native Python output exceeded its bound",
                ));
                break;
            }
            if started.elapsed() >= Duration::from_secs(wall_seconds) {
                failure = Some(ControllerError::limit(
                    "native Python child exceeded its wall-time bound",
                ));
                break;
            }
            if leader_exited_nowait(job.child.id())? {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        // The unreaped leader pins both its PID and process-group ID. Sweep the
        // complete group while that pin is live, then retire the watchdog before
        // reaping so no cleanup path can signal a subsequently reused PGID.
        sweep_pinned_process_group(process_group, job.child.id())?;
        job.finish_watchdog()?;
        let status = job
            .child
            .wait()
            .map_err(|_| ControllerError::policy("native Python leader could not be reaped"))?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| ControllerError::policy("native Python stdout reader failed"))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| ControllerError::policy("native Python stderr reader failed"))?;
        if stdout.io_failed || stderr.io_failed {
            return Err(ControllerError::policy(
                "native Python diagnostic pipe failed",
            ));
        }
        ensure_empty_process_group(process_group)?;
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(Captured {
            status,
            stdout: stdout.bytes,
            stderr: stderr.bytes,
        })
    }

    #[repr(C, align(8))]
    struct OpaqueSigInfo {
        bytes: [u8; 128],
    }

    fn leader_exited_nowait(pid: u32) -> Result<bool> {
        const P_PID: i32 = 1;
        const WNOHANG: i32 = 0x0000_0001;
        const WEXITED: i32 = 0x0000_0004;
        const WNOWAIT: i32 = 0x0000_0020;
        let mut information = OpaqueSigInfo { bytes: [0; 128] };
        let status = unsafe {
            waitid(
                P_PID,
                pid,
                (&raw mut information).cast(),
                WNOHANG | WEXITED | WNOWAIT,
            )
        };
        if status != 0 {
            return Err(ControllerError::policy(
                "native Python leader status is unavailable",
            ));
        }
        // Darwin's siginfo_t begins with signo/errno/code/pid as four native
        // 32-bit integers. waitid clears si_pid for a WNOHANG no-event result.
        let observed_pid = u32::from_ne_bytes(
            information.bytes[12..16]
                .try_into()
                .expect("siginfo PID field"),
        );
        Ok(observed_pid == pid)
    }

    fn sweep_pinned_process_group(process_group: i32, leader: u32) -> Result<()> {
        let leader = i32::try_from(leader)
            .map_err(|_| ControllerError::policy("native Python PID does not fit i32"))?;
        send_job_signal(process_group, leader, 15)?;
        thread::sleep(Duration::from_millis(250));
        send_job_signal(process_group, leader, 9)?;
        Ok(())
    }

    pub(super) fn high_descriptor(file: &File, minimum: i32) -> Result<OwnedFd> {
        duplicate_high(file, minimum)
    }

    pub(super) fn receipt_descriptor(receipt: &[u8]) -> Result<OwnedFd> {
        let read = pipe_with_receipt(receipt)?;
        let descriptor = unsafe { fcntl(read.as_raw_fd(), F_DUPFD_CLOEXEC, 70) };
        if descriptor < 0 {
            return Err(ControllerError::policy(
                "could not duplicate native launch receipt",
            ));
        }
        Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
    }

    pub(super) fn publish_report(path: &Path, payload: &[u8]) -> Result<()> {
        if path.parent() != Some(Path::new(REPORT_PARENT)) {
            return Err(ControllerError::policy(
                "sealed build report is outside its fixed publication parent",
            ));
        }
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| ControllerError::policy("sealed build report name is invalid"))?;
        if name.is_empty()
            || name.len() > 255
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || !name.ends_with(".json")
        {
            return Err(ControllerError::policy(
                "sealed build report name is unsafe",
            ));
        }
        let parent = Path::new(REPORT_PARENT);
        require_root_custody(parent, true)?;
        if fs::symlink_metadata(path).is_ok() {
            return Err(ControllerError::policy(
                "sealed build report already exists",
            ));
        }
        let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(O_NOFOLLOW | O_CLOEXEC);
        let mut file = options.open(&temporary).map_err(|_| {
            ControllerError::policy("sealed build report temporary could not be created")
        })?;
        let result = (|| {
            file.write_all(payload)
                .map_err(|_| ControllerError::policy("sealed build report write failed"))?;
            file.sync_all()
                .map_err(|_| ControllerError::policy("sealed build report fsync failed"))?;
            file.set_permissions(fs::Permissions::from_mode(0o444))
                .map_err(|_| ControllerError::policy("sealed build report mode seal failed"))?;
            file.sync_all()
                .map_err(|_| ControllerError::policy("sealed build report seal fsync failed"))?;
            drop(file);
            fs::hard_link(&temporary, path).map_err(|_| {
                ControllerError::policy("sealed build report no-replace publication failed")
            })?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| ControllerError::policy("report parent fsync failed"))?;
            fs::remove_file(&temporary)
                .map_err(|_| ControllerError::policy("report temporary removal failed"))?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| ControllerError::policy("report parent final fsync failed"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

#[cfg(target_os = "macos")]
use macos::{
    authenticate_runtime, high_descriptor, pin_regular, publish_report, receipt_descriptor,
    require_macos_tcb, require_root_custody, run_captured, validate_root_launch_identity,
    validate_runtime,
};

#[cfg(target_os = "macos")]
fn launch_readiness_macos(launch: ReadinessLaunch) -> Result<u8> {
    use std::{io::Write, os::fd::AsRawFd, process::Command};

    validate_root_launch_identity()?;
    let controller_path = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|_| ControllerError::policy("controller executable identity is unavailable"))?;
    let controller_sha256 = super::sha256_file(&controller_path)?;
    let readiness_environment =
        validate_readiness_environment(&launch, &controller_path, controller_sha256)?;
    let os_tcb_sha256 = require_macos_tcb(&launch.common.expected_macos_build)?;
    require_root_custody(&launch.gate_source, false)?;
    if launch.gate_snapshot.parent().and_then(Path::parent)
        != Some(Path::new(READINESS_STAGING_PARENT))
        || !launch
            .gate_snapshot
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("gate-launch."))
    {
        return Err(ControllerError::policy(
            "readiness gate snapshot is outside its fixed staging parent",
        ));
    }
    let mut gate_source = pin_regular(&launch.gate_source, launch.gate_sha256)?;
    let mut gate_pin = pin_regular(&launch.gate_snapshot, launch.gate_sha256)?;
    let mut gate_execution = pin_regular(&launch.gate_snapshot, launch.gate_sha256)?;
    let mut runtime = authenticate_runtime(
        &launch.common.runtime_root,
        launch.common.runtime_tree_sha256,
        launch.common.python_sha256,
    )?;
    let gate_pin_fd = high_descriptor(&gate_pin.file, 64)?;
    let python_fd = high_descriptor(&runtime.python.file, 65)?;
    let gate_execution_fd = high_descriptor(&gate_execution.file, 66)?;
    let mut command = Command::new("/bin/bash");
    command
        .arg("/dev/fd/10")
        .arg("promotion")
        .current_dir("/")
        .env_clear()
        .envs(readiness_environment)
        .env("KAGEMUSHA_PRODUCTION_READINESS_GATE_LAUNCH_FD", "8")
        .env("KAGEMUSHA_PRODUCTION_READINESS_GATE_EXECUTION_FD", "10")
        .env(
            "KAGEMUSHA_PRODUCTION_READINESS_GATE_SOURCE_PATH",
            &launch.gate_source,
        )
        .env("KAGEMUSHA_PRODUCTION_READINESS_PYTHON_PIN_FD", "9")
        .env(
            "KAGEMUSHA_PRODUCTION_READINESS_NATIVE_LAUNCH_CONTRACT",
            READINESS_CONTRACT,
        )
        .env(
            "KAGEMUSHA_PRODUCTION_READINESS_NATIVE_MACOS_BUILD",
            &launch.common.expected_macos_build,
        )
        .env(
            "KAGEMUSHA_PRODUCTION_READINESS_NATIVE_OS_TCB_SHA256",
            hex(&os_tcb_sha256),
        )
        .env(
            "KAGEMUSHA_PRODUCTION_READINESS_NATIVE_RUNTIME_DEPENDENCY_CONTRACT",
            RUNTIME_DEPENDENCY_CONTRACT,
        );
    let captured = run_captured(
        command,
        vec![
            (gate_pin_fd.as_raw_fd(), 8),
            (python_fd.as_raw_fd(), 9),
            (gate_execution_fd.as_raw_fd(), 10),
        ],
        READINESS_WALL_SECONDS,
        MAX_READINESS_STDOUT_BYTES,
        MAX_READINESS_STDERR_BYTES,
    )?;
    validate_runtime(
        &mut runtime,
        &launch.common.runtime_root,
        launch.common.runtime_tree_sha256,
        launch.common.python_sha256,
    )?;
    macos::validate_pinned(&mut gate_source)?;
    macos::validate_pinned(&mut gate_pin)?;
    macos::validate_pinned(&mut gate_execution)?;
    std::io::stdout()
        .write_all(&captured.stdout)
        .and_then(|_| std::io::stdout().flush())
        .map_err(|_| ControllerError::policy("readiness stdout forwarding failed"))?;
    std::io::stderr()
        .write_all(&captured.stderr)
        .and_then(|_| std::io::stderr().flush())
        .map_err(|_| ControllerError::policy("readiness stderr forwarding failed"))?;
    exit_status(captured.status)
}

#[cfg(target_os = "macos")]
fn launch_builder_macos(launch: BuilderLaunch) -> Result<u8> {
    use std::{io::Write, os::fd::AsRawFd, process::Command};

    validate_root_launch_identity()?;
    validate_builder_environment()?;
    let os_tcb_sha256 = require_macos_tcb(&launch.common.expected_macos_build)?;
    let root = Path::new(
        launch.builder_arguments[1]
            .to_str()
            .ok_or_else(|| ControllerError::policy("builder root is not UTF-8"))?,
    );
    if launch.builder != root.join(BUILDER_RELATIVE) {
        return Err(ControllerError::policy(
            "sealed-builder entrypoint is not the reviewed root-relative script",
        ));
    }
    let mut builder = pin_regular(&launch.builder, launch.builder_sha256)?;
    let mut runtime = authenticate_runtime(
        &launch.common.runtime_root,
        launch.common.runtime_tree_sha256,
        launch.common.python_sha256,
    )?;
    let controller_path = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|_| ControllerError::policy("controller executable identity is unavailable"))?;
    let controller_sha256 = super::sha256_file(&controller_path)?;
    let arguments_sha256 = builder_argument_digest(&launch.builder_arguments);
    let environment_sha256 = builder_environment_digest();
    let native_launch = native_launch_json(
        &launch.common,
        launch.builder_sha256,
        controller_sha256,
        arguments_sha256,
        environment_sha256,
        os_tcb_sha256,
    );
    let receipt = format!("{{\"native_launch\":{native_launch}}}\n");
    let receipt_sha256 = {
        let mut hash = Sha256::new();
        hash.update(receipt.as_bytes());
        hash.finish()
    };
    let receipt_fd = receipt_descriptor(receipt.as_bytes())?;
    let builder_fd = high_descriptor(&builder.file, 71)?;
    let mut command = Command::new(launch.common.runtime_root.join(PYTHON_RELATIVE));
    command
        .arg("-I")
        .arg("-S")
        .arg("/dev/fd/12")
        .args(&launch.builder_arguments)
        .current_dir("/")
        .env_clear()
        .envs(builder_environment())
        .env("KAGEMUSHA_SEALED_BUILDER_LAUNCH_FD", "11")
        .env(
            "KAGEMUSHA_SEALED_BUILDER_LAUNCH_RECEIPT_SHA256",
            hex(&receipt_sha256),
        )
        .env("KAGEMUSHA_SEALED_BUILDER_ENTRYPOINT_FD", "12")
        .env("KAGEMUSHA_SEALED_BUILDER_REVIEWED_ROOT", root);
    let captured = run_captured(
        command,
        vec![(receipt_fd.as_raw_fd(), 11), (builder_fd.as_raw_fd(), 12)],
        BUILDER_WALL_SECONDS,
        MAX_BUILDER_REPORT_BYTES,
        MAX_BUILDER_STDERR_BYTES,
    )?;
    validate_runtime(
        &mut runtime,
        &launch.common.runtime_root,
        launch.common.runtime_tree_sha256,
        launch.common.python_sha256,
    )?;
    macos::validate_pinned(&mut builder)?;
    std::io::stderr()
        .write_all(&captured.stderr)
        .and_then(|_| std::io::stderr().flush())
        .map_err(|_| ControllerError::policy("builder stderr forwarding failed"))?;
    let status = exit_status(captured.status)?;
    if status != 0 {
        return Ok(status);
    }
    validate_inner_report(&captured.stdout)?;
    let inner_sha256 = {
        let mut hash = Sha256::new();
        hash.update(&captured.stdout);
        hash.finish()
    };
    let envelope = format!(
        "{{\"builder_report_hex\":\"{}\",\"builder_report_sha256\":\"{}\",\"builder_report_size_bytes\":{},\"native_launch\":{},\"schema\":\"{}\"}}\n",
        hex(&captured.stdout),
        hex(&inner_sha256),
        captured.stdout.len(),
        native_launch,
        BUILDER_REPORT_SCHEMA,
    );
    if envelope.len() > 1024 * 1024 {
        return Err(ControllerError::policy(
            "native sealed-build report envelope exceeds its bound",
        ));
    }
    publish_report(&launch.report_output, envelope.as_bytes())?;
    let envelope_sha256 = {
        let mut hash = Sha256::new();
        hash.update(envelope.as_bytes());
        hash.finish()
    };
    println!(
        "kagemusha-native-sealed-builder-report-v1 {} {} {}",
        hex(&envelope_sha256),
        envelope.len(),
        launch.report_output.display()
    );
    Ok(0)
}

#[cfg(target_os = "macos")]
fn native_launch_json(
    common: &CommonLaunch,
    builder_sha256: [u8; 32],
    controller_sha256: [u8; 32],
    argument_sha256: [u8; 32],
    environment_sha256: [u8; 32],
    os_tcb_sha256: [u8; 32],
) -> String {
    format!(
        "{{\"argument_contract\":\"{}\",\"argument_sha256\":\"{}\",\"builder_entrypoint_sha256\":\"{}\",\"contract\":\"{}\",\"controller_sha256\":\"{}\",\"environment_contract\":\"{}\",\"environment_sha256\":\"{}\",\"macos_build\":\"{}\",\"os_tcb_contract\":\"{}\",\"os_tcb_sha256\":\"{}\",\"python_interpreter_sha256\":\"{}\",\"python_runtime_tree_sha256\":\"{}\",\"report_publication_contract\":\"{}\",\"runtime_dependency_contract\":\"{}\"}}",
        BUILDER_ARGUMENT_CONTRACT,
        hex(&argument_sha256),
        hex(&builder_sha256),
        BUILDER_CONTRACT,
        hex(&controller_sha256),
        BUILDER_ENVIRONMENT_CONTRACT,
        hex(&environment_sha256),
        common.expected_macos_build,
        OS_TCB_CONTRACT,
        hex(&os_tcb_sha256),
        hex(&common.python_sha256),
        hex(&common.runtime_tree_sha256),
        REPORT_PUBLICATION_CONTRACT,
        RUNTIME_DEPENDENCY_CONTRACT,
    )
}

#[cfg(target_os = "macos")]
fn validate_inner_report(payload: &[u8]) -> Result<()> {
    if payload.is_empty()
        || payload.len() > usize::try_from(MAX_BUILDER_REPORT_BYTES).expect("small bound")
        || !payload.ends_with(b"}\n")
        || payload.first() != Some(&b'{')
        || payload.contains(&0)
        || !payload.is_ascii()
    {
        return Err(ControllerError::policy(
            "sealed builder did not emit one bounded canonical JSON line",
        ));
    }
    let schema = format!("\"schema\":\"{BUILDER_INNER_REPORT_SCHEMA}\"");
    if payload
        .windows(schema.len())
        .filter(|window| *window == schema.as_bytes())
        .count()
        != 1
    {
        return Err(ControllerError::policy(
            "sealed builder inner report schema is not exact",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_builder_environment() -> Result<()> {
    let allowed = BTreeSet::from(["LANG", "LC_ALL", "PATH", "TMPDIR"]);
    for (name, value) in std::env::vars_os() {
        let name = name
            .to_str()
            .ok_or_else(|| ControllerError::policy("builder launch environment is not UTF-8"))?;
        if !allowed.contains(name) || value.as_encoded_bytes().contains(&0) {
            return Err(ControllerError::policy(format!(
                "builder launch environment contains unsupported variable {name}"
            )));
        }
    }
    for (name, value) in [
        ("LANG", "C"),
        ("LC_ALL", "C"),
        ("PATH", "/usr/bin:/bin"),
        ("TMPDIR", "/private/var/tmp"),
    ] {
        if std::env::var_os(name).as_deref() != Some(OsStr::new(value)) {
            return Err(ControllerError::policy(format!(
                "builder launch environment {name} is not exact"
            )));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_readiness_environment(
    launch: &ReadinessLaunch,
    controller_path: &Path,
    controller_sha256: [u8; 32],
) -> Result<BTreeMap<OsString, OsString>> {
    let mut environment = validate_exact_environment(
        std::env::vars_os().collect(),
        READINESS_EXTERNAL_ENVIRONMENT_NAMES,
        "readiness launch environment",
    )?;
    let gate_sha256 = hex(&launch.gate_sha256);
    let python_path = launch.common.runtime_root.join(PYTHON_RELATIVE);
    let python_sha256 = hex(&launch.common.python_sha256);
    let runtime_tree_sha256 = hex(&launch.common.runtime_tree_sha256);
    let controller_sha256 = hex(&controller_sha256);
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH",
        launch.gate_source.as_os_str(),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256",
        OsStr::new(gate_sha256.as_str()),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON",
        python_path.as_os_str(),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256",
        OsStr::new(python_sha256.as_str()),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT",
        launch.common.runtime_root.as_os_str(),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256",
        OsStr::new(runtime_tree_sha256.as_str()),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD",
        OsStr::new(launch.common.expected_macos_build.as_str()),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN",
        controller_path.as_os_str(),
        "readiness launch environment",
    )?;
    require_environment_value(
        &environment,
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256",
        OsStr::new(controller_sha256.as_str()),
        "readiness launch environment",
    )?;
    for name in READINESS_CALLER_ONLY_ENVIRONMENT_NAMES {
        environment.remove(OsStr::new(*name));
    }
    Ok(environment)
}

#[cfg(target_os = "macos")]
fn exit_status(status: std::process::ExitStatus) -> Result<u8> {
    use std::os::unix::process::ExitStatusExt;
    if let Some(code) = status.code() {
        return u8::try_from(code)
            .map_err(|_| ControllerError::policy("native Python exit status is invalid"));
    }
    let signal = status
        .signal()
        .ok_or_else(|| ControllerError::policy("native Python termination is unavailable"))?;
    Ok(u8::try_from(128i32.saturating_add(signal)).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thin_macho(dependency: &str, rpath: Option<&str>) -> Vec<u8> {
        fn command(kind: u32, value: &str) -> Vec<u8> {
            let raw_size = 12 + value.len() + 1;
            let size = (raw_size + 3) & !3;
            let mut bytes = vec![0u8; size];
            bytes[0..4].copy_from_slice(&kind.to_le_bytes());
            bytes[4..8].copy_from_slice(&(size as u32).to_le_bytes());
            bytes[8..12].copy_from_slice(&12u32.to_le_bytes());
            bytes[12..12 + value.len()].copy_from_slice(value.as_bytes());
            bytes
        }
        let mut commands = Vec::new();
        if let Some(value) = rpath {
            commands.extend(command(0x8000_001c, value));
        }
        commands.extend(command(0x0c, dependency));
        let mut bytes = vec![0u8; 32];
        bytes[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&(1u32 + u32::from(rpath.is_some())).to_le_bytes());
        bytes[20..24].copy_from_slice(&(commands.len() as u32).to_le_bytes());
        bytes.extend(commands);
        bytes
    }

    #[test]
    fn macho_rejects_non_os_dependency_outside_runtime() {
        let parsed = macho(&thin_macho("/opt/hostile/libescape.dylib", None))
            .expect("parse synthetic Mach-O")
            .expect("Mach-O image");
        let images = BTreeMap::from([("bin/python3".to_owned(), parsed)]);
        let error = validate_macho_closure(Path::new("/sealed/runtime"), &images)
            .expect_err("outside dependency must reject");
        assert!(error.message.contains("escapes the sealed runtime"));
    }

    #[test]
    fn macho_accepts_unique_runtime_and_os_dependencies() {
        let python = macho(&thin_macho(
            "@rpath/libpython.dylib",
            Some("@executable_path/../lib"),
        ))
        .expect("parse Python")
        .expect("Python image");
        let library = macho(&thin_macho("/usr/lib/libSystem.B.dylib", None))
            .expect("parse library")
            .expect("library image");
        let images = BTreeMap::from([
            ("bin/python3".to_owned(), python),
            ("lib/libpython.dylib".to_owned(), library),
        ]);
        validate_macho_closure(Path::new("/sealed/runtime"), &images).expect("closed dependencies");
    }

    #[test]
    fn loader_token_suffix_requires_one_separator() {
        assert_eq!(loader_token_suffix("").expect("empty token suffix"), "");
        assert_eq!(
            loader_token_suffix("/../lib").expect("relative token suffix"),
            "../lib"
        );
        assert!(loader_token_suffix("lib").is_err());
        assert!(loader_token_suffix("//lib").is_err());
    }

    #[test]
    fn macho_rejects_dyld_environment_command() {
        let mut bytes = thin_macho("/usr/lib/libSystem.B.dylib", None);
        let command_offset = 32;
        bytes[command_offset..command_offset + 4].copy_from_slice(&0x27u32.to_le_bytes());
        let error = macho(&bytes).expect_err("dyld environment must reject");
        assert!(error.message.contains("dyld environment"));
    }

    #[test]
    fn direct_builder_payload_schema_is_not_the_native_envelope_schema() {
        assert_ne!(BUILDER_INNER_REPORT_SCHEMA, BUILDER_REPORT_SCHEMA);
        assert_eq!(
            BUILDER_REPORT_SCHEMA,
            "iroha.kagemusha.native_sealed_candidate_double_build_report.v2"
        );
    }

    #[test]
    fn builder_arguments_are_exact_and_ordered() {
        let mut arguments = Vec::new();
        for name in BUILDER_ARGUMENT_NAMES {
            arguments.push(OsString::from(name));
            let value = match name {
                "--runtime-uid" | "--runtime-gid" => "501".to_owned(),
                name if name.ends_with("sha256") => "1".repeat(64),
                _ => format!("/sealed/{}", name.trim_start_matches("--")),
            };
            arguments.push(OsString::from(value));
        }
        validate_builder_arguments(&arguments).expect("exact arguments");
        arguments.swap(0, 2);
        assert!(validate_builder_arguments(&arguments).is_err());
    }

    fn exact_readiness_environment_entries() -> Vec<(OsString, OsString)> {
        READINESS_BASE_ENVIRONMENT
            .iter()
            .map(|(name, value)| (OsString::from(*name), OsString::from(*value)))
            .chain(
                READINESS_EXTERNAL_ENVIRONMENT_NAMES
                    .iter()
                    .map(|name| (OsString::from(*name), OsString::from("authenticated-value"))),
            )
            .collect()
    }

    #[test]
    fn readiness_environment_rejects_unknown_kagemusha_variable() {
        let exact = exact_readiness_environment_entries();
        validate_exact_environment(
            exact.clone(),
            READINESS_EXTERNAL_ENVIRONMENT_NAMES,
            "test readiness environment",
        )
        .expect("exact inventory");

        let mut hostile = exact;
        hostile.push((
            OsString::from("KAGEMUSHA_HOSTILE_PYTHONSTARTUP"),
            OsString::from("/tmp/hostile.py"),
        ));
        let error = validate_exact_environment(
            hostile,
            READINESS_EXTERNAL_ENVIRONMENT_NAMES,
            "test readiness environment",
        )
        .expect_err("unknown Kagemusha variable must reject");
        assert!(error.message.contains("inventory is not exact"));
    }

    #[test]
    fn readiness_environment_rejects_missing_and_duplicate_variables() {
        let mut missing = exact_readiness_environment_entries();
        missing.retain(|(name, _)| name.as_os_str() != OsStr::new("KAGEMUSHA_V4_ARTIFACT_ROOT"));
        assert!(
            validate_exact_environment(
                missing,
                READINESS_EXTERNAL_ENVIRONMENT_NAMES,
                "test readiness environment",
            )
            .is_err()
        );

        let mut duplicate = exact_readiness_environment_entries();
        duplicate.push((OsString::from("LANG"), OsString::from("C")));
        let error = validate_exact_environment(
            duplicate,
            READINESS_EXTERNAL_ENVIRONMENT_NAMES,
            "test readiness environment",
        )
        .expect_err("duplicate environment entry must reject");
        assert!(error.message.contains("duplicate variable LANG"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn captured_job_reaps_descendant_after_successful_leader_exit() {
        use std::process::Command;

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("(trap '' TERM; sleep 30) & exit 0")
            .env_clear()
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("PATH", "/usr/bin:/bin");
        let captured =
            macos::run_captured(command, Vec::new(), 5, 4096, 4096).expect("descendant cleanup");
        assert!(captured.status.success());
        assert!(captured.stdout.is_empty());
        assert!(captured.stderr.is_empty());
    }
}

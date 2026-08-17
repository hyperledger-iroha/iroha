//! Authority-owned native qualification probe sandbox.
//!
//! The authority receives already-open artifact descriptors.  This module
//! independently hashes those descriptors, executes the fixed probes with a
//! closed descriptor table and a hostile identity, then hashes every
//! descriptor again before it releases a result to the signing service.

use super::{protocol::TairaAuthorityArtifactManifestEntryV1, service::TairaAuthorityErrorV1};
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, Metadata},
    os::unix::{fs::FileExt as _, fs::MetadataExt as _},
};

const CAPABILITY_ARTIFACT_V1: &str = "source/capability/exact12-capability-manifest-v1.norito";
const WHEEL_ARTIFACT_V1: &str = "source/sdk/iroha_python_privacy_v1.whl";
const WORKER_ARTIFACT_V1: &str = "source/worker/iroha_privacy_wallet_worker";
const ABI_LIBRARY_ARTIFACT_V1: &str = "source/abi22/libconnect_norito_bridge.so";
const MAX_PROBE_RESULT_BYTES_V1: usize = 256 * 1024;
#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
const MAX_PROBE_DIAGNOSTIC_BYTES_V1: usize = 64 * 1024;
#[cfg(any(
    test,
    all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
const QUALIFICATION_SERVICE_ID_V1: u32 = 0;
#[cfg(any(
    test,
    all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
const QUALIFICATION_HOSTILE_ID_V1: u32 = 65_534;

#[cfg(any(
    test,
    all(
        target_os = "linux",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
const fn qualification_hostile_identity(
    service_user_id: u32,
    process_group_id: u32,
) -> Option<(u32, u32)> {
    if service_user_id == QUALIFICATION_SERVICE_ID_V1
        && process_group_id == QUALIFICATION_SERVICE_ID_V1
    {
        Some((QUALIFICATION_HOSTILE_ID_V1, QUALIFICATION_HOSTILE_ID_V1))
    } else {
        None
    }
}
const WHEEL_RESULT_FIELDS_V1: [&str; 7] = [
    "capability_binding",
    "capability_binding_sha256",
    "capability_manifest_sha256",
    "compiled_profile_catalog_sha256",
    "native_member",
    "result",
    "wheel_sha256",
];

const PRIVACY_C_EXPORTS_V1: [&str; 5] = [
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
];
const CAPABILITY_PROTOCOLS_V1: [&str; 12] = [
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
];
const CAPABILITY_TUPLE_FIELDS_V1: [&str; 19] = [
    "activation_state",
    "committed_height",
    "compiled_profile_status",
    "engine_id",
    "engine_manifest_digest",
    "execution_mode",
    "limitation",
    "manifest_digest",
    "network_available",
    "operation_schema",
    "parameter_digest",
    "parameter_id",
    "privacy_feature_mask",
    "proof_system_id",
    "protocol_id",
    "readiness",
    "statement_schema_digest",
    "unavailable_reason",
    "verifier_digest",
];

/// Validated role result inserted below
/// `authority_envelope.claims.role_result.probe_results`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct QualificationProbeResultsV1 {
    abi22: Value,
    python_wheel: Value,
}

impl QualificationProbeResultsV1 {
    /// Convert the closed probe result into its canonical envelope value.
    pub(super) fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("abi22".into(), self.abi22.clone());
        object.insert("python_wheel".into(), self.python_wheel.clone());
        Value::Object(object)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RequiredArtifactOrdinalsV1 {
    capability: usize,
    wheel: usize,
    worker: usize,
    abi_library: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DescriptorIdentityV1 {
    device: u64,
    inode: u64,
    length: u64,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

/// Run the native candidate probes in the authority-owned Linux sandbox.
///
/// Artifact paths are never accepted.  `manifest[n]` describes only
/// `artifacts[n]`, and all reads use the received descriptor itself.
pub(super) fn run_qualification_probes(
    artifacts: &mut [File],
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
) -> Result<QualificationProbeResultsV1, TairaAuthorityErrorV1> {
    let required = required_artifact_ordinals(manifest, artifacts.len())?;
    let identities = validate_artifact_descriptors(artifacts, manifest, None)?;

    let probe = platform::run(artifacts, manifest, required);
    // Post-execution validation is deliberately unconditional.  A failing
    // child does not get to hide a concurrent artifact mutation.
    let post_validation = validate_artifact_descriptors(artifacts, manifest, Some(&identities));
    post_validation?;
    let output = probe?;
    parse_probe_result(&output, manifest, required)
}

fn required_artifact_ordinals(
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    descriptor_count: usize,
) -> Result<RequiredArtifactOrdinalsV1, TairaAuthorityErrorV1> {
    if manifest.is_empty() || manifest.len() != descriptor_count {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut names = BTreeSet::new();
    for (index, entry) in manifest.iter().enumerate() {
        if usize::from(entry.ordinal) != index || !names.insert(entry.name.as_str()) {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
    }
    let find = |name: &str| {
        manifest
            .iter()
            .position(|entry| entry.name == name)
            .ok_or(TairaAuthorityErrorV1::Rejected)
    };
    Ok(RequiredArtifactOrdinalsV1 {
        capability: find(CAPABILITY_ARTIFACT_V1)?,
        wheel: find(WHEEL_ARTIFACT_V1)?,
        worker: find(WORKER_ARTIFACT_V1)?,
        abi_library: find(ABI_LIBRARY_ARTIFACT_V1)?,
    })
}

fn validate_artifact_descriptors(
    artifacts: &[File],
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    expected_identities: Option<&[DescriptorIdentityV1]>,
) -> Result<Vec<DescriptorIdentityV1>, TairaAuthorityErrorV1> {
    if artifacts.len() != manifest.len()
        || expected_identities.is_some_and(|expected| expected.len() != artifacts.len())
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut identities = Vec::with_capacity(artifacts.len());
    for (index, (file, expected)) in artifacts.iter().zip(manifest).enumerate() {
        let before = file
            .metadata()
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        if !before.is_file() || before.nlink() != 1 || before.len() != expected.size {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let identity = descriptor_identity(&before);
        if expected_identities.is_some_and(|identities| identities[index] != identity) {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let digest = descriptor_sha256(file, expected.size)?;
        let after = file
            .metadata()
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        if digest != expected.sha256 || descriptor_identity(&after) != identity {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        identities.push(identity);
    }
    Ok(identities)
}

fn descriptor_sha256(file: &File, expected_size: u64) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    while offset < expected_size {
        let remaining = expected_size - offset;
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        let read = file
            .read_at(&mut buffer[..requested], offset)
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        if read == 0 {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(u64::try_from(read).map_err(|_| TairaAuthorityErrorV1::Rejected)?)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
    }
    let mut extra = [0_u8; 1];
    if file
        .read_at(&mut extra, expected_size)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?
        != 0
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(hasher.finalize().into())
}

fn descriptor_identity(metadata: &Metadata) -> DescriptorIdentityV1 {
    DescriptorIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        links: metadata.nlink(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    }
}

fn parse_probe_result(
    bytes: &[u8],
    manifest: &[TairaAuthorityArtifactManifestEntryV1],
    required: RequiredArtifactOrdinalsV1,
) -> Result<QualificationProbeResultsV1, TairaAuthorityErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_PROBE_RESULT_BYTES_V1 || !bytes.is_ascii() {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let value: Value =
        norito::json::from_slice(bytes).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut canonical = norito::json::to_json_pretty(&value)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?
        .into_bytes();
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let outer = exact_object(&value, &["abi22", "python_wheel"])?;
    let abi22 = outer.get("abi22").ok_or(TairaAuthorityErrorV1::Rejected)?;
    let wheel = outer
        .get("python_wheel")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    validate_abi_result(abi22, &manifest[required.abi_library])?;
    validate_wheel_result(
        wheel,
        &manifest[required.capability],
        &manifest[required.wheel],
        abi22,
    )?;
    Ok(QualificationProbeResultsV1 {
        abi22: abi22.clone(),
        python_wheel: wheel.clone(),
    })
}

fn validate_abi_result(
    value: &Value,
    library: &TairaAuthorityArtifactManifestEntryV1,
) -> Result<(), TairaAuthorityErrorV1> {
    let object = exact_object(
        value,
        &[
            "abi_version",
            "compiled_profile_catalog_sha256",
            "library_sha256",
            "privacy_c_exports",
            "result",
        ],
    )?;
    if object.get("abi_version").and_then(Value::as_u64) != Some(22)
        || object.get("result").and_then(Value::as_str) != Some("passed")
        || object.get("library_sha256").and_then(Value::as_str)
            != Some(hex::encode(library.sha256).as_str())
        || !is_sha256_value(object.get("compiled_profile_catalog_sha256"))
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let exports = object
        .get("privacy_c_exports")
        .and_then(Value::as_array)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    if exports.len() != PRIVACY_C_EXPORTS_V1.len()
        || exports
            .iter()
            .zip(PRIVACY_C_EXPORTS_V1)
            .any(|(value, expected)| value.as_str() != Some(expected))
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(())
}

fn validate_wheel_result(
    value: &Value,
    capability: &TairaAuthorityArtifactManifestEntryV1,
    wheel: &TairaAuthorityArtifactManifestEntryV1,
    abi22: &Value,
) -> Result<(), TairaAuthorityErrorV1> {
    let object = exact_object(value, &WHEEL_RESULT_FIELDS_V1)?;
    let abi_catalog = abi22
        .get("compiled_profile_catalog_sha256")
        .and_then(Value::as_str)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let native_member = object
        .get("native_member")
        .and_then(Value::as_str)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    if object.get("result").and_then(Value::as_str) != Some("passed")
        || object
            .get("compiled_profile_catalog_sha256")
            .and_then(Value::as_str)
            != Some(abi_catalog)
        || object
            .get("capability_manifest_sha256")
            .and_then(Value::as_str)
            != Some(hex::encode(capability.sha256).as_str())
        || object.get("wheel_sha256").and_then(Value::as_str)
            != Some(hex::encode(wheel.sha256).as_str())
        || !valid_native_member(native_member)
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let binding = object
        .get("capability_binding")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    validate_capability_binding(binding)?;
    let mut binding_bytes = norito::json::to_json_pretty(binding)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?
        .into_bytes();
    binding_bytes.push(b'\n');
    let binding_digest: [u8; 32] = Sha256::digest(&binding_bytes).into();
    if object
        .get("capability_binding_sha256")
        .and_then(Value::as_str)
        != Some(hex::encode(binding_digest).as_str())
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(())
}

fn validate_capability_binding(value: &Value) -> Result<(), TairaAuthorityErrorV1> {
    let object = exact_object(
        value,
        &[
            "manifest_protocol_tuples",
            "protocol_count",
            "required_network_protocol_tuples",
            "schema",
            "schema_version",
        ],
    )?;
    if object.get("schema").and_then(Value::as_str)
        != Some("iroha.taira.exact12-runtime-capability-binding")
        || object.get("schema_version").and_then(Value::as_u64) != Some(1)
        || object.get("protocol_count").and_then(Value::as_u64) != Some(12)
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let admitted = object
        .get("manifest_protocol_tuples")
        .and_then(Value::as_array)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let required = object
        .get("required_network_protocol_tuples")
        .and_then(Value::as_array)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    if admitted.len() != CAPABILITY_PROTOCOLS_V1.len() || admitted != required {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut shared_manifest_digest = None;
    for ((row, expected_protocol), index) in
        admitted.iter().zip(CAPABILITY_PROTOCOLS_V1).zip(0_usize..)
    {
        shared_manifest_digest = Some(validate_capability_tuple(
            row,
            expected_protocol,
            shared_manifest_digest,
            index == 6,
        )?);
    }
    Ok(())
}

fn validate_capability_tuple<'a>(
    value: &'a Value,
    expected_protocol: &str,
    shared_manifest_digest: Option<&str>,
    is_experimental: bool,
) -> Result<&'a str, TairaAuthorityErrorV1> {
    let row = exact_object(value, &CAPABILITY_TUPLE_FIELDS_V1)?;
    let protocol = row
        .get("protocol_id")
        .and_then(Value::as_str)
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let manifest_digest = row
        .get("manifest_digest")
        .and_then(Value::as_str)
        .filter(|value| is_sha256_text(value))
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    if protocol != expected_protocol
        || shared_manifest_digest.is_some_and(|digest| digest != manifest_digest)
        || row.get("network_available").and_then(Value::as_bool) != Some(true)
        || row.get("compiled_profile_status").and_then(Value::as_str) != Some("available")
        || row.get("activation_state").and_then(Value::as_str) != Some("active")
        || !row.get("unavailable_reason").is_some_and(Value::is_null)
        || row
            .get("committed_height")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            == 0
        || row
            .get("privacy_feature_mask")
            .and_then(Value::as_u64)
            .is_none()
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    if row.get("readiness").and_then(Value::as_str)
        != Some(if is_experimental {
            "available-experimental"
        } else {
            "available"
        })
        || if is_experimental {
            row.get("limitation").and_then(Value::as_str)
                != Some("missing-distribution-wide-knowledge-soundness-evidence")
        } else {
            !row.get("limitation").is_some_and(Value::is_null)
        }
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    for field in [
        "parameter_id",
        "parameter_digest",
        "verifier_digest",
        "statement_schema_digest",
        "engine_manifest_digest",
    ] {
        if !is_sha256_value(row.get(field)) {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
    }
    for field in [
        "operation_schema",
        "execution_mode",
        "proof_system_id",
        "engine_id",
    ] {
        if !row
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(valid_trust_identifier)
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
    }
    Ok(manifest_digest)
}

fn exact_object<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a Map, TairaAuthorityErrorV1> {
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(object)
}

fn is_sha256_value(value: Option<&Value>) -> bool {
    value.and_then(Value::as_str).is_some_and(is_sha256_text)
}

fn is_sha256_text(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_native_member(value: &str) -> bool {
    let Some(file) = value.strip_prefix("iroha_python/_crypto.") else {
        return false;
    };
    !file.is_empty()
        && file.strip_suffix(".so").is_some()
        && !value.contains("//")
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
}

fn valid_trust_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= 128
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod platform {
    use super::*;
    use std::{
        ffi::{c_int, c_long, c_uint, c_void},
        io::Read,
        os::{
            fd::{AsRawFd as _, RawFd},
            unix::{fs::PermissionsExt as _, process::CommandExt as _},
        },
        process::{Child, Command, ExitStatus, Stdio},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    const PYTHON_PATH_V1: &str = "/usr/bin/python3";
    const CANONICAL_EXECUTABLE_FD_V1: RawFd = 9;
    const CANONICAL_ARTIFACT_FDS_V1: [RawFd; 4] = [10, 11, 12, 13];
    const PROBE_TIMEOUT_V1: Duration = Duration::from_secs(120);
    const DESCENDANT_REAP_TIMEOUT_V1: Duration = Duration::from_secs(2);

    pub(super) fn run(
        artifacts: &[File],
        manifest: &[TairaAuthorityArtifactManifestEntryV1],
        required: RequiredArtifactOrdinalsV1,
    ) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        let interpreter = open_fixed_interpreter()?;
        let selected = [
            artifacts[required.capability]
                .try_clone()
                .map_err(|_| TairaAuthorityErrorV1::State)?,
            artifacts[required.wheel]
                .try_clone()
                .map_err(|_| TairaAuthorityErrorV1::State)?,
            artifacts[required.worker]
                .try_clone()
                .map_err(|_| TairaAuthorityErrorV1::State)?,
            artifacts[required.abi_library]
                .try_clone()
                .map_err(|_| TairaAuthorityErrorV1::State)?,
        ];
        let source_fds = selected.map(|file| file.as_raw_fd());
        let executable_fd = interpreter.as_raw_fd();
        let parent_pid = unsafe { getpid() };
        let parent_uid = unsafe { geteuid() };
        let parent_gid = unsafe { getegid() };
        if parent_pid <= 1 || qualification_hostile_identity(parent_uid, parent_gid).is_none() {
            return Err(TairaAuthorityErrorV1::Rejected);
        }

        let mut command = Command::new(format!("/proc/self/fd/{CANONICAL_EXECUTABLE_FD_V1}"));
        command
            .arg("-I")
            .arg("-S")
            .arg("-c")
            .arg(PROBE_SCRIPT_V1)
            .args(CANONICAL_ARTIFACT_FDS_V1.map(|fd| fd.to_string()))
            .arg(hex::encode(manifest[required.capability].sha256))
            .arg(hex::encode(manifest[required.wheel].sha256))
            .arg(hex::encode(manifest[required.worker].sha256))
            .arg(hex::encode(manifest[required.abi_library].sha256))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("PATH", "/usr/bin:/bin")
            .env("PYTHONHASHSEED", "0")
            .current_dir("/");
        // SAFETY: the closure performs only async-signal-safe syscalls and
        // accesses values copied before `fork`.  It does not allocate, lock,
        // or inspect shared Rust state.
        unsafe {
            command.pre_exec(move || {
                install_child_controls(
                    parent_pid,
                    parent_uid,
                    parent_gid,
                    executable_fd,
                    source_fds,
                )
            });
        }
        let mut child = command
            .spawn()
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
        let pid = i32::try_from(child.id()).map_err(|_| TairaAuthorityErrorV1::State)?;
        let stdout = child.stdout.take().ok_or(TairaAuthorityErrorV1::State)?;
        let stderr = child.stderr.take().ok_or(TairaAuthorityErrorV1::State)?;
        let mut child = ChildGroupGuardV1 {
            child,
            pid,
            armed: true,
        };
        let overflow = Arc::new(AtomicBool::new(false));
        let stdout_reader = spawn_bounded_reader(stdout, MAX_PROBE_RESULT_BYTES_V1, &overflow);
        let stderr_reader = spawn_bounded_reader(stderr, MAX_PROBE_DIAGNOSTIC_BYTES_V1, &overflow);
        let (status, forced_termination) = wait_for_child(&mut child.child, pid, &overflow)?;
        let stdout = stdout_reader
            .join()
            .map_err(|_| TairaAuthorityErrorV1::State)??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| TairaAuthorityErrorV1::State)??;
        let descendants = terminate_remaining_group(pid)?;
        child.armed = false;
        if forced_termination
            || descendants
            || overflow.load(Ordering::Acquire)
            || !status.success()
            || !stderr.is_empty()
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        Ok(stdout)
    }

    struct ChildGroupGuardV1 {
        child: Child,
        pid: i32,
        armed: bool,
    }

    impl Drop for ChildGroupGuardV1 {
        fn drop(&mut self) {
            if self.armed {
                let _ = unsafe { kill(-self.pid, SIGKILL) };
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }

    fn open_fixed_interpreter() -> Result<File, TairaAuthorityErrorV1> {
        let file = File::open(PYTHON_PATH_V1).map_err(|_| TairaAuthorityErrorV1::State)?;
        let metadata = file.metadata().map_err(|_| TairaAuthorityErrorV1::State)?;
        let mode = metadata.permissions().mode();
        if !metadata.is_file()
            || metadata.uid() != 0
            || metadata.nlink() != 1
            || mode & 0o022 != 0
            || mode & 0o111 == 0
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        Ok(file)
    }

    fn spawn_bounded_reader<R: Read + Send + 'static>(
        mut reader: R,
        limit: usize,
        overflow: &Arc<AtomicBool>,
    ) -> thread::JoinHandle<Result<Vec<u8>, TairaAuthorityErrorV1>> {
        let overflow = Arc::clone(overflow);
        thread::spawn(move || {
            let mut retained = Vec::with_capacity(limit.min(64 * 1024));
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let read = reader
                    .read(&mut buffer)
                    .map_err(|_| TairaAuthorityErrorV1::State)?;
                if read == 0 {
                    break;
                }
                if retained.len().saturating_add(read) > limit {
                    overflow.store(true, Ordering::Release);
                } else {
                    retained.extend_from_slice(&buffer[..read]);
                }
            }
            Ok(retained)
        })
    }

    fn wait_for_child(
        child: &mut Child,
        pid: i32,
        overflow: &AtomicBool,
    ) -> Result<(ExitStatus, bool), TairaAuthorityErrorV1> {
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().map_err(|_| TairaAuthorityErrorV1::State)? {
                return Ok((status, false));
            }
            if overflow.load(Ordering::Acquire) || started.elapsed() >= PROBE_TIMEOUT_V1 {
                signal_process_group(pid, SIGKILL)?;
                let status = child.wait().map_err(|_| TairaAuthorityErrorV1::State)?;
                return Ok((status, true));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn terminate_remaining_group(pid: i32) -> Result<bool, TairaAuthorityErrorV1> {
        if !process_group_exists(pid)? {
            return Ok(false);
        }
        signal_process_group(pid, SIGKILL)?;
        let started = Instant::now();
        while process_group_exists(pid)? {
            if started.elapsed() >= DESCENDANT_REAP_TIMEOUT_V1 {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(true)
    }

    fn process_group_exists(pid: i32) -> Result<bool, TairaAuthorityErrorV1> {
        let result = unsafe { kill(-pid, 0) };
        if result == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(ESRCH) => Ok(false),
            Some(EPERM) => Ok(true),
            _ => Err(TairaAuthorityErrorV1::State),
        }
    }

    fn signal_process_group(pid: i32, signal: c_int) -> Result<(), TairaAuthorityErrorV1> {
        if unsafe { kill(-pid, signal) } == 0 {
            return Ok(());
        }
        if std::io::Error::last_os_error().raw_os_error() == Some(ESRCH) {
            return Ok(());
        }
        Err(TairaAuthorityErrorV1::State)
    }

    fn install_child_controls(
        parent_pid: i32,
        parent_uid: c_uint,
        parent_gid: c_uint,
        executable_fd: RawFd,
        artifact_fds: [RawFd; 4],
    ) -> std::io::Result<()> {
        if unsafe { setpgid(0, 0) } != 0
            || unsafe { prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) } != 0
            || unsafe { getppid() } != parent_pid
        {
            return Err(last_error());
        }
        stage_descriptors(executable_fd, artifact_fds)?;
        install_resource_limits()?;
        enter_hostile_identity(parent_uid, parent_gid)?;
        if unsafe { prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0
            || unsafe { prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        {
            return Err(last_error());
        }
        install_landlock_restriction()?;
        install_seccomp_filter()?;
        Ok(())
    }

    fn stage_descriptors(executable_fd: RawFd, artifact_fds: [RawFd; 4]) -> std::io::Result<()> {
        let mut staged = [0_i32; 5];
        for (slot, source) in std::iter::once(executable_fd)
            .chain(artifact_fds)
            .enumerate()
        {
            let duplicate = unsafe { fcntl(source, F_DUPFD_CLOEXEC, 64) };
            if duplicate < 0 {
                return Err(last_error());
            }
            staged[slot] = duplicate;
        }
        if unsafe { syscall(SYS_CLOSE_RANGE, 3_u32, u32::MAX, CLOSE_RANGE_CLOEXEC) } < 0 {
            return Err(last_error());
        }
        if unsafe { dup3(staged[0], CANONICAL_EXECUTABLE_FD_V1, O_CLOEXEC) }
            != CANONICAL_EXECUTABLE_FD_V1
        {
            return Err(last_error());
        }
        for (source, target) in staged[1..].iter().zip(CANONICAL_ARTIFACT_FDS_V1) {
            if unsafe { dup3(*source, target, 0) } != target {
                return Err(last_error());
            }
        }
        Ok(())
    }

    fn install_resource_limits() -> std::io::Result<()> {
        let limits = [
            (RLIMIT_CORE, 0_u64),
            (RLIMIT_FSIZE, 512 * 1024 * 1024),
            (RLIMIT_AS, 2 * 1024 * 1024 * 1024),
            (RLIMIT_CPU, 125),
            (RLIMIT_NOFILE, 64),
            (RLIMIT_NPROC, 32),
            (RLIMIT_STACK, 16 * 1024 * 1024),
        ];
        for (resource, ceiling) in limits {
            let limit = RLimit {
                current: ceiling,
                maximum: ceiling,
            };
            if unsafe { setrlimit(resource, &limit) } != 0 {
                return Err(last_error());
            }
        }
        Ok(())
    }

    fn enter_hostile_identity(parent_uid: c_uint, parent_gid: c_uint) -> std::io::Result<()> {
        let Some((hostile_uid, hostile_gid)) =
            qualification_hostile_identity(parent_uid, parent_gid)
        else {
            return Err(std::io::Error::from_raw_os_error(EPERM));
        };
        if unsafe { setgroups(0, std::ptr::null()) } != 0
            || unsafe { setresgid(hostile_gid, hostile_gid, hostile_gid) } != 0
            || unsafe { setresuid(hostile_uid, hostile_uid, hostile_uid) } != 0
        {
            return Err(last_error());
        }
        drop_capabilities()?;
        let mut real_uid = 0;
        let mut effective_uid = 0;
        let mut saved_uid = 0;
        let mut real_gid = 0;
        let mut effective_gid = 0;
        let mut saved_gid = 0;
        if unsafe { getgroups(0, std::ptr::null_mut()) } != 0
            || unsafe { getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) } != 0
            || unsafe { getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid) } != 0
            || [real_uid, effective_uid, saved_uid] != [hostile_uid; 3]
            || [real_gid, effective_gid, saved_gid] != [hostile_gid; 3]
        {
            return Err(std::io::Error::from_raw_os_error(EPERM));
        }
        Ok(())
    }

    fn drop_capabilities() -> std::io::Result<()> {
        let mut header = CapabilityHeader {
            version: LINUX_CAPABILITY_VERSION_3,
            pid: 0,
        };
        let empty = [CapabilityData::default(); 2];
        if unsafe { capset(&mut header, empty.as_ptr()) } != 0 {
            return Err(last_error());
        }
        let mut observed = [CapabilityData::default(); 2];
        if unsafe { capget(&mut header, observed.as_mut_ptr()) } != 0
            || observed != [CapabilityData::default(); 2]
        {
            return Err(std::io::Error::from_raw_os_error(EPERM));
        }
        Ok(())
    }

    fn install_landlock_restriction() -> std::io::Result<()> {
        let abi = unsafe {
            syscall(
                SYS_LANDLOCK_CREATE_RULESET,
                std::ptr::null::<c_void>(),
                0_usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if abi < 4 {
            return Err(std::io::Error::from_raw_os_error(EOPNOTSUPP));
        }
        let ruleset = LandlockRulesetAttr {
            handled_access_fs: 0,
            handled_access_net: LANDLOCK_ACCESS_NET_BIND_TCP | LANDLOCK_ACCESS_NET_CONNECT_TCP,
        };
        let ruleset_fd = unsafe {
            syscall(
                SYS_LANDLOCK_CREATE_RULESET,
                &ruleset,
                std::mem::size_of::<LandlockRulesetAttr>(),
                0_u32,
            )
        };
        if ruleset_fd < 0 {
            return Err(last_error());
        }
        let restricted = unsafe { syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset_fd, 0_u32) };
        unsafe { close(c_int::try_from(ruleset_fd).unwrap_or(-1)) };
        if restricted < 0 {
            return Err(last_error());
        }
        Ok(())
    }

    fn install_seccomp_filter() -> std::io::Result<()> {
        let deny = SECCOMP_RET_ERRNO | u32::try_from(EPERM).unwrap_or(1);
        const FILTER_LENGTH: usize = 6 + DENIED_SYSCALLS.len() * 2 + 1;
        let mut filter = [bpf_statement(BPF_RET_K, SECCOMP_RET_ALLOW); FILTER_LENGTH];
        filter[0] = bpf_statement(BPF_LD_W_ABS, 4);
        filter[1] = bpf_jump(BPF_JMP_JEQ_K, AUDIT_ARCH, 1, 0);
        filter[2] = bpf_statement(BPF_RET_K, SECCOMP_RET_KILL_PROCESS);
        filter[3] = bpf_statement(BPF_LD_W_ABS, 0);
        filter[4] = bpf_jump(BPF_JMP_JSET_K, FORBIDDEN_SYSCALL_ABI_MASK, 0, 1);
        filter[5] = bpf_statement(BPF_RET_K, deny);
        let mut cursor = 6;
        for syscall_number in DENIED_SYSCALLS {
            filter[cursor] = bpf_jump(BPF_JMP_JEQ_K, *syscall_number, 0, 1);
            filter[cursor + 1] = bpf_statement(BPF_RET_K, deny);
            cursor += 2;
        }
        filter[cursor] = bpf_statement(BPF_RET_K, SECCOMP_RET_ALLOW);
        let program = SockFprog {
            length: u16::try_from(filter.len())
                .map_err(|_| std::io::Error::from_raw_os_error(EOVERFLOW))?,
            filter: filter.as_mut_ptr(),
        };
        if unsafe {
            prctl(
                PR_SET_SECCOMP,
                SECCOMP_MODE_FILTER,
                (&raw const program).addr(),
                0,
                0,
            )
        } != 0
        {
            return Err(last_error());
        }
        Ok(())
    }

    const fn bpf_statement(code: u16, value: u32) -> SockFilter {
        SockFilter {
            code,
            jump_true: 0,
            jump_false: 0,
            value,
        }
    }

    const fn bpf_jump(code: u16, value: u32, jump_true: u8, jump_false: u8) -> SockFilter {
        SockFilter {
            code,
            jump_true,
            jump_false,
            value,
        }
    }

    fn last_error() -> std::io::Error {
        std::io::Error::last_os_error()
    }

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    #[repr(C)]
    struct CapabilityData {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }

    #[repr(C)]
    struct CapabilityHeader {
        version: u32,
        pid: i32,
    }

    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
    }

    #[repr(C)]
    struct RLimit {
        current: u64,
        maximum: u64,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct SockFilter {
        code: u16,
        jump_true: u8,
        jump_false: u8,
        value: u32,
    }

    #[repr(C)]
    struct SockFprog {
        length: u16,
        filter: *mut SockFilter,
    }

    unsafe extern "C" {
        fn capget(header: *mut CapabilityHeader, data: *mut CapabilityData) -> c_int;
        fn capset(header: *mut CapabilityHeader, data: *const CapabilityData) -> c_int;
        fn close(fd: c_int) -> c_int;
        fn dup3(old_fd: c_int, new_fd: c_int, flags: c_int) -> c_int;
        fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
        fn getegid() -> c_uint;
        fn geteuid() -> c_uint;
        fn getgroups(size: c_int, list: *mut c_uint) -> c_int;
        fn getpid() -> c_int;
        fn getppid() -> c_int;
        fn getresgid(real: *mut c_uint, effective: *mut c_uint, saved: *mut c_uint) -> c_int;
        fn getresuid(real: *mut c_uint, effective: *mut c_uint, saved: *mut c_uint) -> c_int;
        fn kill(pid: c_int, signal: c_int) -> c_int;
        fn prctl(option: c_int, ...) -> c_int;
        fn setgroups(size: usize, list: *const c_uint) -> c_int;
        fn setpgid(pid: c_int, pgid: c_int) -> c_int;
        fn setresgid(real: c_uint, effective: c_uint, saved: c_uint) -> c_int;
        fn setresuid(real: c_uint, effective: c_uint, saved: c_uint) -> c_int;
        fn setrlimit(resource: c_uint, limit: *const RLimit) -> c_int;
        fn syscall(number: c_long, ...) -> c_long;
    }

    const EPERM: c_int = 1;
    const ESRCH: c_int = 3;
    const EOVERFLOW: c_int = 75;
    const EOPNOTSUPP: c_int = 95;
    const SIGKILL: c_int = 9;
    const O_CLOEXEC: c_int = 0o2_000_000;
    const F_DUPFD_CLOEXEC: c_int = 1030;
    const CLOSE_RANGE_CLOEXEC: c_uint = 4;
    const PR_SET_PDEATHSIG: c_int = 1;
    const PR_SET_DUMPABLE: c_int = 4;
    const PR_SET_SECCOMP: c_int = 22;
    const PR_SET_NO_NEW_PRIVS: c_int = 38;
    const SECCOMP_MODE_FILTER: usize = 2;
    const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const BPF_LD_W_ABS: u16 = 0x20;
    const BPF_JMP_JEQ_K: u16 = 0x15;
    const BPF_JMP_JSET_K: u16 = 0x45;
    const BPF_RET_K: u16 = 0x06;
    const RLIMIT_CPU: c_uint = 0;
    const RLIMIT_FSIZE: c_uint = 1;
    const RLIMIT_STACK: c_uint = 3;
    const RLIMIT_CORE: c_uint = 4;
    const RLIMIT_NPROC: c_uint = 6;
    const RLIMIT_NOFILE: c_uint = 7;
    const RLIMIT_AS: c_uint = 9;
    const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
    const LANDLOCK_CREATE_RULESET_VERSION: c_uint = 1;
    const LANDLOCK_ACCESS_NET_BIND_TCP: u64 = 1;
    const LANDLOCK_ACCESS_NET_CONNECT_TCP: u64 = 2;
    const SYS_CLOSE_RANGE: c_long = 436;
    const SYS_LANDLOCK_CREATE_RULESET: c_long = 444;
    const SYS_LANDLOCK_RESTRICT_SELF: c_long = 446;

    #[cfg(target_arch = "x86_64")]
    const AUDIT_ARCH: u32 = 0xc000_003e;
    #[cfg(target_arch = "x86_64")]
    const FORBIDDEN_SYSCALL_ABI_MASK: u32 = 0x4000_0000;
    #[cfg(target_arch = "aarch64")]
    const AUDIT_ARCH: u32 = 0xc000_00b7;
    #[cfg(target_arch = "aarch64")]
    const FORBIDDEN_SYSCALL_ABI_MASK: u32 = 0;

    #[cfg(target_arch = "x86_64")]
    const DENIED_SYSCALLS: &[u32] = &[
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 109, 112, 272, 288, 299, 307,
        308, 321, 425,
    ];
    #[cfg(target_arch = "aarch64")]
    const DENIED_SYSCALLS: &[u32] = &[
        97, 154, 157, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212,
        242, 243, 268, 269, 280, 425,
    ];

    // The script receives four descriptor numbers and four expected hashes.
    // It never receives an artifact path.  Files staged below /tmp are copied
    // from those descriptors into a private, hostile-identity-owned directory.
    const PROBE_SCRIPT_V1: &str = r#"
import ctypes
import errno
import hashlib
import importlib.machinery
import importlib.util
import json
import os
import pathlib
import re
import resource
import stat
import subprocess
import sys
import tempfile
import zipfile

CAPABILITY_FD, WHEEL_FD, WORKER_FD, ABI_FD = map(int, sys.argv[1:5])
CAPABILITY_SHA, WHEEL_SHA, WORKER_SHA, ABI_SHA = sys.argv[5:9]
ALLOWED_ENV = {"LANG", "LC_ALL", "PATH", "PYTHONHASHSEED"}
if set(os.environ) != ALLOWED_ENV:
    raise SystemExit("scrubbed environment differs")
if os.getuid() != 65534 or os.geteuid() != 65534 or os.getgid() != 65534 or os.getegid() != 65534:
    raise SystemExit("hostile identity differs")
if os.getgroups():
    raise SystemExit("supplementary groups survived")
if os.getpid() != os.getpgrp():
    raise SystemExit("probe is not its private process-group leader")
libc = ctypes.CDLL(None, use_errno=True)
if libc.prctl(39, 0, 0, 0, 0) != 1:
    raise SystemExit("no_new_privs is not active")
if libc.socket(2, 1, 0) != -1 or ctypes.get_errno() != errno.EPERM:
    raise SystemExit("socket denial is not active")
expected_limits = {
    resource.RLIMIT_CORE: 0,
    resource.RLIMIT_FSIZE: 512 * 1024 * 1024,
    resource.RLIMIT_AS: 2 * 1024 * 1024 * 1024,
    resource.RLIMIT_CPU: 125,
    resource.RLIMIT_NOFILE: 64,
    resource.RLIMIT_NPROC: 32,
    resource.RLIMIT_STACK: 16 * 1024 * 1024,
}
for kind, expected in expected_limits.items():
    if resource.getrlimit(kind) != (expected, expected):
        raise SystemExit("resource limit differs")
allowed_fds = {0, 1, 2, CAPABILITY_FD, WHEEL_FD, WORKER_FD, ABI_FD}
observed_fds = set()
for item in os.listdir("/proc/self/fd"):
    try:
        fd = int(item)
        os.fstat(fd)
    except (OSError, ValueError):
        continue
    observed_fds.add(fd)
if observed_fds != allowed_fds:
    raise SystemExit("inherited descriptor table differs")

def read_fd(fd, maximum):
    facts = os.fstat(fd)
    if not stat.S_ISREG(facts.st_mode) or facts.st_nlink != 1 or not 1 <= facts.st_size <= maximum:
        raise SystemExit("artifact descriptor identity differs")
    chunks = []
    offset = 0
    while offset < facts.st_size:
        chunk = os.pread(fd, min(1024 * 1024, facts.st_size - offset), offset)
        if not chunk:
            raise SystemExit("artifact descriptor was truncated")
        chunks.append(chunk)
        offset += len(chunk)
    if os.pread(fd, 1, facts.st_size):
        raise SystemExit("artifact descriptor grew")
    return b"".join(chunks)

def require_hash(payload, expected, label):
    if hashlib.sha256(payload).hexdigest() != expected:
        raise SystemExit(label + " digest differs")

capability = read_fd(CAPABILITY_FD, 256 * 1024)
worker_payload = read_fd(WORKER_FD, 512 * 1024 * 1024)
require_hash(capability, CAPABILITY_SHA, "capability")
require_hash(worker_payload, WORKER_SHA, "worker")
require_hash(read_fd(ABI_FD, 512 * 1024 * 1024), ABI_SHA, "ABI library")

library = ctypes.CDLL("/proc/self/fd/%d" % ABI_FD)
if library.connect_norito_bridge_abi_version() != 22:
    raise SystemExit("ABI version differs")
exports = (
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
)
if any(not callable(getattr(library, name, None)) for name in exports):
    raise SystemExit("ABI export surface differs")
getter = library.iroha_privacy_compiled_profile_catalog_v1
validator = library.iroha_privacy_validate_compiled_profile_catalog_v1
free_buffer = library.iroha_privacy_free_buffer
getter.argtypes = [ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)), ctypes.POINTER(ctypes.c_ulong)]
getter.restype = ctypes.c_int32
validator.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_ulong]
validator.restype = ctypes.c_int32
free_buffer.argtypes = [ctypes.POINTER(ctypes.c_uint8)]
free_buffer.restype = None

def get_catalog():
    pointer = ctypes.POINTER(ctypes.c_uint8)()
    length = ctypes.c_ulong(0)
    if getter(ctypes.byref(pointer), ctypes.byref(length)) != 0 or not pointer or not 16 <= length.value <= 256 * 1024:
        raise SystemExit("compiled catalog getter failed")
    try:
        payload = ctypes.string_at(pointer, length.value)
        copied = (ctypes.c_uint8 * len(payload)).from_buffer_copy(payload)
        if validator(copied, len(payload)) != 0:
            raise SystemExit("compiled catalog validator failed")
        return payload
    finally:
        free_buffer(pointer)

catalog = get_catalog()
if catalog[:4] != b"NRT0" or not any(catalog[4:]) or get_catalog() != catalog:
    raise SystemExit("compiled catalog is not stable canonical Norito")
catalog_sha = hashlib.sha256(catalog).hexdigest()

protocols = (
    "zk-ace-pq-authorization-v0", "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1", "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0", "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0", "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1", "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1", "pq-masp-stark-v0",
)
tuple_fields = {
    "activation_state", "committed_height", "compiled_profile_status", "engine_id",
    "engine_manifest_digest", "execution_mode", "limitation", "manifest_digest",
    "network_available", "operation_schema", "parameter_digest", "parameter_id",
    "privacy_feature_mask", "proof_system_id", "protocol_id", "readiness",
    "statement_schema_digest", "unavailable_reason", "verifier_digest",
}

def require_tuple(row, protocol):
    if not isinstance(row, dict) or set(row) != tuple_fields or row.get("protocol_id") != protocol:
        raise SystemExit("capability tuple shape differs")
    experimental = protocol == "iroha-jindo-polynomial-commitment-v0"
    if (row.get("network_available") is not True or row.get("compiled_profile_status") != "available"
        or row.get("activation_state") != "active" or row.get("unavailable_reason") is not None
        or row.get("readiness") != ("available-experimental" if experimental else "available")
        or row.get("limitation") != ("missing-distribution-wide-knowledge-soundness-evidence" if experimental else None)):
        raise SystemExit("capability tuple is not release-ready")

def normalize(value):
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value).hex()
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, dict):
        if any(not isinstance(key, str) for key in value):
            raise SystemExit("capability tuple key differs")
        return {key: normalize(value[key]) for key in sorted(value)}
    if isinstance(value, (list, tuple)):
        return [normalize(item) for item in value]
    raise SystemExit("capability tuple value is not canonical")

with tempfile.TemporaryDirectory(prefix="iroha-taira-qualification-") as temporary_raw:
    temporary = pathlib.Path(temporary_raw)
    with os.fdopen(os.dup(WHEEL_FD), "rb") as wheel_stream, zipfile.ZipFile(wheel_stream) as wheel:
        infos = wheel.infolist()
        if not infos or len(infos) > 100000:
            raise SystemExit("wheel member count differs")
        names = []
        logical = 0
        for info in infos:
            name = info.filename
            parts = pathlib.PurePosixPath(name)
            mode = info.external_attr >> 16
            if (not name or name.endswith("/") or parts.is_absolute() or any(part in {"", ".", ".."} for part in parts.parts)
                or parts.as_posix() != name or stat.S_ISLNK(mode) or info.flag_bits & 1
                or not 1 <= info.file_size <= 1024 * 1024 * 1024 or info.compress_size == 0
                or info.file_size > info.compress_size * 500):
                raise SystemExit("wheel member identity differs")
            logical += info.file_size
            if logical > 2 * 1024 * 1024 * 1024:
                raise SystemExit("wheel logical size differs")
            names.append(name)
        if len(names) != len(set(names)):
            raise SystemExit("wheel members repeat")
        native_members = [name for name in names if pathlib.PurePosixPath(name).parent.as_posix() == "iroha_python" and pathlib.PurePosixPath(name).name.startswith("_crypto.") and name.endswith(".so")]
        if len(native_members) != 1 or "iroha_python/privacy_wallet_worker.py" not in names:
            raise SystemExit("wheel native layout differs")
        native_member = native_members[0]
        wheel_metadata = [name for name in names if name.endswith(".dist-info/WHEEL")]
        if len(wheel_metadata) != 1:
            raise SystemExit("wheel metadata differs")
        metadata = wheel.read(wheel_metadata[0]).decode("utf-8")
        if "Root-Is-Purelib: false\n" not in metadata or re.search(r"(?m)^Tag: .*aarch64$", metadata) is None:
            raise SystemExit("wheel platform tag differs")
        native_payload = wheel.read(native_member)
        controller_payload = wheel.read("iroha_python/privacy_wallet_worker.py")
    require_hash(read_fd(WHEEL_FD, 1024 * 1024 * 1024), WHEEL_SHA, "wheel")
    if (len(native_payload) < 64 or native_payload[:4] != b"\x7fELF" or native_payload[5] != 1
        or int.from_bytes(native_payload[18:20], "little") != 183):
        raise SystemExit("wheel native member is not AArch64 ELF")

    def stage(name, payload, mode):
        path = temporary / name
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
        try:
            view = memoryview(payload)
            while view:
                written = os.write(fd, view)
                if written <= 0:
                    raise SystemExit("staged artifact short write")
                view = view[written:]
            os.fsync(fd)
        finally:
            os.close(fd)
        return path

    extension = stage(pathlib.PurePosixPath(native_member).name, native_payload, 0o700)
    controller_path = stage("privacy_wallet_worker.py", controller_payload, 0o600)
    worker_path = stage("iroha_privacy_wallet_worker", worker_payload, 0o700)
    name = "iroha_python._crypto"
    loader = importlib.machinery.ExtensionFileLoader(name, str(extension))
    spec = importlib.util.spec_from_loader(name, loader)
    if spec is None:
        raise SystemExit("wheel native import specification differs")
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    required_functions = (
        "connect_norito_bridge_abi_version", "privacy_compiled_profile_catalog_v1",
        "privacy_exact12_capability_manifest_v1", "privacy_validate_compiled_profile_catalog_v1",
        "privacy_validate_exact12_capability_manifest_v1",
    )
    if any(not callable(getattr(module, item, None)) for item in required_functions):
        raise SystemExit("wheel native functions differ")
    if module.connect_norito_bridge_abi_version() != 22 or module.privacy_validate_compiled_profile_catalog_v1(catalog) != 0 or bytes(module.privacy_compiled_profile_catalog_v1()) != catalog:
        raise SystemExit("wheel compiled catalog differs")
    if module.privacy_validate_exact12_capability_manifest_v1(capability) != 0:
        raise SystemExit("wheel rejected capability manifest")
    admitted = module.privacy_exact12_capability_manifest_v1(capability)
    if bytes(getattr(admitted, "canonical_archive", b"")) != capability:
        raise SystemExit("wheel canonical capability bytes differ")
    admitted_rows = [dict(row) for row in admitted.protocol_tuples()]
    required_rows = []
    if [row.get("protocol_id") for row in admitted_rows] != list(protocols):
        raise SystemExit("capability protocol order differs")
    for protocol, row in zip(protocols, admitted_rows):
        require_tuple(row, protocol)
        required = dict(admitted.require_network_capability(protocol))
        require_tuple(required, protocol)
        if required != row:
            raise SystemExit("required capability tuple differs")
        required_rows.append(required)
    binding = {
        "manifest_protocol_tuples": normalize(admitted_rows),
        "protocol_count": 12,
        "required_network_protocol_tuples": normalize(required_rows),
        "schema": "iroha.taira.exact12-runtime-capability-binding",
        "schema_version": 1,
    }

    controller_loader = importlib.machinery.SourceFileLoader("iroha_privacy_wallet_worker_controller", str(controller_path))
    controller_spec = importlib.util.spec_from_loader("iroha_privacy_wallet_worker_controller", controller_loader)
    if controller_spec is None:
        raise SystemExit("worker controller specification differs")
    controller_module = importlib.util.module_from_spec(controller_spec)
    sys.modules[controller_spec.name] = controller_module
    controller_loader.exec_module(controller_module)
    original_popen = controller_module.subprocess.Popen
    def contained_popen(*args, **kwargs):
        if kwargs.get("start_new_session") is not True:
            raise RuntimeError("worker launch isolation request differs")
        kwargs["start_new_session"] = False
        return original_popen(*args, **kwargs)
    controller_module.subprocess.Popen = contained_popen
    with controller_module.PrivacyWalletWorkerControllerV1(worker_path, expected_worker_sha256=WORKER_SHA) as controller:
        controller.ping()

binding_payload = (json.dumps(binding, indent=2, sort_keys=True, ensure_ascii=True, allow_nan=False) + "\n").encode("ascii")
result = {
    "abi22": {
        "abi_version": 22,
        "compiled_profile_catalog_sha256": catalog_sha,
        "library_sha256": ABI_SHA,
        "privacy_c_exports": list(exports),
        "result": "passed",
    },
    "python_wheel": {
        "capability_binding": binding,
        "capability_binding_sha256": hashlib.sha256(binding_payload).hexdigest(),
        "capability_manifest_sha256": CAPABILITY_SHA,
        "compiled_profile_catalog_sha256": catalog_sha,
        "native_member": native_member,
        "result": "passed",
        "wheel_sha256": WHEEL_SHA,
    },
}
sys.stdout.write(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=True, allow_nan=False) + "\n")
"#;
}

#[cfg(not(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
mod platform {
    use super::*;

    pub(super) fn run(
        _artifacts: &[File],
        _manifest: &[TairaAuthorityArtifactManifestEntryV1],
        _required: RequiredArtifactOrdinalsV1,
    ) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        Err(TairaAuthorityErrorV1::Rejected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Seek as _, Write as _},
        os::fd::AsRawFd as _,
    };

    const TEST_PROTOCOLS: [&str; 12] = [
        "zk-ace-pq-authorization-v0",
        "anonymous-pgc-k-out-of-n-v1",
        "verange-transparent-range-v1",
        "iroha-zk-ams-v1",
        "vega-existing-credential-zk-v0",
        "iroha-zk-x509-stark-p256-v0",
        "iroha-jindo-polynomial-commitment-v0",
        "iroha-bootle-lantern-anoncred-v1",
        "orchard-halo2-actions-v1",
        "monero-fcmp-plus-plus-v1",
        "iroha-ivm-private-note-stark-v1",
        "pq-masp-stark-v0",
    ];

    #[test]
    fn qualification_uses_a_distinct_host_identity() {
        assert_eq!(QUALIFICATION_SERVICE_ID_V1, 0);
        assert_eq!(QUALIFICATION_HOSTILE_ID_V1, 65_534);
        assert_ne!(QUALIFICATION_SERVICE_ID_V1, QUALIFICATION_HOSTILE_ID_V1);
        assert_eq!(qualification_hostile_identity(0, 0), Some((65_534, 65_534)));
        assert_eq!(qualification_hostile_identity(1_000, 1_000), None);
        assert_eq!(qualification_hostile_identity(0, 1_000), None);
        assert_eq!(qualification_hostile_identity(1_000, 0), None);
    }

    fn manifest_entry(
        ordinal: u16,
        name: &str,
        bytes: &[u8],
    ) -> TairaAuthorityArtifactManifestEntryV1 {
        TairaAuthorityArtifactManifestEntryV1 {
            ordinal,
            name: name.into(),
            size: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
        }
    }

    fn required_manifest(bytes: &[u8]) -> Vec<TairaAuthorityArtifactManifestEntryV1> {
        vec![
            manifest_entry(0, CAPABILITY_ARTIFACT_V1, bytes),
            manifest_entry(1, WHEEL_ARTIFACT_V1, bytes),
            manifest_entry(2, WORKER_ARTIFACT_V1, bytes),
            manifest_entry(3, ABI_LIBRARY_ARTIFACT_V1, bytes),
        ]
    }

    fn valid_binding() -> Value {
        let rows = TEST_PROTOCOLS
            .into_iter()
            .enumerate()
            .map(|(index, protocol)| {
                let mut row = Map::new();
                row.insert("activation_state".into(), Value::from("active"));
                row.insert("committed_height".into(), Value::from(1_u64));
                row.insert("compiled_profile_status".into(), Value::from("available"));
                row.insert("engine_id".into(), Value::from("engine-v1"));
                row.insert(
                    "engine_manifest_digest".into(),
                    Value::from("11".repeat(32)),
                );
                row.insert("execution_mode".into(), Value::from("native-v1"));
                row.insert(
                    "limitation".into(),
                    if index == 6 {
                        Value::from("missing-distribution-wide-knowledge-soundness-evidence")
                    } else {
                        Value::Null
                    },
                );
                row.insert("manifest_digest".into(), Value::from("22".repeat(32)));
                row.insert("network_available".into(), Value::from(true));
                row.insert("operation_schema".into(), Value::from("operation-v1"));
                row.insert("parameter_digest".into(), Value::from("33".repeat(32)));
                row.insert("parameter_id".into(), Value::from("44".repeat(32)));
                row.insert("privacy_feature_mask".into(), Value::from(1_u64));
                row.insert("proof_system_id".into(), Value::from("proof-v1"));
                row.insert("protocol_id".into(), Value::from(protocol));
                row.insert(
                    "readiness".into(),
                    Value::from(if index == 6 {
                        "available-experimental"
                    } else {
                        "available"
                    }),
                );
                row.insert(
                    "statement_schema_digest".into(),
                    Value::from("55".repeat(32)),
                );
                row.insert("unavailable_reason".into(), Value::Null);
                row.insert("verifier_digest".into(), Value::from("66".repeat(32)));
                Value::Object(row)
            })
            .collect::<Vec<_>>();
        let mut binding = Map::new();
        binding.insert(
            "manifest_protocol_tuples".into(),
            Value::Array(rows.clone()),
        );
        binding.insert("protocol_count".into(), Value::from(12_u64));
        binding.insert(
            "required_network_protocol_tuples".into(),
            Value::Array(rows),
        );
        binding.insert(
            "schema".into(),
            Value::from("iroha.taira.exact12-runtime-capability-binding"),
        );
        binding.insert("schema_version".into(), Value::from(1_u64));
        Value::Object(binding)
    }

    fn valid_probe_result(manifest: &[TairaAuthorityArtifactManifestEntryV1]) -> Vec<u8> {
        let binding = valid_binding();
        let mut binding_bytes = norito::json::to_json_pretty(&binding)
            .expect("serialize binding")
            .into_bytes();
        binding_bytes.push(b'\n');
        let catalog_digest = "77".repeat(32);
        let mut abi = Map::new();
        abi.insert("abi_version".into(), Value::from(22_u64));
        abi.insert(
            "compiled_profile_catalog_sha256".into(),
            Value::from(catalog_digest.clone()),
        );
        abi.insert(
            "library_sha256".into(),
            Value::from(hex::encode(manifest[3].sha256)),
        );
        abi.insert(
            "privacy_c_exports".into(),
            Value::Array(PRIVACY_C_EXPORTS_V1.into_iter().map(Value::from).collect()),
        );
        abi.insert("result".into(), Value::from("passed"));
        let mut wheel = Map::new();
        wheel.insert("capability_binding".into(), binding);
        wheel.insert(
            "capability_binding_sha256".into(),
            Value::from(hex::encode(Sha256::digest(binding_bytes))),
        );
        wheel.insert(
            "capability_manifest_sha256".into(),
            Value::from(hex::encode(manifest[0].sha256)),
        );
        wheel.insert(
            "compiled_profile_catalog_sha256".into(),
            Value::from(catalog_digest),
        );
        wheel.insert(
            "native_member".into(),
            Value::from("iroha_python/_crypto.cpython-313-aarch64-linux-gnu.so"),
        );
        wheel.insert("result".into(), Value::from("passed"));
        wheel.insert(
            "wheel_sha256".into(),
            Value::from(hex::encode(manifest[1].sha256)),
        );
        let mut outer = Map::new();
        outer.insert("abi22".into(), Value::Object(abi));
        outer.insert("python_wheel".into(), Value::Object(wheel));
        let mut bytes = norito::json::to_json_pretty(&Value::Object(outer))
            .expect("serialize result")
            .into_bytes();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn manifest_requires_ordered_distinct_probe_artifacts() {
        let mut manifest = required_manifest(b"artifact");
        assert!(required_artifact_ordinals(&manifest, 4).is_ok());
        manifest[1].ordinal = 2;
        assert_eq!(
            required_artifact_ordinals(&manifest, 4),
            Err(TairaAuthorityErrorV1::Rejected)
        );
        manifest = required_manifest(b"artifact");
        manifest[1].name = manifest[0].name.clone();
        assert_eq!(
            required_artifact_ordinals(&manifest, 4),
            Err(TairaAuthorityErrorV1::Rejected)
        );
        manifest = required_manifest(b"artifact");
        manifest.pop();
        assert_eq!(
            required_artifact_ordinals(&manifest, 3),
            Err(TairaAuthorityErrorV1::Rejected)
        );
    }

    #[test]
    fn descriptor_hashing_does_not_move_shared_offsets() {
        let mut file = tempfile::tempfile().expect("temporary artifact");
        file.write_all(b"artifact").expect("write artifact");
        file.flush().expect("flush artifact");
        let position = file.stream_position().expect("position");
        let expected_digest: [u8; 32] = Sha256::digest(b"artifact").into();
        assert_eq!(
            descriptor_sha256(&file, 8).expect("descriptor digest"),
            expected_digest
        );
        assert_eq!(file.stream_position().expect("position"), position);
        assert!(file.as_raw_fd() >= 0);
    }

    #[test]
    fn post_execution_revalidation_rejects_descriptor_mutation() {
        let mut temporary = tempfile::NamedTempFile::new().expect("temporary artifact");
        temporary.write_all(b"artifact").expect("write artifact");
        temporary.flush().expect("flush artifact");
        let file = temporary.as_file().try_clone().expect("clone artifact");
        let manifest = [manifest_entry(0, "artifact", b"artifact")];
        let identity = validate_artifact_descriptors(std::slice::from_ref(&file), &manifest, None)
            .expect("initial validation");
        file.write_at(b"mutati0n", 0).expect("mutate descriptor");
        file.sync_all().expect("sync mutation");
        assert_eq!(
            validate_artifact_descriptors(std::slice::from_ref(&file), &manifest, Some(&identity),),
            Err(TairaAuthorityErrorV1::Rejected)
        );
    }

    #[test]
    fn exact_seven_field_wheel_result_is_accepted_and_mutation_is_rejected() {
        let manifest = required_manifest(b"artifact");
        let required = required_artifact_ordinals(&manifest, manifest.len()).expect("ordinals");
        let bytes = valid_probe_result(&manifest);
        assert!(parse_probe_result(&bytes, &manifest, required).is_ok());

        let mut value: Value = norito::json::from_slice(&bytes).expect("parse fixture");
        value
            .get_mut("python_wheel")
            .and_then(Value::as_object_mut)
            .expect("wheel object")
            .insert("wheel_sha256".into(), Value::from("88".repeat(32)));
        let mut mutated = norito::json::to_json_pretty(&value)
            .expect("serialize mutation")
            .into_bytes();
        mutated.push(b'\n');
        assert_eq!(
            parse_probe_result(&mutated, &manifest, required),
            Err(TairaAuthorityErrorV1::Rejected)
        );
    }

    #[test]
    fn result_json_shape_is_exact() {
        let mut abi = Map::new();
        abi.insert("result".into(), Value::from("passed"));
        let mut wheel = Map::new();
        wheel.insert("result".into(), Value::from("passed"));
        let result = QualificationProbeResultsV1 {
            abi22: Value::Object(abi.clone()),
            python_wheel: Value::Object(wheel.clone()),
        };
        let value = result.to_json_value();
        let object = exact_object(&value, &["abi22", "python_wheel"]).expect("exact result");
        assert_eq!(object["abi22"], Value::Object(abi));
        assert_eq!(object["python_wheel"], Value::Object(wheel));
    }

    #[test]
    fn native_member_rejects_paths_and_substitutions() {
        assert!(valid_native_member(
            "iroha_python/_crypto.cpython-313-aarch64-linux-gnu.so"
        ));
        for invalid in [
            "/iroha_python/_crypto.so",
            "iroha_python/../_crypto.so",
            "iroha_python/_crypto.so/extra",
            "other/_crypto.so",
            "iroha_python/_crypto.dylib",
        ] {
            assert!(!valid_native_member(invalid), "accepted {invalid}");
        }
    }

    #[test]
    fn wheel_probe_result_contract_has_seven_unique_fields() {
        assert_eq!(WHEEL_RESULT_FIELDS_V1.len(), 7);
        assert_eq!(
            WHEEL_RESULT_FIELDS_V1
                .into_iter()
                .collect::<BTreeSet<_>>()
                .len(),
            7
        );
    }
}

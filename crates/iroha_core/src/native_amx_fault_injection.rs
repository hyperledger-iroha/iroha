//! Feature-isolated process-cut hooks for real-network Native AMX and atomic
//! private-settlement tests.
//!
//! Shipping builds do not compile this module. The dedicated adversarial-test daemon supplies a
//! private, per-peer control directory. A canonical command names one exact protocol source and
//! phase; the hook durably acknowledges that the phase was crossed and then aborts the process.
//! The acknowledgement makes the command one-shot across restart.
#[cfg(test)]
use norito::json::Map;
use norito::json::Value;
#[cfg(feature = "test-network-native-amx-fault-injection")]
use std::{
    env,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use std::{
    fs,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
#[cfg(feature = "test-network-native-amx-fault-injection")]
const CONTROL_DIR_ENV: &str = "IROHA_TEST_CONSENSUS_MESSAGE_CONTROL_DIR";
const COMMAND_FILE: &str = "native-amx-fault-command.norito.json";
const ACK_FILE: &str = "native-amx-fault-ack.norito.json";
const FORMAT_VERSION: u64 = 1;
const MAX_FILE_BYTES: usize = 4 * 1024;
#[cfg(feature = "test-network-native-amx-fault-injection")]
static HOOK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
/// Exact protocol phase at which the adversarial-test daemon aborts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeAmxFaultPhase {
    /// A participant PrepareQC was authenticated and aggregated.
    AfterPrepareQc,
    /// The corresponding participant CommitQC was authenticated and aggregated.
    AfterCommitQc,
    /// The complete result-bearing block overlay exists immediately before WSV publication.
    BeforeWorldCommit,
    /// A private-settlement provisional proof sidecar and its directory entry are durable.
    AfterPrivateSettlementSidecarFsync,
    /// A private-settlement verified delta and its reservations are durable.
    AfterPrivateSettlementStagedDeltaFsync,
    /// A private-settlement Prepare QC is durable in the leg journal.
    AfterPrivateSettlementPrepareQcFsync,
    /// A private-settlement Commit QC is durable in the leg journal.
    AfterPrivateSettlementCommitQcFsync,
    /// A block containing a private-settlement carrier is durable in Kura.
    AfterPrivateSettlementKuraAppend,
    /// A private-settlement carrier has been published atomically to WSV.
    AfterPrivateSettlementWsvApplication,
    /// A committee's terminal private-settlement receipt is durable and queryable.
    AfterPrivateSettlementReceiptPublication,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct FaultCommand {
    phase: NativeAmxFaultPhase,
    source_id: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Evaluation {
    NoCommand,
    NoMatch,
    AlreadyAcknowledged,
    Triggered,
}
/// Abort the feature-isolated daemon after durably acknowledging an exact protocol cut.
#[cfg(feature = "test-network-native-amx-fault-injection")]
pub(crate) fn maybe_abort(phase: NativeAmxFaultPhase, source_id: [u8; 32]) {
    let Some(root) = env::var_os(CONTROL_DIR_ENV).map(PathBuf::from) else {
        return;
    };
    let _guard = HOOK_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match evaluate_and_acknowledge(&root, phase, source_id) {
        Ok(Evaluation::Triggered) => {
            // `abort` models power/process loss and cannot be caught by a task
            // supervisor. The acknowledgement was fsynced before this point.
            std::process::abort();
        }
        Ok(Evaluation::NoCommand | Evaluation::NoMatch | Evaluation::AlreadyAcknowledged) => {}
        Err(error) => {
            // This binary is feature-isolated. A malformed deployment-owned
            // command is itself a test failure, so stop instead of silently
            // claiming that a requested phase cut ran.
            panic!("invalid Native AMX fault-injection command: {error}");
        }
    }
}
fn evaluate_and_acknowledge(
    root: &Path,
    phase: NativeAmxFaultPhase,
    source_id: [u8; 32],
) -> Result<Evaluation, String> {
    validate_private_root(root)?;
    let command_path = root.join(COMMAND_FILE);
    let command_bytes = match read_bounded_regular_file(&command_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Evaluation::NoCommand);
        }
        Err(error) => return Err(format!("read command: {error}")),
    };
    let command = parse_command(&command_bytes)?;
    if command.phase != phase || command.source_id != source_id {
        return Ok(Evaluation::NoMatch);
    }
    let ack_path = root.join(ACK_FILE);
    match read_bounded_regular_file(&ack_path) {
        Ok(ack) if ack == command_bytes => return Ok(Evaluation::AlreadyAcknowledged),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("read acknowledgement: {error}")),
    }
    write_atomic_private_file(root, ACK_FILE, &command_bytes)?;
    let readback = read_bounded_regular_file(&ack_path)
        .map_err(|error| format!("read acknowledgement after fsync: {error}"))?;
    if readback != command_bytes {
        return Err("acknowledgement read-back differs from the exact command".to_owned());
    }
    Ok(Evaluation::Triggered)
}
fn parse_command(bytes: &[u8]) -> Result<FaultCommand, String> {
    if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
        return Err("command size is outside the bounded range".to_owned());
    }
    let value: Value = norito::json::from_slice(bytes).map_err(|error| error.to_string())?;
    if norito::json::to_json(&value)
        .map_err(|error| error.to_string())?
        .as_bytes()
        != bytes
    {
        return Err("command is not canonical Norito JSON".to_owned());
    }
    let object = value
        .as_object()
        .ok_or_else(|| "command is not an object".to_owned())?;
    const FIELDS: [&str; 4] = ["phase", "revision", "source_id", "version"];
    if object.len() != FIELDS.len() || FIELDS.iter().any(|field| !object.contains_key(*field)) {
        return Err("command has missing or unknown fields".to_owned());
    }
    if object.get("version").and_then(Value::as_u64) != Some(FORMAT_VERSION) {
        return Err("unsupported command version".to_owned());
    }
    let _revision = object
        .get("revision")
        .and_then(Value::as_u64)
        .filter(|revision| *revision > 0)
        .ok_or_else(|| "revision must be positive".to_owned())?;
    let phase = match object.get("phase").and_then(Value::as_str) {
        Some("after_prepare_qc") => NativeAmxFaultPhase::AfterPrepareQc,
        Some("after_commit_qc") => NativeAmxFaultPhase::AfterCommitQc,
        Some("before_world_commit") => NativeAmxFaultPhase::BeforeWorldCommit,
        Some("after_private_settlement_sidecar_fsync") => {
            NativeAmxFaultPhase::AfterPrivateSettlementSidecarFsync
        }
        Some("after_private_settlement_staged_delta_fsync") => {
            NativeAmxFaultPhase::AfterPrivateSettlementStagedDeltaFsync
        }
        Some("after_private_settlement_prepare_qc_fsync") => {
            NativeAmxFaultPhase::AfterPrivateSettlementPrepareQcFsync
        }
        Some("after_private_settlement_commit_qc_fsync") => {
            NativeAmxFaultPhase::AfterPrivateSettlementCommitQcFsync
        }
        Some("after_private_settlement_kura_append") => {
            NativeAmxFaultPhase::AfterPrivateSettlementKuraAppend
        }
        Some("after_private_settlement_wsv_application") => {
            NativeAmxFaultPhase::AfterPrivateSettlementWsvApplication
        }
        Some("after_private_settlement_receipt_publication") => {
            NativeAmxFaultPhase::AfterPrivateSettlementReceiptPublication
        }
        _ => return Err("unknown fault phase".to_owned()),
    };
    let source_literal = object
        .get("source_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "source_id must be a lowercase hexadecimal string".to_owned())?;
    if source_literal.len() != 64
        || source_literal
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err("source_id must be exactly 32 lowercase hexadecimal bytes".to_owned());
    }
    let decoded = hex::decode(source_literal).map_err(|error| error.to_string())?;
    let source_id: [u8; 32] = decoded
        .try_into()
        .map_err(|_| "source_id length changed while decoding".to_owned())?;
    Ok(FaultCommand { phase, source_id })
}
fn validate_private_root(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err("control root is not a real directory".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("control root is accessible by group or other users".to_owned());
        }
    }
    Ok(())
}
fn read_bounded_regular_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if !before.file_type().is_file() || before.file_type().is_symlink() {
        return Err(std::io::Error::other("control path is not a regular file"));
    }
    if usize::try_from(before.len())
        .ok()
        .is_none_or(|length| length > MAX_FILE_BYTES)
    {
        return Err(std::io::Error::other("control file exceeds size bound"));
    }
    let mut file = OpenOptions::new().read(true).open(path)?;
    let opened = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != opened.dev() || before.ino() != opened.ino() {
            return Err(std::io::Error::other("control file changed while opening"));
        }
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(MAX_FILE_BYTES));
    Read::take(
        &mut file,
        u64::try_from(MAX_FILE_BYTES + 1).expect("small bound fits u64"),
    )
    .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(std::io::Error::other("control file grew past size bound"));
    }
    Ok(bytes)
}
fn write_atomic_private_file(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let temporary = root.join(format!(".{name}.tmp-{}", std::process::id()));
    let target = root.join(name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&temporary) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temporary).map_err(|remove| remove.to_string())?;
            options.open(&temporary).map_err(|open| open.to_string())?
        }
        Err(error) => return Err(error.to_string()),
    };
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(&temporary, &target).map_err(|error| error.to_string())?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn command(revision: u64, phase: NativeAmxFaultPhase, source_id: [u8; 32]) -> Vec<u8> {
        let phase = match phase {
            NativeAmxFaultPhase::AfterPrepareQc => "after_prepare_qc",
            NativeAmxFaultPhase::AfterCommitQc => "after_commit_qc",
            NativeAmxFaultPhase::BeforeWorldCommit => "before_world_commit",
            NativeAmxFaultPhase::AfterPrivateSettlementSidecarFsync => {
                "after_private_settlement_sidecar_fsync"
            }
            NativeAmxFaultPhase::AfterPrivateSettlementStagedDeltaFsync => {
                "after_private_settlement_staged_delta_fsync"
            }
            NativeAmxFaultPhase::AfterPrivateSettlementPrepareQcFsync => {
                "after_private_settlement_prepare_qc_fsync"
            }
            NativeAmxFaultPhase::AfterPrivateSettlementCommitQcFsync => {
                "after_private_settlement_commit_qc_fsync"
            }
            NativeAmxFaultPhase::AfterPrivateSettlementKuraAppend => {
                "after_private_settlement_kura_append"
            }
            NativeAmxFaultPhase::AfterPrivateSettlementWsvApplication => {
                "after_private_settlement_wsv_application"
            }
            NativeAmxFaultPhase::AfterPrivateSettlementReceiptPublication => {
                "after_private_settlement_receipt_publication"
            }
        };
        let mut object = Map::new();
        object.insert("phase".to_owned(), Value::from(phase));
        object.insert("revision".to_owned(), Value::from(revision));
        object.insert("source_id".to_owned(), Value::from(hex::encode(source_id)));
        object.insert("version".to_owned(), Value::from(FORMAT_VERSION));
        norito::json::to_json(&Value::Object(object))
            .expect("encode command")
            .into_bytes()
    }
    #[test]
    fn exact_phase_and_source_trigger_once() {
        let root = tempfile::tempdir().expect("temporary root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
                .expect("private root");
        }
        let source_id = [0xA5; 32];
        let bytes = command(1, NativeAmxFaultPhase::AfterCommitQc, source_id);
        fs::write(root.path().join(COMMAND_FILE), &bytes).expect("write command");
        assert_eq!(
            evaluate_and_acknowledge(root.path(), NativeAmxFaultPhase::AfterPrepareQc, source_id)
                .expect("phase mismatch is valid"),
            Evaluation::NoMatch
        );
        assert_eq!(
            evaluate_and_acknowledge(root.path(), NativeAmxFaultPhase::AfterCommitQc, [0xA6; 32])
                .expect("source mismatch is valid"),
            Evaluation::NoMatch
        );
        assert_eq!(
            evaluate_and_acknowledge(root.path(), NativeAmxFaultPhase::AfterCommitQc, source_id)
                .expect("exact command triggers"),
            Evaluation::Triggered
        );
        assert_eq!(
            evaluate_and_acknowledge(root.path(), NativeAmxFaultPhase::AfterCommitQc, source_id)
                .expect("acknowledged command is one-shot"),
            Evaluation::AlreadyAcknowledged
        );
        assert_eq!(
            fs::read(root.path().join(ACK_FILE)).expect("read acknowledgement"),
            bytes
        );
    }
    #[test]
    fn parser_rejects_unknown_fields_and_noncanonical_source() {
        let valid = command(1, NativeAmxFaultPhase::BeforeWorldCommit, [7; 32]);
        assert!(parse_command(&valid).is_ok());
        let mut value: Value = norito::json::from_slice(&valid).expect("parse valid command");
        value
            .as_object_mut()
            .expect("command object")
            .insert("extra".to_owned(), Value::from(true));
        let unknown = norito::json::to_json(&value).expect("encode unknown field");
        assert!(parse_command(unknown.as_bytes()).is_err());
        value
            .as_object_mut()
            .expect("command object")
            .remove("extra");
        value.as_object_mut().expect("command object").insert(
            "source_id".to_owned(),
            Value::from(hex::encode_upper([7; 32])),
        );
        let uppercase = norito::json::to_json(&value).expect("encode uppercase source");
        assert!(parse_command(uppercase.as_bytes()).is_err());
    }

    #[test]
    fn parser_accepts_every_private_settlement_durability_boundary() {
        let phases = [
            NativeAmxFaultPhase::AfterPrivateSettlementSidecarFsync,
            NativeAmxFaultPhase::AfterPrivateSettlementStagedDeltaFsync,
            NativeAmxFaultPhase::AfterPrivateSettlementPrepareQcFsync,
            NativeAmxFaultPhase::AfterPrivateSettlementCommitQcFsync,
            NativeAmxFaultPhase::AfterPrivateSettlementKuraAppend,
            NativeAmxFaultPhase::AfterPrivateSettlementWsvApplication,
            NativeAmxFaultPhase::AfterPrivateSettlementReceiptPublication,
        ];
        for (index, phase) in phases.into_iter().enumerate() {
            let encoded = command(
                u64::try_from(index + 1).expect("small revision"),
                phase,
                [u8::try_from(index + 1).expect("small source seed"); 32],
            );
            assert_eq!(parse_command(&encoded).expect("parse phase").phase, phase);
        }
    }
}

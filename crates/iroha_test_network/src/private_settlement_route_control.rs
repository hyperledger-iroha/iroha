//! Test-only controller for authenticated private-settlement HTTP routes.
use color_eyre::eyre::{Result, eyre};
use iroha_crypto::sha256;
use norito::json::{Map, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::time::sleep;
pub(crate) const CONTROL_DIR_ENV: &str = "IROHA_TEST_PRIVATE_SETTLEMENT_ROUTE_CONTROL_DIR";
const PRIVATE_SETTLEMENT_ROUTE_COMMAND_FILE: &str = "private-settlement-route-command.norito.json";
const PRIVATE_SETTLEMENT_ROUTE_ACK_FILE: &str = "private-settlement-route-ack.norito.json";
const PRIVATE_SETTLEMENT_ROUTE_FORMAT_VERSION: u64 = 1;
const MAX_ACK_BYTES: usize = 1024 * 1024;
const ACK_POLL: Duration = Duration::from_millis(10);
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
/// Authenticated private-settlement Torii phase controlled after ordinary request auth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateSettlementRouteControlPhase {
    /// Restricted provisional upload and availability-share persistence.
    RestrictedDa,
    /// Prepare vote and Prepare-certificate persistence.
    Prepare,
    /// Commit vote and Commit-certificate persistence.
    Commit,
}

impl PrivateSettlementRouteControlPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RestrictedDa => "restricted_da",
            Self::Prepare => "prepare",
            Self::Commit => "commit",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "restricted_da" => Ok(Self::RestrictedDa),
            "prepare" => Ok(Self::Prepare),
            "commit" => Ok(Self::Commit),
            _ => Err(eyre!(
                "unknown private-settlement route-control phase `{value}`"
            )),
        }
    }
}

/// Action installed at the post-authentication private-settlement route boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateSettlementRouteControlAction {
    /// Reject the first configured number of matching requests, then pass.
    Loss,
    /// Retain each matching request until an explicit [`Self::Pass`] command.
    Hold,
    /// Heal a prior command and pass retained or subsequent requests.
    Pass,
}

impl PrivateSettlementRouteControlAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Loss => "loss",
            Self::Hold => "hold",
            Self::Pass => "pass",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "loss" => Ok(Self::Loss),
            "hold" => Ok(Self::Hold),
            "pass" => Ok(Self::Pass),
            _ => Err(eyre!(
                "unknown private-settlement route-control action `{value}`"
            )),
        }
    }
}

/// Exact canonical command bytes installed for one controlled peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateSettlementRouteControlCommand {
    /// Monotonic controller-local revision.
    pub revision: u64,
    /// SHA-256 of `canonical_bytes`.
    pub sha256: String,
    /// Exact fsynced command bytes consumed by the daemon.
    pub canonical_bytes: Vec<u8>,
}

/// Durable daemon acknowledgement for authenticated APS route control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateSettlementRouteControlAck {
    /// Applied command revision.
    pub revision: u64,
    /// SHA-256 of the exact command bytes.
    pub command_sha256: String,
    /// Exact predecessor Hold command released by this Pass revision, when any.
    pub predecessor_command_sha256: Option<String>,
    /// Controlled phase.
    pub phase: PrivateSettlementRouteControlPhase,
    /// Applied action.
    pub action: PrivateSettlementRouteControlAction,
    /// Exact bundle identity.
    pub bundle_id: [u8; 32],
    /// Deterministic trial seed.
    pub seed: u64,
    /// Number of authenticated matching requests observed.
    pub matched: u64,
    /// Number admitted into the ordinary production handler.
    pub passed: u64,
    /// Number rejected as controlled loss.
    pub dropped: u64,
    /// Number durably acknowledged before being held.
    pub held: u64,
    /// Number released by this healing revision.
    pub released: u64,
    /// SHA-256 of each exact authenticated request occurrence in admission order.
    pub request_digests: Vec<String>,
}
/// Controls authenticated HTTP request loss and recovery in a dedicated test daemon.
#[derive(Debug)]
pub struct PrivateSettlementRouteControl {
    root: PathBuf,
    root_identity: RootIdentity,
    next_private_settlement_route_revision: Mutex<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RootIdentity {
    device: u64,
    inode: u64,
    owner: u32,
}
impl PrivateSettlementRouteControl {
    #[cfg(unix)]
    pub(crate) fn create(root: PathBuf) -> Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let root = root.canonicalize()?;
        let root_identity = validate_private_root(&fs::symlink_metadata(&root)?)?;
        Ok(Self {
            root,
            root_identity,
            next_private_settlement_route_revision: Mutex::new(0),
        })
    }
    #[cfg(not(unix))]
    pub(crate) fn create(_root: PathBuf) -> Result<Self> {
        Err(eyre!(
            "private-settlement route control requires Unix ownership/no-follow semantics"
        ))
    }
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    /// Atomically install one exact post-authentication APS route command.
    ///
    /// `drop_first` and `match_limit` must be non-zero only for [`PrivateSettlementRouteControlAction::Loss`].
    /// A hold uses `(0, 1)` and a healing pass uses `(0, 0)`.
    pub fn arm_private_settlement_route_control(
        &self,
        phase: PrivateSettlementRouteControlPhase,
        action: PrivateSettlementRouteControlAction,
        bundle_id: [u8; 32],
        seed: u64,
        drop_first: u64,
        match_limit: u64,
    ) -> Result<PrivateSettlementRouteControlCommand> {
        match action {
            PrivateSettlementRouteControlAction::Loss
                if match_limit > 0 && drop_first <= match_limit && match_limit <= 10_000 => {}
            PrivateSettlementRouteControlAction::Hold if drop_first == 0 && match_limit == 1 => {}
            PrivateSettlementRouteControlAction::Pass if drop_first == 0 && match_limit == 0 => {}
            _ => return Err(eyre!("invalid private-settlement route-control bounds")),
        }
        validate_root_identity(&self.root, self.root_identity)?;
        // Retain the revision lock through installation so concurrent callers cannot
        // overwrite a newer command with an older revision.
        let mut next = self
            .next_private_settlement_route_revision
            .lock()
            .expect("private-settlement route-control revision lock poisoned");
        let revision = next
            .checked_add(1)
            .ok_or_else(|| eyre!("private-settlement route-control revision overflow"))?;
        let value = object_value([
            ("action", Value::from(action.as_str())),
            ("bundle_id", Value::from(crate::hex_lower(&bundle_id))),
            ("drop_first", Value::from(drop_first)),
            (
                "format_version",
                Value::from(PRIVATE_SETTLEMENT_ROUTE_FORMAT_VERSION),
            ),
            ("match_limit", Value::from(match_limit)),
            ("phase", Value::from(phase.as_str())),
            ("revision", Value::from(revision)),
            ("seed", Value::from(seed)),
        ]);
        let canonical_bytes = canonical_json(&value)?;
        let sha256 = crate::hex_lower(&sha256(&canonical_bytes));
        write_atomic_private_file(
            &self.root,
            PRIVATE_SETTLEMENT_ROUTE_COMMAND_FILE,
            &canonical_bytes,
            self.root_identity.owner,
        )?;
        validate_root_identity(&self.root, self.root_identity)?;
        *next = revision;
        Ok(PrivateSettlementRouteControlCommand {
            revision,
            sha256,
            canonical_bytes,
        })
    }
    /// Return the exact acknowledgement bytes together with their parsed shape.
    pub fn read_private_settlement_route_control_ack_bytes(
        &self,
    ) -> Result<(Vec<u8>, PrivateSettlementRouteControlAck)> {
        validate_root_identity(&self.root, self.root_identity)?;
        let bytes = read_bounded_private_file(
            &self.root.join(PRIVATE_SETTLEMENT_ROUTE_ACK_FILE),
            MAX_ACK_BYTES,
            self.root_identity.owner,
        )?;
        validate_root_identity(&self.root, self.root_identity)?;
        let ack = parse_private_settlement_route_ack(&bytes)?;
        Ok((bytes, ack))
    }
    /// Wait for a durable acknowledgement of the exact installed command.
    pub async fn wait_for_private_settlement_route_control(
        &self,
        command: &PrivateSettlementRouteControlCommand,
        timeout: Duration,
    ) -> Result<(Vec<u8>, PrivateSettlementRouteControlAck)> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.read_private_settlement_route_control_ack_bytes() {
                Ok((bytes, ack))
                    if ack.revision == command.revision && ack.command_sha256 == command.sha256 =>
                {
                    return Ok((bytes, ack));
                }
                Ok((_, ack)) if ack.revision >= command.revision => {
                    return Err(eyre!(
                        "private-settlement route acknowledgement differs from revision {}: {ack:?}",
                        command.revision
                    ));
                }
                Ok(_) | Err(_) if Instant::now() < deadline => {}
                Err(error) => return Err(error),
                Ok((_, ack)) => {
                    return Err(eyre!(
                        "timed out waiting for private-settlement route revision {}; latest={ack:?}",
                        command.revision
                    ));
                }
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for private-settlement route revision {}",
                    command.revision
                ));
            }
            sleep(ACK_POLL).await;
        }
    }
}
fn decode_lower_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
fn decode_lower_hex_32(value: &str) -> Option<[u8; 32]> {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (output, pair) in decoded.iter_mut().zip(bytes.chunks_exact(2)) {
        let high = decode_lower_hex_nibble(pair[0])?;
        let low = decode_lower_hex_nibble(pair[1])?;
        *output = (high << 4) | low;
    }
    Some(decoded)
}
fn parse_private_settlement_route_ack(bytes: &[u8]) -> Result<PrivateSettlementRouteControlAck> {
    if bytes.is_empty() || bytes.len() > MAX_ACK_BYTES {
        return Err(eyre!(
            "private-settlement route acknowledgement has invalid size"
        ));
    }
    let value: Value = norito::json::from_slice(bytes)?;
    if canonical_json(&value)?.as_slice() != bytes {
        return Err(eyre!(
            "private-settlement route acknowledgement is not canonical"
        ));
    }
    let object = exact_object(
        &value,
        &[
            "action",
            "bundle_id",
            "command_sha256",
            "dropped",
            "format_version",
            "held",
            "matched",
            "passed",
            "phase",
            "predecessor_command_sha256",
            "released",
            "request_digests",
            "revision",
            "seed",
        ],
        "private-settlement route acknowledgement",
    )?;
    if required_u64(object, "format_version")? != PRIVATE_SETTLEMENT_ROUTE_FORMAT_VERSION {
        return Err(eyre!(
            "unsupported private-settlement route acknowledgement version"
        ));
    }
    let revision = required_u64(object, "revision")?;
    if revision == 0 {
        return Err(eyre!(
            "private-settlement route acknowledgement revision must be positive"
        ));
    }
    let phase = PrivateSettlementRouteControlPhase::parse(required_string(object, "phase")?)?;
    let action = PrivateSettlementRouteControlAction::parse(required_string(object, "action")?)?;
    let bundle_id = decode_lower_hex_32(required_string(object, "bundle_id")?)
        .ok_or_else(|| eyre!("private-settlement route bundle id is not canonical"))?;
    let command_sha256 = required_string(object, "command_sha256")?.to_owned();
    if decode_lower_hex_32(&command_sha256).is_none() {
        return Err(eyre!(
            "private-settlement route command SHA-256 is not canonical"
        ));
    }
    let predecessor_command_sha256 = match object.get("predecessor_command_sha256") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if decode_lower_hex_32(value).is_some() => Some(value.clone()),
        _ => {
            return Err(eyre!(
                "private-settlement route predecessor command SHA-256 is invalid"
            ));
        }
    };
    let request_digests = object
        .get("request_digests")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("private-settlement route acknowledgement lacks request digests"))?
        .iter()
        .map(|value| {
            let digest = value
                .as_str()
                .ok_or_else(|| eyre!("private-settlement route request digest is not a string"))?;
            if decode_lower_hex_32(digest).is_none() {
                return Err(eyre!(
                    "private-settlement route request digest is not canonical"
                ));
            }
            Ok(digest.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let matched = required_u64(object, "matched")?;
    let passed = required_u64(object, "passed")?;
    let dropped = required_u64(object, "dropped")?;
    let held = required_u64(object, "held")?;
    let released = required_u64(object, "released")?;
    if request_digests.len() != usize::try_from(matched)?
        || passed.saturating_add(dropped).saturating_add(held) != matched
        || released > held
        || (released == 0 && predecessor_command_sha256.is_some())
        || (released > 0 && predecessor_command_sha256.is_none())
    {
        return Err(eyre!(
            "private-settlement route acknowledgement counters are inconsistent"
        ));
    }
    Ok(PrivateSettlementRouteControlAck {
        revision,
        command_sha256,
        predecessor_command_sha256,
        phase,
        action,
        bundle_id,
        seed: required_u64(object, "seed")?,
        matched,
        passed,
        dropped,
        held,
        released,
        request_digests,
    })
}
fn exact_object<'a>(value: &'a Value, fields: &[&str], label: &str) -> Result<&'a Map> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("route-control {label} is not an object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(eyre!("route-control {label} has unexpected fields"));
    }
    Ok(object)
}
fn required_u64(object: &Map, field: &str) -> Result<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("route-control acknowledgement lacks integer `{field}`"))
}
fn required_string<'a>(object: &'a Map, field: &str) -> Result<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("route-control acknowledgement lacks string `{field}`"))
}
fn canonical_json(value: &Value) -> Result<Vec<u8>> {
    Ok(norito::json::to_json(value)?.into_bytes())
}
fn object_value<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (key, value) in entries {
        object.insert(key.to_owned(), value);
    }
    Value::Object(object)
}
#[cfg(unix)]
fn read_bounded_private_file(path: &Path, max_bytes: usize, owner: u32) -> Result<Vec<u8>> {
    use std::os::unix::fs::OpenOptionsExt;
    let named_before = fs::symlink_metadata(path)?;
    validate_private_file(&named_before, owner)?;
    if usize::try_from(named_before.len())
        .ok()
        .is_none_or(|length| length > max_bytes)
    {
        return Err(eyre!("route-control acknowledgement is too large"));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits())
        .open(path)?;
    let opened_before = file.metadata()?;
    validate_private_file(&opened_before, owner)?;
    if !same_file(&named_before, &opened_before) {
        return Err(eyre!("route-control acknowledgement identity changed"));
    }
    if usize::try_from(opened_before.len())
        .ok()
        .is_none_or(|length| length > max_bytes)
    {
        return Err(eyre!("route-control acknowledgement is too large"));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened_before.len())
            .unwrap_or(max_bytes)
            .min(max_bytes),
    );
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes)? + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(eyre!("route-control acknowledgement is too large"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut confirmation = Vec::with_capacity(bytes.len());
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes)? + 1)
        .read_to_end(&mut confirmation)?;
    if confirmation != bytes {
        return Err(eyre!(
            "route-control acknowledgement changed while confirming"
        ));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    validate_private_file(&opened_after, owner)?;
    validate_private_file(&named_after, owner)?;
    if !same_file(&opened_before, &opened_after)
        || !same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != u64::try_from(bytes.len())?
        || opened_before.modified().ok() != opened_after.modified().ok()
    {
        return Err(eyre!("route-control acknowledgement changed while reading"));
    }
    Ok(bytes)
}
#[cfg(not(unix))]
fn read_bounded_private_file(_path: &Path, _max_bytes: usize, _owner: u32) -> Result<Vec<u8>> {
    Err(eyre!(
        "private-settlement route control requires Unix ownership/no-follow semantics"
    ))
}
fn write_atomic_private_file(root: &Path, name: &str, bytes: &[u8], owner: u32) -> Result<()> {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = root.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence));
    let final_path = root.join(name);
    match fs::symlink_metadata(&final_path) {
        Ok(metadata) => validate_private_file(&metadata, owner)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        let written = file.metadata()?;
        validate_private_file(&written, owner)?;
        if written.len() != u64::try_from(bytes.len())? {
            return Err(eyre!("route-control write length changed before install"));
        }
        fs::rename(&temp, &final_path)?;
        let installed = fs::symlink_metadata(&final_path)?;
        validate_private_file(&installed, owner)?;
        if !same_file(&written, &installed) || installed.len() != written.len() {
            return Err(eyre!(
                "route-control file identity changed during atomic install"
            ));
        }
        File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
#[cfg(unix)]
fn validate_private_root(metadata: &fs::Metadata) -> Result<RootIdentity> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(eyre!("unsafe private-settlement route-control root"));
    }
    Ok(RootIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        owner: metadata.uid(),
    })
}
#[cfg(not(unix))]
fn validate_private_root(_metadata: &fs::Metadata) -> Result<RootIdentity> {
    Err(eyre!(
        "private-settlement route control requires Unix ownership/no-follow semantics"
    ))
}
fn validate_root_identity(root: &Path, expected: RootIdentity) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    let actual = validate_private_root(&metadata)?;
    if actual != expected {
        return Err(eyre!(
            "private-settlement route-control root identity changed"
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn validate_private_file(metadata: &fs::Metadata, owner: u32) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!("unsafe private-settlement route-control file"));
    }
    Ok(())
}
#[cfg(not(unix))]
fn validate_private_file(_metadata: &fs::Metadata, _owner: u32) -> Result<()> {
    Err(eyre!(
        "private-settlement route control requires Unix ownership/no-follow semantics"
    ))
}
#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn private_settlement_route_control_command_and_ack_bind_exact_occurrences() {
        let parent = tempdir().expect("temporary parent");
        let control = PrivateSettlementRouteControl::create(parent.path().join("control"))
            .expect("create route control");
        let bundle_id = [0x51; 32];
        let command = control
            .arm_private_settlement_route_control(
                PrivateSettlementRouteControlPhase::Prepare,
                PrivateSettlementRouteControlAction::Loss,
                bundle_id,
                9,
                5,
                25,
            )
            .expect("arm route loss");
        assert_eq!(command.revision, 1);
        assert_eq!(
            command.sha256,
            crate::hex_lower(&sha256(&command.canonical_bytes))
        );
        let ack = object_value([
            ("action", Value::from("loss")),
            ("bundle_id", Value::from(crate::hex_lower(&bundle_id))),
            ("command_sha256", Value::from(command.sha256.clone())),
            ("dropped", Value::from(5_u64)),
            (
                "format_version",
                Value::from(PRIVATE_SETTLEMENT_ROUTE_FORMAT_VERSION),
            ),
            ("held", Value::from(0_u64)),
            ("matched", Value::from(25_u64)),
            ("passed", Value::from(20_u64)),
            ("phase", Value::from("prepare")),
            ("predecessor_command_sha256", Value::Null),
            ("released", Value::from(0_u64)),
            (
                "request_digests",
                Value::Array(
                    (0..25)
                        .map(|index| Value::from(format!("{index:064x}")))
                        .collect(),
                ),
            ),
            ("revision", Value::from(command.revision)),
            ("seed", Value::from(9_u64)),
        ]);
        let bytes = canonical_json(&ack).expect("canonical acknowledgement");
        write_atomic_private_file(
            control.root(),
            PRIVATE_SETTLEMENT_ROUTE_ACK_FILE,
            &bytes,
            control.root_identity.owner,
        )
        .unwrap();
        let (observed, ack) = control
            .read_private_settlement_route_control_ack_bytes()
            .unwrap();
        assert_eq!(observed, bytes);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let (_, applied) = runtime
            .block_on(control.wait_for_private_settlement_route_control(&command, Duration::ZERO))
            .unwrap();
        assert_eq!(applied, ack);
        let mut wrong_command = command.clone();
        wrong_command.sha256 = "a".repeat(64);
        assert!(
            runtime
                .block_on(
                    control
                        .wait_for_private_settlement_route_control(&wrong_command, Duration::ZERO)
                )
                .is_err()
        );
        assert_eq!(ack.bundle_id, bundle_id);
        assert_eq!(ack.matched, 25);
        assert_eq!(ack.dropped, 5);
        assert_eq!(ack.passed, 20);
        assert_eq!(ack.predecessor_command_sha256, None);
    }
    #[test]
    fn private_settlement_route_heal_ack_binds_all_released_holds() {
        let predecessor = "4".repeat(64);
        let ack = object_value([
            ("action", Value::from("pass")),
            ("bundle_id", Value::from("51".repeat(32))),
            ("command_sha256", Value::from("5".repeat(64))),
            ("dropped", Value::from(0_u64)),
            (
                "format_version",
                Value::from(PRIVATE_SETTLEMENT_ROUTE_FORMAT_VERSION),
            ),
            ("held", Value::from(2_u64)),
            ("matched", Value::from(2_u64)),
            ("passed", Value::from(0_u64)),
            ("phase", Value::from("commit")),
            (
                "predecessor_command_sha256",
                Value::from(predecessor.clone()),
            ),
            ("released", Value::from(2_u64)),
            (
                "request_digests",
                Value::Array(vec![
                    Value::from("6".repeat(64)),
                    Value::from("7".repeat(64)),
                ]),
            ),
            ("revision", Value::from(2_u64)),
            ("seed", Value::from(11_u64)),
        ]);
        let parsed = parse_private_settlement_route_ack(
            &canonical_json(&ack).expect("canonical healing acknowledgement"),
        )
        .expect("parse healing acknowledgement");
        assert_eq!(parsed.predecessor_command_sha256, Some(predecessor));
        assert_eq!(parsed.held, 2);
        assert_eq!(parsed.released, 2);
    }
    #[test]
    fn controller_rejects_invalid_bounds_before_writing() {
        let parent = tempdir().expect("temporary parent");
        let control = PrivateSettlementRouteControl::create(parent.path().join("control")).unwrap();
        for (action, dropped, limit) in [
            (PrivateSettlementRouteControlAction::Loss, 2, 1),
            (PrivateSettlementRouteControlAction::Loss, 0, 10_001),
            (PrivateSettlementRouteControlAction::Hold, 0, 2),
            (PrivateSettlementRouteControlAction::Pass, 1, 0),
        ] {
            assert!(
                control
                    .arm_private_settlement_route_control(
                        PrivateSettlementRouteControlPhase::Commit,
                        action,
                        [1; 32],
                        0,
                        dropped,
                        limit,
                    )
                    .is_err()
            );
        }
        assert!(
            !control
                .root()
                .join(PRIVATE_SETTLEMENT_ROUTE_COMMAND_FILE)
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_files_reject_links_and_changed_root() {
        use std::os::unix::fs::symlink;
        let parent = tempdir().unwrap();
        let control = PrivateSettlementRouteControl::create(parent.path().join("control")).unwrap();
        write_atomic_private_file(
            control.root(),
            "original",
            b"content",
            control.root_identity.owner,
        )
        .unwrap();
        let source = control.root().join("original");
        let alias = control.root().join("alias");
        symlink(&source, &alias).unwrap();
        assert!(read_bounded_private_file(&alias, 64, control.root_identity.owner).is_err());
        fs::remove_file(&alias).unwrap();
        fs::hard_link(&source, &alias).unwrap();
        assert!(read_bounded_private_file(&source, 64, control.root_identity.owner).is_err());
        fs::remove_file(&alias).unwrap();
        assert_eq!(
            read_bounded_private_file(&source, 64, control.root_identity.owner).unwrap(),
            b"content"
        );
        assert!(read_bounded_private_file(&source, 2, control.root_identity.owner).is_err());
        fs::rename(control.root(), parent.path().join("retired")).unwrap();
        let replacement =
            PrivateSettlementRouteControl::create(control.root().to_path_buf()).unwrap();
        assert!(validate_root_identity(replacement.root(), control.root_identity).is_err());
    }
    #[test]
    fn concurrent_commands_install_the_highest_revision_last() {
        let parent = tempdir().unwrap();
        let control = std::sync::Arc::new(
            PrivateSettlementRouteControl::create(parent.path().join("control")).unwrap(),
        );
        let workers = (0..8)
            .map(|seed| {
                let control = std::sync::Arc::clone(&control);
                std::thread::spawn(move || {
                    control
                        .arm_private_settlement_route_control(
                            PrivateSettlementRouteControlPhase::Prepare,
                            PrivateSettlementRouteControlAction::Pass,
                            [1; 32],
                            seed,
                            0,
                            0,
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let mut revisions = workers
            .into_iter()
            .map(|worker| worker.join().unwrap().revision)
            .collect::<Vec<_>>();
        revisions.sort_unstable();
        assert_eq!(revisions, (1..=8).collect::<Vec<_>>());
        let installed = read_bounded_private_file(
            &control.root().join(PRIVATE_SETTLEMENT_ROUTE_COMMAND_FILE),
            4096,
            control.root_identity.owner,
        )
        .unwrap();
        let value: Value = norito::json::from_slice(&installed).unwrap();
        assert_eq!(value.get("revision").and_then(Value::as_u64), Some(8));
    }
}
